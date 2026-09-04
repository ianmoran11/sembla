use std::mem;
use std::time::{Duration, Instant};

use cudarc::driver::{
    CudaContext, CudaEvent, CudaFunction, CudaSlice, CudaStream, DeviceRepr, DriverError,
    LaunchArgs, LaunchConfig, PinnedHostSlice, PushKernelArg, ValidAsZeroBits,
};
use cudarc::nvrtc::{compile_ptx_with_opts, CompileOptions};
use sembla_ir::{AttrType, ParamValue, ValidatedModel};
use sembla_runtime::core::{
    ColumnData, DeviceObservationEligibility, GroupedViewValue, InputTable, ObservationValue,
    ParamEnv, StateStore, TableInit, ViewValue,
};
use sha2::{Digest, Sha256};

use crate::codegen::{
    decode_grouped_histogram, generate_fused_batch, grouped_observation_layout,
    host_observation_fallback, FusedBuffer, GroupedObservationAxisLayout, GroupedObservationLayout,
    FUSED_BUFFER_COUNT, GROUPED_OBSERVATION_KEY_SPACE_LIMIT,
};
use crate::types::{CudaDeviceIdentity, CudaRunResult, CudaTickObservation, HashMode};
use crate::{generate, CudaAvailability, CudaError, GeneratedCuda, PhiloxCoordinate};

mod final_state;
mod layout;
mod observation;
mod tick;

use final_state::{
    checked_arena_len, estimate_isolated_sweep_capacity, final_state_component_bytes,
    FinalStateAllocationInjection, PinnedFinalStateBuffers,
};
use layout::{
    build_layout, downloaded_state_bytes, global_table, hash_state, pack_initial_state,
    pack_params, unpack_inputs, unpack_state_into,
};

/// Hidden final-state readback routes used by CUDA sweeps. The CLI selects its
/// production default explicitly; this diagnostic API retains its legacy
/// materialized default for compatibility.
#[doc(hidden)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum CudaFinalStateReadbackMode {
    #[default]
    Materialized,
    PackedPageable,
    PackedPinned,
}

impl CudaFinalStateReadbackMode {
    pub const fn label(self) -> &'static str {
        match self {
            Self::Materialized => "materialized",
            Self::PackedPageable => "packed-pageable",
            Self::PackedPinned => "packed-pinned",
        }
    }
}

/// Exact logical component bytes participating in the canonical final-state
/// digest. Device-side padding is deliberately excluded.
#[doc(hidden)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct CudaFinalStateDownloadedBytes {
    pub state: usize,
    pub inputs: usize,
    pub input_counts: usize,
    pub total: usize,
}

/// Retained pinned and cacheable-buffer accounting for one CUDA lane.
#[doc(hidden)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct CudaFinalStateBufferAccounting {
    pub buffer_set_count: usize,
    pub underlying_pinned_allocation_count: usize,
    pub pinned_bytes: usize,
    pub cacheable_staging_bytes: usize,
}

/// Conservative sweep admission including final-state treatment memory.
///
/// `device_bytes` includes the complete per-lane device-buffer census, a
/// context/module/stream reserve for every lane, one process-level reserve,
/// allocation-granularity rounding, and a 25% safety margin. `host_bytes` is
/// an assumption rather than an OS admission check: the caller must provide at
/// least this much available host memory. It covers the coordinator state,
/// four state-sized retained/readback copies per lane, per-lane working
/// reserve, a process reserve, and the same safety margin.
#[doc(hidden)]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CudaSweepCapacityEstimate {
    pub workers: usize,
    pub device_bytes: usize,
    pub device_bytes_per_lane_before_margin: usize,
    pub fixed_device_bytes_before_margin: usize,
    pub host_bytes: usize,
    pub safety_margin_percent: usize,
    pub final_state_bytes_per_lane: CudaFinalStateDownloadedBytes,
    pub requested_pinned_bytes_per_lane: usize,
    pub requested_cacheable_staging_bytes_per_lane: usize,
    pub requested_pinned_bytes: usize,
    pub requested_cacheable_staging_bytes: usize,
    pub requested_buffer_set_count: usize,
    pub requested_underlying_pinned_allocation_count: usize,
}

/// Digest plus diagnostic-only attribution for one CUDA final-state seam.
#[doc(hidden)]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CudaFinalStateReadback {
    pub digest: [u8; 32],
    pub mode: CudaFinalStateReadbackMode,
    /// One-time lazy allocation. It is excluded from `total` and is zero after
    /// the lane's retained set has been created.
    pub allocation: Duration,
    /// The blocking pageable `memcpy_dtov` host-call interval. Cudarc 0.17.6
    /// exposes no separate completion-wait boundary for this API.
    pub pageable_dtoh_host_api: Option<Duration>,
    /// Pinned D2H enqueue/API calls on the backend's existing lane stream.
    pub pinned_dtoh_enqueue_api: Option<Duration>,
    /// Same-stream completion plus the pinned destinations' recorded events.
    pub wait_to_pinned_host_readable: Option<Duration>,
    /// Copy from write-combined pinned memory to retained cacheable buffers.
    pub pinned_to_cacheable_staging_copy: Option<Duration>,
    /// `None` means reconstruction is not applicable to this mode.
    pub host_state_reconstruction: Option<Duration>,
    pub cpu_sha256: Duration,
    /// Complete final-state seam excluding one-time allocation.
    pub total: Duration,
    pub downloaded_bytes: CudaFinalStateDownloadedBytes,
    pub buffer_accounting: CudaFinalStateBufferAccounting,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CudaDeviceObservations {
    pub views: Vec<ViewValue>,
    pub grouped_views: Vec<GroupedViewValue>,
    /// Flattened descriptor/variant counts for the legacy generic CSV. Present
    /// only when grouped views are the model's sole declared observations.
    pub generic_enum_counts: Option<Vec<usize>>,
}

pub type CudaFiredPerBox = Vec<(String, Vec<(u32, usize)>)>;
pub type CudaDeferredPerResourceTable = Vec<(String, usize)>;
pub type ReusedCudaTickObservation = (
    u32,
    CudaFiredPerBox,
    CudaDeferredPerResourceTable,
    Option<CudaDeviceObservations>,
);
pub type FusedReusedCudaTickObservations = Vec<Result<ReusedCudaTickObservation, CudaError>>;

pub type TimedReusedCudaTickObservation = (
    u32,
    CudaFiredPerBox,
    CudaDeferredPerResourceTable,
    Option<CudaDeviceObservations>,
    [std::time::Duration; 5],
);
type DownloadedStateParts = (Vec<u8>, Vec<u8>, Vec<u64>);

/// Scan, order identity, branch, then exact-key payload recovery. Kernel
/// boundaries between these passes provide device-wide ordering without a
/// mutex or retry loop in generated validation code.
const VALIDATION_REDUCTION_PASSES: u64 = 4;

fn control_count_launch_config(elements: u64) -> LaunchConfig {
    const BLOCK: u32 = 256;
    const MAX_BLOCKS: u64 = 65_535;
    let blocks = elements.div_ceil(u64::from(BLOCK)).clamp(1, MAX_BLOCKS) as u32;
    LaunchConfig {
        grid_dim: (blocks, 1, 1),
        block_dim: (BLOCK, 1, 1),
        shared_mem_bytes: BLOCK * mem::size_of::<u64>() as u32,
    }
}

fn control_reports_from_counts(
    model: &ValidatedModel,
    fired_counts: &[u64],
    deferred_counts: &[u64],
) -> Result<(CudaFiredPerBox, CudaDeferredPerResourceTable), CudaError> {
    let expected_rules = model.transitions().len();
    if fired_counts.len() != expected_rules {
        return Err(CudaError::DeviceExecution(format!(
            "CUDA fired-count readback returned {} values for {expected_rules} transitions",
            fired_counts.len()
        )));
    }
    let expected_tables = model
        .model()
        .boxes
        .iter()
        .map(|model_box| model_box.tables.len())
        .sum::<usize>();
    if deferred_counts.len() != expected_tables {
        return Err(CudaError::DeviceExecution(format!(
            "CUDA deferred-count readback returned {} values for {expected_tables} tables",
            deferred_counts.len()
        )));
    }

    let mut fired_per_box = Vec::with_capacity(model.model().boxes.len());
    for (box_index, model_box) in model.model().boxes.iter().enumerate() {
        let fired = model
            .transitions()
            .iter()
            .filter(|transition| transition.box_index == box_index)
            .map(|transition| {
                let count = fired_counts[transition.rule_id as usize];
                usize::try_from(count)
                    .map(|count| (transition.rule_id, count))
                    .map_err(|_| {
                        CudaError::DeviceExecution(format!(
                            "CUDA fired count for rule {} exceeds host usize",
                            transition.rule_id
                        ))
                    })
            })
            .collect::<Result<Vec<_>, _>>()?;
        fired_per_box.push((model_box.name.clone(), fired));
    }

    let qualify = model.model().boxes.len() > 1;
    let mut deferred_per_resource_table = Vec::new();
    let mut global_table = 0;
    for model_box in &model.model().boxes {
        for table in &model_box.tables {
            let count = deferred_counts[global_table];
            if count != 0 {
                let name = if qualify {
                    format!("{}.{}", model_box.name, table.name)
                } else {
                    table.name.clone()
                };
                let count = usize::try_from(count).map_err(|_| {
                    CudaError::DeviceExecution(format!(
                        "CUDA deferred count for table '{name}' exceeds host usize"
                    ))
                })?;
                deferred_per_resource_table.push((name, count));
            }
            global_table += 1;
        }
    }
    Ok((fired_per_box, deferred_per_resource_table))
}

fn finish_validation_reduction_pass(
    stream: &std::sync::Arc<cudarc::driver::CudaStream>,
    advance: &CudaFunction,
    commit: &CudaFunction,
    status: &mut CudaSlice<u64>,
    phase: u64,
    one: LaunchConfig,
    batch: Option<&FusedBatchMeta>,
) -> Result<(), CudaError> {
    let function = if phase + 1 < VALIDATION_REDUCTION_PASSES {
        advance
    } else {
        commit
    };
    let mut args = fused_launch_builder(stream, function, batch);
    args.arg(status);
    args.launch_generated(one).map(|_| ()).map_err(driver_error)
}

#[derive(Debug)]
struct Layout {
    row_counts: Vec<u64>,
    resource_offsets: Vec<u64>,
    resource_count: usize,
    column_offsets: Vec<u64>,
    state_len: usize,
    state_logical_len: usize,
    ports: Vec<(usize, usize)>,
    input_offsets: Vec<u64>,
    input_len: usize,
    input_logical_len: usize,
    candidate_offsets: Vec<u64>,
    candidate_count: usize,
    claim_instance_offsets: Vec<u64>,
    claim_instance_count: usize,
    aggregate_offsets: Vec<u64>,
    aggregate_len: usize,
    aggregate_max_groups: usize,
    write_offsets: Vec<u64>,
    owner_count: usize,
}

#[derive(Debug)]
struct FusedBatchMeta {
    capacity: usize,
    active_width: usize,
    strides: CudaSlice<u64>,
    strides_host: Vec<usize>,
    active: CudaSlice<u8>,
    active_host: Vec<u8>,
    seeds: CudaSlice<u64>,
    host_states: Vec<StateStore>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CudaFusedBatchMetadata {
    pub capacity: usize,
    pub active_width: usize,
    pub contexts: usize,
    pub modules: usize,
    pub streams: usize,
    pub nvrtc_compiles: usize,
    pub generated_source_sha256: String,
}

struct FusedLaunchArgs<'a> {
    inner: LaunchArgs<'a>,
    grid_y: u32,
}

impl<'a> FusedLaunchArgs<'a> {
    fn arg<T>(&mut self, arg: T) -> &mut Self
    where
        LaunchArgs<'a>: PushKernelArg<T>,
    {
        self.inner.arg(arg);
        self
    }

    fn launch_generated(
        &mut self,
        mut config: LaunchConfig,
    ) -> Result<Option<(CudaEvent, CudaEvent)>, DriverError> {
        config.grid_dim.1 = self.grid_y;
        launch_generated_kernel(&mut self.inner, config)
    }
}

/// Submit a kernel whose source and argument ABI are both generated by this
/// crate. Keeping the raw cudarc launch here makes the otherwise implicit ABI
/// invariant visible and gives reviews one boundary to audit.
fn launch_generated_kernel(
    args: &mut LaunchArgs<'_>,
    config: LaunchConfig,
) -> Result<Option<(CudaEvent, CudaEvent)>, DriverError> {
    // SAFETY: every caller uses a function emitted by this crate's code
    // generator and appends arguments in that generated function's exact ABI
    // order. Referenced host/device values outlive the queued stream work.
    unsafe { args.launch(config) }
}

fn fused_launch_builder<'a>(
    stream: &'a std::sync::Arc<cudarc::driver::CudaStream>,
    function: &'a CudaFunction,
    batch: Option<&'a FusedBatchMeta>,
) -> FusedLaunchArgs<'a> {
    let mut inner = stream.launch_builder(function);
    let grid_y = if let Some(batch) = batch {
        inner
            .arg(&batch.strides)
            .arg(&batch.active)
            .arg(&batch.seeds);
        batch.active_width as u32
    } else {
        1
    };
    FusedLaunchArgs { inner, grid_y }
}

#[cfg(test)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct ValidationLaunchGeometry {
    grid: u32,
    block: u32,
}

#[cfg(test)]
impl ValidationLaunchGeometry {
    fn config(self) -> LaunchConfig {
        LaunchConfig {
            grid_dim: (self.grid, 1, 1),
            block_dim: (self.block, 1, 1),
            shared_mem_bytes: 0,
        }
    }
}

#[cfg(test)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct ConflictLaunchGeometry {
    grid: u32,
    block: u32,
}

#[cfg(test)]
impl ConflictLaunchGeometry {
    fn config(self) -> LaunchConfig {
        LaunchConfig {
            grid_dim: (self.grid, 1, 1),
            block_dim: (self.block, 1, 1),
            shared_mem_bytes: 0,
        }
    }
}

#[derive(Debug)]
pub struct CudaBackend {
    model: ValidatedModel,
    host_state: StateStore,
    pristine_host_tables: Vec<TableInit>,
    host_tables: Vec<TableInit>,
    generated: GeneratedCuda,
    layout: Layout,
    // Declared before the stream and device sources so its Drop synchronization
    // runs before either can be released.
    pinned_final_state: Option<PinnedFinalStateBuffers>,
    stream: std::sync::Arc<cudarc::driver::CudaStream>,
    transition_functions: Vec<CudaFunction>,
    reset_status: CudaFunction,
    build_aggregate_partials: CudaFunction,
    finish_aggregates: CudaFunction,
    record_aggregate_errors: CudaFunction,
    validate_transition: CudaFunction,
    check_errors: CudaFunction,
    validate_claims: CudaFunction,
    validate_claim_compatibility: CudaFunction,
    init_conflict_winners: CudaFunction,
    build_claim_instances: CudaFunction,
    reduce_claim_keys: CudaFunction,
    reduce_claim_rules: CudaFunction,
    reduce_claim_entities: CudaFunction,
    reduce_claim_instances: CudaFunction,
    resolve_conflicts: CudaFunction,
    validate_effects: CudaFunction,
    init_effect_owners: CudaFunction,
    prepare_effects: CudaFunction,
    apply_effects: CudaFunction,
    validate_outputs: CudaFunction,
    prepare_outputs: CudaFunction,
    build_output_partials: CudaFunction,
    finish_outputs: CudaFunction,
    check_output_errors: CudaFunction,
    init_observations: CudaFunction,
    observe_view: CudaFunction,
    init_grouped_extrema: CudaFunction,
    bound_grouped_view: CudaFunction,
    init_grouped_histogram: CudaFunction,
    observe_grouped_view: CudaFunction,
    init_generic_enum_counts: CudaFunction,
    observe_generic_enum: CudaFunction,
    init_control_counts: CudaFunction,
    count_fired: CudaFunction,
    count_deferred: CudaFunction,
    philox_vectors_kernel: CudaFunction,
    init_validation_scratch: CudaFunction,
    advance_validation_phase: CudaFunction,
    commit_validation_status: CudaFunction,
    mark_effect_active: CudaFunction,
    state: CudaSlice<u8>,
    next_state: CudaSlice<u8>,
    pristine_state: CudaSlice<u8>,
    column_offsets: CudaSlice<u64>,
    row_counts: CudaSlice<u64>,
    resource_offsets: CudaSlice<u64>,
    inputs: CudaSlice<u8>,
    next_inputs: CudaSlice<u8>,
    input_offsets: CudaSlice<u64>,
    input_counts: CudaSlice<u64>,
    next_input_counts: CudaSlice<u64>,
    params: CudaSlice<u8>,
    aggregates: CudaSlice<u8>,
    aggregate_partials: CudaSlice<u8>,
    aggregate_errors: CudaSlice<u8>,
    aggregate_facts: CudaSlice<u8>,
    aggregate_active: CudaSlice<u8>,
    aggregate_offsets: CudaSlice<u64>,
    candidate_offsets: CudaSlice<u64>,
    claim_instance_offsets: CudaSlice<u64>,
    enabled: CudaSlice<u8>,
    times: CudaSlice<f64>,
    candidate_errors: CudaSlice<u8>,
    wins: CudaSlice<u8>,
    deferred: CudaSlice<u8>,
    fired_counts: CudaSlice<u64>,
    deferred_counts: CudaSlice<u64>,
    instance_resources: CudaSlice<u64>,
    instance_keys: CudaSlice<u64>,
    instance_rules: CudaSlice<u32>,
    instance_entities: CudaSlice<u32>,
    winner_keys: CudaSlice<u64>,
    winner_rules: CudaSlice<u32>,
    winner_entities: CudaSlice<u32>,
    winner_instances: CudaSlice<u64>,
    write_offsets: CudaSlice<u64>,
    owners: CudaSlice<i32>,
    owner_values: CudaSlice<u64>,
    output_partials: CudaSlice<u64>,
    output_errors: CudaSlice<u8>,
    observation_values: CudaSlice<i64>,
    grouped_extrema: CudaSlice<i64>,
    grouped_axis_mins: CudaSlice<i64>,
    grouped_axis_cardinalities: CudaSlice<u64>,
    grouped_histogram: CudaSlice<u64>,
    generic_enum_counts: CudaSlice<u64>,
    effect_active: CudaSlice<u32>,
    status: CudaSlice<u64>,
    seed: u64,
    next_tick: u32,
    hash_mode: HashMode,
    device_identity: CudaDeviceIdentity,
    host_state_current: bool,
    fused_batch: Option<FusedBatchMeta>,
    // No public setter exists. The hardware unit test uses this private seam
    // to confirm the four validation launches under explicit geometries.
    #[cfg(test)]
    validation_launch_override: Option<ValidationLaunchGeometry>,
    // The hardware test varies all segmented-argmin launches, including
    // deliberately undersubscribed grids, to prove geometry independence.
    #[cfg(test)]
    conflict_launch_override: Option<ConflictLaunchGeometry>,
}

impl CudaBackend {
    /// Applies the same explicit availability gate used by [`Self::new`].
    /// This seam makes no-device behavior testable without depending on the
    /// machine running the test and never constructs another backend.
    pub fn check_availability(availability: CudaAvailability) -> Result<(), CudaError> {
        availability.require()
    }

    /// Computes a conservative admission bound without creating a CUDA
    /// context, compiling a module, or allocating device memory.
    pub fn estimate_isolated_sweep_capacity(
        model: &ValidatedModel,
        initial_tables: &[TableInit],
        workers: usize,
        final_state_mode: CudaFinalStateReadbackMode,
    ) -> Result<CudaSweepCapacityEstimate, CudaError> {
        let generated = generate(model)?;
        let layout = build_layout(model, initial_tables, &generated)?;
        let parameter_bytes = model
            .model()
            .params
            .len()
            .max(1)
            .checked_mul(8)
            .ok_or_else(|| {
                CudaError::InvalidInput("CUDA sweep parameter size overflow".to_owned())
            })?;
        estimate_isolated_sweep_capacity(
            &layout,
            &generated,
            parameter_bytes,
            workers,
            final_state_mode,
        )
    }

    /// Returns `(free, total)` bytes for device zero. The short-lived context
    /// exists only for the capacity query and is dropped before worker lanes
    /// are constructed; lane constructors independently select device zero.
    pub fn device_zero_memory_info() -> Result<(usize, usize), CudaError> {
        let driver_library = unsafe { cudarc::driver::sys::is_culib_present() };
        if !driver_library {
            return Err(CudaError::DriverMissing);
        }
        let device_count = classify_device_count(CudaContext::device_count())?;
        if device_count <= 0 {
            return Err(CudaError::NoDevice);
        }
        let context = CudaContext::new(0).map_err(|error| CudaError::Driver(error.to_string()))?;
        context
            .bind_to_thread()
            .map_err(|error| CudaError::Driver(error.to_string()))?;
        cudarc::driver::result::mem_get_info().map_err(|error| CudaError::Driver(error.to_string()))
    }

    /// Constructs the single native-f64 CUDA path. Driver/device/toolkit
    /// absence is an error; this API never constructs the CPU oracle.
    pub fn new(
        model: &ValidatedModel,
        initial_tables: Vec<TableInit>,
        params: &ParamEnv,
        seed: u64,
        hash_mode: HashMode,
    ) -> Result<Self, CudaError> {
        Self::new_with_stream_mode(model, initial_tables, params, seed, hash_mode, false, None)
    }

    /// Experimental sweep-spike constructor using an explicitly non-blocking
    /// stream. Ordinary CUDA execution continues to use the default stream.
    #[doc(hidden)]
    pub fn new_nonblocking_stream(
        model: &ValidatedModel,
        initial_tables: Vec<TableInit>,
        params: &ParamEnv,
        seed: u64,
        hash_mode: HashMode,
    ) -> Result<Self, CudaError> {
        Self::new_with_stream_mode(model, initial_tables, params, seed, hash_mode, true, None)
    }

    /// Hidden one-context/module/default-stream grid-y backend used only by
    /// the fused sweep Gate-1 spike.
    #[doc(hidden)]
    pub fn new_fused_batch(
        model: &ValidatedModel,
        initial_tables: Vec<TableInit>,
        params: &ParamEnv,
        seed: u64,
        capacity: usize,
        hash_mode: HashMode,
    ) -> Result<Self, CudaError> {
        if !matches!(capacity, 1 | 2 | 4) {
            return Err(CudaError::InvalidInput(format!(
                "fused CUDA draw capacity must be one of 1, 2, or 4; got {capacity}"
            )));
        }
        Self::new_with_stream_mode(
            model,
            initial_tables,
            params,
            seed,
            hash_mode,
            false,
            Some(capacity),
        )
    }

    fn new_with_stream_mode(
        model: &ValidatedModel,
        initial_tables: Vec<TableInit>,
        params: &ParamEnv,
        seed: u64,
        hash_mode: HashMode,
        nonblocking_stream: bool,
        fused_capacity: Option<usize>,
    ) -> Result<Self, CudaError> {
        let driver_library = unsafe { cudarc::driver::sys::is_culib_present() };
        if !driver_library {
            return Err(CudaError::DriverMissing);
        }
        let device_count = classify_device_count(CudaContext::device_count())?;
        let nvrtc_library = unsafe { cudarc::nvrtc::sys::is_culib_present() };
        Self::check_availability(CudaAvailability {
            driver_library,
            device_count: usize::try_from(device_count).unwrap_or(0),
            nvrtc_library,
        })?;

        // Reuse the oracle's constructor for the exact schema/range checks and
        // retain its buffers for every subsequent host readback.
        let host_state = StateStore::new(model, initial_tables.clone())
            .map_err(|error| CudaError::InvalidInput(error.to_string()))?;

        let generated = if fused_capacity.is_some() {
            generate_fused_batch(model)?
        } else {
            generate(model)?
        };
        let dump_path = generated.dump_if_requested()?;
        let context = CudaContext::new(0).map_err(|error| CudaError::Driver(error.to_string()))?;
        let gpu_model = context
            .name()
            .map_err(|error| CudaError::Driver(error.to_string()))?;
        let mut driver_version = 0_i32;
        let driver_result =
            unsafe { cudarc::driver::sys::cuDriverGetVersion(&mut driver_version as *mut i32) };
        if driver_result != cudarc::driver::sys::CUresult::CUDA_SUCCESS {
            return Err(CudaError::Driver(format!(
                "cuDriverGetVersion failed with {driver_result:?}"
            )));
        }
        let device_identity = CudaDeviceIdentity {
            gpu_model,
            driver_version: format_cuda_driver_version(driver_version),
        };
        let options = CompileOptions {
            ftz: Some(false),
            prec_div: Some(true),
            prec_sqrt: Some(true),
            fmad: Some(false),
            options: vec!["--std=c++14".to_owned()],
            name: Some(format!("sembla-{}.cu", generated.source_sha256)),
            ..Default::default()
        };
        let ptx = compile_ptx_with_opts(&generated.source, options).map_err(|error| {
            let dump = dump_path
                .as_ref()
                .map(|path| format!("; generated source: {}", path.display()))
                .unwrap_or_default();
            CudaError::Compilation(format!("{error}{dump}"))
        })?;
        let module = context
            .load_module(ptx)
            .map_err(|error| CudaError::Driver(error.to_string()))?;
        let stream = if nonblocking_stream {
            context
                .new_stream()
                .map_err(|error| CudaError::Driver(error.to_string()))?
        } else {
            context.default_stream()
        };

        let transition_functions = generated
            .transition_kernels
            .iter()
            .map(|name| {
                module
                    .load_function(name)
                    .map_err(|error| CudaError::Driver(error.to_string()))
            })
            .collect::<Result<Vec<_>, _>>()?;
        let load = |name: &str| {
            module
                .load_function(name)
                .map_err(|error| CudaError::Driver(error.to_string()))
        };
        let reset_status = load("sembla_reset_status")?;
        let build_aggregate_partials = load("sembla_build_aggregate_partials")?;
        let finish_aggregates = load("sembla_finish_aggregates")?;
        let record_aggregate_errors = load("sembla_record_aggregate_errors")?;
        let validate_transition = load("sembla_validate_transition")?;
        let check_errors = load("sembla_check_candidate_errors")?;
        let validate_claims = load("sembla_validate_claims")?;
        let validate_claim_compatibility = load("sembla_validate_claim_compatibility")?;
        let init_conflict_winners = load("sembla_init_conflict_winners")?;
        let build_claim_instances = load("sembla_build_claim_instances")?;
        let reduce_claim_keys = load("sembla_reduce_claim_keys")?;
        let reduce_claim_rules = load("sembla_reduce_claim_rules")?;
        let reduce_claim_entities = load("sembla_reduce_claim_entities")?;
        let reduce_claim_instances = load("sembla_reduce_claim_instances")?;
        let resolve_conflicts = load("sembla_resolve_conflicts")?;
        let validate_effects = load("sembla_validate_effects")?;
        let init_effect_owners = load("sembla_init_effect_owners")?;
        let prepare_effects = load("sembla_prepare_effects")?;
        let apply_effects = load("sembla_apply_effects")?;
        let validate_outputs = load("sembla_validate_outputs")?;
        let prepare_outputs = load("sembla_prepare_outputs")?;
        let build_output_partials = load("sembla_build_output_partials")?;
        let finish_outputs = load("sembla_finish_outputs")?;
        let check_output_errors = load("sembla_check_output_errors")?;
        let init_observations = load("sembla_init_observations")?;
        let observe_view = load("sembla_observe_view")?;
        let init_grouped_extrema = load("sembla_init_grouped_extrema")?;
        let bound_grouped_view = load("sembla_bound_grouped_view")?;
        let init_grouped_histogram = load("sembla_init_grouped_histogram")?;
        let observe_grouped_view = load("sembla_observe_grouped_view")?;
        let init_generic_enum_counts = load("sembla_init_generic_enum_counts")?;
        let observe_generic_enum = load("sembla_observe_generic_enum")?;
        let init_control_counts = load("sembla_init_control_counts")?;
        let count_fired = load("sembla_count_fired")?;
        let count_deferred = load("sembla_count_deferred")?;
        let philox_vectors_kernel = load("sembla_philox_vectors")?;
        let init_validation_scratch = load("sembla_init_validation_scratch")?;
        let advance_validation_phase = load("sembla_advance_validation_phase")?;
        let commit_validation_status = load("sembla_commit_validation_status")?;
        let mark_effect_active = load("sembla_mark_effect_active")?;

        let layout = build_layout(model, &initial_tables, &generated)?;
        let state_bytes = pack_initial_state(model, &initial_tables, &layout)?;
        let params_bytes = pack_params(model, params)?;
        let slot_count = fused_capacity.unwrap_or(1);
        let arena_len =
            |per_slot: usize, label: &str| checked_arena_len(per_slot, slot_count, label);
        arena_len(state_bytes.len(), "state bytes")?;
        let state = stream
            .memcpy_stod(&state_bytes.repeat(slot_count))
            .map_err(driver_error)?;
        let next_state = stream.clone_dtod(&state).map_err(driver_error)?;
        let pristine_state = stream.clone_dtod(&state).map_err(driver_error)?;
        let column_offsets = stream
            .memcpy_stod(&nonempty(&layout.column_offsets))
            .map_err(driver_error)?;
        let row_counts = stream
            .memcpy_stod(&nonempty(&layout.row_counts))
            .map_err(driver_error)?;
        let resource_offsets = stream
            .memcpy_stod(&nonempty(&layout.resource_offsets))
            .map_err(driver_error)?;
        let input_zeroes = vec![0_u8; arena_len(layout.input_len.max(1), "inputs")?];
        let inputs = stream.memcpy_stod(&input_zeroes).map_err(driver_error)?;
        let next_inputs = stream.memcpy_stod(&input_zeroes).map_err(driver_error)?;
        let input_offsets = stream
            .memcpy_stod(&nonempty(&layout.input_offsets))
            .map_err(driver_error)?;
        let input_count_zeroes = vec![0_u64; arena_len(layout.ports.len().max(1), "input counts")?];
        let input_counts = stream
            .memcpy_stod(&input_count_zeroes)
            .map_err(driver_error)?;
        let next_input_counts = stream
            .memcpy_stod(&input_count_zeroes)
            .map_err(driver_error)?;
        arena_len(params_bytes.len(), "parameter bytes")?;
        let params = stream
            .memcpy_stod(&params_bytes.repeat(slot_count))
            .map_err(driver_error)?;
        let aggregates = stream
            .memcpy_stod(&vec![
                0_u8;
                arena_len(layout.aggregate_len.max(1), "aggregates")?
            ])
            .map_err(driver_error)?;
        let aggregate_partials_stride =
            layout.aggregate_len.max(1).checked_mul(2).ok_or_else(|| {
                CudaError::InvalidInput("aggregate partial size overflow".to_owned())
            })?;
        let aggregate_partials = stream
            .memcpy_stod(&vec![
                0_u8;
                arena_len(
                    aggregate_partials_stride,
                    "aggregate partials"
                )?
            ])
            .map_err(driver_error)?;
        let aggregate_errors_stride = (layout.aggregate_max_groups + 2).max(2);
        let aggregate_errors = stream
            .alloc_zeros::<u8>(arena_len(aggregate_errors_stride, "aggregate errors")?)
            .map_err(driver_error)?;
        let aggregate_meta_stride = generated.aggregate_group_tables.len().max(1);
        let aggregate_facts = stream
            .alloc_zeros::<u8>(arena_len(aggregate_meta_stride, "aggregate facts")?)
            .map_err(driver_error)?;
        let aggregate_active = stream
            .alloc_zeros::<u8>(arena_len(aggregate_meta_stride, "aggregate active")?)
            .map_err(driver_error)?;
        let aggregate_offsets = stream
            .memcpy_stod(&nonempty(&layout.aggregate_offsets))
            .map_err(driver_error)?;
        let candidate_offsets = stream
            .memcpy_stod(&nonempty(&layout.candidate_offsets))
            .map_err(driver_error)?;
        let claim_instance_offsets = stream
            .memcpy_stod(&nonempty(&layout.claim_instance_offsets))
            .map_err(driver_error)?;
        let candidate_len = layout.candidate_count.max(1);
        let enabled = stream
            .alloc_zeros::<u8>(arena_len(candidate_len, "enabled candidates")?)
            .map_err(driver_error)?;
        let times = stream
            .alloc_zeros::<f64>(arena_len(candidate_len, "candidate times")?)
            .map_err(driver_error)?;
        let candidate_error_len = candidate_len.checked_mul(2).ok_or_else(|| {
            CudaError::InvalidInput("candidate error buffer size overflow".to_owned())
        })?;
        let candidate_errors = stream
            .alloc_zeros::<u8>(arena_len(candidate_error_len, "candidate errors")?)
            .map_err(driver_error)?;
        let wins = stream
            .alloc_zeros::<u8>(arena_len(candidate_len, "candidate wins")?)
            .map_err(driver_error)?;
        let deferred_len = candidate_len
            .checked_mul(layout.row_counts.len().max(1))
            .ok_or_else(|| CudaError::InvalidInput("deferred metadata size overflow".to_owned()))?;
        let deferred = stream
            .alloc_zeros::<u8>(arena_len(deferred_len, "deferred metadata")?)
            .map_err(driver_error)?;
        let fired_counts_stride = layout.candidate_offsets.len().max(1);
        let fired_counts = stream
            .alloc_zeros::<u64>(arena_len(fired_counts_stride, "fired counts")?)
            .map_err(driver_error)?;
        let deferred_counts_stride = layout.row_counts.len().max(1);
        let deferred_counts = stream
            .alloc_zeros::<u64>(arena_len(deferred_counts_stride, "deferred counts")?)
            .map_err(driver_error)?;
        let claim_instance_len = layout.claim_instance_count.max(1);
        let claim_arena_len = arena_len(claim_instance_len, "claim instances")?;
        let instance_resources = stream
            .alloc_zeros::<u64>(claim_arena_len)
            .map_err(driver_error)?;
        let instance_keys = stream
            .alloc_zeros::<u64>(claim_arena_len)
            .map_err(driver_error)?;
        let instance_rules = stream
            .alloc_zeros::<u32>(claim_arena_len)
            .map_err(driver_error)?;
        let instance_entities = stream
            .alloc_zeros::<u32>(claim_arena_len)
            .map_err(driver_error)?;
        let resource_len = layout.resource_count.max(1);
        let resource_arena_len = arena_len(resource_len, "conflict winners")?;
        let winner_keys = stream
            .alloc_zeros::<u64>(resource_arena_len)
            .map_err(driver_error)?;
        let winner_rules = stream
            .alloc_zeros::<u32>(resource_arena_len)
            .map_err(driver_error)?;
        let winner_entities = stream
            .alloc_zeros::<u32>(resource_arena_len)
            .map_err(driver_error)?;
        let winner_instances = stream
            .alloc_zeros::<u64>(resource_arena_len)
            .map_err(driver_error)?;
        let write_offsets = stream
            .memcpy_stod(&nonempty(&layout.write_offsets))
            .map_err(driver_error)?;
        let owner_stride = layout.owner_count.max(1);
        let owners = stream
            .alloc_zeros::<i32>(arena_len(owner_stride, "effect owners")?)
            .map_err(driver_error)?;
        let owner_values = stream
            .alloc_zeros::<u64>(arena_len(owner_stride, "effect values")?)
            .map_err(driver_error)?;
        let output_field_count = layout.input_offsets.len().max(1);
        let output_partials_stride = output_field_count
            .checked_mul(2)
            .ok_or_else(|| CudaError::InvalidInput("output partial size overflow".to_owned()))?;
        let output_errors_stride = output_field_count
            .checked_mul(3)
            .ok_or_else(|| CudaError::InvalidInput("output error size overflow".to_owned()))?;
        let output_partials = stream
            .alloc_zeros::<u64>(arena_len(output_partials_stride, "output partials")?)
            .map_err(driver_error)?;
        let output_errors = stream
            .alloc_zeros::<u8>(arena_len(output_errors_stride, "output errors")?)
            .map_err(driver_error)?;
        let observation_values_stride = generated.observation_view_tables.len().max(1);
        let observation_values = stream
            .alloc_zeros::<i64>(arena_len(observation_values_stride, "observation values")?)
            .map_err(driver_error)?;
        let grouped_extrema_len = generated
            .grouped_observation_band_axes
            .checked_mul(2)
            .ok_or_else(|| CudaError::InvalidInput("grouped extrema size overflow".to_owned()))?;
        let grouped_extrema_stride = grouped_extrema_len.max(1);
        let grouped_extrema = stream
            .alloc_zeros::<i64>(arena_len(grouped_extrema_stride, "grouped extrema")?)
            .map_err(driver_error)?;
        let grouped_axis_count = generated
            .grouped_observation_views
            .iter()
            .try_fold(0_usize, |count, view| count.checked_add(view.axes.len()))
            .ok_or_else(|| CudaError::InvalidInput("grouped axis count overflow".to_owned()))?;
        let grouped_axis_stride = grouped_axis_count.max(1);
        let grouped_axis_mins = stream
            .alloc_zeros::<i64>(arena_len(grouped_axis_stride, "grouped axis minima")?)
            .map_err(driver_error)?;
        let grouped_axis_cardinalities = stream
            .alloc_zeros::<u64>(arena_len(
                grouped_axis_stride,
                "grouped axis cardinalities",
            )?)
            .map_err(driver_error)?;
        let grouped_histogram_len = if generated.grouped_observation_views.is_empty() {
            1
        } else {
            GROUPED_OBSERVATION_KEY_SPACE_LIMIT
        };
        let grouped_histogram = stream
            .alloc_zeros::<u64>(arena_len(grouped_histogram_len, "grouped histogram")?)
            .map_err(driver_error)?;
        let generic_enum_stride = generated.generic_enum_count.max(1);
        let generic_enum_counts = stream
            .alloc_zeros::<u64>(arena_len(generic_enum_stride, "generic enum counts")?)
            .map_err(driver_error)?;
        // status[0..=3] is the committed diagnostic; status[4..=11] is the
        // per-launch validation-reduction scratch (phase, scan, ordering
        // identity, code, branch, and selected payload).
        let status = stream
            .alloc_zeros::<u64>(arena_len(12, "validation status")?)
            .map_err(driver_error)?;
        let effect_active_stride = layout.candidate_offsets.len().max(1);
        let effect_active = stream
            .alloc_zeros::<u32>(arena_len(effect_active_stride, "effect active")?)
            .map_err(driver_error)?;

        let fused_batch = if let Some(capacity) = fused_capacity {
            let mut strides = vec![0_u64; FUSED_BUFFER_COUNT];
            macro_rules! stride {
                ($slot:ident, $value:expr) => {
                    strides[FusedBuffer::$slot as usize] = u64::try_from($value).map_err(|_| {
                        CudaError::InvalidInput(format!(
                            "fused CUDA {} stride exceeds u64",
                            stringify!($slot)
                        ))
                    })?;
                };
            }
            stride!(State, state_bytes.len().max(1));
            stride!(NextState, state_bytes.len().max(1));
            stride!(Inputs, layout.input_len.max(1));
            stride!(NextInputs, layout.input_len.max(1));
            stride!(InputCounts, layout.ports.len().max(1));
            stride!(NextInputCounts, layout.ports.len().max(1));
            stride!(Params, params_bytes.len().max(1));
            stride!(Aggregates, layout.aggregate_len.max(1));
            stride!(AggregatePartials, aggregate_partials_stride);
            stride!(AggregateErrors, aggregate_errors_stride);
            stride!(AggregateFacts, aggregate_meta_stride);
            stride!(AggregateActive, aggregate_meta_stride);
            stride!(Enabled, candidate_len);
            stride!(Times, candidate_len);
            stride!(CandidateErrors, candidate_error_len);
            stride!(Wins, candidate_len);
            stride!(Deferred, deferred_len);
            stride!(FiredCounts, fired_counts_stride);
            stride!(DeferredCounts, deferred_counts_stride);
            stride!(InstanceResources, claim_instance_len);
            stride!(InstanceKeys, claim_instance_len);
            stride!(InstanceRules, claim_instance_len);
            stride!(InstanceEntities, claim_instance_len);
            stride!(WinnerKeys, resource_len);
            stride!(WinnerRules, resource_len);
            stride!(WinnerEntities, resource_len);
            stride!(WinnerInstances, resource_len);
            stride!(Owners, owner_stride);
            stride!(OwnerValues, owner_stride);
            stride!(OutputPartials, output_partials_stride);
            stride!(OutputErrors, output_errors_stride);
            stride!(ObservationValues, observation_values_stride);
            stride!(GroupedExtrema, grouped_extrema_stride);
            stride!(GroupedAxisMins, grouped_axis_stride);
            stride!(GroupedAxisCardinalities, grouped_axis_stride);
            stride!(GroupedHistogram, grouped_histogram_len);
            stride!(GenericEnumCounts, generic_enum_stride);
            stride!(EffectActive, effect_active_stride);
            stride!(Status, 12);
            let host_states = (0..capacity)
                .map(|_| {
                    StateStore::new(model, initial_tables.clone())
                        .map_err(|error| CudaError::InvalidInput(error.to_string()))
                })
                .collect::<Result<Vec<_>, _>>()?;
            let strides_host = strides
                .iter()
                .map(|stride| usize::try_from(*stride).expect("usize stride encoded as u64"))
                .collect();
            Some(FusedBatchMeta {
                capacity,
                active_width: 0,
                strides: stream.memcpy_stod(&strides).map_err(driver_error)?,
                strides_host,
                active: stream.alloc_zeros::<u8>(capacity).map_err(driver_error)?,
                active_host: vec![0_u8; capacity],
                seeds: stream.alloc_zeros::<u64>(capacity).map_err(driver_error)?,
                host_states,
            })
        } else {
            None
        };

        Ok(Self {
            model: model.clone(),
            host_state,
            pristine_host_tables: initial_tables.clone(),
            host_tables: initial_tables,
            generated,
            layout,
            pinned_final_state: None,
            stream,
            transition_functions,
            reset_status,
            build_aggregate_partials,
            finish_aggregates,
            record_aggregate_errors,
            validate_transition,
            check_errors,
            validate_claims,
            validate_claim_compatibility,
            init_conflict_winners,
            build_claim_instances,
            reduce_claim_keys,
            reduce_claim_rules,
            reduce_claim_entities,
            reduce_claim_instances,
            resolve_conflicts,
            validate_effects,
            init_effect_owners,
            prepare_effects,
            apply_effects,
            validate_outputs,
            prepare_outputs,
            build_output_partials,
            finish_outputs,
            check_output_errors,
            init_observations,
            observe_view,
            init_grouped_extrema,
            bound_grouped_view,
            init_grouped_histogram,
            observe_grouped_view,
            init_generic_enum_counts,
            observe_generic_enum,
            init_control_counts,
            count_fired,
            count_deferred,
            philox_vectors_kernel,
            init_validation_scratch,
            advance_validation_phase,
            commit_validation_status,
            mark_effect_active,
            state,
            next_state,
            pristine_state,
            column_offsets,
            row_counts,
            resource_offsets,
            inputs,
            next_inputs,
            input_offsets,
            input_counts,
            next_input_counts,
            params,
            aggregates,
            aggregate_partials,
            aggregate_errors,
            aggregate_facts,
            aggregate_active,
            aggregate_offsets,
            candidate_offsets,
            claim_instance_offsets,
            enabled,
            times,
            candidate_errors,
            wins,
            deferred,
            fired_counts,
            deferred_counts,
            instance_resources,
            instance_keys,
            instance_rules,
            instance_entities,
            winner_keys,
            winner_rules,
            winner_entities,
            winner_instances,
            write_offsets,
            owners,
            owner_values,
            output_partials,
            output_errors,
            observation_values,
            grouped_extrema,
            grouped_axis_mins,
            grouped_axis_cardinalities,
            grouped_histogram,
            generic_enum_counts,
            effect_active,
            status,
            seed,
            next_tick: 0,
            hash_mode,
            device_identity,
            host_state_current: true,
            fused_batch,
            #[cfg(test)]
            validation_launch_override: None,
            #[cfg(test)]
            conflict_launch_override: None,
        })
    }

    pub fn generated(&self) -> &GeneratedCuda {
        &self.generated
    }

    pub fn device_identity(&self) -> &CudaDeviceIdentity {
        &self.device_identity
    }

    /// Restores every draw-mutable buffer in place and explicitly installs the
    /// draw's parameters and random seed. Device allocations, compiled code,
    /// layout metadata, and device identity are retained.
    pub fn reset_draw(&mut self, params: &ParamEnv, seed: u64) -> Result<(), CudaError> {
        // Parameter validation/packing is fallible and therefore happens before
        // any retained state is changed.
        let params_bytes = pack_params(&self.model, params)?;
        self.host_state
            .reset_backend_draw(&self.model, &self.pristine_host_tables)
            .map_err(|error| CudaError::InvalidInput(error.to_string()))?;

        self.stream
            .memcpy_dtod(&self.pristine_state, &mut self.state)
            .map_err(driver_error)?;
        self.stream
            .memcpy_dtod(&self.pristine_state, &mut self.next_state)
            .map_err(driver_error)?;
        self.stream
            .memcpy_htod(&params_bytes, &mut self.params)
            .map_err(driver_error)?;

        macro_rules! zero {
            ($($buffer:ident),+ $(,)?) => {
                $(self.stream.memset_zeros(&mut self.$buffer).map_err(driver_error)?;)+
            };
        }
        zero!(
            inputs,
            next_inputs,
            input_counts,
            next_input_counts,
            aggregates,
            aggregate_partials,
            aggregate_errors,
            aggregate_facts,
            aggregate_active,
            enabled,
            times,
            candidate_errors,
            wins,
            deferred,
            fired_counts,
            deferred_counts,
            instance_resources,
            instance_keys,
            instance_rules,
            instance_entities,
            winner_keys,
            winner_rules,
            winner_entities,
            winner_instances,
            owners,
            owner_values,
            output_partials,
            output_errors,
            observation_values,
            grouped_extrema,
            grouped_axis_mins,
            grouped_axis_cardinalities,
            grouped_histogram,
            generic_enum_counts,
            effect_active,
            status,
        );
        self.stream.synchronize().map_err(driver_error)?;
        self.seed = seed;
        self.next_tick = 0;
        self.host_state_current = true;
        Ok(())
    }

    /// Resets one contiguous grid-y batch without rebuilding the CUDA module.
    #[doc(hidden)]
    pub fn reset_fused_batch(
        &mut self,
        params: &[ParamEnv],
        seeds: &[u64],
    ) -> Result<(), CudaError> {
        let capacity = self
            .fused_batch
            .as_ref()
            .ok_or_else(|| CudaError::InvalidInput("backend is not fused-batch CUDA".to_owned()))?
            .capacity;
        if params.is_empty() || params.len() > capacity || params.len() != seeds.len() {
            return Err(CudaError::InvalidInput(format!(
                "fused CUDA batch requires equal nonzero params/seeds lengths <= capacity {capacity}; got params={} seeds={}",
                params.len(),
                seeds.len()
            )));
        }
        let mut params_arena = Vec::new();
        for env in params {
            params_arena.extend(pack_params(&self.model, env)?);
        }
        let params_stride = params_arena.len() / params.len();
        params_arena.resize(checked_arena_len(params_stride, capacity, "parameters")?, 0);
        let mut seed_arena = vec![0_u64; capacity];
        seed_arena[..seeds.len()].copy_from_slice(seeds);
        let mut active_host = vec![0_u8; capacity];
        active_host[..params.len()].fill(1);

        self.stream
            .memcpy_dtod(&self.pristine_state, &mut self.state)
            .map_err(driver_error)?;
        self.stream
            .memcpy_dtod(&self.pristine_state, &mut self.next_state)
            .map_err(driver_error)?;
        self.stream
            .memcpy_htod(&params_arena, &mut self.params)
            .map_err(driver_error)?;
        macro_rules! zero {
            ($($buffer:ident),+ $(,)?) => {
                $(self.stream.memset_zeros(&mut self.$buffer).map_err(driver_error)?;)+
            };
        }
        zero!(
            inputs,
            next_inputs,
            input_counts,
            next_input_counts,
            aggregates,
            aggregate_partials,
            aggregate_errors,
            aggregate_facts,
            aggregate_active,
            enabled,
            times,
            candidate_errors,
            wins,
            deferred,
            fired_counts,
            deferred_counts,
            instance_resources,
            instance_keys,
            instance_rules,
            instance_entities,
            winner_keys,
            winner_rules,
            winner_entities,
            winner_instances,
            owners,
            owner_values,
            output_partials,
            output_errors,
            observation_values,
            grouped_extrema,
            grouped_axis_mins,
            grouped_axis_cardinalities,
            grouped_histogram,
            generic_enum_counts,
            effect_active,
            status,
        );
        let batch = self.fused_batch.as_mut().expect("fused batch exists");
        for state in &mut batch.host_states {
            state
                .reset_backend_draw(&self.model, &self.pristine_host_tables)
                .map_err(|error| CudaError::InvalidInput(error.to_string()))?;
        }
        batch.active_width = params.len();
        batch.active_host = active_host;
        self.stream
            .memcpy_htod(&batch.active_host, &mut batch.active)
            .map_err(driver_error)?;
        self.stream
            .memcpy_htod(&seed_arena, &mut batch.seeds)
            .map_err(driver_error)?;
        self.stream.synchronize().map_err(driver_error)?;
        self.next_tick = 0;
        self.host_state_current = false;
        Ok(())
    }

    #[doc(hidden)]
    pub fn fused_batch_metadata(&self) -> Option<CudaFusedBatchMetadata> {
        self.fused_batch
            .as_ref()
            .map(|batch| CudaFusedBatchMetadata {
                capacity: batch.capacity,
                active_width: batch.active_width,
                contexts: 1,
                modules: 1,
                streams: 1,
                nvrtc_compiles: 1,
                generated_source_sha256: self.generated.source_sha256.clone(),
            })
    }

    /// Deactivates one fused slot after a slot-local host-side error. Later
    /// grid-y launches skip it while healthy peers continue in lockstep.
    #[doc(hidden)]
    pub fn deactivate_fused_slot(&mut self, slot: usize) -> Result<(), CudaError> {
        let batch = self
            .fused_batch
            .as_mut()
            .ok_or_else(|| CudaError::InvalidInput("backend is not fused-batch CUDA".to_owned()))?;
        if slot >= batch.active_width {
            return Err(CudaError::InvalidInput(format!(
                "invalid fused CUDA slot {slot}"
            )));
        }
        if batch.active_host[slot] != 0 {
            batch.active_host[slot] = 0;
            self.stream
                .memcpy_htod(&batch.active_host, &mut batch.active)
                .map_err(driver_error)?;
        }
        Ok(())
    }

    /// Returns a current final host snapshot without consuming the retained
    /// backend. This is the sweep lifecycle's final-state/hash seam.
    #[doc(hidden)]
    pub fn ensure_observed_state(&mut self) -> Result<&StateStore, CudaError> {
        self.ensure_host_state()?;
        Ok(&self.host_state)
    }

    fn ensure_pinned_final_state_buffers(
        &mut self,
        injection: FinalStateAllocationInjection,
    ) -> Result<Duration, CudaError> {
        if self.pinned_final_state.is_some() {
            return Ok(Duration::ZERO);
        }
        let bytes = final_state_component_bytes(&self.layout)?;
        let allocation_started = Instant::now();
        let buffers = PinnedFinalStateBuffers::allocate(&self.stream, bytes, injection)?;
        let allocation = allocation_started.elapsed();
        self.pinned_final_state = Some(buffers);
        Ok(allocation)
    }

    /// Produces the canonical final-state digest through the selected readback
    /// route. Packed modes always download every logical component,
    /// even if the retained materialized host snapshot is already current.
    #[doc(hidden)]
    pub fn final_state_readback(
        &mut self,
        mode: CudaFinalStateReadbackMode,
    ) -> Result<CudaFinalStateReadback, CudaError> {
        if self.fused_batch.is_some() {
            return Err(CudaError::InvalidInput(
                "final-state readback diagnostics do not support fused CUDA batches".to_owned(),
            ));
        }
        match mode {
            CudaFinalStateReadbackMode::Materialized => {
                let total_started = Instant::now();
                let mut pageable_dtoh_host_api = Duration::ZERO;
                let mut host_state_reconstruction = Duration::ZERO;
                let mut downloaded_bytes = CudaFinalStateDownloadedBytes::default();
                if !self.host_state_current {
                    let transfer_started = Instant::now();
                    let (state, inputs, input_counts) = self.download_state_parts()?;
                    pageable_dtoh_host_api = transfer_started.elapsed();
                    downloaded_bytes = downloaded_state_bytes(&state, &inputs, &input_counts)?;

                    let reconstruction_started = Instant::now();
                    self.reconstruct_state_store(&state, &inputs, &input_counts)?;
                    host_state_reconstruction = reconstruction_started.elapsed();
                }
                let hash_started = Instant::now();
                let digest = self.host_state.state_hash();
                let cpu_sha256 = hash_started.elapsed();
                Ok(CudaFinalStateReadback {
                    digest,
                    mode,
                    allocation: Duration::ZERO,
                    pageable_dtoh_host_api: Some(pageable_dtoh_host_api),
                    pinned_dtoh_enqueue_api: None,
                    wait_to_pinned_host_readable: None,
                    pinned_to_cacheable_staging_copy: None,
                    host_state_reconstruction: Some(host_state_reconstruction),
                    cpu_sha256,
                    total: total_started.elapsed(),
                    downloaded_bytes,
                    buffer_accounting: CudaFinalStateBufferAccounting::default(),
                })
            }
            CudaFinalStateReadbackMode::PackedPageable => {
                let total_started = Instant::now();
                let transfer_started = Instant::now();
                let (state, inputs, input_counts) = self.download_state_parts()?;
                let pageable_dtoh_host_api = transfer_started.elapsed();
                let downloaded_bytes = downloaded_state_bytes(&state, &inputs, &input_counts)?;
                let hash_started = Instant::now();
                let digest = hash_state(&self.model, &self.layout, &state, &inputs, &input_counts);
                let cpu_sha256 = hash_started.elapsed();
                Ok(CudaFinalStateReadback {
                    digest,
                    mode,
                    allocation: Duration::ZERO,
                    pageable_dtoh_host_api: Some(pageable_dtoh_host_api),
                    pinned_dtoh_enqueue_api: None,
                    wait_to_pinned_host_readable: None,
                    pinned_to_cacheable_staging_copy: None,
                    host_state_reconstruction: None,
                    cpu_sha256,
                    total: total_started.elapsed(),
                    downloaded_bytes,
                    buffer_accounting: CudaFinalStateBufferAccounting::default(),
                })
            }
            CudaFinalStateReadbackMode::PackedPinned => {
                let downloaded_bytes = final_state_component_bytes(&self.layout)?;
                let allocation =
                    self.ensure_pinned_final_state_buffers(FinalStateAllocationInjection::None)?;
                let total_started = Instant::now();
                let enqueue_started = Instant::now();
                self.pinned_final_state
                    .as_mut()
                    .expect("packed-pinned allocation installed its owner")
                    .enqueue_downloads(
                        &self.state,
                        &self.inputs,
                        &self.input_counts,
                        downloaded_bytes,
                    )?;
                let pinned_dtoh_enqueue_api = enqueue_started.elapsed();

                let wait_started = Instant::now();
                self.pinned_final_state
                    .as_ref()
                    .expect("packed-pinned owner remains installed")
                    .wait_until_readable()?;
                let wait_to_pinned_host_readable = wait_started.elapsed();

                let staging_started = Instant::now();
                self.pinned_final_state
                    .as_mut()
                    .expect("packed-pinned owner remains installed")
                    .stage()?;
                let pinned_to_cacheable_staging_copy = staging_started.elapsed();

                let hash_started = Instant::now();
                let buffers = self
                    .pinned_final_state
                    .as_ref()
                    .expect("packed-pinned owner remains installed");
                let digest = hash_state(
                    &self.model,
                    &self.layout,
                    buffers.state(),
                    buffers.inputs(),
                    buffers.input_counts(),
                );
                let cpu_sha256 = hash_started.elapsed();
                let buffer_accounting = buffers.accounting();
                Ok(CudaFinalStateReadback {
                    digest,
                    mode,
                    allocation,
                    pageable_dtoh_host_api: None,
                    pinned_dtoh_enqueue_api: Some(pinned_dtoh_enqueue_api),
                    wait_to_pinned_host_readable: Some(wait_to_pinned_host_readable),
                    pinned_to_cacheable_staging_copy: Some(pinned_to_cacheable_staging_copy),
                    host_state_reconstruction: None,
                    cpu_sha256,
                    total: total_started.elapsed(),
                    downloaded_bytes,
                    buffer_accounting,
                })
            }
        }
    }

    /// Evaluates checked coordinate Philox vectors on the device. This is a
    /// test/diagnostic surface for proving that the device implementation is
    /// bit-identical to `sembla_runtime::rng::draw_u32x4`.
    pub fn philox_vectors(
        &self,
        coordinates: &[PhiloxCoordinate],
    ) -> Result<Vec<[u32; 4]>, CudaError> {
        if coordinates.is_empty() {
            return Ok(Vec::new());
        }
        let count = u32::try_from(coordinates.len()).map_err(|_| {
            CudaError::InvalidInput("Philox vector count exceeds u32 capacity".to_owned())
        })?;
        let seeds = coordinates
            .iter()
            .map(|value| value.seed)
            .collect::<Vec<_>>();
        let ticks = coordinates
            .iter()
            .map(|value| value.tick)
            .collect::<Vec<_>>();
        let rules = coordinates
            .iter()
            .map(|value| value.rule_word)
            .collect::<Vec<_>>();
        let entities = coordinates
            .iter()
            .map(|value| value.entity_id)
            .collect::<Vec<_>>();
        let draws = coordinates
            .iter()
            .map(|value| value.draw_index)
            .collect::<Vec<_>>();
        let seeds = self.stream.memcpy_stod(&seeds).map_err(driver_error)?;
        let ticks = self.stream.memcpy_stod(&ticks).map_err(driver_error)?;
        let rules = self.stream.memcpy_stod(&rules).map_err(driver_error)?;
        let entities = self.stream.memcpy_stod(&entities).map_err(driver_error)?;
        let draws = self.stream.memcpy_stod(&draws).map_err(driver_error)?;
        let output_len = coordinates
            .len()
            .checked_mul(4)
            .ok_or_else(|| CudaError::InvalidInput("Philox output size overflow".to_owned()))?;
        let mut output = self
            .stream
            .alloc_zeros::<u32>(output_len)
            .map_err(driver_error)?;
        let mut args = self.stream.launch_builder(&self.philox_vectors_kernel);
        args.arg(&seeds)
            .arg(&ticks)
            .arg(&rules)
            .arg(&entities)
            .arg(&draws)
            .arg(&mut output)
            .arg(&count);
        launch_generated_kernel(&mut args, LaunchConfig::for_num_elems(count))
            .map_err(driver_error)?;
        let output = self.stream.memcpy_dtov(&output).map_err(driver_error)?;
        Ok(output
            .chunks_exact(4)
            .map(|words| [words[0], words[1], words[2], words[3]])
            .collect())
    }

    pub fn run(&mut self, ticks: u32) -> Result<CudaRunResult, CudaError> {
        let mut per_tick_state_hashes = if self.hash_mode == HashMode::EveryTick {
            Vec::with_capacity(ticks as usize)
        } else {
            Vec::new()
        };
        for _ in 0..ticks {
            self.execute_tick()?;
            if self.hash_mode == HashMode::EveryTick {
                per_tick_state_hashes.push(self.download_hash()?);
            }
        }
        let final_state_hash = match per_tick_state_hashes.last() {
            Some(hash) => *hash,
            None => self.download_hash()?,
        };
        Ok(CudaRunResult {
            final_state_hash,
            per_tick_state_hashes,
        })
    }

    fn download_fused_state_stores(&mut self) -> Result<Vec<Result<(), CudaError>>, CudaError> {
        let (state, inputs, input_counts) = self.download_state_parts()?;
        let batch = self
            .fused_batch
            .as_mut()
            .ok_or_else(|| CudaError::InvalidInput("backend is not fused-batch CUDA".to_owned()))?;
        let state_stride = batch.strides_host[FusedBuffer::State as usize];
        let input_stride = batch.strides_host[FusedBuffer::Inputs as usize];
        let count_stride = batch.strides_host[FusedBuffer::InputCounts as usize];
        let mut results = Vec::with_capacity(batch.active_width);
        let mut deactivate = false;
        for slot in 0..batch.active_width {
            if batch.active_host[slot] == 0 {
                results.push(Ok(()));
                continue;
            }
            let result = (|| {
                let mut tables = self.pristine_host_tables.clone();
                let state_begin = slot * state_stride;
                let input_begin = slot * input_stride;
                let count_begin = slot * count_stride;
                unpack_state_into(
                    &self.model,
                    &self.layout,
                    &state[state_begin..state_begin + state_stride],
                    &mut tables,
                )?;
                let input_tables = unpack_inputs(
                    &self.model,
                    &self.layout,
                    &inputs[input_begin..input_begin + input_stride],
                    &input_counts[count_begin..count_begin + count_stride],
                );
                batch.host_states[slot]
                    .refresh_backend_snapshot(&self.model, &tables, input_tables)
                    .map_err(|error| CudaError::DeviceExecution(error.to_string()))
            })();
            if result.is_err() {
                batch.active_host[slot] = 0;
                deactivate = true;
            }
            results.push(result);
        }
        if deactivate {
            self.stream
                .memcpy_htod(&batch.active_host, &mut batch.active)
                .map_err(driver_error)?;
        }
        Ok(results)
    }

    fn download_state_store(&mut self) -> Result<(), CudaError> {
        let (state, inputs, input_counts) = self.download_state_parts()?;
        self.reconstruct_state_store(&state, &inputs, &input_counts)
    }

    fn download_state_parts(&self) -> Result<DownloadedStateParts, CudaError> {
        if self.fused_batch.is_some() {
            let state = self.stream.memcpy_dtov(&self.state).map_err(driver_error)?;
            let inputs = self
                .stream
                .memcpy_dtov(&self.inputs)
                .map_err(driver_error)?;
            let input_counts = self
                .stream
                .memcpy_dtov(&self.input_counts)
                .map_err(driver_error)?;
            return Ok((state, inputs, input_counts));
        }
        let state = if self.layout.state_logical_len == 0 {
            Vec::new()
        } else {
            let source = self
                .state
                .try_slice(0..self.layout.state_logical_len)
                .ok_or_else(|| {
                    CudaError::DeviceExecution(
                        "logical state view exceeds CUDA device slice".to_owned(),
                    )
                })?;
            self.stream.memcpy_dtov(&source).map_err(driver_error)?
        };
        let inputs = if self.layout.input_logical_len == 0 {
            Vec::new()
        } else {
            let source = self
                .inputs
                .try_slice(0..self.layout.input_logical_len)
                .ok_or_else(|| {
                    CudaError::DeviceExecution(
                        "logical input view exceeds CUDA device slice".to_owned(),
                    )
                })?;
            self.stream.memcpy_dtov(&source).map_err(driver_error)?
        };
        let input_counts = if self.layout.ports.is_empty() {
            Vec::new()
        } else {
            let source = self
                .input_counts
                .try_slice(0..self.layout.ports.len())
                .ok_or_else(|| {
                    CudaError::DeviceExecution(
                        "logical input-count view exceeds CUDA device slice".to_owned(),
                    )
                })?;
            self.stream.memcpy_dtov(&source).map_err(driver_error)?
        };
        Ok((state, inputs, input_counts))
    }

    fn reconstruct_state_store(
        &mut self,
        state: &[u8],
        inputs: &[u8],
        input_counts: &[u64],
    ) -> Result<(), CudaError> {
        unpack_state_into(&self.model, &self.layout, state, &mut self.host_tables)?;
        let inputs = unpack_inputs(&self.model, &self.layout, inputs, input_counts);
        self.host_state
            .refresh_backend_snapshot(&self.model, &self.host_tables, inputs)
            .map_err(|error| CudaError::DeviceExecution(error.to_string()))?;
        self.host_state_current = true;
        Ok(())
    }

    fn download_hash(&self) -> Result<[u8; 32], CudaError> {
        let (state, inputs, input_counts) = self.download_state_parts()?;
        Ok(hash_state(
            &self.model,
            &self.layout,
            &state,
            &inputs,
            &input_counts,
        ))
    }
}

fn format_cuda_driver_version(version: i32) -> String {
    format!("{}.{}", version / 1000, (version % 1000) / 10)
}

fn classify_device_count(
    result: Result<i32, cudarc::driver::DriverError>,
) -> Result<i32, CudaError> {
    match result {
        Ok(count) => Ok(count),
        Err(cudarc::driver::DriverError(cudarc::driver::sys::CUresult::CUDA_ERROR_NO_DEVICE)) => {
            Err(CudaError::NoDevice)
        }
        Err(error) => Err(CudaError::Driver(error.to_string())),
    }
}

fn driver_error(error: cudarc::driver::DriverError) -> CudaError {
    CudaError::Driver(error.to_string())
}

fn device_status(status: &[u64]) -> CudaError {
    let message = match status[0] {
        1 => format!("aggregate {} produced an out-of-range group", status[1]),
        2 => format!("aggregate {} overflowed Int", status[1]),
        3 => format!("candidate {} overflowed Int", status[1]),
        4 => format!(
            "candidates {} and {} have incompatible claim ordering",
            status[1], status[2]
        ),
        5 => format!("candidate {} effect overflowed Int", status[1]),
        6 => format!("candidate {} produced an out-of-range Enum", status[1]),
        7 => format!("candidate {} produced an out-of-range Ref", status[1]),
        8 => format!(
            "double write at cell {} by rules {} and {}",
            status[1], status[2], status[3]
        ),
        9 => format!("wire output field {} overflowed Int", status[1]),
        10 => format!("candidate {} claim expression overflowed Int", status[1]),
        code => format!("unknown device status {code}"),
    };
    CudaError::DeviceExecution(message)
}

fn nonempty(values: &[u64]) -> Vec<u64> {
    if values.is_empty() {
        vec![0]
    } else {
        values.to_vec()
    }
}

#[cfg(test)]
#[path = "backend_control_reports_tests.rs"]
mod control_reports_tests;

#[cfg(test)]
#[path = "backend_probe_tests.rs"]
mod probe_tests;

#[cfg(test)]
#[path = "backend_diagnostic_equality_hardware_tests.rs"]
mod diagnostic_equality_hardware;

#[cfg(test)]
#[path = "backend_sweep_capacity_tests.rs"]
mod sweep_capacity_tests;

#[cfg(test)]
#[path = "backend_conflict_geometry_hardware_tests.rs"]
mod conflict_geometry_hardware;
