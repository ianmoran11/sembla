//! Retained-lane, bounded, lockstep, and fused sweep scheduling.

use super::*;

#[allow(clippy::too_many_arguments)]
pub(crate) fn run_concurrent_sweep_spike(
    model: &sembla_ir::ValidatedModel,
    initial_tables: &[TableInit],
    construction_params: &ParamEnv,
    options: &SweepOptions,
    workers: usize,
    prepared: &[SweepPreparedDraw],
    mode: SweepConcurrencyMode,
    final_state_mode: SweepCudaFinalStateMode,
) -> Result<SweepConcurrentExecution, String> {
    let setup_started = Instant::now();
    let draw_zero_delay = sweep_concurrency_spike_draw_zero_delay()?;
    let next_draw = std::sync::atomic::AtomicUsize::new(0);
    let lane_construction_failed = std::sync::atomic::AtomicBool::new(false);
    let ready = std::sync::Barrier::new(workers + 1);
    let start = std::sync::Barrier::new(workers + 1);
    let lockstep_tick = std::sync::Barrier::new(workers);
    let execution_started = std::sync::OnceLock::<Instant>::new();
    let (lanes, setup_elapsed, execution_window_elapsed) = std::thread::scope(
        |scope| -> Result<(Vec<_>, Duration, Duration), String> {
            let handles = (0..workers)
                .map(|lane| {
                    let next_draw = &next_draw;
                    let lane_construction_failed = &lane_construction_failed;
                    let ready = &ready;
                    let start = &start;
                    let lockstep_tick = &lockstep_tick;
                    let execution_started = &execution_started;
                    scope.spawn(move || -> Result<_, String> {
                        // CUDA contexts are thread-current. Constructing a backend on
                        // the coordinator and moving it here produces
                        // CUDA_ERROR_INVALID_CONTEXT on the first driver operation.
                        // Each isolated lane therefore owns and uses its backend on
                        // one worker thread for its complete lifetime.
                        let backend = std::panic::catch_unwind(
                            std::panic::AssertUnwindSafe(|| -> Result<_, String> {
                                let backend = SweepBackend::new_concurrency_lane(
                                    model,
                                    initial_tables.to_vec(),
                                    construction_params,
                                    options.seed,
                                    options.backend,
                                    mode,
                                )?;
                                let identity = backend.identity();
                                Ok((identity, backend))
                            }),
                        )
                        .unwrap_or_else(|_| {
                            Err("sweep concurrency spike worker panicked during backend construction"
                                .to_owned())
                        });
                        if backend.is_err() {
                            lane_construction_failed
                                .store(true, std::sync::atomic::Ordering::Release);
                        }
                        // Both barriers must be reached even if construction failed,
                        // otherwise one failed lane would deadlock every healthy lane.
                        ready.wait();
                        start.wait();
                        if lane_construction_failed.load(std::sync::atomic::Ordering::Acquire) {
                            return match backend {
                                Err(error) => Err(error),
                                Ok(_) => {
                                    Err("sweep concurrency spike peer backend construction failed"
                                        .to_owned())
                                }
                            };
                        }
                        let (identity, mut backend) = backend?;
                        let execution_started = *execution_started
                            .get()
                            .expect("coordinator sets execution start before release");
                        let mut completed = Vec::new();
                        if mode == SweepConcurrencyMode::CudaLockstepNonblocking {
                            for index in (lane..prepared.len()).step_by(workers) {
                                let draw = &prepared[index];
                                let start_offset = execution_started.elapsed();
                                let started = Instant::now();
                                if draw.k == 0 && !draw_zero_delay.is_zero() {
                                    std::thread::sleep(draw_zero_delay);
                                }
                                let execution = std::panic::catch_unwind(
                                    std::panic::AssertUnwindSafe(|| {
                                        backend
                                            .run_draw_lockstep(
                                                model,
                                                &draw.params,
                                                draw.execution_seed,
                                                options.ticks,
                                                &options.enabled_features,
                                                lockstep_tick,
                                                final_state_mode,
                                            )
                                            .map_err(|error| {
                                                format!("lane {lane} of {workers}: {error}")
                                            })
                                    }),
                                )
                                .unwrap_or_else(|_| {
                                    Err(format!(
                                        "draw {}: lockstep worker panicked after entering the barrier protocol",
                                        draw.k
                                    ))
                                });
                                let elapsed = started.elapsed();
                                completed.push(SweepConcurrentCompletedDraw {
                                    index,
                                    lane,
                                    start_offset,
                                    finish_offset: execution_started.elapsed(),
                                    elapsed,
                                    execution,
                                });
                            }
                        } else {
                            loop {
                                let index =
                                    next_draw.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                                let Some(draw) = prepared.get(index) else {
                                    break;
                                };
                                let start_offset = execution_started.elapsed();
                                let started = Instant::now();
                                if draw.k == 0 && !draw_zero_delay.is_zero() {
                                    std::thread::sleep(draw_zero_delay);
                                }
                                let execution = backend
                                    .run_draw(
                                        model,
                                        &draw.params,
                                        draw.execution_seed,
                                        options.ticks,
                                        &options.enabled_features,
                                        final_state_mode,
                                    )
                                    .map_err(|error| {
                                        format!("lane {lane} of {workers}: {error}")
                                    });
                                let elapsed = started.elapsed();
                                completed.push(SweepConcurrentCompletedDraw {
                                    index,
                                    lane,
                                    start_offset,
                                    finish_offset: execution_started.elapsed(),
                                    elapsed,
                                    execution,
                                });
                            }
                        }
                        Ok((identity, completed))
                    })
                })
                .collect::<Vec<_>>();
            ready.wait();
            let setup_elapsed = setup_started.elapsed();
            let execution_start = Instant::now();
            execution_started
                .set(execution_start)
                .expect("execution start is set exactly once");
            start.wait();

            let mut lanes = Vec::with_capacity(workers);
            let mut errors = Vec::new();
            for (lane, handle) in handles.into_iter().enumerate() {
                match handle.join() {
                    Ok(Ok(result)) => lanes.push(result),
                    Ok(Err(error)) => errors.push((lane, error)),
                    Err(_) => errors.push((
                        lane,
                        "sweep concurrency spike worker panicked before returning its draws"
                            .to_owned(),
                    )),
                }
            }
            if !errors.is_empty() {
                let selected = errors
                    .iter()
                    .find(|(_, error)| !error.contains("peer backend construction failed"))
                    .unwrap_or(&errors[0]);
                return Err(format!("concurrency lane {}: {}", selected.0, selected.1));
            }
            Ok((lanes, setup_elapsed, execution_start.elapsed()))
        },
    )?;
    let identity = lanes
        .first()
        .expect("concurrency spike has at least one backend")
        .0
        .clone();
    if lanes
        .iter()
        .skip(1)
        .any(|(lane_identity, _)| lane_identity != &identity)
    {
        return Err("sweep concurrency spike backend identity changed across lanes".to_owned());
    }
    let mut draws = lanes
        .into_iter()
        .flat_map(|(_, completed)| completed)
        .collect::<Vec<_>>();
    draws.sort_by_key(|draw| draw.index);
    if draws.len() != prepared.len()
        || draws
            .iter()
            .enumerate()
            .any(|(index, draw)| draw.index != index)
    {
        return Err("sweep concurrency spike did not return every draw exactly once".to_owned());
    }
    Ok(SweepConcurrentExecution {
        setup_elapsed,
        execution_window_elapsed,
        identity,
        draws,
    })
}

pub(crate) struct SweepConcurrentInputs<'a> {
    pub(crate) model: &'a sembla_ir::ValidatedModel,
    pub(crate) initial_tables: &'a [TableInit],
    pub(crate) construction_params: &'a ParamEnv,
    pub(crate) options: &'a SweepOptions,
    pub(crate) prepared: &'a [SweepPreparedDraw],
    pub(crate) final_state_mode: SweepCudaFinalStateMode,
}

pub(crate) struct SweepBoundedConcurrentExecution {
    pub(crate) setup_elapsed: Duration,
    pub(crate) execution_window_elapsed: Duration,
    pub(crate) publication_elapsed: Duration,
    pub(crate) identity: manifest::BackendIdentity,
    pub(crate) timings: Vec<SweepConcurrencySpikeTimingDraw>,
    pub(crate) maximum_pending_results: usize,
}

/// Keeps supported sweep policy testable without changing the production
/// backend selected by that policy. Production delegates every operation to
/// the existing CUDA/CPU implementations; tests may replace only these
/// hardware boundaries while exercising the same option resolution,
/// scheduler, publication, manifest, and timing paths.
pub(crate) trait SweepRuntime: Sync {
    fn preflight_cuda_capacity(
        &self,
        model: &sembla_ir::ValidatedModel,
        initial_tables: &[TableInit],
        workers: usize,
        final_state_mode: SweepCudaFinalStateMode,
    ) -> Result<SweepFinalStateAdmission, String>;

    fn new_backend(
        &self,
        model: &sembla_ir::ValidatedModel,
        initial: Vec<TableInit>,
        initial_params: &ParamEnv,
        seed: u64,
        backend: BackendSelection,
    ) -> Result<SweepBackend, String>;

    fn new_concurrency_lane(
        &self,
        model: &sembla_ir::ValidatedModel,
        initial: Vec<TableInit>,
        initial_params: &ParamEnv,
        seed: u64,
        backend: BackendSelection,
        mode: SweepConcurrencyMode,
    ) -> Result<SweepBackend, String>;
}

pub(crate) struct ProductionSweepRuntime;

impl SweepRuntime for ProductionSweepRuntime {
    fn preflight_cuda_capacity(
        &self,
        model: &sembla_ir::ValidatedModel,
        initial_tables: &[TableInit],
        workers: usize,
        final_state_mode: SweepCudaFinalStateMode,
    ) -> Result<SweepFinalStateAdmission, String> {
        preflight_cuda_sweep_capacity(model, initial_tables, workers, final_state_mode)
    }

    fn new_backend(
        &self,
        model: &sembla_ir::ValidatedModel,
        initial: Vec<TableInit>,
        initial_params: &ParamEnv,
        seed: u64,
        backend: BackendSelection,
    ) -> Result<SweepBackend, String> {
        SweepBackend::new(model, initial, initial_params, seed, backend)
    }

    fn new_concurrency_lane(
        &self,
        model: &sembla_ir::ValidatedModel,
        initial: Vec<TableInit>,
        initial_params: &ParamEnv,
        seed: u64,
        backend: BackendSelection,
        mode: SweepConcurrencyMode,
    ) -> Result<SweepBackend, String> {
        SweepBackend::new_concurrency_lane(model, initial, initial_params, seed, backend, mode)
    }
}

pub(crate) fn run_bounded_concurrent_sweep<R, F>(
    inputs: SweepConcurrentInputs<'_>,
    workers: usize,
    mode: SweepConcurrencyMode,
    runtime: &R,
    mut publish: F,
) -> Result<SweepBoundedConcurrentExecution, String>
where
    R: SweepRuntime,
    F: FnMut(&manifest::BackendIdentity, SweepConcurrentCompletedDraw) -> Result<(), String>,
{
    let SweepConcurrentInputs {
        model,
        initial_tables,
        construction_params,
        options,
        prepared,
        final_state_mode,
    } = inputs;
    let setup_started = Instant::now();
    let draw_zero_delay = sweep_concurrency_spike_draw_zero_delay()?;
    let next_draw = std::sync::atomic::AtomicUsize::new(0);
    // Two lane-widths preserve genuine free-running scheduling beyond the
    // initially active draws while bounding speculative completed results.
    // A delayed low-k draw therefore cannot create storage proportional to the
    // total draw count, but another lane can still claim and run later work.
    let admission_window = workers
        .checked_mul(2)
        .ok_or_else(|| "concurrent sweep admission window overflow".to_owned())?;
    let published_prefix = std::sync::atomic::AtomicUsize::new(0);
    let admission_mutex = std::sync::Mutex::new(());
    let admission_changed = std::sync::Condvar::new();
    let construction_failed = std::sync::atomic::AtomicBool::new(false);
    let identities = std::sync::Mutex::new(
        std::iter::repeat_with(|| None)
            .take(workers)
            .collect::<Vec<Option<Result<manifest::BackendIdentity, String>>>>(),
    );
    let ready = std::sync::Barrier::new(workers + 1);
    let start = std::sync::Barrier::new(workers + 1);
    let execution_started = std::sync::OnceLock::<Instant>::new();
    let (sender, receiver) = std::sync::mpsc::sync_channel(workers);

    let (
        identity,
        setup_elapsed,
        execution_window_elapsed,
        publication_elapsed,
        timings,
        maximum_pending_results,
    ) = std::thread::scope(|scope| -> Result<_, String> {
        let handles = (0..workers)
            .map(|lane| {
                let sender = sender.clone();
                let next_draw = &next_draw;
                let published_prefix = &published_prefix;
                let admission_mutex = &admission_mutex;
                let admission_changed = &admission_changed;
                let construction_failed = &construction_failed;
                let identities = &identities;
                let ready = &ready;
                let start = &start;
                let execution_started = &execution_started;
                scope.spawn(move || -> Result<(), String> {
                    let backend = std::panic::catch_unwind(std::panic::AssertUnwindSafe(
                        || -> Result<_, String> {
                            // The constructor runs here, on the worker thread:
                            // CUDA contexts must never be constructed on the
                            // coordinator and moved across this boundary.
                            let backend = runtime.new_concurrency_lane(
                                model,
                                initial_tables.to_vec(),
                                construction_params,
                                options.seed,
                                options.backend,
                                mode,
                            )?;
                            Ok((backend.identity(), backend))
                        },
                    ))
                    .unwrap_or_else(|_| {
                        Err(
                            "concurrent sweep worker panicked during backend construction"
                                .to_owned(),
                        )
                    });
                    identities.lock().expect("identity mutex poisoned")[lane] = Some(
                        backend
                            .as_ref()
                            .map(|(identity, _)| identity.clone())
                            .map_err(Clone::clone),
                    );
                    ready.wait();
                    start.wait();
                    if construction_failed.load(std::sync::atomic::Ordering::Acquire) {
                        return backend.map(|_| ());
                    }
                    let (_, mut backend) = backend?;
                    let execution_started = *execution_started
                        .get()
                        .expect("coordinator sets execution start before release");
                    loop {
                        let index = next_draw.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                        let Some(draw) = prepared.get(index) else {
                            break;
                        };
                        let mut admission = admission_mutex.lock().map_err(|_| {
                            "concurrent sweep admission mutex was poisoned".to_owned()
                        })?;
                        while index
                            >= published_prefix
                                .load(std::sync::atomic::Ordering::Acquire)
                                .saturating_add(admission_window)
                        {
                            admission = admission_changed.wait(admission).map_err(|_| {
                                "concurrent sweep admission mutex was poisoned".to_owned()
                            })?;
                        }
                        drop(admission);

                        let start_offset = execution_started.elapsed();
                        let started = Instant::now();
                        if draw.k == 0 && !draw_zero_delay.is_zero() {
                            std::thread::sleep(draw_zero_delay);
                        }
                        let execution =
                            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                                backend
                                    .run_draw(
                                        model,
                                        &draw.params,
                                        draw.execution_seed,
                                        options.ticks,
                                        &options.enabled_features,
                                        final_state_mode,
                                    )
                                    .map_err(|error| format!("lane {lane} of {workers}: {error}"))
                            }))
                            .unwrap_or_else(|_| {
                                Err(format!("draw {}: concurrent worker panicked", draw.k))
                            });
                        let completed = SweepConcurrentCompletedDraw {
                            index,
                            lane,
                            start_offset,
                            finish_offset: execution_started.elapsed(),
                            elapsed: started.elapsed(),
                            execution,
                        };
                        sender.send(completed).map_err(|_| {
                            "concurrent sweep coordinator stopped receiving results".to_owned()
                        })?;
                    }
                    Ok(())
                })
            })
            .collect::<Vec<_>>();
        drop(sender);
        ready.wait();
        let setup_elapsed = setup_started.elapsed();

        let identity_result = {
            let identities = identities.lock().expect("identity mutex poisoned");
            let mut errors = identities
                .iter()
                .enumerate()
                .filter_map(|(lane, identity)| {
                    match identity
                        .as_ref()
                        .expect("every ready lane recorded identity")
                    {
                        Ok(_) => None,
                        Err(error) => Some((lane, error.clone())),
                    }
                });
            if let Some((lane, error)) = errors.next() {
                Err(format!("concurrency lane {lane}: {error}"))
            } else {
                let first = identities[0]
                    .as_ref()
                    .expect("lane zero identity exists")
                    .as_ref()
                    .expect("construction errors handled")
                    .clone();
                if identities.iter().skip(1).any(|identity| {
                    identity
                        .as_ref()
                        .expect("ready lane identity exists")
                        .as_ref()
                        .is_ok_and(|identity| identity != &first)
                }) {
                    Err("concurrent sweep backend identity changed across lanes".to_owned())
                } else {
                    Ok(first)
                }
            }
        };
        if identity_result.is_err() {
            construction_failed.store(true, std::sync::atomic::Ordering::Release);
        }
        let execution_start = Instant::now();
        execution_started
            .set(execution_start)
            .expect("execution start is set exactly once");
        start.wait();

        if let Err(error) = identity_result {
            for handle in handles {
                let _ = handle.join();
            }
            return Err(error);
        }
        let identity = identity_result.expect("identity error returned above");
        let mut pending = std::collections::BTreeMap::new();
        let mut next_publish = 0_usize;
        let mut received = 0_usize;
        let mut maximum_pending_results = 0_usize;
        let mut execution_window_elapsed = Duration::ZERO;
        let mut publication_elapsed = Duration::ZERO;
        let mut publication_error = None;
        let mut timings = Vec::with_capacity(prepared.len());

        while received < prepared.len() {
            let completed = receiver.recv().map_err(|_| {
                "concurrent sweep workers stopped before returning every draw".to_owned()
            })?;
            received += 1;
            execution_window_elapsed = execution_window_elapsed.max(completed.finish_offset);
            if pending.insert(completed.index, completed).is_some() {
                return Err("concurrent sweep returned a draw more than once".to_owned());
            }
            maximum_pending_results = maximum_pending_results.max(pending.len());
            while let Some(completed) = pending.remove(&next_publish) {
                let final_state = completed
                    .execution
                    .as_ref()
                    .ok()
                    .and_then(|execution| execution.final_state.as_ref())
                    .map(|diagnostic| SweepFinalStateTiming::new(diagnostic, completed.elapsed))
                    .transpose()?;
                timings.push(SweepConcurrencySpikeTimingDraw {
                    k: prepared[completed.index].k,
                    lane: completed.lane,
                    start_offset_ms: duration_ms(completed.start_offset),
                    finish_offset_ms: duration_ms(completed.finish_offset),
                    wall_time_ms: duration_ms(completed.elapsed),
                    final_state,
                });
                if publication_error.is_none() {
                    let publication_started = Instant::now();
                    if let Err(error) = publish(&identity, completed) {
                        publication_error = Some(error);
                    }
                    publication_elapsed += publication_started.elapsed();
                }
                next_publish += 1;
                published_prefix.store(next_publish, std::sync::atomic::Ordering::Release);
                admission_changed.notify_all();
            }
        }

        let mut lane_errors = Vec::new();
        for (lane, handle) in handles.into_iter().enumerate() {
            match handle.join() {
                Ok(Ok(())) => {}
                Ok(Err(error)) => lane_errors.push(format!("concurrency lane {lane}: {error}")),
                Err(_) => lane_errors.push(format!("concurrency lane {lane}: worker panicked")),
            }
        }
        if let Some(error) = publication_error {
            return Err(error);
        }
        if let Some(error) = lane_errors.into_iter().next() {
            return Err(error);
        }
        if next_publish != prepared.len() || !pending.is_empty() {
            return Err("concurrent sweep did not return every draw exactly once".to_owned());
        }
        Ok((
            identity,
            setup_elapsed,
            execution_window_elapsed,
            publication_elapsed,
            timings,
            maximum_pending_results,
        ))
    })?;

    Ok(SweepBoundedConcurrentExecution {
        setup_elapsed,
        execution_window_elapsed,
        publication_elapsed,
        identity,
        timings,
        maximum_pending_results,
    })
}

pub(crate) fn fused_publishable_prefix(
    statuses: &[(usize, bool)],
    expected_draws: usize,
) -> Result<usize, String> {
    for (position, (index, _)) in statuses.iter().enumerate() {
        if *index != position {
            return Err(format!(
                "fused CUDA spike returned draw index {index} at position {position}"
            ));
        }
    }
    if let Some(position) = statuses.iter().position(|(_, failed)| *failed) {
        return Ok(position);
    }
    if statuses.len() != expected_draws {
        return Err(format!(
            "fused CUDA spike returned {} successful draws, expected {expected_draws}",
            statuses.len()
        ));
    }
    Ok(statuses.len())
}

#[cfg(feature = "cuda")]
pub(crate) fn run_fused_sweep_spike(
    model: &sembla_ir::ValidatedModel,
    initial_tables: &[TableInit],
    construction_params: &ParamEnv,
    options: &SweepOptions,
    capacity: usize,
    prepared: &[SweepPreparedDraw],
) -> Result<SweepConcurrentExecution, String> {
    let setup_started = Instant::now();
    let mut backend = sembla_cuda::CudaBackend::new_fused_batch(
        model,
        initial_tables.to_vec(),
        construction_params,
        options.seed,
        capacity,
        HashMode::FinalOnly,
    )
    .map_err(|error| error.to_string())?;
    report_cuda_observation_eligibility(backend.observation_eligibility());
    let device = backend.device_identity();
    let identity = manifest::BackendIdentity::cuda_native_f64(
        device.gpu_model.clone(),
        device.driver_version.clone(),
    );
    let setup_elapsed = setup_started.elapsed();
    let execution_started = Instant::now();
    let mut completed = Vec::with_capacity(prepared.len());
    'chunks: for chunk in prepared.chunks(capacity) {
        let params = chunk
            .iter()
            .map(|draw| draw.params.clone())
            .collect::<Vec<_>>();
        let seeds = chunk
            .iter()
            .map(|draw| draw.execution_seed)
            .collect::<Vec<_>>();
        let chunk_start = execution_started.elapsed();
        let started = Instant::now();
        if let Err(error) = backend.reset_fused_batch(&params, &seeds) {
            let elapsed = started.elapsed();
            let finish_offset = execution_started.elapsed();
            for (slot, draw) in chunk.iter().enumerate() {
                completed.push(SweepConcurrentCompletedDraw {
                    index: usize::try_from(draw.k).expect("draw index is u32"),
                    lane: slot,
                    start_offset: chunk_start,
                    finish_offset,
                    elapsed,
                    execution: Err(error.to_string()),
                });
            }
            break;
        }
        let mut failures = (0..chunk.len())
            .map(|_| None)
            .collect::<Vec<Option<String>>>();
        let mut outputs = Vec::with_capacity(chunk.len());
        let mut batch_transport_failed = false;
        for (slot, draw) in chunk.iter().enumerate() {
            match RunOutputAccumulator::new(model, &draw.params, options.ticks) {
                Ok(output) => outputs.push(Some(output)),
                Err(error) => {
                    failures[slot] = Some(error);
                    outputs.push(None);
                    if let Err(error) = backend.deactivate_fused_slot(slot) {
                        let error = error.to_string();
                        for failure in &mut failures {
                            failure.get_or_insert_with(|| error.clone());
                        }
                        batch_transport_failed = true;
                        break;
                    }
                }
            }
        }
        outputs.resize_with(chunk.len(), || None);
        for tick in 0..options.ticks {
            if batch_transport_failed || failures.iter().all(Option::is_some) {
                break;
            }
            let observations = match backend.run_tick_observed_reused_fused() {
                Ok(observations) => observations,
                Err(error) => {
                    let error = error.to_string();
                    for failure in &mut failures {
                        failure.get_or_insert_with(|| error.clone());
                    }
                    batch_transport_failed = true;
                    break;
                }
            };
            for (slot, observation) in observations.into_iter().enumerate() {
                if failures[slot].is_some() {
                    continue;
                }
                let processed = (|| -> Result<(), String> {
                    let (observed_tick, fired, deferred, device_views) =
                        observation.map_err(|error| error.to_string())?;
                    debug_assert_eq!(observed_tick, tick);
                    let (views, grouped_views, generic_enum_counts) = match device_views {
                        Some(observation) => (
                            observation.views,
                            observation.grouped_views,
                            observation.generic_enum_counts,
                        ),
                        None => {
                            let state = backend
                                .fused_observed_state(slot)
                                .map_err(|error| error.to_string())?;
                            (
                                executor::observe_views(model, state, &chunk[slot].params)
                                    .map_err(|error| error.to_string())?,
                                executor::observe_grouped_views(model, state, &chunk[slot].params)
                                    .map_err(|error| error.to_string())?,
                                None,
                            )
                        }
                    };
                    let report =
                        cuda_tick_report(model, tick, fired, deferred, views, grouped_views);
                    outputs[slot]
                        .as_mut()
                        .expect("healthy fused slot has an output accumulator")
                        .push_tick_with_enum_counts(
                            backend
                                .fused_observed_state(slot)
                                .map_err(|error| error.to_string())?,
                            tick,
                            report,
                            generic_enum_counts.as_deref(),
                        )
                })();
                if let Err(error) = processed {
                    failures[slot] = Some(format!("tick {tick}: {error}"));
                    if let Err(error) = backend.deactivate_fused_slot(slot) {
                        let error = error.to_string();
                        for failure in &mut failures {
                            failure.get_or_insert_with(|| error.clone());
                        }
                        batch_transport_failed = true;
                        break;
                    }
                }
            }
            if batch_transport_failed {
                break;
            }
        }
        let mut states = vec![None; chunk.len()];
        if failures.iter().any(Option::is_none) {
            match backend.ensure_fused_observed_states() {
                Ok(results) => {
                    for (slot, result) in results.into_iter().enumerate() {
                        if failures[slot].is_some() {
                            continue;
                        }
                        match result {
                            Ok(Some(state)) => states[slot] = Some(state),
                            Ok(None) => {
                                failures[slot] = Some(
                                    "healthy fused slot has no reconstructed final state"
                                        .to_owned(),
                                );
                            }
                            Err(error) => failures[slot] = Some(error.to_string()),
                        }
                    }
                }
                Err(error) => {
                    let error = error.to_string();
                    for failure in &mut failures {
                        failure.get_or_insert_with(|| error.clone());
                    }
                    batch_transport_failed = true;
                }
            }
        }
        let elapsed = started.elapsed();
        let finish_offset = execution_started.elapsed();
        for (slot, draw) in chunk.iter().enumerate() {
            let execution = if let Some(error) = failures[slot].take() {
                Err(error)
            } else {
                outputs[slot]
                    .take()
                    .expect("healthy fused slot has an output accumulator")
                    .finish(model, None)
                    .and_then(|output| {
                        let state = states[slot]
                            .as_ref()
                            .ok_or_else(|| "healthy fused slot has no final state".to_owned())?;
                        Ok(SweepDrawOutput {
                            output,
                            final_state_hash: state.state_hash(),
                            final_state: None,
                        })
                    })
            };
            completed.push(SweepConcurrentCompletedDraw {
                index: usize::try_from(draw.k).expect("draw index is u32"),
                lane: slot,
                start_offset: chunk_start,
                finish_offset,
                elapsed,
                execution,
            });
        }
        if batch_transport_failed {
            break 'chunks;
        }
    }
    Ok(SweepConcurrentExecution {
        setup_elapsed,
        execution_window_elapsed: execution_started.elapsed(),
        identity,
        draws: completed,
    })
}

#[cfg(not(feature = "cuda"))]
pub(crate) fn run_fused_sweep_spike(
    _model: &sembla_ir::ValidatedModel,
    _initial_tables: &[TableInit],
    _construction_params: &ParamEnv,
    _options: &SweepOptions,
    _capacity: usize,
    _prepared: &[SweepPreparedDraw],
) -> Result<SweepConcurrentExecution, String> {
    Err("cuda fused-draw spike unavailable: crate built without the 'cuda' feature".to_owned())
}
