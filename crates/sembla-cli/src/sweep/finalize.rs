//! Final sweep artifacts, timing documents, and published hashes.

use super::*;

type ConcurrencyTiming = (
    Duration,
    Duration,
    Vec<SweepConcurrencySpikeTimingDraw>,
    usize,
);
type FusedTiming = (Duration, Duration, Vec<SweepFusedSpikeTimingChunk>);

pub(super) struct SweepFinalization<'a> {
    pub options: &'a SweepOptions,
    pub publication: SweepPublication<'a>,
    pub csv_manifest: String,
    pub effective_ir_hash: String,
    pub theta_sha256: Option<&'a str>,
    pub draw_count: u32,
    pub draw_workers: usize,
    pub concurrency_mode: SweepConcurrencyMode,
    pub fused_capacity: Option<usize>,
    pub final_state_selection: SweepCudaFinalStateSelection,
    pub final_state_admission: SweepFinalStateAdmission,
    pub draw_durations: Vec<Duration>,
    pub draw_final_state_timings: Vec<Option<SweepFinalStateTiming>>,
    pub concurrency_timing: Option<ConcurrencyTiming>,
    pub fused_timing: Option<FusedTiming>,
    pub setup_elapsed: Duration,
    pub sweep_started: Instant,
}

pub(super) fn finalize_sweep(input: SweepFinalization<'_>) -> Result<(), String> {
    let SweepFinalization {
        options,
        publication,
        csv_manifest,
        effective_ir_hash,
        theta_sha256,
        draw_count,
        draw_workers,
        concurrency_mode,
        fused_capacity,
        final_state_selection,
        final_state_admission,
        draw_durations,
        draw_final_state_timings,
        mut concurrency_timing,
        fused_timing,
        setup_elapsed,
        sweep_started,
    } = input;
    let final_publication_started = Instant::now();
    let summary = summary_csv(
        publication.reported_columns.as_deref().unwrap_or_default(),
        &publication.all_series,
        options.ticks,
    )?;
    let out = Path::new(&options.out);
    write_atomic(out.join("manifest.csv"), csv_manifest.as_bytes())?;
    write_atomic(out.join("summary.csv"), summary.as_bytes())?;
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
    if let Some((_, publication_elapsed, _, _)) = &mut concurrency_timing {
        *publication_elapsed += final_publication_started.elapsed();
    }
    if let Some(path) = &options.timing_json {
        let backend = match options.backend {
            BackendSelection::Cpu => "cpu",
            BackendSelection::Cuda => "cuda",
        };
        let repository_commit = repository_commit()?;
        let binary_sha256 = current_binary_sha256()?;
        let mut json = if let Some((execution_window, publication, chunks)) = fused_timing {
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
        } else if let Some((execution_window, publication, draws, maximum_pending_results)) =
            concurrency_timing
        {
            let final_state_buffer_accounting = aggregate_final_state_buffer_accounting(
                final_state_selection.mode,
                final_state_admission,
                draws
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
                publication_wall_time_ms: duration_ms(publication),
                final_state_buffer_accounting,
                draw_timings: draws,
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
    if let Some(theta_sha256) = theta_sha256 {
        println!(
            "manifest_sha256={manifest_hash} summary_sha256={summary_hash} theta_file_sha256={theta_sha256}"
        );
    } else {
        println!("manifest_sha256={manifest_hash} summary_sha256={summary_hash}");
    }
    Ok(())
}
