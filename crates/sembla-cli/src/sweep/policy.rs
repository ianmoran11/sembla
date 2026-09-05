//! Sweep execution-policy selection and CUDA capacity admission.

use super::*;

pub(crate) const SWEEP_CONCURRENCY_SPIKE_WORKERS_ENV: &str = "SEMBLA_SWEEP_SPIKE_DRAW_WORKERS";
pub(crate) const SWEEP_CONCURRENCY_SPIKE_DELAY_DRAW_ZERO_ENV: &str =
    "SEMBLA_SWEEP_SPIKE_DELAY_DRAW_ZERO_MS";
pub(crate) const SWEEP_CONCURRENCY_SPIKE_CUDA_LOCKSTEP_ENV: &str =
    "SEMBLA_SWEEP_SPIKE_CUDA_LOCKSTEP_STREAMS";
pub(crate) const SWEEP_CONCURRENCY_SPIKE_CUDA_FREE_STREAMS_ENV: &str =
    "SEMBLA_SWEEP_SPIKE_CUDA_FREE_STREAMS";
pub(crate) const SWEEP_CUDA_FUSED_DRAWS_ENV: &str = "SEMBLA_SWEEP_SPIKE_CUDA_FUSED_DRAWS";
/// Hidden override for CUDA sweep final-state extraction. Packed pageable is
/// the production default; this is not a supported CLI option and never enters
/// scientific manifests.
pub(crate) const SWEEP_CUDA_FINAL_STATE_MODE_ENV: &str = "SEMBLA_SWEEP_CUDA_FINAL_STATE_MODE";
pub(crate) const RETIRED_SWEEP_CUDA_DEVICE_FINAL_SHA256_ENV: &str =
    "SEMBLA_SWEEP_EXPERIMENT_DEVICE_FINAL_SHA256";
pub(crate) const RETIRED_SWEEP_CUDA_DEVICE_FINAL_SHA256_VERIFY_ENV: &str =
    "SEMBLA_SWEEP_EXPERIMENT_DEVICE_FINAL_SHA256_VERIFY";

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) enum SweepCudaFinalStateMode {
    Materialized,
    #[default]
    PackedPageable,
    PackedPinned,
}

impl SweepCudaFinalStateMode {
    pub(super) const fn label(self) -> &'static str {
        match self {
            Self::Materialized => "materialized",
            Self::PackedPageable => "packed-pageable",
            Self::PackedPinned => "packed-pinned",
        }
    }

    #[cfg(feature = "cuda")]
    pub(super) const fn cuda(self) -> sembla_cuda::CudaFinalStateReadbackMode {
        match self {
            Self::Materialized => sembla_cuda::CudaFinalStateReadbackMode::Materialized,
            Self::PackedPageable => sembla_cuda::CudaFinalStateReadbackMode::PackedPageable,
            Self::PackedPinned => sembla_cuda::CudaFinalStateReadbackMode::PackedPinned,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct SweepCudaFinalStateSelection {
    pub(crate) mode: SweepCudaFinalStateMode,
    pub(crate) explicitly_set: bool,
}

pub(crate) fn sweep_cuda_final_state_selection(
    backend: BackendSelection,
) -> Result<SweepCudaFinalStateSelection, String> {
    let selected = std::env::var_os(SWEEP_CUDA_FINAL_STATE_MODE_ENV);
    let retired = [
        RETIRED_SWEEP_CUDA_DEVICE_FINAL_SHA256_ENV,
        RETIRED_SWEEP_CUDA_DEVICE_FINAL_SHA256_VERIFY_ENV,
    ]
    .into_iter()
    .filter(|name| std::env::var_os(name).is_some())
    .collect::<Vec<_>>();
    if !retired.is_empty() {
        let names = retired.join(", ");
        if selected.is_some() {
            return Err(format!(
                "{SWEEP_CUDA_FINAL_STATE_MODE_ENV} conflicts with retired final-state variables: {names}"
            ));
        }
        return Err(format!(
            "retired CUDA device-SHA variable(s) are no longer accepted: {names}; use {SWEEP_CUDA_FINAL_STATE_MODE_ENV}=materialized|packed-pageable|packed-pinned for CUDA sweep diagnostics"
        ));
    }
    let Some(raw) = selected else {
        return Ok(SweepCudaFinalStateSelection::default());
    };
    let raw = raw
        .into_string()
        .map_err(|_| format!("{SWEEP_CUDA_FINAL_STATE_MODE_ENV} contains invalid UTF-8"))?;
    let mode = match raw.as_str() {
        "materialized" => SweepCudaFinalStateMode::Materialized,
        "packed-pageable" => SweepCudaFinalStateMode::PackedPageable,
        "packed-pinned" => SweepCudaFinalStateMode::PackedPinned,
        _ => {
            return Err(format!(
                "invalid {SWEEP_CUDA_FINAL_STATE_MODE_ENV} value '{raw}' (expected 'materialized', 'packed-pageable', or 'packed-pinned')"
            ));
        }
    };
    if backend != BackendSelection::Cuda {
        return Err(format!(
            "{SWEEP_CUDA_FINAL_STATE_MODE_ENV} is accepted only for CUDA sweep execution"
        ));
    }
    Ok(SweepCudaFinalStateSelection {
        mode,
        explicitly_set: true,
    })
}

pub(crate) fn reject_cuda_sweep_final_state_selector_for_run() -> Result<(), String> {
    if std::env::var_os(SWEEP_CUDA_FINAL_STATE_MODE_ENV).is_some() {
        return Err(format!(
            "{SWEEP_CUDA_FINAL_STATE_MODE_ENV} is accepted only for CUDA sweep execution, not 'run'"
        ));
    }
    for name in [
        RETIRED_SWEEP_CUDA_DEVICE_FINAL_SHA256_ENV,
        RETIRED_SWEEP_CUDA_DEVICE_FINAL_SHA256_VERIFY_ENV,
    ] {
        if std::env::var_os(name).is_some() {
            return Err(format!(
                "retired CUDA device-SHA variable {name} is no longer accepted"
            ));
        }
    }
    Ok(())
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum SweepConcurrencyMode {
    /// Isolated retained backends on default streams, dynamically scheduled.
    IndependentDefaultStreams,
    /// Isolated retained backends on non-blocking streams with tick barriers.
    CudaLockstepNonblocking,
    /// Isolated retained backends on non-blocking streams, dynamically
    /// scheduled with no tick barriers: lanes advance as fast as their own
    /// readbacks complete.
    CudaFreeNonblocking,
}

#[derive(Clone)]
pub(crate) struct SweepPreparedDraw {
    pub(crate) k: u32,
    pub(crate) params: ParamEnv,
    pub(crate) execution_seed: u64,
}

pub(crate) struct SweepConcurrentCompletedDraw {
    pub(crate) index: usize,
    pub(crate) lane: usize,
    pub(crate) start_offset: Duration,
    pub(crate) finish_offset: Duration,
    pub(crate) elapsed: Duration,
    pub(crate) execution: Result<SweepDrawOutput, String>,
}

pub(crate) struct SweepConcurrentExecution {
    pub(super) setup_elapsed: Duration,
    pub(super) execution_window_elapsed: Duration,
    pub(super) identity: manifest::BackendIdentity,
    pub(super) draws: Vec<SweepConcurrentCompletedDraw>,
}

pub(crate) fn sweep_draw_workers(
    draw_count: u32,
    backend: BackendSelection,
    supported_workers: Option<usize>,
) -> Result<usize, String> {
    let hidden_workers = std::env::var_os(SWEEP_CONCURRENCY_SPIKE_WORKERS_ENV)
        .map(|raw| {
            let raw = raw.to_string_lossy();
            let workers = raw.parse::<usize>().map_err(|_| {
                format!(
                    "{SWEEP_CONCURRENCY_SPIKE_WORKERS_ENV} must be a positive integer, found '{raw}'"
                )
            })?;
            if workers == 0 {
                return Err(format!(
                    "{SWEEP_CONCURRENCY_SPIKE_WORKERS_ENV} must be greater than zero"
                ));
            }
            Ok(workers)
        })
        .transpose()?;
    if let (Some(supported), Some(hidden)) = (supported_workers, hidden_workers) {
        if supported != hidden {
            return Err(format!(
                "--draw-workers {supported} conflicts with {SWEEP_CONCURRENCY_SPIKE_WORKERS_ENV}={hidden}"
            ));
        }
    }
    let workers = supported_workers.or(hidden_workers).unwrap_or(1);
    let draw_count = usize::try_from(draw_count).expect("draw count is u32");
    if workers > draw_count {
        let source = if supported_workers.is_some() {
            format!("--draw-workers {workers}")
        } else {
            format!("{SWEEP_CONCURRENCY_SPIKE_WORKERS_ENV}={workers}")
        };
        return Err(format!("{source} exceeds draw count {draw_count}"));
    }
    if supported_workers.is_some() && workers > 1 && backend != BackendSelection::Cuda {
        return Err("--draw-workers greater than 1 requires --backend cuda".to_owned());
    }
    // Preserve the default-off CPU evidence seam while keeping it unreachable
    // through the supported CUDA-only option.
    if supported_workers.is_none()
        && workers > 1
        && backend == BackendSelection::Cpu
        && std::env::var_os("SEMBLA_EVAL_THREADS").is_none()
    {
        return Err(format!(
            "{SWEEP_CONCURRENCY_SPIKE_WORKERS_ENV}>1 with --backend cpu requires an explicit SEMBLA_EVAL_THREADS budget per draw"
        ));
    }
    Ok(workers)
}

pub(crate) fn sweep_concurrency_mode(
    draw_count: u32,
    workers: usize,
    backend: BackendSelection,
    supported_option_present: bool,
) -> Result<SweepConcurrencyMode, String> {
    let lockstep_raw = std::env::var_os(SWEEP_CONCURRENCY_SPIKE_CUDA_LOCKSTEP_ENV);
    let free_raw = std::env::var_os(SWEEP_CONCURRENCY_SPIKE_CUDA_FREE_STREAMS_ENV);
    if lockstep_raw.is_some() && free_raw.is_some() {
        return Err(format!(
            "{SWEEP_CONCURRENCY_SPIKE_CUDA_LOCKSTEP_ENV} and {SWEEP_CONCURRENCY_SPIKE_CUDA_FREE_STREAMS_ENV} are mutually exclusive"
        ));
    }
    if supported_option_present && lockstep_raw.is_some() {
        return Err(format!(
            "--draw-workers is incompatible with experimental {SWEEP_CONCURRENCY_SPIKE_CUDA_LOCKSTEP_ENV}"
        ));
    }
    if supported_option_present {
        if let Some(raw) = free_raw.as_ref() {
            let raw = raw.to_string_lossy();
            if raw != "1" {
                return Err(format!(
                    "{SWEEP_CONCURRENCY_SPIKE_CUDA_FREE_STREAMS_ENV} must be 1 when set, found '{raw}'"
                ));
            }
            if workers <= 1 {
                return Err(format!(
                    "{SWEEP_CONCURRENCY_SPIKE_CUDA_FREE_STREAMS_ENV}=1 requires --draw-workers greater than 1"
                ));
            }
        }
        return Ok(if workers > 1 {
            SweepConcurrencyMode::CudaFreeNonblocking
        } else {
            SweepConcurrencyMode::IndependentDefaultStreams
        });
    }
    if let Some(raw) = free_raw {
        let raw = raw.to_string_lossy();
        if raw != "1" {
            return Err(format!(
                "{SWEEP_CONCURRENCY_SPIKE_CUDA_FREE_STREAMS_ENV} must be 1 when set, found '{raw}'"
            ));
        }
        if backend != BackendSelection::Cuda {
            return Err(format!(
                "{SWEEP_CONCURRENCY_SPIKE_CUDA_FREE_STREAMS_ENV}=1 requires --backend cuda"
            ));
        }
        if workers <= 1 {
            return Err(format!(
                "{SWEEP_CONCURRENCY_SPIKE_CUDA_FREE_STREAMS_ENV}=1 requires {SWEEP_CONCURRENCY_SPIKE_WORKERS_ENV}>1"
            ));
        }
        // Dynamic scheduling claims draws one at a time, so uneven draw counts
        // are fine; no lockstep divisibility requirement applies.
        return Ok(SweepConcurrencyMode::CudaFreeNonblocking);
    }
    if let Some(raw) = lockstep_raw {
        let raw = raw.to_string_lossy();
        if raw != "1" {
            return Err(format!(
                "{SWEEP_CONCURRENCY_SPIKE_CUDA_LOCKSTEP_ENV} must be 1 when set, found '{raw}'"
            ));
        }
        if backend != BackendSelection::Cuda {
            return Err(format!(
                "{SWEEP_CONCURRENCY_SPIKE_CUDA_LOCKSTEP_ENV}=1 requires --backend cuda"
            ));
        }
        if workers <= 1 {
            return Err(format!(
                "{SWEEP_CONCURRENCY_SPIKE_CUDA_LOCKSTEP_ENV}=1 requires {SWEEP_CONCURRENCY_SPIKE_WORKERS_ENV}>1"
            ));
        }
        let draw_count = usize::try_from(draw_count).expect("draw count is u32");
        if draw_count % workers != 0 {
            return Err(format!(
                "lockstep CUDA spike requires draw count {draw_count} to be divisible by worker count {workers}"
            ));
        }
        return Ok(SweepConcurrencyMode::CudaLockstepNonblocking);
    }
    Ok(SweepConcurrencyMode::IndependentDefaultStreams)
}

pub(crate) fn sweep_cuda_fused_draw_capacity(
    backend: BackendSelection,
    supported_option_present: bool,
) -> Result<Option<usize>, String> {
    let Some(raw) = std::env::var_os(SWEEP_CUDA_FUSED_DRAWS_ENV) else {
        return Ok(None);
    };
    if supported_option_present {
        return Err(format!(
            "--draw-workers is incompatible with experimental {SWEEP_CUDA_FUSED_DRAWS_ENV}"
        ));
    }
    let raw = raw.to_string_lossy();
    let capacity = raw.parse::<usize>().map_err(|_| {
        format!("{SWEEP_CUDA_FUSED_DRAWS_ENV} must be one of 1, 2, or 4, found '{raw}'")
    })?;
    if !matches!(capacity, 1 | 2 | 4) {
        return Err(format!(
            "{SWEEP_CUDA_FUSED_DRAWS_ENV} must be one of 1, 2, or 4, found '{raw}'"
        ));
    }
    if backend != BackendSelection::Cuda {
        return Err(format!(
            "{SWEEP_CUDA_FUSED_DRAWS_ENV} requires --backend cuda"
        ));
    }
    if std::env::var_os(SWEEP_CONCURRENCY_SPIKE_CUDA_LOCKSTEP_ENV).is_some()
        || std::env::var_os(SWEEP_CONCURRENCY_SPIKE_CUDA_FREE_STREAMS_ENV).is_some()
        || std::env::var_os(SWEEP_CONCURRENCY_SPIKE_DELAY_DRAW_ZERO_ENV).is_some()
    {
        return Err(format!(
            "{SWEEP_CUDA_FUSED_DRAWS_ENV} is incompatible with the lockstep-stream, free-stream, and draw-delay spike controls"
        ));
    }
    Ok(Some(capacity))
}

pub(crate) fn sweep_concurrency_spike_draw_zero_delay() -> Result<Duration, String> {
    let Some(raw) = std::env::var_os(SWEEP_CONCURRENCY_SPIKE_DELAY_DRAW_ZERO_ENV) else {
        return Ok(Duration::ZERO);
    };
    let raw = raw.to_string_lossy();
    let milliseconds = raw.parse::<u64>().map_err(|_| {
        format!(
            "{SWEEP_CONCURRENCY_SPIKE_DELAY_DRAW_ZERO_ENV} must be an unsigned integer, found '{raw}'"
        )
    })?;
    Ok(Duration::from_millis(milliseconds))
}

#[cfg(feature = "cuda")]
pub(crate) const SWEEP_TEST_CUDA_FREE_MEMORY_ENV: &str = "SEMBLA_SWEEP_TEST_CUDA_FREE_MEMORY_BYTES";

#[cfg(feature = "cuda")]
pub(crate) fn capacity_mib(bytes: usize) -> f64 {
    bytes as f64 / (1024.0 * 1024.0)
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct SweepFinalStateAdmission {
    pub(crate) requested_lane_count: usize,
    pub(crate) requested_pinned_bytes_per_lane: usize,
    pub(crate) requested_cacheable_staging_bytes_per_lane: usize,
    pub(crate) requested_pinned_bytes: usize,
    pub(crate) requested_cacheable_staging_bytes: usize,
    pub(crate) requested_buffer_set_count: usize,
    pub(crate) requested_underlying_pinned_allocation_count: usize,
}

impl SweepFinalStateAdmission {
    pub(crate) fn without_treatment(requested_lane_count: usize) -> Self {
        Self {
            requested_lane_count,
            ..Self::default()
        }
    }
}

#[cfg(feature = "cuda")]
pub(crate) struct CudaSweepCapacityDecision {
    workers: usize,
    required: usize,
    free: usize,
    total: usize,
    per_lane: usize,
    fixed: usize,
    margin_percent: usize,
    host_assumption: usize,
    requested_pinned_bytes: usize,
    requested_cacheable_staging_bytes: usize,
}

#[cfg(feature = "cuda")]
pub(crate) fn ensure_cuda_sweep_capacity(
    decision: &CudaSweepCapacityDecision,
) -> Result<(), String> {
    if decision.required > decision.free {
        return Err(format!(
            "insufficient CUDA device memory for --draw-workers {}: conservative bound {:.1} MiB exceeds {:.1} MiB free on device 0 ({:.1} MiB total); no lanes were constructed. The bound includes {:.1} MiB per lane plus {:.1} MiB fixed before a {}% safety margin. Host-memory assumption: at least {:.1} MiB available, including requested packed-pinned page-locked bytes {:.1} MiB and cacheable staging bytes {:.1} MiB across {} lanes; this is not a universal OS page-lock limit and host memory is not silently capped or overcommitted by this preflight",
            decision.workers,
            capacity_mib(decision.required),
            capacity_mib(decision.free),
            capacity_mib(decision.total),
            capacity_mib(decision.per_lane),
            capacity_mib(decision.fixed),
            decision.margin_percent,
            capacity_mib(decision.host_assumption),
            capacity_mib(decision.requested_pinned_bytes),
            capacity_mib(decision.requested_cacheable_staging_bytes),
            decision.workers,
        ));
    }
    Ok(())
}

#[cfg(feature = "cuda")]
pub(crate) fn preflight_cuda_sweep_capacity_with_memory(
    model: &sembla_ir::ValidatedModel,
    initial_tables: &[TableInit],
    workers: usize,
    final_state_mode: SweepCudaFinalStateMode,
    actual_free: usize,
    total: usize,
) -> Result<SweepFinalStateAdmission, String> {
    let estimate = CudaBackend::estimate_isolated_sweep_capacity(
        model,
        initial_tables,
        workers,
        final_state_mode.cuda(),
    )
    .map_err(|error| format!("could not establish conservative CUDA capacity: {error}"))?;
    let required = estimate.device_bytes;
    let host_assumption = estimate.host_bytes;
    let per_lane = estimate.device_bytes_per_lane_before_margin;
    let fixed = estimate.fixed_device_bytes_before_margin;
    let margin_percent = estimate.safety_margin_percent;
    // Tests may lower, but never raise, the observed free-memory limit. This
    // makes rejection deterministic without providing a way to bypass safety.
    let free = match std::env::var_os(SWEEP_TEST_CUDA_FREE_MEMORY_ENV) {
        Some(raw) => {
            let raw = raw.to_string_lossy();
            let injected = raw.parse::<usize>().map_err(|_| {
                format!(
                    "{SWEEP_TEST_CUDA_FREE_MEMORY_ENV} must be an unsigned byte count, found '{raw}'"
                )
            })?;
            actual_free.min(injected)
        }
        None => actual_free,
    };
    let decision = CudaSweepCapacityDecision {
        workers,
        required,
        free,
        total,
        per_lane,
        fixed,
        margin_percent,
        host_assumption,
        requested_pinned_bytes: estimate.requested_pinned_bytes,
        requested_cacheable_staging_bytes: estimate.requested_cacheable_staging_bytes,
    };
    ensure_cuda_sweep_capacity(&decision)?;
    eprintln!(
        "CUDA draw-worker preflight admitted {workers} lanes: conservative device bound {:.1} MiB <= {:.1} MiB free on device 0 ({margin_percent}% safety margin); host-memory assumption: at least {:.1} MiB available, including requested packed-pinned page-locked bytes {:.1} MiB and cacheable staging bytes {:.1} MiB across {workers} lanes (not a universal OS page-lock limit)",
        capacity_mib(required),
        capacity_mib(free),
        capacity_mib(host_assumption),
        capacity_mib(estimate.requested_pinned_bytes),
        capacity_mib(estimate.requested_cacheable_staging_bytes),
    );
    Ok(SweepFinalStateAdmission {
        requested_lane_count: workers,
        requested_pinned_bytes_per_lane: estimate.requested_pinned_bytes_per_lane,
        requested_cacheable_staging_bytes_per_lane: estimate
            .requested_cacheable_staging_bytes_per_lane,
        requested_pinned_bytes: estimate.requested_pinned_bytes,
        requested_cacheable_staging_bytes: estimate.requested_cacheable_staging_bytes,
        requested_buffer_set_count: estimate.requested_buffer_set_count,
        requested_underlying_pinned_allocation_count: estimate
            .requested_underlying_pinned_allocation_count,
    })
}

#[cfg(feature = "cuda")]
pub(crate) fn preflight_cuda_sweep_capacity(
    model: &sembla_ir::ValidatedModel,
    initial_tables: &[TableInit],
    workers: usize,
    final_state_mode: SweepCudaFinalStateMode,
) -> Result<SweepFinalStateAdmission, String> {
    let (actual_free, total) = CudaBackend::device_zero_memory_info()
        .map_err(|error| format!("could not query free CUDA device memory: {error}"))?;
    preflight_cuda_sweep_capacity_with_memory(
        model,
        initial_tables,
        workers,
        final_state_mode,
        actual_free,
        total,
    )
}

#[cfg(not(feature = "cuda"))]
pub(crate) fn preflight_cuda_sweep_capacity(
    _model: &sembla_ir::ValidatedModel,
    _initial_tables: &[TableInit],
    _workers: usize,
    _final_state_mode: SweepCudaFinalStateMode,
) -> Result<SweepFinalStateAdmission, String> {
    Err("cuda backend unavailable: crate built without the 'cuda' feature; CUDA draw-worker capacity cannot be established".to_owned())
}
