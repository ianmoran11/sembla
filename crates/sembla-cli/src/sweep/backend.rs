//! Retained CPU/CUDA draw backends and final-state diagnostics.

use super::*;

#[cfg(test)]
pub(crate) static SWEEP_BACKEND_CONSTRUCTIONS: std::sync::atomic::AtomicUsize =
    std::sync::atomic::AtomicUsize::new(0);
#[cfg(test)]
pub(crate) static SWEEP_BACKEND_CONSTRUCTION_TEST_LOCK: std::sync::Mutex<()> =
    std::sync::Mutex::new(());

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct SweepFinalStateDownloadedBytes {
    pub(crate) state: usize,
    pub(crate) inputs: usize,
    pub(crate) input_counts: usize,
    pub(crate) total: usize,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct SweepFinalStateBufferAccounting {
    pub(crate) buffer_set_count: usize,
    pub(crate) underlying_pinned_allocation_count: usize,
    pub(crate) pinned_bytes: usize,
    pub(crate) cacheable_staging_bytes: usize,
}

#[derive(Clone, Debug)]
pub(crate) struct SweepFinalStateDiagnostic {
    pub(crate) mode: SweepCudaFinalStateMode,
    pub(crate) allocation: Duration,
    pub(crate) pageable_dtoh_host_api: Option<Duration>,
    pub(crate) pinned_dtoh_enqueue_api: Option<Duration>,
    pub(crate) wait_to_pinned_host_readable: Option<Duration>,
    pub(crate) pinned_to_cacheable_staging_copy: Option<Duration>,
    pub(crate) host_state_reconstruction: Option<Duration>,
    pub(crate) cpu_sha256: Duration,
    pub(crate) total: Duration,
    pub(crate) downloaded_bytes: SweepFinalStateDownloadedBytes,
    pub(crate) buffer_accounting: SweepFinalStateBufferAccounting,
}

#[cfg(feature = "cuda")]
impl From<sembla_cuda::CudaFinalStateReadback> for SweepFinalStateDiagnostic {
    fn from(value: sembla_cuda::CudaFinalStateReadback) -> Self {
        Self {
            mode: match value.mode {
                sembla_cuda::CudaFinalStateReadbackMode::Materialized => {
                    SweepCudaFinalStateMode::Materialized
                }
                sembla_cuda::CudaFinalStateReadbackMode::PackedPageable => {
                    SweepCudaFinalStateMode::PackedPageable
                }
                sembla_cuda::CudaFinalStateReadbackMode::PackedPinned => {
                    SweepCudaFinalStateMode::PackedPinned
                }
            },
            allocation: value.allocation,
            pageable_dtoh_host_api: value.pageable_dtoh_host_api,
            pinned_dtoh_enqueue_api: value.pinned_dtoh_enqueue_api,
            wait_to_pinned_host_readable: value.wait_to_pinned_host_readable,
            pinned_to_cacheable_staging_copy: value.pinned_to_cacheable_staging_copy,
            host_state_reconstruction: value.host_state_reconstruction,
            cpu_sha256: value.cpu_sha256,
            total: value.total,
            downloaded_bytes: SweepFinalStateDownloadedBytes {
                state: value.downloaded_bytes.state,
                inputs: value.downloaded_bytes.inputs,
                input_counts: value.downloaded_bytes.input_counts,
                total: value.downloaded_bytes.total,
            },
            buffer_accounting: SweepFinalStateBufferAccounting {
                buffer_set_count: value.buffer_accounting.buffer_set_count,
                underlying_pinned_allocation_count: value
                    .buffer_accounting
                    .underlying_pinned_allocation_count,
                pinned_bytes: value.buffer_accounting.pinned_bytes,
                cacheable_staging_bytes: value.buffer_accounting.cacheable_staging_bytes,
            },
        }
    }
}

pub(crate) enum SweepBackend {
    Cpu {
        state: StateStore,
        initial: Vec<TableInit>,
    },
    #[cfg(feature = "cuda")]
    Cuda(CudaBackend),
    #[cfg(test)]
    LocalConformance {
        state: StateStore,
        initial: Vec<TableInit>,
        pinned_buffer_allocated: bool,
        fail_pinned_allocation: bool,
    },
}

pub(crate) struct SweepDrawOutput {
    pub(super) output: RunOutput,
    pub(super) final_state_hash: [u8; 32],
    pub(super) final_state: Option<SweepFinalStateDiagnostic>,
}

impl SweepBackend {
    pub(super) fn new(
        model: &sembla_ir::ValidatedModel,
        initial: Vec<TableInit>,
        initial_params: &ParamEnv,
        seed: u64,
        backend: BackendSelection,
    ) -> Result<Self, String> {
        #[cfg(test)]
        SWEEP_BACKEND_CONSTRUCTIONS.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        match backend {
            BackendSelection::Cpu => {
                let state =
                    StateStore::new(model, initial.clone()).map_err(|error| error.to_string())?;
                Ok(Self::Cpu { state, initial })
            }
            BackendSelection::Cuda => {
                #[cfg(feature = "cuda")]
                {
                    let backend =
                        CudaBackend::new(model, initial, initial_params, seed, HashMode::FinalOnly)
                            .map_err(|error| error.to_string())?;
                    report_cuda_observation_eligibility(backend.observation_eligibility());
                    Ok(Self::Cuda(backend))
                }
                #[cfg(not(feature = "cuda"))]
                {
                    let _ = (model, initial, initial_params, seed);
                    Err(
                        "cuda backend unavailable: crate built without the 'cuda' feature"
                            .to_owned(),
                    )
                }
            }
        }
    }

    #[cfg(test)]
    pub(crate) fn new_local_conformance(
        model: &sembla_ir::ValidatedModel,
        initial: Vec<TableInit>,
    ) -> Result<Self, String> {
        Self::new_local_conformance_with_pinned_failure(model, initial, false)
    }

    #[cfg(test)]
    pub(crate) fn new_local_conformance_with_pinned_failure(
        model: &sembla_ir::ValidatedModel,
        initial: Vec<TableInit>,
        fail_pinned_allocation: bool,
    ) -> Result<Self, String> {
        let state = StateStore::new(model, initial.clone()).map_err(|error| error.to_string())?;
        Ok(Self::LocalConformance {
            state,
            initial,
            pinned_buffer_allocated: false,
            fail_pinned_allocation,
        })
    }

    pub(super) fn new_concurrency_lane(
        model: &sembla_ir::ValidatedModel,
        initial: Vec<TableInit>,
        initial_params: &ParamEnv,
        seed: u64,
        backend: BackendSelection,
        mode: SweepConcurrencyMode,
    ) -> Result<Self, String> {
        if mode == SweepConcurrencyMode::IndependentDefaultStreams {
            return Self::new(model, initial, initial_params, seed, backend);
        }
        #[cfg(test)]
        SWEEP_BACKEND_CONSTRUCTIONS.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        match backend {
            BackendSelection::Cuda => {
                #[cfg(feature = "cuda")]
                {
                    let backend = CudaBackend::new_nonblocking_stream(
                        model,
                        initial,
                        initial_params,
                        seed,
                        HashMode::FinalOnly,
                    )
                    .map_err(|error| error.to_string())?;
                    report_cuda_observation_eligibility(backend.observation_eligibility());
                    Ok(Self::Cuda(backend))
                }
                #[cfg(not(feature = "cuda"))]
                {
                    let _ = (model, initial, initial_params, seed);
                    Err(
                        "cuda backend unavailable: crate built without the 'cuda' feature"
                            .to_owned(),
                    )
                }
            }
            BackendSelection::Cpu => {
                Err("CUDA non-blocking-stream spike requires --backend cuda".to_owned())
            }
        }
    }

    pub(super) fn identity(&self) -> manifest::BackendIdentity {
        match self {
            Self::Cpu { .. } => manifest::BackendIdentity::cpu_oracle(),
            #[cfg(test)]
            Self::LocalConformance { .. } => manifest::BackendIdentity::cpu_oracle(),
            #[cfg(feature = "cuda")]
            Self::Cuda(backend) => {
                let device = backend.device_identity();
                manifest::BackendIdentity::cuda_native_f64(
                    device.gpu_model.clone(),
                    device.driver_version.clone(),
                )
            }
        }
    }

    pub(super) fn run_draw(
        &mut self,
        model: &sembla_ir::ValidatedModel,
        params: &ParamEnv,
        seed: u64,
        ticks: u32,
        enabled_features: &FeatureSet,
        _final_state_mode: SweepCudaFinalStateMode,
    ) -> Result<SweepDrawOutput, String> {
        match self {
            Self::Cpu { state, initial } => {
                state
                    .reset_backend_draw(model, initial)
                    .map_err(|error| error.to_string())?;
                let output = run_results_output_with_features(
                    model,
                    state,
                    params,
                    seed,
                    ticks,
                    HashMode::FinalOnly,
                    enabled_features,
                )?;
                Ok(SweepDrawOutput {
                    output,
                    final_state_hash: state.state_hash(),
                    final_state: None,
                })
            }
            #[cfg(test)]
            Self::LocalConformance {
                state,
                initial,
                pinned_buffer_allocated,
                fail_pinned_allocation,
            } => {
                state
                    .reset_backend_draw(model, initial)
                    .map_err(|error| error.to_string())?;
                let output = run_results_output_with_features(
                    model,
                    state,
                    params,
                    seed,
                    ticks,
                    HashMode::FinalOnly,
                    enabled_features,
                )?;
                let allocation = if _final_state_mode == SweepCudaFinalStateMode::PackedPinned
                    && !*pinned_buffer_allocated
                {
                    if *fail_pinned_allocation {
                        return Err(
                            "injected packed-pinned cacheable staging allocation failure: requested 1 byte for one lane; no pageable fallback"
                                .to_owned(),
                        );
                    }
                    *pinned_buffer_allocated = true;
                    Duration::from_nanos(1)
                } else {
                    Duration::ZERO
                };
                let seam_started = Instant::now();
                let hash_started = Instant::now();
                let final_state_hash = state.state_hash();
                let cpu_sha256 = hash_started.elapsed();
                let downloaded_bytes = match _final_state_mode {
                    SweepCudaFinalStateMode::Materialized => {
                        SweepFinalStateDownloadedBytes::default()
                    }
                    SweepCudaFinalStateMode::PackedPageable
                    | SweepCudaFinalStateMode::PackedPinned => SweepFinalStateDownloadedBytes {
                        state: 1,
                        inputs: 0,
                        input_counts: 0,
                        total: 1,
                    },
                };
                let buffer_accounting =
                    if _final_state_mode == SweepCudaFinalStateMode::PackedPinned {
                        SweepFinalStateBufferAccounting {
                            buffer_set_count: 1,
                            underlying_pinned_allocation_count: 1,
                            pinned_bytes: 1,
                            cacheable_staging_bytes: 1,
                        }
                    } else {
                        SweepFinalStateBufferAccounting::default()
                    };
                Ok(SweepDrawOutput {
                    output,
                    final_state_hash,
                    final_state: Some(SweepFinalStateDiagnostic {
                        mode: _final_state_mode,
                        allocation,
                        pageable_dtoh_host_api: (_final_state_mode
                            != SweepCudaFinalStateMode::PackedPinned)
                            .then_some(Duration::ZERO),
                        pinned_dtoh_enqueue_api: (_final_state_mode
                            == SweepCudaFinalStateMode::PackedPinned)
                            .then_some(Duration::ZERO),
                        wait_to_pinned_host_readable: (_final_state_mode
                            == SweepCudaFinalStateMode::PackedPinned)
                            .then_some(Duration::ZERO),
                        pinned_to_cacheable_staging_copy: (_final_state_mode
                            == SweepCudaFinalStateMode::PackedPinned)
                            .then_some(Duration::ZERO),
                        host_state_reconstruction: matches!(
                            _final_state_mode,
                            SweepCudaFinalStateMode::Materialized
                        )
                        .then_some(Duration::ZERO),
                        cpu_sha256,
                        total: seam_started.elapsed(),
                        downloaded_bytes,
                        buffer_accounting,
                    }),
                })
            }
            #[cfg(feature = "cuda")]
            Self::Cuda(backend) => {
                backend
                    .reset_draw(params, seed)
                    .map_err(|error| error.to_string())?;
                let mut output = RunOutputAccumulator::new(model, params, ticks)?;
                for tick in 0..ticks {
                    run_cuda_tick(backend, model, params, &mut output, tick)?;
                }
                let output = output.finish(model, None)?;
                let final_state = backend
                    .final_state_readback(_final_state_mode.cuda())
                    .map_err(|error| error.to_string())?;
                let final_state_hash = final_state.digest;
                Ok(SweepDrawOutput {
                    output,
                    final_state_hash,
                    final_state: Some(final_state.into()),
                })
            }
        }
    }

    #[allow(clippy::too_many_arguments)]
    pub(super) fn run_draw_lockstep(
        &mut self,
        model: &sembla_ir::ValidatedModel,
        params: &ParamEnv,
        seed: u64,
        ticks: u32,
        _enabled_features: &FeatureSet,
        tick_barrier: &std::sync::Barrier,
        final_state_mode: SweepCudaFinalStateMode,
    ) -> Result<SweepDrawOutput, String> {
        #[cfg(not(feature = "cuda"))]
        let _ = (model, params, seed, final_state_mode);
        match self {
            Self::Cpu { .. } => {
                // Preserve the barrier protocol even on an impossible route so
                // a validation error cannot strand CUDA peers.
                tick_barrier.wait();
                for _ in 0..ticks {
                    tick_barrier.wait();
                }
                Err("CUDA lockstep-stream spike requires --backend cuda".to_owned())
            }
            #[cfg(test)]
            Self::LocalConformance { .. } => {
                tick_barrier.wait();
                for _ in 0..ticks {
                    tick_barrier.wait();
                }
                Err("local conformance backend does not implement lockstep mode".to_owned())
            }
            #[cfg(feature = "cuda")]
            Self::Cuda(backend) => {
                let reset = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    backend
                        .reset_draw(params, seed)
                        .map_err(|error| error.to_string())
                }))
                .unwrap_or_else(|_| {
                    Err("lockstep worker panicked while resetting its draw".to_owned())
                });
                let mut failure = reset.err();
                let mut output = if failure.is_none() {
                    match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                        RunOutputAccumulator::new(model, params, ticks)
                    }))
                    .unwrap_or_else(|_| {
                        Err(
                            "lockstep worker panicked while creating its output accumulator"
                                .to_owned(),
                        )
                    }) {
                        Ok(output) => Some(output),
                        Err(error) => {
                            failure = Some(error);
                            None
                        }
                    }
                } else {
                    None
                };

                // All lanes finish reset before tick zero. Every lane reaches
                // every later barrier even after a local error, so one failing
                // draw cannot deadlock its peers.
                tick_barrier.wait();
                for tick in 0..ticks {
                    if failure.is_none() {
                        let tick_result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(
                            || -> Result<(), String> {
                                run_cuda_tick(
                                    backend,
                                    model,
                                    params,
                                    output
                                        .as_mut()
                                        .expect("output exists while lockstep draw is healthy"),
                                    tick,
                                )
                            },
                        ))
                        .unwrap_or_else(|_| Err(format!("tick {tick}: lockstep worker panicked")));
                        if let Err(error) = tick_result {
                            failure = Some(error);
                        }
                    }
                    tick_barrier.wait();
                }
                if let Some(error) = failure {
                    return Err(error);
                }
                let output = output
                    .expect("healthy lockstep draw has an accumulator")
                    .finish(model, None)?;
                let final_state = backend
                    .final_state_readback(final_state_mode.cuda())
                    .map_err(|error| error.to_string())?;
                let final_state_hash = final_state.digest;
                Ok(SweepDrawOutput {
                    output,
                    final_state_hash,
                    final_state: Some(final_state.into()),
                })
            }
        }
    }
}

#[cfg(feature = "cuda")]
fn run_cuda_tick(
    backend: &mut CudaBackend,
    model: &sembla_ir::ValidatedModel,
    params: &ParamEnv,
    output: &mut RunOutputAccumulator,
    tick: u32,
) -> Result<(), String> {
    let (observed_tick, fired_per_box, deferred_per_resource_table, device_views) = backend
        .run_tick_observed_reused()
        .map_err(|error| format!("tick {tick}: {error}"))?;
    debug_assert_eq!(observed_tick, tick);
    let (views, grouped_views, generic_enum_counts) = match device_views {
        Some(observation) => (
            observation.views,
            observation.grouped_views,
            observation.generic_enum_counts,
        ),
        None => {
            let views = executor::observe_views(model, backend.observed_state(), params)
                .map_err(|error| format!("tick {tick}: {error}"))?;
            let grouped_views =
                executor::observe_grouped_views(model, backend.observed_state(), params)
                    .map_err(|error| format!("tick {tick}: {error}"))?;
            (views, grouped_views, None)
        }
    };
    let report = cuda_tick_report(
        model,
        tick,
        fired_per_box,
        deferred_per_resource_table,
        views,
        grouped_views,
    );
    output.push_tick_with_enum_counts(
        backend.observed_state(),
        tick,
        report,
        generic_enum_counts.as_deref(),
    )
}
