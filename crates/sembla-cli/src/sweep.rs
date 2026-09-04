//! Sweep orchestration, concurrency, CUDA admission, and sweep artifacts.

use super::*;

mod backend;
mod concurrency;
mod draw;
mod options;
mod policy;
mod publication;
mod timing;

pub(crate) use backend::*;
pub(crate) use concurrency::*;
pub(crate) use options::*;
pub(crate) use policy::*;
pub(crate) use publication::*;
pub(crate) use timing::*;

pub(crate) fn sweep_file_result(path: &str, options: SweepOptions) -> Result<(), String> {
    sweep_file_result_with_runtime(path, options, &ProductionSweepRuntime)
}

pub(crate) fn sweep_file_result_with_runtime<R: SweepRuntime>(
    path: &str,
    options: SweepOptions,
    runtime: &R,
) -> Result<(), String> {
    let sweep_started = Instant::now();
    let RunInput { model, plan } = read_executable_input(path, None, &options.enabled_features)?;
    if options.export_pairs.is_some() && model.model().summaries.is_empty() {
        return Err(format!(
            "model '{}' declares no summaries; --export-pairs requires declared summaries (DESIGN.md §4.6)",
            model.model().name
        ));
    }
    if options.export_pairs.is_some() && options.noise_mode == manifest::NoiseMode::Crn {
        eprintln!(
            "warning: --export-pairs with --noise crn is unsuitable for NPE training (DECISIONS.md §G5); use --noise independent"
        );
    }
    let effective_ir_hash = manifest::canonical_ir_hash(&model)?;
    let theta_file = options
        .theta_file
        .as_deref()
        .map(|theta_path| read_theta_file(&model, theta_path))
        .transpose()?;
    let draw_count = match (&theta_file, options.draws) {
        (Some(theta), None) => u32::try_from(theta.assignments.len())
            .expect("theta-file length was checked while reading"),
        (None, Some(draws)) => draws,
        _ => unreachable!("sweep option exclusivity was checked while parsing"),
    };
    let supported_workers_present = options.draw_workers.is_some();
    let final_state_selection = sweep_cuda_final_state_selection(options.backend)?;
    let fused_capacity =
        sweep_cuda_fused_draw_capacity(options.backend, supported_workers_present)?;
    if fused_capacity.is_some() && final_state_selection.explicitly_set {
        return Err(format!(
            "{SWEEP_CUDA_FINAL_STATE_MODE_ENV} is incompatible with experimental {SWEEP_CUDA_FUSED_DRAWS_ENV}"
        ));
    }
    let draw_workers = sweep_draw_workers(draw_count, options.backend, options.draw_workers)?;
    let concurrency_mode = sweep_concurrency_mode(
        draw_count,
        draw_workers,
        options.backend,
        supported_workers_present,
    )?;
    if fused_capacity.is_some() && draw_workers != 1 {
        return Err(format!(
            "{SWEEP_CUDA_FUSED_DRAWS_ENV} is incompatible with {SWEEP_CONCURRENCY_SPIKE_WORKERS_ENV}>1"
        ));
    }

    let (population_source, population_sha256) =
        manifest::population_identity(&options.population)?;
    let mut run_manifest = manifest::RunManifest::new(
        manifest::ManifestKind::Sweep,
        options.seed,
        options.ticks,
        population_source,
        population_sha256,
    );
    run_manifest.model = Some(model.model().name.clone());
    run_manifest.dt = Some(model.model().dt);
    run_manifest.enabled_features = options.enabled_features.iter().cloned().collect();
    if let Some(plan) = &plan {
        let (plan_identity, linked_source) = manifest::plan_identity_tuples(plan)?;
        run_manifest.plan = Some(plan_identity);
        run_manifest.linked_source = linked_source;
    } else {
        run_manifest.ir_hash = Some(effective_ir_hash.clone());
    }
    run_manifest.noise_mode = Some(options.noise_mode);
    run_manifest.theta_source = Some(match &theta_file {
        Some(theta) => manifest::ThetaSource {
            kind: manifest::ThetaSourceKind::File,
            sha256: theta.sha256.clone(),
            algorithm: manifest::HASH_ALGORITHM.to_owned(),
        },
        None => manifest::ThetaSource {
            kind: manifest::ThetaSourceKind::Prior,
            // Prior-mode theta comes from declarations in the effective,
            // canonical IR. Plan manifests use their plan tuple as run
            // identity, while this digest continues to identify the priors.
            sha256: effective_ir_hash.clone(),
            algorithm: manifest::HASH_ALGORITHM.to_owned(),
        },
    });
    let pinned = match options.params.as_deref() {
        Some(params_path) => read_param_overrides(&model, params_path)?,
        None => Vec::new(),
    };
    let initialized = initialized_tables(&model, &options.population)?;
    run_manifest.initial_state = initialized.state_hash.map(state_artifact_tuple);
    let initial_tables = initialized.tables;
    // Construction values are placeholders only: draw zero also follows the
    // same explicit reset and reseed path as every later draw.
    let construction_params = ParamEnv::defaults(&model);
    let mut final_state_admission = SweepFinalStateAdmission::without_treatment(draw_workers);
    if options.backend == BackendSelection::Cuda
        && ((draw_workers > 1 && concurrency_mode == SweepConcurrencyMode::CudaFreeNonblocking)
            || final_state_selection.mode == SweepCudaFinalStateMode::PackedPinned)
    {
        // This is deliberately before output-directory creation and before any
        // worker thread can construct a retained backend. Packed-pinned also
        // takes this path for one worker so arithmetic/capacity rejection is
        // pre-output even though actual page locking remains lazy per lane.
        final_state_admission = runtime.preflight_cuda_capacity(
            &model,
            &initial_tables,
            draw_workers,
            final_state_selection.mode,
        )?;
    }
    let out = Path::new(&options.out);
    std::fs::create_dir_all(out).map_err(|error| format!("{}: {error}", out.display()))?;
    if let Some(timing_path) = options.timing_json.as_deref().map(Path::new) {
        let output_directory = out
            .canonicalize()
            .map_err(|error| format!("{}: {error}", out.display()))?;
        if canonical_parent_with_final_component(timing_path)
            .is_some_and(|path| path.starts_with(&output_directory))
        {
            return Err(format!(
                "--timing-json path '{}' must be outside the sweep output directory '{}'",
                timing_path.display(),
                out.display()
            ));
        }
        if options
            .export_pairs
            .as_deref()
            .is_some_and(|path| paths_resolve_to_same_file(timing_path, Path::new(path)))
        {
            return Err("--timing-json path conflicts with --export-pairs output".to_owned());
        }
    }
    remove_previous_sweep_outputs(out)?;
    if let Some(export_path) = options.export_pairs.as_deref().map(Path::new) {
        if let Some(parent) = export_path
            .parent()
            .filter(|parent| !parent.as_os_str().is_empty())
        {
            std::fs::create_dir_all(parent)
                .map_err(|error| format!("{}: {error}", parent.display()))?;
        }
    }

    let mut publication = SweepPublication::new(
        &model,
        run_manifest,
        out,
        options.export_pairs.is_some(),
        draw_count,
    );

    let mut csv_manifest = if theta_file.is_some() {
        String::from("# theta_source=file\n# parameter_status")
    } else {
        String::from("# parameter_status")
    };
    for declaration in &model.model().params {
        let supplied_by_file = theta_file.as_ref().is_some_and(|theta| {
            theta.assignments.iter().any(|assignment| {
                assignment
                    .iter()
                    .any(|value| value.name == declaration.name)
            })
        });
        let status = if supplied_by_file {
            "file"
        } else if pinned.iter().any(|pin| pin.name == declaration.name) {
            "pinned"
        } else if declaration.prior.is_some() {
            "sampled"
        } else {
            "default"
        };
        csv_manifest.push_str(&format!(",{}={status}", declaration.name));
    }
    csv_manifest.push_str("\nk");
    for declaration in &model.model().params {
        csv_manifest.push(',');
        csv_manifest.push_str(&declaration.name);
    }
    csv_manifest.push('\n');

    let mut draw_durations = Vec::with_capacity(draw_count as usize);
    let mut draw_final_state_timings = Vec::with_capacity(draw_count as usize);
    let mut concurrency_spike_timing = None;
    let mut fused_spike_timing = None;
    let setup_elapsed;

    if let Some(capacity) = fused_capacity {
        eprintln!(
            "EXPERIMENTAL CUDA fused grid-y spike: capacity {capacity}; default sweep behavior remains sequential"
        );
        let prepared = draw::prepare_draws(
            &model,
            &options,
            theta_file.as_ref(),
            &pinned,
            draw_count,
            &mut csv_manifest,
        )?;
        let fused = run_fused_sweep_spike(
            &model,
            &initial_tables,
            &construction_params,
            &options,
            capacity,
            &prepared,
        )?;
        setup_elapsed = fused.setup_elapsed;
        publication.run_manifest.backend_identity = Some(fused.identity);
        let completion_statuses = fused
            .draws
            .iter()
            .map(|draw| (draw.index, draw.execution.is_err()))
            .collect::<Vec<_>>();
        let publishable_prefix = fused_publishable_prefix(&completion_statuses, prepared.len())?;
        let publication_started = Instant::now();
        let timing_chunks = fused
            .draws
            .chunks(capacity)
            .enumerate()
            .map(|(chunk_index, chunk)| SweepFusedSpikeTimingChunk {
                chunk_index,
                first_k: prepared[chunk[0].index].k,
                active_slots: chunk.len(),
                capacity,
                start_offset_ms: duration_ms(chunk[0].start_offset),
                finish_offset_ms: duration_ms(chunk[0].finish_offset),
                shared_chunk_wall_time_ms: duration_ms(chunk[0].elapsed),
            })
            .collect::<Vec<_>>();
        for (position, completed) in fused.draws.into_iter().enumerate() {
            let prepared_draw = &prepared[completed.index];
            draw_durations.push(completed.elapsed);
            if position == publishable_prefix {
                if let Err(error) = completed.execution {
                    return Err(format!("draw {}: {error}", prepared_draw.k));
                }
                unreachable!("publishable prefix stops at the first failed draw");
            }
            let execution = completed
                .execution
                .expect("draw before fused publishable prefix succeeded");
            publication.publish(prepared_draw, execution)?;
        }
        fused_spike_timing = Some((
            fused.execution_window_elapsed,
            publication_started.elapsed(),
            timing_chunks,
        ));
    } else if draw_workers == 1 {
        let setup_started = Instant::now();
        let mut backend = runtime.new_backend(
            &model,
            initial_tables,
            &construction_params,
            options.seed,
            options.backend,
        )?;
        setup_elapsed = setup_started.elapsed();
        // This sole retained object cannot span devices. Capture identity once
        // at construction in the unchanged manifest field/schema.
        publication.run_manifest.backend_identity = Some(backend.identity());
        // Deliberately sequential: declaration order within each k, then k order.
        for draw in 0..draw_count {
            let prepared =
                draw::prepare_draw(&model, &options, theta_file.as_ref(), &pinned, draw)?;
            draw::append_parameter_manifest_row(&mut csv_manifest, &prepared);
            let draw_started = Instant::now();
            let execution = backend
                .run_draw(
                    &model,
                    &prepared.params,
                    prepared.execution_seed,
                    options.ticks,
                    &options.enabled_features,
                    final_state_selection.mode,
                )
                .map_err(|error| format!("lane 0 of 1: {error}"))?;
            let draw_elapsed = draw_started.elapsed();
            draw_final_state_timings.push(
                execution
                    .final_state
                    .as_ref()
                    .map(|diagnostic| SweepFinalStateTiming::new(diagnostic, draw_elapsed))
                    .transpose()?,
            );
            draw_durations.push(draw_elapsed);
            publication.publish(&prepared, execution)?;
        }
    } else {
        match concurrency_mode {
            SweepConcurrencyMode::CudaLockstepNonblocking => {
                eprintln!(
                    "EXPERIMENTAL CUDA lockstep-stream spike: {draw_workers} draw lanes on non-blocking streams; default sweep behavior remains sequential"
                );
            }
            SweepConcurrencyMode::CudaFreeNonblocking => {
                if supported_workers_present {
                    eprintln!(
                        "CUDA sweep: {draw_workers} retained draw lanes on free-running non-blocking streams"
                    );
                } else {
                    eprintln!(
                        "EXPERIMENTAL CUDA free-running non-blocking-stream spike: {draw_workers} draw lanes on non-blocking streams without tick barriers; default sweep behavior remains sequential"
                    );
                }
            }
            SweepConcurrencyMode::IndependentDefaultStreams => {
                eprintln!(
                    "EXPERIMENTAL sweep concurrency spike: {draw_workers} isolated {:?} backends; default sweep behavior remains sequential",
                    options.backend
                );
            }
        }
        let prepared = draw::prepare_draws(
            &model,
            &options,
            theta_file.as_ref(),
            &pinned,
            draw_count,
            &mut csv_manifest,
        )?;

        if concurrency_mode == SweepConcurrencyMode::CudaFreeNonblocking {
            let concurrent = run_bounded_concurrent_sweep(
                SweepConcurrentInputs {
                    model: &model,
                    initial_tables: &initial_tables,
                    construction_params: &construction_params,
                    options: &options,
                    prepared: &prepared,
                    final_state_mode: final_state_selection.mode,
                },
                draw_workers,
                concurrency_mode,
                runtime,
                |identity, completed| {
                    let prepared_draw = &prepared[completed.index];
                    draw_durations.push(completed.elapsed);
                    let execution = completed
                        .execution
                        .map_err(|error| format!("draw {}: {error}", prepared_draw.k))?;
                    if publication.run_manifest.backend_identity.is_none() {
                        publication.run_manifest.backend_identity = Some(identity.clone());
                    }
                    publication.publish(prepared_draw, execution)?;
                    Ok(())
                },
            )?;
            setup_elapsed = concurrent.setup_elapsed;
            publication.run_manifest.backend_identity = Some(concurrent.identity);
            debug_assert!(concurrent.maximum_pending_results <= draw_workers.saturating_mul(2));
            concurrency_spike_timing = Some((
                concurrent.execution_window_elapsed,
                concurrent.publication_elapsed,
                concurrent.timings,
                concurrent.maximum_pending_results,
            ));
        } else {
            let concurrent = run_concurrent_sweep_spike(
                &model,
                &initial_tables,
                &construction_params,
                &options,
                draw_workers,
                &prepared,
                concurrency_mode,
                final_state_selection.mode,
            )?;
            setup_elapsed = concurrent.setup_elapsed;
            publication.run_manifest.backend_identity = Some(concurrent.identity);
            let publication_started = Instant::now();
            let mut timing_draws = Vec::with_capacity(concurrent.draws.len());
            for completed in concurrent.draws {
                let prepared_draw = &prepared[completed.index];
                let final_state = completed
                    .execution
                    .as_ref()
                    .ok()
                    .and_then(|execution| execution.final_state.as_ref())
                    .map(|diagnostic| SweepFinalStateTiming::new(diagnostic, completed.elapsed))
                    .transpose()?;
                timing_draws.push(SweepConcurrencySpikeTimingDraw {
                    k: prepared_draw.k,
                    lane: completed.lane,
                    start_offset_ms: duration_ms(completed.start_offset),
                    finish_offset_ms: duration_ms(completed.finish_offset),
                    wall_time_ms: duration_ms(completed.elapsed),
                    final_state,
                });
                draw_durations.push(completed.elapsed);
                let execution = completed
                    .execution
                    .map_err(|error| format!("draw {}: {error}", prepared_draw.k))?;
                publication.publish(prepared_draw, execution)?;
            }
            concurrency_spike_timing = Some((
                concurrent.execution_window_elapsed,
                publication_started.elapsed(),
                timing_draws,
                draw_count as usize,
            ));
        }
    }

    let final_publication_started = Instant::now();
    let summary = summary_csv(
        publication.reported_columns.as_deref().unwrap_or_default(),
        &publication.all_series,
        options.ticks,
    )?;
    let manifest_path = out.join("manifest.csv");
    let summary_path = out.join("summary.csv");
    write_atomic(manifest_path, csv_manifest.as_bytes())?;
    write_atomic(summary_path, summary.as_bytes())?;
    manifest::write(&out.join("run-manifest.json"), &publication.run_manifest)?;
    if let (Some(export_path), Some(pairs_csv)) = (
        options.export_pairs.as_deref().map(Path::new),
        publication.pairs_csv,
    ) {
        write_atomic(export_path, pairs_csv.as_bytes())?;
        let pairs_sha256 = hex(&Sha256::digest(pairs_csv.as_bytes()));
        let metadata = manifest::PairsMetadata::for_sweep(
            &publication.run_manifest,
            effective_ir_hash,
            draw_count,
            publication.parameter_columns,
            publication.summary_columns,
            pairs_sha256,
        )?;
        manifest::write_pairs_metadata(&manifest::pairs_sidecar_path(export_path), &metadata)?;
    }
    if let Some((_, publication_elapsed, _, _)) = &mut concurrency_spike_timing {
        *publication_elapsed += final_publication_started.elapsed();
    }
    if let Some(path) = &options.timing_json {
        let backend = match options.backend {
            BackendSelection::Cpu => "cpu",
            BackendSelection::Cuda => "cuda",
        };
        let repository_commit = repository_commit()?;
        let binary_sha256 = current_binary_sha256()?;
        let mut json = if let Some((execution_window, publication, chunks)) = fused_spike_timing {
            let maximum_active_slots = chunks
                .iter()
                .map(|chunk| chunk.active_slots)
                .max()
                .unwrap_or(0);
            serde_json::to_string_pretty(&SweepFusedSpikeTimingDocument {
                schema: "sembla-cuda-fused-draw-spike-timing-v1",
                backend,
                draws: draw_count,
                ticks_per_draw: options.ticks,
                requested_capacity: fused_capacity.expect("fused timing has a capacity"),
                maximum_active_slots,
                setup_wall_time_ms: duration_ms(setup_elapsed),
                execution_window_wall_time_ms: duration_ms(execution_window),
                publication_wall_time_ms: duration_ms(publication),
                chunks,
                whole_sweep_wall_time_ms: duration_ms(sweep_started.elapsed()),
                repository_commit,
                binary_sha256,
            })
        } else if let Some((
            execution_window,
            publication_elapsed,
            timing_draws,
            maximum_pending_results,
        )) = concurrency_spike_timing
        {
            let final_state_buffer_accounting = aggregate_final_state_buffer_accounting(
                final_state_selection.mode,
                final_state_admission,
                timing_draws
                    .iter()
                    .map(|timing| (timing.lane, &timing.final_state)),
            )?;
            serde_json::to_string_pretty(&SweepConcurrencySpikeTimingDocument {
                schema: "sembla-sweep-concurrency-spike-timing-v3",
                backend,
                draws: draw_count,
                ticks_per_draw: options.ticks,
                requested_draw_workers: draw_workers,
                effective_draw_workers: draw_workers,
                maximum_pending_results,
                execution_mode: match concurrency_mode {
                    SweepConcurrencyMode::CudaLockstepNonblocking => {
                        "cuda-lockstep-nonblocking-streams"
                    }
                    SweepConcurrencyMode::CudaFreeNonblocking => "cuda-free-nonblocking-streams",
                    SweepConcurrencyMode::IndependentDefaultStreams => "independent-backends",
                },
                setup_wall_time_ms: duration_ms(setup_elapsed),
                execution_window_wall_time_ms: duration_ms(execution_window),
                publication_wall_time_ms: duration_ms(publication_elapsed),
                final_state_buffer_accounting,
                draw_timings: timing_draws,
                whole_sweep_wall_time_ms: duration_ms(sweep_started.elapsed()),
                repository_commit,
                binary_sha256,
            })
        } else {
            let draw_zero_duration = draw_durations[0];
            let draw_timings = draw_durations
                .into_iter()
                .zip(draw_final_state_timings)
                .enumerate()
                .map(|(k, (elapsed, final_state))| SweepTimingDraw {
                    k: u32::try_from(k).expect("draw count is u32"),
                    wall_time_ms: duration_ms(elapsed),
                    final_state,
                })
                .collect::<Vec<_>>();
            let final_state_buffer_accounting = aggregate_final_state_buffer_accounting(
                final_state_selection.mode,
                final_state_admission,
                draw_timings.iter().map(|timing| (0, &timing.final_state)),
            )?;
            serde_json::to_string_pretty(&SweepTimingDocument {
                schema: "sembla-sweep-timing-v3",
                backend,
                draws: draw_count,
                ticks_per_draw: options.ticks,
                setup_wall_time_ms: duration_ms(setup_elapsed),
                draw_zero_including_setup_wall_time_ms: duration_ms(
                    setup_elapsed + draw_zero_duration,
                ),
                final_state_buffer_accounting,
                draw_timings,
                whole_sweep_wall_time_ms: duration_ms(sweep_started.elapsed()),
                repository_commit,
                binary_sha256,
            })
        }
        .map_err(|error| format!("could not serialize sweep timing JSON: {error}"))?;
        json.push('\n');
        write_atomic(path, json.as_bytes())?;
    }
    let manifest_hash = hex(&Sha256::digest(csv_manifest.as_bytes()));
    let summary_hash = hex(&Sha256::digest(summary.as_bytes()));
    if let Some(theta) = &theta_file {
        println!(
            "manifest_sha256={manifest_hash} summary_sha256={summary_hash} theta_file_sha256={}",
            theta.sha256
        );
    } else {
        println!("manifest_sha256={manifest_hash} summary_sha256={summary_hash}");
    }
    Ok(())
}
