//! Sweep orchestration, concurrency, CUDA admission, and sweep artifacts.

use super::*;

#[derive(Clone, Debug)]
pub(crate) struct SweepOptions {
    pub(crate) seed: u64,
    pub(crate) draws: Option<u32>,
    pub(crate) theta_file: Option<String>,
    pub(crate) noise_mode: manifest::NoiseMode,
    pub(crate) ticks: u32,
    pub(crate) population: String,
    pub(crate) out: String,
    pub(crate) params: Option<String>,
    pub(crate) export_pairs: Option<String>,
    pub(crate) timing_json: Option<String>,
    pub(crate) backend: BackendSelection,
    /// `None` preserves whether the supported option was omitted. The
    /// resolved execution default is still exactly one worker.
    pub(crate) draw_workers: Option<usize>,
    pub(crate) enabled_features: FeatureSet,
}

pub(crate) fn parse_sweep_options(flags: &[String]) -> Result<SweepOptions, String> {
    let mut seed = None;
    let mut draws = None;
    let mut ticks = None;
    let mut population = None;
    let mut out = None;
    let mut params = None;
    let mut theta_file = None;
    let mut noise_mode = None;
    let mut export_pairs = None;
    let mut timing_json = None;
    let mut backend = None;
    let mut draw_workers = None;
    let mut enabled_features = FeatureSet::new();
    let mut index = 0;
    while index < flags.len() {
        let flag = flags[index].as_str();
        let value = flags
            .get(index + 1)
            .ok_or_else(|| format!("missing value for '{flag}'"))?;
        match flag {
            "--seed" => set_once(&mut seed, parse_number(value, flag)?, flag)?,
            "--draws" => set_once(&mut draws, parse_number(value, flag)?, flag)?,
            "--theta-file" => set_once(&mut theta_file, value.clone(), flag)?,
            "--noise" => {
                let value = match value.as_str() {
                    "crn" => manifest::NoiseMode::Crn,
                    "independent" => manifest::NoiseMode::Independent,
                    _ => {
                        return Err(format!(
                            "invalid value '{value}' for '--noise' (expected 'crn' or 'independent')"
                        ));
                    }
                };
                set_once(&mut noise_mode, value, flag)?;
            }
            "--ticks" => set_once(&mut ticks, parse_number(value, flag)?, flag)?,
            "--population" => {
                if value.parse::<usize>().is_err() && !Path::new(value).is_file() {
                    return Err(format!(
                        "invalid numeric value or population file '{value}' for '{flag}'"
                    ));
                }
                set_once(&mut population, value.clone(), flag)?;
            }
            "--out" => set_once(&mut out, value.clone(), flag)?,
            "--params" => set_once(&mut params, value.clone(), flag)?,
            "--export-pairs" => set_once(&mut export_pairs, value.clone(), flag)?,
            "--timing-json" => set_once(&mut timing_json, value.clone(), flag)?,
            "--backend" => set_once(&mut backend, parse_backend(value)?, flag)?,
            "--draw-workers" => {
                let value: usize = parse_number(value, flag)?;
                if value == 0 {
                    return Err("'--draw-workers' must be greater than zero".to_owned());
                }
                set_once(&mut draw_workers, value, flag)?;
            }
            "--enable" => {
                enabled_features.insert(parse_feature(value)?);
            }
            _ => return Err(format!("unknown sweep flag '{flag}'")),
        }
        index += 2;
    }
    if draws.is_some() && theta_file.is_some() {
        return Err("'--theta-file' cannot be combined with '--draws'".to_owned());
    }
    if draws.is_none() && theta_file.is_none() {
        return Err("missing required flag '--draws' or '--theta-file'".to_owned());
    }
    if draws == Some(0) {
        return Err("'--draws' must be greater than zero".to_owned());
    }
    Ok(SweepOptions {
        seed: seed.ok_or_else(|| "missing required flag '--seed'".to_owned())?,
        draws,
        theta_file,
        noise_mode: noise_mode.unwrap_or(manifest::NoiseMode::Crn),
        ticks: ticks.ok_or_else(|| "missing required flag '--ticks'".to_owned())?,
        population: population.ok_or_else(|| "missing required flag '--population'".to_owned())?,
        out: out.ok_or_else(|| "missing required flag '--out'".to_owned())?,
        params,
        export_pairs,
        timing_json,
        backend: backend.unwrap_or_default(),
        draw_workers,
        enabled_features,
    })
}

pub(crate) fn sweep_file(path: &str, options: SweepOptions) -> i32 {
    match sweep_file_result(path, options) {
        Ok(()) => 0,
        Err(error) => {
            eprintln!("{error}");
            1
        }
    }
}

#[derive(Debug)]
pub(crate) struct ThetaFile {
    assignments: Vec<Vec<ParamOverride>>,
    sha256: String,
}

pub(crate) fn read_theta_file(
    model: &sembla_ir::ValidatedModel,
    path: &str,
) -> Result<ThetaFile, String> {
    let bytes = std::fs::read(path).map_err(|error| format!("{path}: {error}"))?;
    let value: serde_json::Value =
        serde_json::from_slice(&bytes).map_err(|error| format!("{path}: {error}"))?;
    let entries = value
        .as_array()
        .ok_or_else(|| format!("{path}: theta file must be a JSON array"))?;
    if entries.is_empty() {
        return Err(format!(
            "{path}: theta file must contain at least one theta assignment"
        ));
    }
    u32::try_from(entries.len())
        .map_err(|_| format!("{path}: theta file contains more than u32::MAX assignments"))?;

    let mut assignments = Vec::with_capacity(entries.len());
    for (index, entry) in entries.iter().enumerate() {
        let object = entry
            .as_object()
            .ok_or_else(|| format!("{path}: theta assignment {index} must be a JSON object"))?;
        for declaration in model
            .model()
            .params
            .iter()
            .filter(|declaration| declaration.prior.is_some())
        {
            if !object.contains_key(&declaration.name) {
                return Err(format!(
                    "{path}: theta assignment {index} is missing prior-bearing parameter '{}'",
                    declaration.name
                ));
            }
        }

        let mut overrides = Vec::with_capacity(object.len());
        for (name, value) in object {
            let declaration = model
                .model()
                .params
                .iter()
                .find(|parameter| parameter.name == *name)
                .ok_or_else(|| {
                    format!("{path}: theta assignment {index} has unknown parameter '{name}'")
                })?;
            let value = param_value_from_json(
                declaration,
                value,
                &format!("{path}: theta assignment {index}"),
            )?;
            overrides.push(ParamOverride::new(name, value));
        }
        ParamEnv::resolve(model, &overrides)
            .map_err(|error| format!("{path}: theta assignment {index}: {error}"))?;
        assignments.push(overrides);
    }

    Ok(ThetaFile {
        assignments,
        sha256: hex(&Sha256::digest(&bytes)),
    })
}

pub(crate) fn params_from_theta_assignment(
    model: &sembla_ir::ValidatedModel,
    path: &str,
    draw: u32,
    assignment: &[ParamOverride],
    pinned: &[ParamOverride],
) -> Result<ParamEnv, String> {
    for supplied in assignment {
        if pinned.iter().any(|pin| pin.name == supplied.name) {
            return Err(format!(
                "{path}: theta assignment {draw} parameter '{}' is also supplied by --params",
                supplied.name
            ));
        }
    }
    let mut overrides = Vec::with_capacity(pinned.len() + assignment.len());
    overrides.extend_from_slice(pinned);
    overrides.extend_from_slice(assignment);
    ParamEnv::resolve(model, &overrides)
        .map_err(|error| format!("{path}: theta assignment {draw}: {error}"))
}

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
    const fn label(self) -> &'static str {
        match self {
            Self::Materialized => "materialized",
            Self::PackedPageable => "packed-pageable",
            Self::PackedPinned => "packed-pinned",
        }
    }

    #[cfg(feature = "cuda")]
    const fn cuda(self) -> sembla_cuda::CudaFinalStateReadbackMode {
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
    setup_elapsed: Duration,
    execution_window_elapsed: Duration,
    identity: manifest::BackendIdentity,
    draws: Vec<SweepConcurrentCompletedDraw>,
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

#[allow(clippy::too_many_arguments)]
pub(crate) fn publish_sweep_draw(
    draw: u32,
    params: &ParamEnv,
    execution_seed: u64,
    execution: SweepDrawOutput,
    reported_columns: &mut Option<Vec<String>>,
    pairs_csv: &mut Option<String>,
    parameter_columns: &[String],
    summary_columns: &[String],
    run_manifest: &mut manifest::RunManifest,
    out: &Path,
    export_pairs: bool,
) -> Result<Vec<Vec<ReportedValue>>, String> {
    let output = execution.output;
    if let Some(columns) = reported_columns.as_ref() {
        if columns != &output.series.columns {
            return Err(format!(
                "draw {draw}: reported column schema changed across draws"
            ));
        }
    } else {
        *reported_columns = Some(output.series.columns.clone());
    }
    if let Some(csv) = pairs_csv {
        append_pairs_row(
            csv,
            draw,
            params,
            parameter_columns,
            &output.summaries,
            summary_columns,
        )?;
    }
    let hashes = execution_hashes_with_state_hash(&output, execution.final_state_hash);
    let grouped_outputs = grouped_output_records(&output.grouped);
    run_manifest.executions.push(manifest::ManifestExecution {
        k: draw,
        seed: Some(execution_seed),
        scenario: None,
        model: None,
        ir_hash: None,
        dt: None,
        resolved_theta: manifest::resolved_theta(params),
        results_sha256: hashes.results_sha256,
        final_state_sha256: hashes.final_state_sha256,
        observation_sha256: Some(hashes.observation_sha256),
        grouped_outputs,
    });
    let draw_path = out.join(format!("draw_{draw}.csv"));
    std::fs::write(&draw_path, output.csv.as_bytes())
        .map_err(|error| format!("{}: {error}", draw_path.display()))?;
    for grouped in &output.grouped {
        let path = grouped_output_path(&draw_path, &grouped.view);
        std::fs::write(&path, grouped.csv.as_bytes())
            .map_err(|error| format!("{}: {error}", path.display()))?;
    }
    if export_pairs {
        let draw_summaries = PathBuf::from(format!("{}.summaries.csv", draw_path.display()));
        std::fs::write(&draw_summaries, output.summaries_csv.as_bytes())
            .map_err(|error| format!("{}: {error}", draw_summaries.display()))?;
    }
    Ok(output.series.rows)
}

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

    let mut parameter_columns = model
        .model()
        .params
        .iter()
        .map(|parameter| parameter.name.clone())
        .collect::<Vec<_>>();
    parameter_columns.sort();
    let summary_columns = model
        .model()
        .summaries
        .iter()
        .map(|summary| summary.name.clone())
        .collect::<Vec<_>>();
    let mut pairs_csv = options.export_pairs.as_ref().map(|_| {
        let mut columns = vec!["k".to_owned()];
        columns.extend(parameter_columns.iter().cloned());
        columns.extend(summary_columns.iter().cloned());
        let mut csv = columns
            .iter()
            .map(|column| csv_field(column))
            .collect::<Vec<_>>()
            .join(",");
        csv.push('\n');
        csv
    });

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

    let mut all_series = Vec::with_capacity(draw_count as usize);
    let mut reported_columns: Option<Vec<String>> = None;
    let mut draw_durations = Vec::with_capacity(draw_count as usize);
    let mut draw_final_state_timings = Vec::with_capacity(draw_count as usize);
    let mut concurrency_spike_timing = None;
    let mut fused_spike_timing = None;
    let setup_elapsed;

    if let Some(capacity) = fused_capacity {
        eprintln!(
            "EXPERIMENTAL CUDA fused grid-y spike: capacity {capacity}; default sweep behavior remains sequential"
        );
        let mut prepared = Vec::with_capacity(draw_count as usize);
        for draw in 0..draw_count {
            let params = match &theta_file {
                Some(theta) => params_from_theta_assignment(
                    &model,
                    options.theta_file.as_deref().expect("theta path exists"),
                    draw,
                    &theta.assignments[draw as usize],
                    &pinned,
                )?,
                None => sample_parameters_for_draw(&model, options.seed, draw, &pinned)
                    .map_err(|error| format!("draw {draw}: {error}"))?,
            };
            csv_manifest.push_str(&draw.to_string());
            for (_, value) in params.values() {
                csv_manifest.push(',');
                csv_manifest.push_str(&param_value_csv(value));
            }
            csv_manifest.push('\n');
            let execution_seed = match options.noise_mode {
                manifest::NoiseMode::Crn => options.seed,
                manifest::NoiseMode::Independent => derive_sweep_replica_seed(options.seed, draw),
            };
            prepared.push(SweepPreparedDraw {
                k: draw,
                params,
                execution_seed,
            });
        }
        let fused = run_fused_sweep_spike(
            &model,
            &initial_tables,
            &construction_params,
            &options,
            capacity,
            &prepared,
        )?;
        setup_elapsed = fused.setup_elapsed;
        run_manifest.backend_identity = Some(fused.identity);
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
            all_series.push(publish_sweep_draw(
                prepared_draw.k,
                &prepared_draw.params,
                prepared_draw.execution_seed,
                execution,
                &mut reported_columns,
                &mut pairs_csv,
                &parameter_columns,
                &summary_columns,
                &mut run_manifest,
                out,
                options.export_pairs.is_some(),
            )?);
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
        run_manifest.backend_identity = Some(backend.identity());
        // Deliberately sequential: declaration order within each k, then k order.
        for draw in 0..draw_count {
            let params = match &theta_file {
                Some(theta) => params_from_theta_assignment(
                    &model,
                    options.theta_file.as_deref().expect("theta path exists"),
                    draw,
                    &theta.assignments[draw as usize],
                    &pinned,
                )?,
                None => sample_parameters_for_draw(&model, options.seed, draw, &pinned)
                    .map_err(|error| format!("draw {draw}: {error}"))?,
            };
            csv_manifest.push_str(&draw.to_string());
            for (_, value) in params.values() {
                csv_manifest.push(',');
                csv_manifest.push_str(&param_value_csv(value));
            }
            csv_manifest.push('\n');

            let execution_seed = match options.noise_mode {
                manifest::NoiseMode::Crn => options.seed,
                manifest::NoiseMode::Independent => derive_sweep_replica_seed(options.seed, draw),
            };
            let draw_started = Instant::now();
            let execution = backend
                .run_draw(
                    &model,
                    &params,
                    execution_seed,
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
            all_series.push(publish_sweep_draw(
                draw,
                &params,
                execution_seed,
                execution,
                &mut reported_columns,
                &mut pairs_csv,
                &parameter_columns,
                &summary_columns,
                &mut run_manifest,
                out,
                options.export_pairs.is_some(),
            )?);
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
        let mut prepared = Vec::with_capacity(draw_count as usize);
        for draw in 0..draw_count {
            let params = match &theta_file {
                Some(theta) => params_from_theta_assignment(
                    &model,
                    options.theta_file.as_deref().expect("theta path exists"),
                    draw,
                    &theta.assignments[draw as usize],
                    &pinned,
                )?,
                None => sample_parameters_for_draw(&model, options.seed, draw, &pinned)
                    .map_err(|error| format!("draw {draw}: {error}"))?,
            };
            csv_manifest.push_str(&draw.to_string());
            for (_, value) in params.values() {
                csv_manifest.push(',');
                csv_manifest.push_str(&param_value_csv(value));
            }
            csv_manifest.push('\n');
            let execution_seed = match options.noise_mode {
                manifest::NoiseMode::Crn => options.seed,
                manifest::NoiseMode::Independent => derive_sweep_replica_seed(options.seed, draw),
            };
            prepared.push(SweepPreparedDraw {
                k: draw,
                params,
                execution_seed,
            });
        }

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
                    if run_manifest.backend_identity.is_none() {
                        run_manifest.backend_identity = Some(identity.clone());
                    }
                    all_series.push(publish_sweep_draw(
                        prepared_draw.k,
                        &prepared_draw.params,
                        prepared_draw.execution_seed,
                        execution,
                        &mut reported_columns,
                        &mut pairs_csv,
                        &parameter_columns,
                        &summary_columns,
                        &mut run_manifest,
                        out,
                        options.export_pairs.is_some(),
                    )?);
                    Ok(())
                },
            )?;
            setup_elapsed = concurrent.setup_elapsed;
            run_manifest.backend_identity = Some(concurrent.identity);
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
            run_manifest.backend_identity = Some(concurrent.identity);
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
                all_series.push(publish_sweep_draw(
                    prepared_draw.k,
                    &prepared_draw.params,
                    prepared_draw.execution_seed,
                    execution,
                    &mut reported_columns,
                    &mut pairs_csv,
                    &parameter_columns,
                    &summary_columns,
                    &mut run_manifest,
                    out,
                    options.export_pairs.is_some(),
                )?);
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
        reported_columns.as_deref().unwrap_or_default(),
        &all_series,
        options.ticks,
    )?;
    let manifest_path = out.join("manifest.csv");
    let summary_path = out.join("summary.csv");
    std::fs::write(&manifest_path, csv_manifest.as_bytes())
        .map_err(|error| format!("{}: {error}", manifest_path.display()))?;
    std::fs::write(&summary_path, summary.as_bytes())
        .map_err(|error| format!("{}: {error}", summary_path.display()))?;
    manifest::write(&out.join("run-manifest.json"), &run_manifest)?;
    if let (Some(export_path), Some(pairs_csv)) =
        (options.export_pairs.as_deref().map(Path::new), pairs_csv)
    {
        std::fs::write(export_path, pairs_csv.as_bytes())
            .map_err(|error| format!("{}: {error}", export_path.display()))?;
        let pairs_sha256 = hex(&Sha256::digest(pairs_csv.as_bytes()));
        let metadata = manifest::PairsMetadata::for_sweep(
            &run_manifest,
            effective_ir_hash,
            draw_count,
            parameter_columns,
            summary_columns,
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
        std::fs::write(path, json).map_err(|error| format!("{path}: {error}"))?;
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

pub(crate) fn remove_previous_sweep_outputs(directory: &Path) -> Result<(), String> {
    for entry in
        std::fs::read_dir(directory).map_err(|error| format!("{}: {error}", directory.display()))?
    {
        let path = entry
            .map_err(|error| format!("{}: {error}", directory.display()))?
            .path();
        let name = path
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("");
        if name == "manifest.csv"
            || name == "run-manifest.json"
            || name == "summary.csv"
            || (name.starts_with("draw_") && name.ends_with(".csv"))
        {
            std::fs::remove_file(&path).map_err(|error| format!("{}: {error}", path.display()))?;
        }
    }
    Ok(())
}

pub(crate) fn param_value_csv(value: &ParamValue) -> String {
    match value {
        ParamValue::Real { value } => value.to_string(),
        ParamValue::Int { value } => value.to_string(),
    }
}

pub(crate) fn append_pairs_row(
    csv: &mut String,
    draw: u32,
    params: &ParamEnv,
    parameter_columns: &[String],
    summaries: &[SummaryValue],
    summary_columns: &[String],
) -> Result<(), String> {
    let mut values = params.values().collect::<Vec<_>>();
    values.sort_by(|left, right| left.0.cmp(right.0));
    if values
        .iter()
        .map(|(name, _)| *name)
        .ne(parameter_columns.iter().map(String::as_str))
    {
        return Err(format!(
            "draw {draw}: resolved parameter columns do not match the export schema"
        ));
    }
    if summaries
        .iter()
        .map(|summary| summary.name.as_str())
        .ne(summary_columns.iter().map(String::as_str))
    {
        return Err(format!(
            "draw {draw}: summary columns do not match model declaration order"
        ));
    }

    csv.push_str(&draw.to_string());
    for (_, value) in values {
        csv.push(',');
        csv.push_str(&param_value_csv(value));
    }
    for summary in summaries {
        csv.push(',');
        csv.push_str(&ReportedValue::from(summary.value).csv());
    }
    csv.push('\n');
    Ok(())
}

pub(crate) fn summary_csv(
    columns: &[String],
    all_series: &[Vec<Vec<ReportedValue>>],
    ticks: u32,
) -> Result<String, String> {
    const PERCENTILES: [usize; 5] = [5, 25, 50, 75, 95];
    let mut csv = String::from("tick");
    for name in columns {
        for percentile in PERCENTILES {
            csv.push(',');
            csv.push_str(&csv_field(&format!("{name}_p{percentile:02}")));
        }
    }
    csv.push('\n');
    for tick in 0..ticks as usize {
        csv.push_str(&tick.to_string());
        for (column, column_name) in columns.iter().enumerate() {
            let mut values = all_series
                .iter()
                .map(|series| {
                    series
                        .get(tick)
                        .and_then(|row| row.get(column))
                        .copied()
                        .ok_or_else(|| {
                            format!("reported series is missing tick {tick} column '{column_name}'")
                        })
                })
                .collect::<Result<Vec<_>, _>>()?;
            if let Some(first) = values.first().copied() {
                for value in values.iter().skip(1).copied() {
                    value.cmp(first)?;
                }
            }
            values.sort_by(|left, right| {
                left.cmp(*right)
                    .expect("reported column type was checked before sorting")
            });
            for percentile in PERCENTILES {
                // Deterministic nearest index to p * (n - 1).
                let index = ((values.len() - 1) * percentile + 50) / 100;
                csv.push(',');
                csv.push_str(&values[index].csv());
            }
        }
        csv.push('\n');
    }
    Ok(csv)
}

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
    output: RunOutput,
    final_state_hash: [u8; 32],
    final_state: Option<SweepFinalStateDiagnostic>,
}

impl SweepBackend {
    fn new(
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

    fn new_concurrency_lane(
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

    fn identity(&self) -> manifest::BackendIdentity {
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

    fn run_draw(
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
                    let (observed_tick, fired_per_box, deferred_per_resource_table, device_views) =
                        backend
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
                            let views =
                                executor::observe_views(model, backend.observed_state(), params)
                                    .map_err(|error| format!("tick {tick}: {error}"))?;
                            let grouped_views = executor::observe_grouped_views(
                                model,
                                backend.observed_state(),
                                params,
                            )
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
                    )?;
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
    fn run_draw_lockstep(
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
                                let (
                                    observed_tick,
                                    fired_per_box,
                                    deferred_per_resource_table,
                                    device_views,
                                ) = backend
                                    .run_tick_observed_reused()
                                    .map_err(|error| format!("tick {tick}: {error}"))?;
                                debug_assert_eq!(observed_tick, tick);
                                let (views, grouped_views, generic_enum_counts) = match device_views
                                {
                                    Some(observation) => (
                                        observation.views,
                                        observation.grouped_views,
                                        observation.generic_enum_counts,
                                    ),
                                    None => {
                                        let views = executor::observe_views(
                                            model,
                                            backend.observed_state(),
                                            params,
                                        )
                                        .map_err(|error| format!("tick {tick}: {error}"))?;
                                        let grouped_views = executor::observe_grouped_views(
                                            model,
                                            backend.observed_state(),
                                            params,
                                        )
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
                                output
                                    .as_mut()
                                    .expect("output exists while lockstep draw is healthy")
                                    .push_tick_with_enum_counts(
                                        backend.observed_state(),
                                        tick,
                                        report,
                                        generic_enum_counts.as_deref(),
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

#[derive(Clone, Serialize)]
pub(crate) struct SweepFinalStateDownloadedBytesTiming {
    state: usize,
    inputs: usize,
    input_counts: usize,
    total: usize,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub(crate) struct SweepFinalStateBufferAccountingTiming {
    buffer_set_count: usize,
    underlying_pinned_allocation_count: usize,
    pinned_bytes: usize,
    cacheable_staging_bytes: usize,
}

pub(crate) const FINAL_STATE_TIMER_TOLERANCE: Duration = Duration::from_micros(1);
pub(crate) const FINAL_STATE_TIMER_TOLERANCE_MS: f64 = 0.001;

#[derive(Clone, Serialize)]
pub(crate) struct SweepFinalStateTiming {
    schema: &'static str,
    mode: &'static str,
    one_time_allocation_ms: f64,
    pageable_dtoh_host_api_ms: Option<f64>,
    pinned_dtoh_enqueue_api_ms: Option<f64>,
    wait_to_pinned_host_readable_ms: Option<f64>,
    pinned_to_cacheable_staging_copy_ms: Option<f64>,
    host_state_reconstruction_ms: Option<f64>,
    cpu_sha256_ms: f64,
    attributed_phase_sum_ms: f64,
    unattributed_timer_overhead_ms: f64,
    final_state_seam_total_ms: f64,
    final_state_seam_total_excludes_one_time_allocation: bool,
    timer_tolerance_ms: f64,
    phases_reconcile: bool,
    allocation_plus_seam_reconciles_with_draw_wall: bool,
    downloaded_bytes: SweepFinalStateDownloadedBytesTiming,
    buffer_accounting: SweepFinalStateBufferAccountingTiming,
}

impl SweepFinalStateTiming {
    pub(crate) fn new(
        value: &SweepFinalStateDiagnostic,
        draw_wall: Duration,
    ) -> Result<Self, String> {
        let attributed = [
            value.pageable_dtoh_host_api,
            value.pinned_dtoh_enqueue_api,
            value.wait_to_pinned_host_readable,
            value.pinned_to_cacheable_staging_copy,
            value.host_state_reconstruction,
            Some(value.cpu_sha256),
        ]
        .into_iter()
        .flatten()
        .try_fold(Duration::ZERO, |total, phase| total.checked_add(phase))
        .ok_or_else(|| "final-state attributed phase duration overflow".to_owned())?;
        let unattributed = value.total.checked_sub(attributed).ok_or_else(|| {
            format!(
                "{} final-state attributed phases exceed the measured seam total",
                value.mode.label()
            )
        })?;
        let allocation_plus_seam = value
            .allocation
            .checked_add(value.total)
            .ok_or_else(|| "final-state allocation/seam duration overflow".to_owned())?;
        let draw_with_tolerance = draw_wall
            .checked_add(FINAL_STATE_TIMER_TOLERANCE)
            .ok_or_else(|| "draw timing tolerance overflow".to_owned())?;
        if allocation_plus_seam > draw_with_tolerance {
            return Err(format!(
                "{} final-state allocation plus seam time exceeds draw wall time by more than {:.3} ms",
                value.mode.label(),
                FINAL_STATE_TIMER_TOLERANCE_MS
            ));
        }
        Ok(Self {
            schema: "sembla-cuda-final-state-readback-v2",
            mode: value.mode.label(),
            one_time_allocation_ms: duration_ms(value.allocation),
            pageable_dtoh_host_api_ms: value.pageable_dtoh_host_api.map(duration_ms),
            pinned_dtoh_enqueue_api_ms: value.pinned_dtoh_enqueue_api.map(duration_ms),
            wait_to_pinned_host_readable_ms: value.wait_to_pinned_host_readable.map(duration_ms),
            pinned_to_cacheable_staging_copy_ms: value
                .pinned_to_cacheable_staging_copy
                .map(duration_ms),
            host_state_reconstruction_ms: value.host_state_reconstruction.map(duration_ms),
            cpu_sha256_ms: duration_ms(value.cpu_sha256),
            attributed_phase_sum_ms: duration_ms(attributed),
            unattributed_timer_overhead_ms: duration_ms(unattributed),
            final_state_seam_total_ms: duration_ms(value.total),
            final_state_seam_total_excludes_one_time_allocation: true,
            timer_tolerance_ms: FINAL_STATE_TIMER_TOLERANCE_MS,
            phases_reconcile: true,
            allocation_plus_seam_reconciles_with_draw_wall: true,
            downloaded_bytes: SweepFinalStateDownloadedBytesTiming {
                state: value.downloaded_bytes.state,
                inputs: value.downloaded_bytes.inputs,
                input_counts: value.downloaded_bytes.input_counts,
                total: value.downloaded_bytes.total,
            },
            buffer_accounting: SweepFinalStateBufferAccountingTiming {
                buffer_set_count: value.buffer_accounting.buffer_set_count,
                underlying_pinned_allocation_count: value
                    .buffer_accounting
                    .underlying_pinned_allocation_count,
                pinned_bytes: value.buffer_accounting.pinned_bytes,
                cacheable_staging_bytes: value.buffer_accounting.cacheable_staging_bytes,
            },
        })
    }
}

#[derive(Clone, Serialize)]
pub(crate) struct SweepFinalStateBufferAccountingDocument {
    pub(crate) mode: &'static str,
    pub(crate) requested_lane_count: usize,
    pub(crate) retained_lane_count: usize,
    pub(crate) requested_pinned_bytes_per_lane: usize,
    pub(crate) requested_cacheable_staging_bytes_per_lane: usize,
    pub(crate) requested_pinned_bytes: usize,
    pub(crate) effective_pinned_bytes: usize,
    pub(crate) requested_cacheable_staging_bytes: usize,
    pub(crate) effective_cacheable_staging_bytes: usize,
    pub(crate) requested_buffer_set_count: usize,
    pub(crate) buffer_set_count: usize,
    pub(crate) requested_underlying_pinned_allocation_count: usize,
    pub(crate) underlying_pinned_allocation_count: usize,
}

pub(crate) fn aggregate_final_state_buffer_accounting<'a>(
    mode: SweepCudaFinalStateMode,
    admission: SweepFinalStateAdmission,
    lane_timings: impl IntoIterator<Item = (usize, &'a Option<SweepFinalStateTiming>)>,
) -> Result<SweepFinalStateBufferAccountingDocument, String> {
    let mut lanes = std::collections::BTreeMap::new();
    for (lane, timing) in lane_timings {
        let Some(timing) = timing else {
            continue;
        };
        if let Some(previous) = lanes.insert(lane, timing.buffer_accounting.clone()) {
            if previous != timing.buffer_accounting {
                return Err(format!(
                    "final-state buffer accounting changed across draws on lane {lane}"
                ));
            }
        }
    }
    let retained_lane_count = admission.requested_lane_count;
    let mut effective_pinned_bytes = 0_usize;
    let mut effective_cacheable_staging_bytes = 0_usize;
    let mut buffer_set_count = 0_usize;
    let mut underlying_pinned_allocation_count = 0_usize;
    for accounting in lanes.values() {
        effective_pinned_bytes = effective_pinned_bytes
            .checked_add(accounting.pinned_bytes)
            .ok_or_else(|| "effective pinned-byte accounting overflow".to_owned())?;
        effective_cacheable_staging_bytes = effective_cacheable_staging_bytes
            .checked_add(accounting.cacheable_staging_bytes)
            .ok_or_else(|| "effective cacheable staging-byte accounting overflow".to_owned())?;
        buffer_set_count = buffer_set_count
            .checked_add(accounting.buffer_set_count)
            .ok_or_else(|| "effective pinned buffer-set count overflow".to_owned())?;
        underlying_pinned_allocation_count = underlying_pinned_allocation_count
            .checked_add(accounting.underlying_pinned_allocation_count)
            .ok_or_else(|| "effective pinned allocation-count overflow".to_owned())?;
    }
    if buffer_set_count > retained_lane_count {
        return Err(format!(
            "packed-pinned buffer-set count {buffer_set_count} exceeds retained lane count {retained_lane_count}"
        ));
    }
    let maximum_underlying = retained_lane_count
        .checked_mul(3)
        .ok_or_else(|| "retained lane allocation bound overflow".to_owned())?;
    if underlying_pinned_allocation_count > maximum_underlying {
        return Err(format!(
            "packed-pinned underlying allocation count {underlying_pinned_allocation_count} exceeds three per retained lane ({maximum_underlying})"
        ));
    }
    if mode == SweepCudaFinalStateMode::PackedPinned {
        if admission.requested_buffer_set_count != admission.requested_lane_count
            || buffer_set_count != lanes.len()
            || buffer_set_count > admission.requested_buffer_set_count
            || underlying_pinned_allocation_count
                > admission.requested_underlying_pinned_allocation_count
        {
            return Err(format!(
                "packed-pinned effective allocation counts exceed or disagree with admission: requested lanes/sets/allocations {}/{}/{}, effective used lanes/sets/allocations {}/{}/{}",
                admission.requested_lane_count,
                admission.requested_buffer_set_count,
                admission.requested_underlying_pinned_allocation_count,
                lanes.len(),
                buffer_set_count,
                underlying_pinned_allocation_count,
            ));
        }
        let expected_pinned = admission
            .requested_pinned_bytes_per_lane
            .checked_mul(buffer_set_count)
            .ok_or_else(|| "effective pinned-byte expectation overflow".to_owned())?;
        let expected_staging = admission
            .requested_cacheable_staging_bytes_per_lane
            .checked_mul(buffer_set_count)
            .ok_or_else(|| "effective staging-byte expectation overflow".to_owned())?;
        let allocations_per_lane = admission
            .requested_underlying_pinned_allocation_count
            .checked_div(admission.requested_lane_count)
            .ok_or_else(|| "packed-pinned requested lane count is zero".to_owned())?;
        let expected_allocations = allocations_per_lane
            .checked_mul(buffer_set_count)
            .ok_or_else(|| "effective pinned allocation-count expectation overflow".to_owned())?;
        if effective_pinned_bytes != expected_pinned
            || effective_cacheable_staging_bytes != expected_staging
            || underlying_pinned_allocation_count != expected_allocations
        {
            return Err(format!(
                "packed-pinned effective used-lane accounting does not match admission: expected sets/allocations/pinned/staging bytes {}/{}/{}/{}, effective {}/{}/{}/{}",
                buffer_set_count,
                expected_allocations,
                expected_pinned,
                expected_staging,
                buffer_set_count,
                underlying_pinned_allocation_count,
                effective_pinned_bytes,
                effective_cacheable_staging_bytes,
            ));
        }
    }
    if mode != SweepCudaFinalStateMode::PackedPinned
        && (buffer_set_count != 0
            || underlying_pinned_allocation_count != 0
            || effective_pinned_bytes != 0
            || effective_cacheable_staging_bytes != 0)
    {
        return Err("non-pinned final-state mode allocated treatment buffers".to_owned());
    }
    Ok(SweepFinalStateBufferAccountingDocument {
        mode: mode.label(),
        requested_lane_count: admission.requested_lane_count,
        retained_lane_count,
        requested_pinned_bytes_per_lane: admission.requested_pinned_bytes_per_lane,
        requested_cacheable_staging_bytes_per_lane: admission
            .requested_cacheable_staging_bytes_per_lane,
        requested_pinned_bytes: admission.requested_pinned_bytes,
        effective_pinned_bytes,
        requested_cacheable_staging_bytes: admission.requested_cacheable_staging_bytes,
        effective_cacheable_staging_bytes,
        requested_buffer_set_count: admission.requested_buffer_set_count,
        buffer_set_count,
        requested_underlying_pinned_allocation_count: admission
            .requested_underlying_pinned_allocation_count,
        underlying_pinned_allocation_count,
    })
}

#[derive(Serialize)]
pub(crate) struct SweepTimingDraw {
    k: u32,
    wall_time_ms: f64,
    final_state: Option<SweepFinalStateTiming>,
}

#[derive(Serialize)]
pub(crate) struct SweepTimingDocument {
    schema: &'static str,
    backend: &'static str,
    draws: u32,
    ticks_per_draw: u32,
    setup_wall_time_ms: f64,
    draw_zero_including_setup_wall_time_ms: f64,
    final_state_buffer_accounting: SweepFinalStateBufferAccountingDocument,
    draw_timings: Vec<SweepTimingDraw>,
    whole_sweep_wall_time_ms: f64,
    repository_commit: String,
    binary_sha256: String,
}

#[derive(Serialize)]
pub(crate) struct SweepFusedSpikeTimingChunk {
    chunk_index: usize,
    first_k: u32,
    active_slots: usize,
    capacity: usize,
    start_offset_ms: f64,
    finish_offset_ms: f64,
    shared_chunk_wall_time_ms: f64,
}

#[derive(Serialize)]
pub(crate) struct SweepFusedSpikeTimingDocument {
    schema: &'static str,
    backend: &'static str,
    draws: u32,
    ticks_per_draw: u32,
    requested_capacity: usize,
    maximum_active_slots: usize,
    setup_wall_time_ms: f64,
    execution_window_wall_time_ms: f64,
    publication_wall_time_ms: f64,
    chunks: Vec<SweepFusedSpikeTimingChunk>,
    whole_sweep_wall_time_ms: f64,
    repository_commit: String,
    binary_sha256: String,
}

#[derive(Serialize)]
pub(crate) struct SweepConcurrencySpikeTimingDraw {
    pub(crate) k: u32,
    pub(crate) lane: usize,
    pub(crate) start_offset_ms: f64,
    pub(crate) finish_offset_ms: f64,
    pub(crate) wall_time_ms: f64,
    pub(crate) final_state: Option<SweepFinalStateTiming>,
}

#[derive(Serialize)]
pub(crate) struct SweepConcurrencySpikeTimingDocument {
    schema: &'static str,
    backend: &'static str,
    draws: u32,
    ticks_per_draw: u32,
    requested_draw_workers: usize,
    effective_draw_workers: usize,
    maximum_pending_results: usize,
    execution_mode: &'static str,
    setup_wall_time_ms: f64,
    execution_window_wall_time_ms: f64,
    publication_wall_time_ms: f64,
    final_state_buffer_accounting: SweepFinalStateBufferAccountingDocument,
    draw_timings: Vec<SweepConcurrencySpikeTimingDraw>,
    whole_sweep_wall_time_ms: f64,
    repository_commit: String,
    binary_sha256: String,
}
