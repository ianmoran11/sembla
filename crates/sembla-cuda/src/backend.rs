use std::mem;
use std::time::{Duration, Instant};

use cudarc::driver::{
    CudaContext, CudaEvent, CudaFunction, CudaSlice, CudaStream, DeviceRepr, DriverError,
    LaunchArgs, LaunchConfig, PinnedHostSlice, PushKernelArg, ValidAsZeroBits,
};
use cudarc::nvrtc::{compile_ptx_with_opts, CompileOptions};
use sembla_ir::{AttrType, ParamValue, ValidatedModel};
use sembla_runtime::eval::ParamEnv;
use sembla_runtime::executor::{
    DeviceObservationEligibility, GroupedViewValue, ObservationValue, ViewValue,
};
use sembla_runtime::state::{ColumnData, InputTable, StateStore, TableInit};
use sha2::{Digest, Sha256};

use crate::codegen::{
    decode_grouped_histogram, generate_fused_batch, grouped_observation_layout,
    host_observation_fallback, FusedBuffer, GroupedObservationAxisLayout, GroupedObservationLayout,
    FUSED_BUFFER_COUNT, GROUPED_OBSERVATION_KEY_SPACE_LIMIT,
};
use crate::{generate, CudaAvailability, CudaError, GeneratedCuda, PhiloxCoordinate};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum HashMode {
    #[default]
    FinalOnly,
    EveryTick,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CudaRunResult {
    pub final_state_hash: [u8; 32],
    pub per_tick_state_hashes: Vec<[u8; 32]>,
}

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
pub struct CudaDeviceIdentity {
    pub gpu_model: String,
    pub driver_version: String,
}

/// Conservative capacity bound for isolated retained CUDA sweep lanes.
///
/// `device_bytes` includes the complete per-lane device-buffer census, a
/// context/module/stream reserve for every lane, one process-level reserve,
/// allocation-granularity rounding, and a 25% safety margin. `host_bytes` is
/// an assumption rather than an OS admission check: the caller must provide at
/// least this much available host memory. It covers the coordinator state,
/// four state-sized retained/readback copies per lane, per-lane working
/// reserve, a process reserve, and the same safety margin.
#[derive(Clone, Debug)]
pub struct CudaTickObservation {
    pub tick: u32,
    pub state: StateStore,
    pub fired_per_box: Vec<(String, Vec<(u32, usize)>)>,
    pub deferred_per_resource_table: Vec<(String, usize)>,
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
    unsafe { args.launch(one) }
        .map(|_| ())
        .map_err(driver_error)
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

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
enum FinalStateAllocationInjection {
    #[default]
    None,
    #[cfg(test)]
    Pinned(&'static str),
    #[cfg(test)]
    Staging(&'static str),
}

impl FinalStateAllocationInjection {
    fn rejects_pinned(self, _label: &str) -> bool {
        match self {
            Self::None => false,
            #[cfg(test)]
            Self::Pinned(injected) => injected == _label,
            #[cfg(test)]
            Self::Staging(_) => false,
        }
    }

    fn rejects_staging(self, _label: &str) -> bool {
        match self {
            Self::None => false,
            #[cfg(test)]
            Self::Pinned(_) => false,
            #[cfg(test)]
            Self::Staging(injected) => injected == _label,
        }
    }
}

fn allocate_cacheable_staging<T>(
    len: usize,
    label: &'static str,
    injection: FinalStateAllocationInjection,
) -> Result<Vec<T>, CudaError>
where
    T: Copy + Default,
{
    let bytes = len.checked_mul(mem::size_of::<T>()).ok_or_else(|| {
        CudaError::InvalidInput(format!("packed-pinned {label} staging byte size overflow"))
    })?;
    if injection.rejects_staging(label) {
        return Err(CudaError::DeviceExecution(format!(
            "injected packed-pinned cacheable staging allocation failure for {label}: requested {bytes} bytes for one lane"
        )));
    }
    let mut cacheable = Vec::new();
    cacheable.try_reserve_exact(len).map_err(|error| {
        CudaError::DeviceExecution(format!(
            "packed-pinned cacheable staging allocation failed for {label}: requested {bytes} bytes for one lane: {error}"
        ))
    })?;
    cacheable.resize(len, T::default());
    if cacheable.capacity() != len {
        let effective_bytes = cacheable
            .capacity()
            .checked_mul(mem::size_of::<T>())
            .ok_or_else(|| {
                CudaError::InvalidInput(format!(
                    "packed-pinned {label} effective staging capacity overflow"
                ))
            })?;
        return Err(CudaError::DeviceExecution(format!(
            "packed-pinned cacheable staging allocation for {label} retained {effective_bytes} bytes, but admission reserved exactly {bytes} bytes for one lane"
        )));
    }
    Ok(cacheable)
}

#[derive(Debug)]
struct PinnedFinalStateComponent<T> {
    label: &'static str,
    pinned: PinnedHostSlice<T>,
    cacheable: Vec<T>,
}

impl<T> PinnedFinalStateComponent<T>
where
    T: Copy + Default + DeviceRepr + ValidAsZeroBits,
{
    fn allocate(
        stream: &std::sync::Arc<CudaStream>,
        len: usize,
        label: &'static str,
        injection: FinalStateAllocationInjection,
    ) -> Result<Option<Self>, CudaError> {
        if len == 0 {
            return Ok(None);
        }
        let bytes = len.checked_mul(mem::size_of::<T>()).ok_or_else(|| {
            CudaError::InvalidInput(format!(
                "packed-pinned {label} destination byte size overflow"
            ))
        })?;
        if bytes >= isize::MAX as usize {
            return Err(CudaError::InvalidInput(format!(
                "packed-pinned {label} destination requests {bytes} bytes, which exceeds cudarc's pinned-slice bound"
            )));
        }

        let cacheable = allocate_cacheable_staging(len, label, injection)?;

        if injection.rejects_pinned(label) {
            return Err(CudaError::DeviceExecution(format!(
                "injected packed-pinned page-locked allocation failure for {label}: requested {bytes} bytes for one lane"
            )));
        }
        // SAFETY: cudarc returns uninitialized page-locked memory. This owner
        // never exposes or reads it until a full-slice `memcpy_dtoh` has been
        // enqueued on `stream`, that stream has synchronized successfully, and
        // the destination's recorded event has also synchronized via
        // `PinnedHostSlice::as_slice`. Zero-length components never enter this
        // constructor. The checked byte bound above prevents cudarc's internal
        // multiplication/assertions from overflowing or panicking.
        let pinned = unsafe { stream.context().alloc_pinned::<T>(len) }.map_err(|error| {
            CudaError::Driver(format!(
                "packed-pinned page-locked allocation failed for {label}: requested {bytes} bytes for one lane: {error}"
            ))
        })?;
        Ok(Some(Self {
            label,
            pinned,
            cacheable,
        }))
    }

    fn wait_until_readable(&self) -> Result<(), CudaError> {
        self.pinned.as_slice().map(|_| ()).map_err(|error| {
            CudaError::Driver(format!(
                "packed-pinned {} destination did not become host-readable: {error}",
                self.label
            ))
        })
    }

    fn stage(&mut self) -> Result<(), CudaError> {
        let pinned = self.pinned.as_slice().map_err(|error| {
            CudaError::Driver(format!(
                "packed-pinned {} destination was not readable for cacheable staging: {error}",
                self.label
            ))
        })?;
        self.cacheable.copy_from_slice(pinned);
        Ok(())
    }
}

#[derive(Debug)]
struct PinnedFinalStateBuffers {
    // This is an Arc clone of the backend's existing default/non-blocking lane
    // stream. No treatment-specific stream is created.
    stream: std::sync::Arc<CudaStream>,
    state: Option<PinnedFinalStateComponent<u8>>,
    inputs: Option<PinnedFinalStateComponent<u8>>,
    input_counts: Option<PinnedFinalStateComponent<u64>>,
    accounting: CudaFinalStateBufferAccounting,
}

impl PinnedFinalStateBuffers {
    fn allocate(
        stream: &std::sync::Arc<CudaStream>,
        bytes: CudaFinalStateDownloadedBytes,
        injection: FinalStateAllocationInjection,
    ) -> Result<Self, CudaError> {
        let state = PinnedFinalStateComponent::allocate(stream, bytes.state, "state", injection)?;
        let inputs =
            PinnedFinalStateComponent::allocate(stream, bytes.inputs, "inputs", injection)?;
        let input_count_len = bytes
            .input_counts
            .checked_div(mem::size_of::<u64>())
            .ok_or_else(|| {
                CudaError::InvalidInput("input-count size divisor is zero".to_owned())
            })?;
        let input_counts = PinnedFinalStateComponent::allocate(
            stream,
            input_count_len,
            "input counts",
            injection,
        )?;

        let underlying_pinned_allocation_count = usize::from(state.is_some())
            + usize::from(inputs.is_some())
            + usize::from(input_counts.is_some());
        let cacheable_staging_bytes = state
            .as_ref()
            .and_then(|component| {
                component
                    .cacheable
                    .capacity()
                    .checked_mul(mem::size_of::<u8>())
            })
            .unwrap_or(0)
            .checked_add(
                inputs
                    .as_ref()
                    .and_then(|component| {
                        component
                            .cacheable
                            .capacity()
                            .checked_mul(mem::size_of::<u8>())
                    })
                    .unwrap_or(0),
            )
            .and_then(|value| {
                value.checked_add(
                    input_counts
                        .as_ref()
                        .and_then(|component| {
                            component
                                .cacheable
                                .capacity()
                                .checked_mul(mem::size_of::<u64>())
                        })
                        .unwrap_or(0),
                )
            })
            .ok_or_else(|| {
                CudaError::InvalidInput(
                    "packed-pinned cacheable staging capacity overflow".to_owned(),
                )
            })?;
        Ok(Self {
            stream: std::sync::Arc::clone(stream),
            state,
            inputs,
            input_counts,
            accounting: CudaFinalStateBufferAccounting {
                buffer_set_count: 1,
                underlying_pinned_allocation_count,
                pinned_bytes: bytes.total,
                cacheable_staging_bytes,
            },
        })
    }

    fn wait_until_readable(&self) -> Result<(), CudaError> {
        self.stream.synchronize().map_err(|error| {
            CudaError::Driver(format!(
                "packed-pinned existing lane stream did not complete final-state D2H work: {error}"
            ))
        })?;
        if let Some(component) = &self.state {
            component.wait_until_readable()?;
        }
        if let Some(component) = &self.inputs {
            component.wait_until_readable()?;
        }
        if let Some(component) = &self.input_counts {
            component.wait_until_readable()?;
        }
        Ok(())
    }

    fn stage(&mut self) -> Result<(), CudaError> {
        if let Some(component) = &mut self.state {
            component.stage()?;
        }
        if let Some(component) = &mut self.inputs {
            component.stage()?;
        }
        if let Some(component) = &mut self.input_counts {
            component.stage()?;
        }
        Ok(())
    }

    fn state(&self) -> &[u8] {
        self.state
            .as_ref()
            .map_or(&[], |component| component.cacheable.as_slice())
    }

    fn inputs(&self) -> &[u8] {
        self.inputs
            .as_ref()
            .map_or(&[], |component| component.cacheable.as_slice())
    }

    fn input_counts(&self) -> &[u64] {
        self.input_counts
            .as_ref()
            .map_or(&[], |component| component.cacheable.as_slice())
    }
}

impl Drop for PinnedFinalStateBuffers {
    fn drop(&mut self) {
        // A failed enqueue/wait may unwind with work still pending. Synchronize
        // the retained lane stream before the pinned destinations are freed.
        self.stream.context().record_err(self.stream.synchronize());
    }
}

fn checked_final_state_component_bytes(
    state: usize,
    inputs: usize,
    input_count_len: usize,
) -> Result<CudaFinalStateDownloadedBytes, CudaError> {
    let input_counts = input_count_len
        .checked_mul(mem::size_of::<u64>())
        .ok_or_else(|| CudaError::InvalidInput("input-count byte total overflow".to_owned()))?;
    let total = state
        .checked_add(inputs)
        .and_then(|bytes| bytes.checked_add(input_counts))
        .ok_or_else(|| CudaError::InvalidInput("final-state byte total overflow".to_owned()))?;
    Ok(CudaFinalStateDownloadedBytes {
        state,
        inputs,
        input_counts,
        total,
    })
}

fn final_state_component_bytes(
    layout: &Layout,
) -> Result<CudaFinalStateDownloadedBytes, CudaError> {
    checked_final_state_component_bytes(
        layout.state_logical_len,
        layout.input_logical_len,
        layout.ports.len(),
    )
}

fn checked_arena_len(per_slot: usize, slots: usize, label: &str) -> Result<usize, CudaError> {
    per_slot.checked_mul(slots).ok_or_else(|| {
        CudaError::InvalidInput(format!(
            "fused CUDA {label} arena size overflow for capacity {slots}"
        ))
    })
}

const SWEEP_CAPACITY_MIB: usize = 1024 * 1024;
const SWEEP_CAPACITY_ALLOCATION_GRANULARITY: usize = 64 * 1024;
const SWEEP_CAPACITY_FIXED_DEVICE_RESERVE: usize = 512 * SWEEP_CAPACITY_MIB;
const SWEEP_CAPACITY_CONTEXT_STREAM_RESERVE: usize = 512 * SWEEP_CAPACITY_MIB;
const SWEEP_CAPACITY_MAX_GENERATED_SOURCE_BYTES: usize = 16 * SWEEP_CAPACITY_MIB;
const SWEEP_CAPACITY_MODULE_SOURCE_MULTIPLIER: usize = 64;
const SWEEP_CAPACITY_FUNCTION_RESERVE: usize = 4 * SWEEP_CAPACITY_MIB;
const SWEEP_CAPACITY_MAX_LOADED_FUNCTIONS: usize = 4096;
const SWEEP_CAPACITY_FIXED_LOADED_FUNCTIONS: usize = 48;
const SWEEP_CAPACITY_FIXED_HOST_RESERVE: usize = 1024 * SWEEP_CAPACITY_MIB;
const SWEEP_CAPACITY_PER_LANE_HOST_RESERVE: usize = 256 * SWEEP_CAPACITY_MIB;
const SWEEP_CAPACITY_MARGIN_NUMERATOR: usize = 5;
const SWEEP_CAPACITY_MARGIN_DENOMINATOR: usize = 4;

fn checked_capacity_add(total: &mut usize, value: usize, label: &str) -> Result<(), CudaError> {
    *total = total.checked_add(value).ok_or_else(|| {
        CudaError::InvalidInput(format!("CUDA sweep {label} capacity estimate overflow"))
    })?;
    Ok(())
}

fn rounded_device_allocation(
    elements: usize,
    element_bytes: usize,
    label: &str,
) -> Result<usize, CudaError> {
    let bytes = elements.checked_mul(element_bytes).ok_or_else(|| {
        CudaError::InvalidInput(format!("CUDA sweep {label} buffer size overflow"))
    })?;
    let bytes = bytes.max(1);
    bytes
        .checked_add(SWEEP_CAPACITY_ALLOCATION_GRANULARITY - 1)
        .map(|value| value / SWEEP_CAPACITY_ALLOCATION_GRANULARITY)
        .and_then(|units| units.checked_mul(SWEEP_CAPACITY_ALLOCATION_GRANULARITY))
        .ok_or_else(|| {
            CudaError::InvalidInput(format!(
                "CUDA sweep {label} allocation-rounded size overflow"
            ))
        })
}

fn with_capacity_safety_margin(bytes: usize, label: &str) -> Result<usize, CudaError> {
    bytes
        .checked_mul(SWEEP_CAPACITY_MARGIN_NUMERATOR)
        .map(|value| value.div_ceil(SWEEP_CAPACITY_MARGIN_DENOMINATOR))
        .ok_or_else(|| {
            CudaError::InvalidInput(format!("CUDA sweep {label} safety margin overflow"))
        })
}

fn isolated_lane_device_buffer_bytes(
    layout: &Layout,
    generated: &GeneratedCuda,
    parameter_bytes: usize,
) -> Result<usize, CudaError> {
    let mut total = 0_usize;
    let mut add = |elements: usize, element_bytes: usize, label: &str| {
        checked_capacity_add(
            &mut total,
            rounded_device_allocation(elements, element_bytes, label)?,
            label,
        )
    };
    let nonempty_len = |length: usize| length.max(1);

    for label in ["state", "next state", "pristine state"] {
        add(layout.state_len, 1, label)?;
    }
    add(
        nonempty_len(layout.column_offsets.len()),
        8,
        "column offsets",
    )?;
    add(nonempty_len(layout.row_counts.len()), 8, "row counts")?;
    add(
        nonempty_len(layout.resource_offsets.len()),
        8,
        "resource offsets",
    )?;
    add(layout.input_len.max(1), 1, "inputs")?;
    add(layout.input_len.max(1), 1, "next inputs")?;
    add(nonempty_len(layout.input_offsets.len()), 8, "input offsets")?;
    add(layout.ports.len().max(1), 8, "input counts")?;
    add(layout.ports.len().max(1), 8, "next input counts")?;
    add(parameter_bytes.max(1), 1, "parameters")?;
    add(layout.aggregate_len.max(1), 1, "aggregates")?;
    add(
        layout
            .aggregate_len
            .max(1)
            .checked_mul(2)
            .ok_or_else(|| CudaError::InvalidInput("aggregate partial size overflow".to_owned()))?,
        1,
        "aggregate partials",
    )?;
    add(
        (layout.aggregate_max_groups + 2).max(2),
        1,
        "aggregate errors",
    )?;
    let aggregate_meta = generated.aggregate_group_tables.len().max(1);
    add(aggregate_meta, 1, "aggregate facts")?;
    add(aggregate_meta, 1, "aggregate active")?;
    add(
        nonempty_len(layout.aggregate_offsets.len()),
        8,
        "aggregate offsets",
    )?;
    add(
        nonempty_len(layout.candidate_offsets.len()),
        8,
        "candidate offsets",
    )?;
    add(
        nonempty_len(layout.claim_instance_offsets.len()),
        8,
        "claim instance offsets",
    )?;
    let candidates = layout.candidate_count.max(1);
    add(candidates, 1, "enabled candidates")?;
    add(candidates, 8, "candidate times")?;
    add(
        candidates
            .checked_mul(2)
            .ok_or_else(|| CudaError::InvalidInput("candidate error size overflow".to_owned()))?,
        1,
        "candidate errors",
    )?;
    add(candidates, 1, "candidate wins")?;
    add(
        candidates
            .checked_mul(layout.row_counts.len().max(1))
            .ok_or_else(|| CudaError::InvalidInput("deferred metadata size overflow".to_owned()))?,
        1,
        "deferred metadata",
    )?;
    add(layout.candidate_offsets.len().max(1), 8, "fired counts")?;
    add(layout.row_counts.len().max(1), 8, "deferred counts")?;
    let claims = layout.claim_instance_count.max(1);
    add(claims, 8, "instance resources")?;
    add(claims, 8, "instance keys")?;
    add(claims, 4, "instance rules")?;
    add(claims, 4, "instance entities")?;
    let resources = layout.resource_count.max(1);
    add(resources, 8, "winner keys")?;
    add(resources, 4, "winner rules")?;
    add(resources, 4, "winner entities")?;
    add(resources, 8, "winner instances")?;
    add(nonempty_len(layout.write_offsets.len()), 8, "write offsets")?;
    let owners = layout.owner_count.max(1);
    add(owners, 4, "effect owners")?;
    add(owners, 8, "effect values")?;
    let output_fields = layout.input_offsets.len().max(1);
    add(
        output_fields
            .checked_mul(2)
            .ok_or_else(|| CudaError::InvalidInput("output partial size overflow".to_owned()))?,
        8,
        "output partials",
    )?;
    add(
        output_fields
            .checked_mul(3)
            .ok_or_else(|| CudaError::InvalidInput("output error size overflow".to_owned()))?,
        1,
        "output errors",
    )?;
    add(
        generated.observation_view_tables.len().max(1),
        8,
        "observation values",
    )?;
    add(
        generated
            .grouped_observation_band_axes
            .checked_mul(2)
            .ok_or_else(|| CudaError::InvalidInput("grouped extrema size overflow".to_owned()))?
            .max(1),
        8,
        "grouped extrema",
    )?;
    let grouped_axes = generated
        .grouped_observation_views
        .iter()
        .try_fold(0_usize, |count, view| count.checked_add(view.axes.len()))
        .ok_or_else(|| CudaError::InvalidInput("grouped axis count overflow".to_owned()))?
        .max(1);
    add(grouped_axes, 8, "grouped axis minima")?;
    add(grouped_axes, 8, "grouped axis cardinalities")?;
    add(
        if generated.grouped_observation_views.is_empty() {
            1
        } else {
            GROUPED_OBSERVATION_KEY_SPACE_LIMIT
        },
        8,
        "grouped histogram",
    )?;
    add(
        generated.generic_enum_count.max(1),
        8,
        "generic enum counts",
    )?;
    add(12, 8, "validation status")?;
    add(layout.candidate_offsets.len().max(1), 4, "effect active")?;
    Ok(total)
}

fn estimate_isolated_sweep_capacity(
    layout: &Layout,
    generated: &GeneratedCuda,
    parameter_bytes: usize,
    workers: usize,
    final_state_mode: CudaFinalStateReadbackMode,
) -> Result<CudaSweepCapacityEstimate, CudaError> {
    if workers == 0 {
        return Err(CudaError::InvalidInput(
            "CUDA sweep worker count must be greater than zero".to_owned(),
        ));
    }
    let census = isolated_lane_device_buffer_bytes(layout, generated, parameter_bytes)?;
    if generated.source.len() > SWEEP_CAPACITY_MAX_GENERATED_SOURCE_BYTES {
        return Err(CudaError::InvalidInput(format!(
            "generated CUDA source is {} bytes; the conservative sweep capacity bound supports at most {} bytes",
            generated.source.len(),
            SWEEP_CAPACITY_MAX_GENERATED_SOURCE_BYTES
        )));
    }
    let loaded_functions = generated
        .transition_kernels
        .len()
        .checked_add(SWEEP_CAPACITY_FIXED_LOADED_FUNCTIONS)
        .ok_or_else(|| CudaError::InvalidInput("CUDA function-count overflow".to_owned()))?;
    if loaded_functions > SWEEP_CAPACITY_MAX_LOADED_FUNCTIONS {
        return Err(CudaError::InvalidInput(format!(
            "CUDA model loads {loaded_functions} functions; the conservative sweep capacity bound supports at most {SWEEP_CAPACITY_MAX_LOADED_FUNCTIONS}"
        )));
    }
    // Module/JIT memory is model-dependent even though it is not represented by
    // a CudaSlice. Bound it from generated source bytes and loaded-function
    // count, then fail closed above rather than extrapolating to arbitrary
    // codegen shapes. The deliberately large multipliers cover PTX/SASS/debug
    // metadata and driver bookkeeping separately from the context/stream bound.
    let module_reserve = generated
        .source
        .len()
        .checked_mul(SWEEP_CAPACITY_MODULE_SOURCE_MULTIPLIER)
        .and_then(|bytes| {
            loaded_functions
                .checked_mul(SWEEP_CAPACITY_FUNCTION_RESERVE)
                .and_then(|functions| bytes.checked_add(functions))
        })
        .ok_or_else(|| CudaError::InvalidInput("CUDA module reserve overflow".to_owned()))?;
    let per_lane_device = census
        .checked_add(SWEEP_CAPACITY_CONTEXT_STREAM_RESERVE)
        .and_then(|bytes| bytes.checked_add(module_reserve))
        .ok_or_else(|| CudaError::InvalidInput("CUDA sweep lane reserve overflow".to_owned()))?;
    let device_before_margin = per_lane_device
        .checked_mul(workers)
        .and_then(|bytes| bytes.checked_add(SWEEP_CAPACITY_FIXED_DEVICE_RESERVE))
        .ok_or_else(|| CudaError::InvalidInput("CUDA sweep device estimate overflow".to_owned()))?;
    let final_state_bytes_per_lane = final_state_component_bytes(layout)?;
    let (requested_pinned_bytes_per_lane, requested_cacheable_staging_bytes_per_lane) =
        if final_state_mode == CudaFinalStateReadbackMode::PackedPinned {
            (
                final_state_bytes_per_lane.total,
                final_state_bytes_per_lane.total,
            )
        } else {
            (0, 0)
        };
    let requested_pinned_bytes = requested_pinned_bytes_per_lane
        .checked_mul(workers)
        .ok_or_else(|| {
            CudaError::InvalidInput("CUDA sweep pinned-byte estimate overflow".to_owned())
        })?;
    let requested_cacheable_staging_bytes = requested_cacheable_staging_bytes_per_lane
        .checked_mul(workers)
        .ok_or_else(|| {
            CudaError::InvalidInput("CUDA sweep staging-byte estimate overflow".to_owned())
        })?;
    let allocations_per_lane = usize::from(final_state_bytes_per_lane.state != 0)
        + usize::from(final_state_bytes_per_lane.inputs != 0)
        + usize::from(final_state_bytes_per_lane.input_counts != 0);
    let requested_buffer_set_count =
        usize::from(final_state_mode == CudaFinalStateReadbackMode::PackedPinned)
            .checked_mul(workers)
            .ok_or_else(|| {
                CudaError::InvalidInput("CUDA sweep pinned buffer-set count overflow".to_owned())
            })?;
    let requested_underlying_pinned_allocation_count = if requested_buffer_set_count == 0 {
        0
    } else {
        allocations_per_lane.checked_mul(workers).ok_or_else(|| {
            CudaError::InvalidInput("CUDA sweep pinned allocation-count overflow".to_owned())
        })?
    };
    let retained_host_state = layout
        .state_len
        .checked_mul(4)
        .and_then(|bytes| bytes.checked_add(SWEEP_CAPACITY_PER_LANE_HOST_RESERVE))
        .and_then(|bytes| bytes.checked_add(requested_pinned_bytes_per_lane))
        .and_then(|bytes| bytes.checked_add(requested_cacheable_staging_bytes_per_lane))
        .ok_or_else(|| {
            CudaError::InvalidInput("CUDA sweep host lane estimate overflow".to_owned())
        })?;
    let host_before_margin = retained_host_state
        .checked_mul(workers)
        .and_then(|bytes| bytes.checked_add(layout.state_len))
        .and_then(|bytes| bytes.checked_add(SWEEP_CAPACITY_FIXED_HOST_RESERVE))
        .ok_or_else(|| CudaError::InvalidInput("CUDA sweep host estimate overflow".to_owned()))?;
    Ok(CudaSweepCapacityEstimate {
        workers,
        device_bytes: with_capacity_safety_margin(device_before_margin, "device")?,
        device_bytes_per_lane_before_margin: per_lane_device,
        fixed_device_bytes_before_margin: SWEEP_CAPACITY_FIXED_DEVICE_RESERVE,
        host_bytes: with_capacity_safety_margin(host_before_margin, "host")?,
        safety_margin_percent: 25,
        final_state_bytes_per_lane,
        requested_pinned_bytes_per_lane,
        requested_cacheable_staging_bytes_per_lane,
        requested_pinned_bytes,
        requested_cacheable_staging_bytes,
        requested_buffer_set_count,
        requested_underlying_pinned_allocation_count,
    })
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

    unsafe fn launch(
        &mut self,
        mut config: LaunchConfig,
    ) -> Result<Option<(CudaEvent, CudaEvent)>, DriverError> {
        config.grid_dim.1 = self.grid_y;
        unsafe { self.inner.launch(config) }
    }
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
                let stream = std::sync::Arc::clone(&self.stream);
                let enqueue_started = Instant::now();
                {
                    let buffers = self
                        .pinned_final_state
                        .as_mut()
                        .expect("packed-pinned allocation installed its owner");
                    if let Some(destination) = &mut buffers.state {
                        let source =
                            self.state
                                .try_slice(0..downloaded_bytes.state)
                                .ok_or_else(|| {
                                    CudaError::DeviceExecution(
                                        "packed-pinned logical state view exceeds device slice"
                                            .to_owned(),
                                    )
                                })?;
                        stream
                            .memcpy_dtoh(&source, &mut destination.pinned)
                            .map_err(|error| {
                                CudaError::Driver(format!(
                                    "packed-pinned state D2H enqueue failed for {} bytes: {error}",
                                    downloaded_bytes.state
                                ))
                            })?;
                    }
                    if let Some(destination) = &mut buffers.inputs {
                        let source = self
                            .inputs
                            .try_slice(0..downloaded_bytes.inputs)
                            .ok_or_else(|| {
                                CudaError::DeviceExecution(
                                    "packed-pinned logical input view exceeds device slice"
                                        .to_owned(),
                                )
                            })?;
                        stream
                            .memcpy_dtoh(&source, &mut destination.pinned)
                            .map_err(|error| {
                                CudaError::Driver(format!(
                                    "packed-pinned input D2H enqueue failed for {} bytes: {error}",
                                    downloaded_bytes.inputs
                                ))
                            })?;
                    }
                    if let Some(destination) = &mut buffers.input_counts {
                        let count_len = downloaded_bytes.input_counts / mem::size_of::<u64>();
                        let source =
                            self.input_counts.try_slice(0..count_len).ok_or_else(|| {
                                CudaError::DeviceExecution(
                                    "packed-pinned logical input-count view exceeds device slice"
                                        .to_owned(),
                                )
                            })?;
                        stream
                            .memcpy_dtoh(&source, &mut destination.pinned)
                            .map_err(|error| {
                                CudaError::Driver(format!(
                                    "packed-pinned input-count D2H enqueue failed for {} bytes: {error}",
                                    downloaded_bytes.input_counts
                                ))
                            })?;
                    }
                }
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
                let buffer_accounting = buffers.accounting;
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

    /// Returns the once-per-run IR eligibility decision used by this backend.
    pub fn observation_eligibility(&self) -> &DeviceObservationEligibility {
        &self.generated.observation_eligibility
    }

    /// Executes one tick on CUDA and downloads a read-only observation snapshot.
    /// State remains resident on the device for subsequent ticks.
    pub fn run_tick_observed(&mut self) -> Result<CudaTickObservation, CudaError> {
        let (tick, fired_per_box, deferred_per_resource_table, _) =
            self.run_tick_observed_reused()?;
        self.ensure_host_state()?;
        Ok(CudaTickObservation {
            tick,
            state: self.host_state.clone(),
            fired_per_box,
            deferred_per_resource_table,
        })
    }

    /// Executes one observed tick while retaining the host state allocation.
    /// `Some(views)` is the all-device fast path; `None` means the complete
    /// state was downloaded and the caller must use host observation.
    #[doc(hidden)]
    pub fn run_tick_observed_reused(&mut self) -> Result<ReusedCudaTickObservation, CudaError> {
        let tick = self.next_tick;
        self.execute_tick()?;
        let views = self.observe_device_views(tick)?;
        let (fired_counts, deferred_counts) = self.readback_control()?;
        let (fired_per_box, deferred_per_resource_table) =
            control_reports_from_counts(&self.model, &fired_counts, &deferred_counts)?;
        host_observation_fallback(views.is_none(), || self.download_state_store())?;
        Ok((tick, fired_per_box, deferred_per_resource_table, views))
    }

    /// Executes one grid-y tick for every active fused slot. Transport errors
    /// fail the batch; semantic device errors remain isolated by slot.
    #[doc(hidden)]
    pub fn run_tick_observed_reused_fused(
        &mut self,
    ) -> Result<FusedReusedCudaTickObservations, CudaError> {
        let active_width = self
            .fused_batch
            .as_ref()
            .ok_or_else(|| CudaError::InvalidInput("backend is not fused-batch CUDA".to_owned()))?
            .active_width;
        if active_width == 0 {
            return Err(CudaError::InvalidInput(
                "fused batch must be reset with at least one active draw before execution"
                    .to_owned(),
            ));
        }
        let tick = self.next_tick;
        let statuses = self.execute_tick_batch_statuses()?;
        let views = self.observe_device_views_batch(tick)?;
        let fired = self
            .stream
            .memcpy_dtov(&self.fired_counts)
            .map_err(driver_error)?;
        let deferred = self
            .stream
            .memcpy_dtov(&self.deferred_counts)
            .map_err(driver_error)?;
        let reconstruction = if !self.generated.observation_eligibility.eligible {
            self.download_fused_state_stores()?
        } else {
            (0..active_width).map(|_| Ok(())).collect()
        };
        let batch = self
            .fused_batch
            .as_ref()
            .ok_or_else(|| CudaError::InvalidInput("backend is not fused-batch CUDA".to_owned()))?;
        let fired_stride = batch.strides_host[FusedBuffer::FiredCounts as usize];
        let deferred_stride = batch.strides_host[FusedBuffer::DeferredCounts as usize];
        let mut results = Vec::with_capacity(statuses.len());
        for (slot, ((status, views), reconstruction)) in statuses
            .into_iter()
            .zip(views)
            .zip(reconstruction)
            .enumerate()
        {
            let result = (|| {
                status?;
                reconstruction?;
                let views = views?;
                let fired_begin = slot * fired_stride;
                let deferred_begin = slot * deferred_stride;
                let (fired_per_box, deferred_per_resource_table) = control_reports_from_counts(
                    &self.model,
                    &fired[fired_begin..fired_begin + self.layout.candidate_offsets.len()],
                    &deferred[deferred_begin..deferred_begin + self.layout.row_counts.len()],
                )?;
                Ok((tick, fired_per_box, deferred_per_resource_table, views))
            })();
            if result.is_err() {
                self.deactivate_fused_slot(slot)?;
            }
            results.push(result);
        }
        Ok(results)
    }

    #[doc(hidden)]
    pub fn fused_observed_state(&self, slot: usize) -> Result<&StateStore, CudaError> {
        self.fused_batch
            .as_ref()
            .and_then(|batch| batch.host_states.get(slot))
            .ok_or_else(|| CudaError::InvalidInput(format!("invalid fused CUDA slot {slot}")))
    }

    #[doc(hidden)]
    pub fn ensure_fused_observed_states(
        &mut self,
    ) -> Result<Vec<Result<Option<StateStore>, CudaError>>, CudaError> {
        let reconstruction = self.download_fused_state_stores()?;
        let batch = self.fused_batch.as_ref().expect("fused batch exists");
        Ok(reconstruction
            .into_iter()
            .enumerate()
            .map(|(slot, result)| {
                result.map(|()| {
                    (batch.active_host[slot] != 0).then(|| batch.host_states[slot].clone())
                })
            })
            .collect())
    }

    /// Executes one observed CUDA tick and returns durations in this order:
    /// kernels, control readback, state transfer, state reconstruction, and
    /// host control-report assembly. `execute_tick` already synchronizes
    /// through its terminal status D2H copy, so this instrumentation
    /// deliberately inserts no second sync.
    pub fn run_tick_observed_timed(
        &mut self,
    ) -> Result<(CudaTickObservation, [std::time::Duration; 5]), CudaError> {
        let (tick, fired_per_box, deferred_per_resource_table, _, mut phases) =
            self.run_tick_observed_reused_timed()?;
        let started = std::time::Instant::now();
        self.ensure_host_state()?;
        let state = self.host_state.clone();
        phases[3] += started.elapsed();
        Ok((
            CudaTickObservation {
                tick,
                state,
                fired_per_box,
                deferred_per_resource_table,
            },
            phases,
        ))
    }

    /// Timed counterpart of [`Self::run_tick_observed_reused`].
    #[doc(hidden)]
    pub fn run_tick_observed_reused_timed(
        &mut self,
    ) -> Result<TimedReusedCudaTickObservation, CudaError> {
        let tick = self.next_tick;

        let started = std::time::Instant::now();
        self.execute_tick()?;
        let views = self.observe_device_views(tick)?;
        let kernels = started.elapsed();

        let started = std::time::Instant::now();
        let (fired_counts, deferred_counts) = self.readback_control()?;
        let readback_control = started.elapsed();

        let started = std::time::Instant::now();
        let (fired_per_box, deferred_per_resource_table) =
            control_reports_from_counts(&self.model, &fired_counts, &deferred_counts)?;
        let report = started.elapsed();

        let (state_transfer, state_reconstruct) =
            host_observation_fallback(views.is_none(), || {
                let started = std::time::Instant::now();
                let (state, inputs, input_counts) = self.download_state_parts()?;
                let state_transfer = started.elapsed();

                let started = std::time::Instant::now();
                self.reconstruct_state_store(&state, &inputs, &input_counts)?;
                Ok((state_transfer, started.elapsed()))
            })?
            .unwrap_or((std::time::Duration::ZERO, std::time::Duration::ZERO));

        Ok((
            tick,
            fired_per_box,
            deferred_per_resource_table,
            views,
            [
                kernels,
                readback_control,
                state_transfer,
                state_reconstruct,
                report,
            ],
        ))
    }

    /// Returns the backend-owned host snapshot. It is current after fallback
    /// ticks; fast-path callers must not inspect it until final extraction.
    #[doc(hidden)]
    pub fn observed_state(&self) -> &StateStore {
        &self.host_state
    }

    /// Hashes the current tick. Fast-path runs deliberately transfer raw device
    /// bytes here for `HashMode::EveryTick`; fallback runs reuse host state.
    #[doc(hidden)]
    pub fn observed_hash(&self) -> Result<[u8; 32], CudaError> {
        if self.host_state_current {
            Ok(self.host_state.state_hash())
        } else {
            self.download_hash()
        }
    }

    /// Moves the final state out, downloading it exactly once when fast-path
    /// ticks left the retained host snapshot stale.
    #[doc(hidden)]
    pub fn into_observed_state(mut self) -> Result<StateStore, CudaError> {
        self.ensure_host_state()?;
        Ok(self.host_state)
    }

    fn observe_device_views_batch(
        &mut self,
        _tick: u32,
    ) -> Result<Vec<Result<Option<CudaDeviceObservations>, CudaError>>, CudaError> {
        let active_width = self
            .fused_batch
            .as_ref()
            .ok_or_else(|| CudaError::InvalidInput("backend is not fused-batch CUDA".to_owned()))?
            .active_width;
        if !self.generated.observation_eligibility.eligible {
            return Ok((0..active_width).map(|_| Ok(None)).collect());
        }
        let mut slot_errors = (0..active_width)
            .map(|_| None)
            .collect::<Vec<Option<CudaError>>>();
        let strides = self
            .fused_batch
            .as_ref()
            .expect("fused batch exists")
            .strides_host
            .clone();

        let scalar_count =
            u32::try_from(self.generated.observation_view_tables.len()).map_err(|_| {
                CudaError::InvalidInput("observation view count exceeds u32".to_owned())
            })?;
        if scalar_count != 0 {
            let mut init = fused_launch_builder(
                &self.stream,
                &self.init_observations,
                self.fused_batch.as_ref(),
            );
            init.arg(&mut self.observation_values).arg(&scalar_count);
            unsafe { init.launch(LaunchConfig::for_num_elems(scalar_count)) }
                .map_err(driver_error)?;
        }
        for (view_index, table) in self
            .generated
            .observation_view_tables
            .iter()
            .copied()
            .enumerate()
        {
            let rows = u32::try_from(self.layout.row_counts[table]).map_err(|_| {
                CudaError::InvalidInput("observation row count exceeds u32".to_owned())
            })?;
            if rows == 0 {
                continue;
            }
            let view_index = u32::try_from(view_index).map_err(|_| {
                CudaError::InvalidInput("observation view index exceeds u32".to_owned())
            })?;
            let mut config = LaunchConfig::for_num_elems(rows);
            config.grid_dim.0 = config.grid_dim.0.min(1024);
            config.shared_mem_bytes = config.block_dim.0 * 8;
            let mut observe =
                fused_launch_builder(&self.stream, &self.observe_view, self.fused_batch.as_ref());
            observe
                .arg(&self.state)
                .arg(&self.column_offsets)
                .arg(&self.row_counts)
                .arg(&self.params)
                .arg(&mut self.observation_values)
                .arg(&view_index);
            unsafe { observe.launch(config) }.map_err(driver_error)?;
        }
        let scalar_arena = self
            .stream
            .memcpy_dtov(&self.observation_values)
            .map_err(driver_error)?;
        let scalar_stride = strides[FusedBuffer::ObservationValues as usize];
        let scalar_names = self
            .model
            .model()
            .boxes
            .iter()
            .flat_map(|model_box| {
                model_box
                    .views
                    .iter()
                    .map(move |view| (model_box.name.clone(), view.name.clone()))
            })
            .collect::<Vec<_>>();
        let mut slot_views = (0..active_width)
            .map(|slot| {
                let begin = slot * scalar_stride;
                scalar_names
                    .iter()
                    .cloned()
                    .zip(
                        scalar_arena[begin..begin + scalar_count as usize]
                            .iter()
                            .copied(),
                    )
                    .map(|((box_name, name), value)| ViewValue {
                        box_name,
                        name,
                        value: ObservationValue::Int(value),
                    })
                    .collect::<Vec<_>>()
            })
            .collect::<Vec<_>>();

        let grouped_specs = self.generated.grouped_observation_views.clone();
        let band_count = self.generated.grouped_observation_band_axes;
        let band_arena = if band_count == 0 {
            vec![0_i64; active_width]
        } else {
            let band_count_u32 = u32::try_from(band_count).map_err(|_| {
                CudaError::InvalidInput("grouped band axis count exceeds u32".to_owned())
            })?;
            let mut init = fused_launch_builder(
                &self.stream,
                &self.init_grouped_extrema,
                self.fused_batch.as_ref(),
            );
            init.arg(&mut self.grouped_extrema).arg(&band_count_u32);
            unsafe { init.launch(LaunchConfig::for_num_elems(band_count_u32)) }
                .map_err(driver_error)?;
            for (view_index, view) in grouped_specs.iter().enumerate() {
                if !view.axes.iter().any(|axis| {
                    matches!(
                        axis,
                        crate::codegen::GroupedObservationAxis::BandedInt { .. }
                    )
                }) {
                    continue;
                }
                let rows = u32::try_from(self.layout.row_counts[view.table]).map_err(|_| {
                    CudaError::InvalidInput("grouped observation row count exceeds u32".to_owned())
                })?;
                if rows == 0 {
                    continue;
                }
                let view_index = u32::try_from(view_index).map_err(|_| {
                    CudaError::InvalidInput("grouped observation view index exceeds u32".to_owned())
                })?;
                let mut config = LaunchConfig::for_num_elems(rows);
                config.grid_dim.0 = config.grid_dim.0.min(1024);
                let mut bound = fused_launch_builder(
                    &self.stream,
                    &self.bound_grouped_view,
                    self.fused_batch.as_ref(),
                );
                bound
                    .arg(&self.state)
                    .arg(&self.column_offsets)
                    .arg(&self.row_counts)
                    .arg(&mut self.grouped_extrema)
                    .arg(&view_index);
                unsafe { bound.launch(config) }.map_err(driver_error)?;
            }
            self.stream
                .memcpy_dtov(&self.grouped_extrema)
                .map_err(driver_error)?
        };
        let extrema_stride = strides[FusedBuffer::GroupedExtrema as usize];
        let mut layouts = Vec::with_capacity(active_width);
        for (slot, slot_error) in slot_errors.iter_mut().enumerate().take(active_width) {
            let begin = slot * extrema_stride;
            let result = grouped_specs
                .iter()
                .map(|view| {
                    grouped_observation_layout(
                        view,
                        &self.layout.row_counts,
                        &band_arena[begin..begin + band_count * 2],
                    )
                })
                .collect::<Result<Vec<_>, CudaError>>();
            match result {
                Ok(slot_layouts) => layouts.push(slot_layouts),
                Err(error) => {
                    *slot_error = Some(error);
                    layouts.push(
                        grouped_specs
                            .iter()
                            .map(|view| GroupedObservationLayout {
                                axes: view
                                    .axes
                                    .iter()
                                    .map(|_| GroupedObservationAxisLayout {
                                        minimum: 0,
                                        cardinality: 0,
                                    })
                                    .collect(),
                                key_space_size: 0,
                            })
                            .collect(),
                    );
                    self.deactivate_fused_slot(slot)?;
                }
            }
        }
        let axis_stride = strides[FusedBuffer::GroupedAxisMins as usize];
        let capacity = self
            .fused_batch
            .as_ref()
            .expect("fused batch exists")
            .capacity;
        let mut axis_mins = vec![0_i64; axis_stride * capacity];
        let mut axis_cardinalities = vec![0_u64; axis_stride * capacity];
        for (slot, slot_layouts) in layouts.iter().enumerate() {
            let begin = slot * axis_stride;
            let mins = slot_layouts
                .iter()
                .flat_map(|layout| layout.axes.iter().map(|axis| axis.minimum));
            let cardinalities = slot_layouts
                .iter()
                .flat_map(|layout| layout.axes.iter().map(|axis| axis.cardinality));
            for (index, value) in mins.enumerate() {
                axis_mins[begin + index] = value;
            }
            for (index, value) in cardinalities.enumerate() {
                axis_cardinalities[begin + index] = value;
            }
        }
        if axis_stride != 0 {
            self.stream
                .memcpy_htod(&axis_mins, &mut self.grouped_axis_mins)
                .map_err(driver_error)?;
            self.stream
                .memcpy_htod(&axis_cardinalities, &mut self.grouped_axis_cardinalities)
                .map_err(driver_error)?;
        }

        let histogram_stride = strides[FusedBuffer::GroupedHistogram as usize];
        let mut slot_grouped = vec![Vec::new(); active_width];
        for (view_index, view) in grouped_specs.iter().enumerate() {
            let max_key_space = layouts
                .iter()
                .map(|slot| slot[view_index].key_space_size)
                .max()
                .unwrap_or(0);
            if max_key_space == 0 {
                continue;
            }
            let key_space = u32::try_from(max_key_space)
                .map_err(|_| CudaError::InvalidInput("grouped key space exceeds u32".to_owned()))?;
            let key_space_u64 = u64::from(key_space);
            let mut init = fused_launch_builder(
                &self.stream,
                &self.init_grouped_histogram,
                self.fused_batch.as_ref(),
            );
            init.arg(&mut self.grouped_histogram).arg(&key_space_u64);
            unsafe { init.launch(LaunchConfig::for_num_elems(key_space)) }.map_err(driver_error)?;
            let rows = u32::try_from(self.layout.row_counts[view.table]).map_err(|_| {
                CudaError::InvalidInput("grouped observation row count exceeds u32".to_owned())
            })?;
            if rows != 0 {
                let view_index_u32 = u32::try_from(view_index).map_err(|_| {
                    CudaError::InvalidInput("grouped observation index exceeds u32".to_owned())
                })?;
                let mut config = LaunchConfig::for_num_elems(rows);
                config.grid_dim.0 = config.grid_dim.0.min(1024);
                let mut observe = fused_launch_builder(
                    &self.stream,
                    &self.observe_grouped_view,
                    self.fused_batch.as_ref(),
                );
                observe
                    .arg(&self.state)
                    .arg(&self.column_offsets)
                    .arg(&self.row_counts)
                    .arg(&self.params)
                    .arg(&self.grouped_axis_mins)
                    .arg(&self.grouped_axis_cardinalities)
                    .arg(&mut self.grouped_histogram)
                    .arg(&view_index_u32);
                unsafe { observe.launch(config) }.map_err(driver_error)?;
            }
            let counters = self
                .stream
                .memcpy_dtov(&self.grouped_histogram)
                .map_err(driver_error)?;
            for slot in 0..active_width {
                if slot_errors[slot].is_some() {
                    continue;
                }
                let layout = &layouts[slot][view_index];
                if layout.key_space_size == 0 {
                    continue;
                }
                let begin = slot * histogram_stride;
                match decode_grouped_histogram(
                    view,
                    layout,
                    &counters[begin..begin + layout.key_space_size],
                ) {
                    Ok(values) => slot_grouped[slot].extend(values),
                    Err(error) => {
                        slot_errors[slot] = Some(error);
                        self.deactivate_fused_slot(slot)?;
                    }
                }
            }
        }

        let generic_count = self.generated.generic_enum_count;
        let mut generic_by_slot = if generic_count == 0 {
            let value = (self.generated.observation_view_tables.is_empty()
                && !self.generated.grouped_observation_views.is_empty())
            .then(Vec::new);
            vec![value; active_width]
        } else {
            let count_u32 = u32::try_from(generic_count).map_err(|_| {
                CudaError::InvalidInput("generic enum observation count exceeds u32".to_owned())
            })?;
            let count_u64 = u64::from(count_u32);
            let mut init = fused_launch_builder(
                &self.stream,
                &self.init_generic_enum_counts,
                self.fused_batch.as_ref(),
            );
            init.arg(&mut self.generic_enum_counts).arg(&count_u64);
            unsafe { init.launch(LaunchConfig::for_num_elems(count_u32)) }.map_err(driver_error)?;
            for (index, observation) in self
                .generated
                .generic_enum_observations
                .clone()
                .iter()
                .enumerate()
            {
                let rows =
                    u32::try_from(self.layout.row_counts[observation.table]).map_err(|_| {
                        CudaError::InvalidInput("generic enum row count exceeds u32".to_owned())
                    })?;
                if rows == 0 {
                    continue;
                }
                let index = u32::try_from(index).map_err(|_| {
                    CudaError::InvalidInput("generic enum index exceeds u32".to_owned())
                })?;
                let mut observe = fused_launch_builder(
                    &self.stream,
                    &self.observe_generic_enum,
                    self.fused_batch.as_ref(),
                );
                observe
                    .arg(&self.state)
                    .arg(&self.column_offsets)
                    .arg(&self.row_counts)
                    .arg(&mut self.generic_enum_counts)
                    .arg(&index);
                unsafe { observe.launch(LaunchConfig::for_num_elems(rows)) }
                    .map_err(driver_error)?;
            }
            let counts = self
                .stream
                .memcpy_dtov(&self.generic_enum_counts)
                .map_err(driver_error)?;
            let stride = strides[FusedBuffer::GenericEnumCounts as usize];
            let mut by_slot = vec![None; active_width];
            for slot in 0..active_width {
                if slot_errors[slot].is_some() {
                    continue;
                }
                let begin = slot * stride;
                match counts[begin..begin + generic_count]
                    .iter()
                    .copied()
                    .map(|count| {
                        usize::try_from(count).map_err(|_| {
                            CudaError::InvalidInput(
                                "generic enum count exceeds host usize".to_owned(),
                            )
                        })
                    })
                    .collect::<Result<Vec<_>, CudaError>>()
                {
                    Ok(counts) => by_slot[slot] = Some(counts),
                    Err(error) => {
                        slot_errors[slot] = Some(error);
                        self.deactivate_fused_slot(slot)?;
                    }
                }
            }
            by_slot
        };

        Ok((0..active_width)
            .map(|slot| match slot_errors[slot].take() {
                Some(error) => Err(error),
                None => Ok(Some(CudaDeviceObservations {
                    views: mem::take(&mut slot_views[slot]),
                    grouped_views: mem::take(&mut slot_grouped[slot]),
                    generic_enum_counts: generic_by_slot[slot].take(),
                })),
            })
            .collect())
    }

    fn observe_device_views(
        &mut self,
        tick: u32,
    ) -> Result<Option<CudaDeviceObservations>, CudaError> {
        if !self.generated.observation_eligibility.eligible {
            return Ok(None);
        }

        let scalar_count =
            u32::try_from(self.generated.observation_view_tables.len()).map_err(|_| {
                CudaError::InvalidInput("observation view count exceeds u32".to_owned())
            })?;
        if scalar_count != 0 {
            let mut init = fused_launch_builder(
                &self.stream,
                &self.init_observations,
                self.fused_batch.as_ref(),
            );
            init.arg(&mut self.observation_values).arg(&scalar_count);
            unsafe { init.launch(LaunchConfig::for_num_elems(scalar_count)) }
                .map_err(driver_error)?;
        }

        for (view_index, table) in self
            .generated
            .observation_view_tables
            .iter()
            .copied()
            .enumerate()
        {
            let rows = u32::try_from(self.layout.row_counts[table]).map_err(|_| {
                CudaError::InvalidInput("observation row count exceeds u32".to_owned())
            })?;
            if rows == 0 {
                continue;
            }
            let view_index = u32::try_from(view_index).map_err(|_| {
                CudaError::InvalidInput("observation view index exceeds u32".to_owned())
            })?;
            let mut config = LaunchConfig::for_num_elems(rows);
            config.grid_dim.0 = config.grid_dim.0.min(1024);
            config.shared_mem_bytes = config.block_dim.0 * 8;
            let mut observe =
                fused_launch_builder(&self.stream, &self.observe_view, self.fused_batch.as_ref());
            observe
                .arg(&self.state)
                .arg(&self.column_offsets)
                .arg(&self.row_counts)
                .arg(&self.params)
                .arg(&mut self.observation_values)
                .arg(&view_index);
            unsafe { observe.launch(config) }.map_err(driver_error)?;
        }
        let scalar_values = if scalar_count == 0 {
            Vec::new()
        } else {
            self.stream
                .memcpy_dtov(&self.observation_values.slice(..scalar_count as usize))
                .map_err(driver_error)?
        };
        let scalar_names = self.model.model().boxes.iter().flat_map(|model_box| {
            model_box
                .views
                .iter()
                .map(move |view| (model_box.name.clone(), view.name.clone()))
        });
        let views = scalar_names
            .zip(scalar_values)
            .map(|((box_name, name), value)| ViewValue {
                box_name,
                name,
                value: ObservationValue::Int(value),
            })
            .collect();

        let grouped_specs = self.generated.grouped_observation_views.clone();
        let band_count = self.generated.grouped_observation_band_axes;
        let band_extrema = if band_count == 0 {
            Vec::new()
        } else {
            let band_count_u32 = u32::try_from(band_count).map_err(|_| {
                CudaError::InvalidInput("grouped band axis count exceeds u32".to_owned())
            })?;
            let mut init = fused_launch_builder(
                &self.stream,
                &self.init_grouped_extrema,
                self.fused_batch.as_ref(),
            );
            init.arg(&mut self.grouped_extrema).arg(&band_count_u32);
            unsafe { init.launch(LaunchConfig::for_num_elems(band_count_u32)) }
                .map_err(driver_error)?;
            for (view_index, view) in grouped_specs.iter().enumerate() {
                if !view.axes.iter().any(|axis| {
                    matches!(
                        axis,
                        crate::codegen::GroupedObservationAxis::BandedInt { .. }
                    )
                }) {
                    continue;
                }
                let rows = u32::try_from(self.layout.row_counts[view.table]).map_err(|_| {
                    CudaError::InvalidInput("grouped observation row count exceeds u32".to_owned())
                })?;
                if rows == 0 {
                    continue;
                }
                let view_index = u32::try_from(view_index).map_err(|_| {
                    CudaError::InvalidInput("grouped observation view index exceeds u32".to_owned())
                })?;
                let mut config = LaunchConfig::for_num_elems(rows);
                config.grid_dim.0 = config.grid_dim.0.min(1024);
                let mut bound = fused_launch_builder(
                    &self.stream,
                    &self.bound_grouped_view,
                    self.fused_batch.as_ref(),
                );
                bound
                    .arg(&self.state)
                    .arg(&self.column_offsets)
                    .arg(&self.row_counts)
                    .arg(&mut self.grouped_extrema)
                    .arg(&view_index);
                unsafe { bound.launch(config) }.map_err(driver_error)?;
            }
            self.stream
                .memcpy_dtov(&self.grouped_extrema.slice(..band_count * 2))
                .map_err(driver_error)?
        };

        let layouts = grouped_specs
            .iter()
            .map(|view| grouped_observation_layout(view, &self.layout.row_counts, &band_extrema))
            .collect::<Result<Vec<GroupedObservationLayout>, CudaError>>()?;
        let axis_mins = layouts
            .iter()
            .flat_map(|layout| layout.axes.iter().map(|axis| axis.minimum))
            .collect::<Vec<_>>();
        let axis_cardinalities = layouts
            .iter()
            .flat_map(|layout| layout.axes.iter().map(|axis| axis.cardinality))
            .collect::<Vec<_>>();
        if !axis_mins.is_empty() {
            self.stream
                .memcpy_htod(&axis_mins, &mut self.grouped_axis_mins)
                .map_err(driver_error)?;
            self.stream
                .memcpy_htod(&axis_cardinalities, &mut self.grouped_axis_cardinalities)
                .map_err(driver_error)?;
        }

        let mut grouped_views = Vec::new();
        for (view_index, (view, layout)) in grouped_specs.iter().zip(&layouts).enumerate() {
            if layout.key_space_size == 0 {
                eprintln!(
                    "cuda_device_grouped_observation tick={tick} box={:?} view={:?} key_space_size=0 occupied_groups=0 emitted_groups=0",
                    view.box_name, view.name
                );
                continue;
            }
            let key_space = u32::try_from(layout.key_space_size)
                .map_err(|_| CudaError::InvalidInput("grouped key space exceeds u32".to_owned()))?;
            let key_space_u64 = u64::from(key_space);
            let mut init = fused_launch_builder(
                &self.stream,
                &self.init_grouped_histogram,
                self.fused_batch.as_ref(),
            );
            init.arg(&mut self.grouped_histogram).arg(&key_space_u64);
            unsafe { init.launch(LaunchConfig::for_num_elems(key_space)) }.map_err(driver_error)?;

            let rows = u32::try_from(self.layout.row_counts[view.table]).map_err(|_| {
                CudaError::InvalidInput("grouped observation row count exceeds u32".to_owned())
            })?;
            if rows != 0 {
                let view_index = u32::try_from(view_index).map_err(|_| {
                    CudaError::InvalidInput("grouped observation view index exceeds u32".to_owned())
                })?;
                let mut config = LaunchConfig::for_num_elems(rows);
                config.grid_dim.0 = config.grid_dim.0.min(1024);
                let mut observe = fused_launch_builder(
                    &self.stream,
                    &self.observe_grouped_view,
                    self.fused_batch.as_ref(),
                );
                observe
                    .arg(&self.state)
                    .arg(&self.column_offsets)
                    .arg(&self.row_counts)
                    .arg(&self.params)
                    .arg(&self.grouped_axis_mins)
                    .arg(&self.grouped_axis_cardinalities)
                    .arg(&mut self.grouped_histogram)
                    .arg(&view_index);
                unsafe { observe.launch(config) }.map_err(driver_error)?;
            }
            let counters = self
                .stream
                .memcpy_dtov(&self.grouped_histogram.slice(..layout.key_space_size))
                .map_err(driver_error)?;
            let occupied = counters.iter().filter(|count| **count != 0).count();
            let emitted = decode_grouped_histogram(view, layout, &counters)?;
            eprintln!(
                "cuda_device_grouped_observation tick={tick} box={:?} view={:?} key_space_size={} occupied_groups={} emitted_groups={}",
                view.box_name,
                view.name,
                layout.key_space_size,
                occupied,
                emitted.len()
            );
            debug_assert_eq!(occupied, emitted.len());
            grouped_views.extend(emitted);
        }

        let generic_enum_count = self.generated.generic_enum_count;
        let generic_enum_counts = if generic_enum_count == 0 {
            (self.generated.observation_view_tables.is_empty()
                && !self.generated.grouped_observation_views.is_empty())
            .then(Vec::new)
        } else {
            let generic_enum_count_u32 = u32::try_from(generic_enum_count).map_err(|_| {
                CudaError::InvalidInput("generic enum observation count exceeds u32".to_owned())
            })?;
            let generic_enum_count_u64 = u64::from(generic_enum_count_u32);
            let mut init = fused_launch_builder(
                &self.stream,
                &self.init_generic_enum_counts,
                self.fused_batch.as_ref(),
            );
            init.arg(&mut self.generic_enum_counts)
                .arg(&generic_enum_count_u64);
            unsafe { init.launch(LaunchConfig::for_num_elems(generic_enum_count_u32)) }
                .map_err(driver_error)?;

            let observations = self.generated.generic_enum_observations.clone();
            for (observation_index, observation) in observations.iter().enumerate() {
                let rows =
                    u32::try_from(self.layout.row_counts[observation.table]).map_err(|_| {
                        CudaError::InvalidInput(
                            "generic enum observation row count exceeds u32".to_owned(),
                        )
                    })?;
                if rows == 0 {
                    continue;
                }
                let observation_index = u32::try_from(observation_index).map_err(|_| {
                    CudaError::InvalidInput("generic enum observation index exceeds u32".to_owned())
                })?;
                let mut config = LaunchConfig::for_num_elems(rows);
                config.grid_dim.0 = config.grid_dim.0.min(1024);
                let mut observe = fused_launch_builder(
                    &self.stream,
                    &self.observe_generic_enum,
                    self.fused_batch.as_ref(),
                );
                observe
                    .arg(&self.state)
                    .arg(&self.column_offsets)
                    .arg(&self.row_counts)
                    .arg(&mut self.generic_enum_counts)
                    .arg(&observation_index);
                unsafe { observe.launch(config) }.map_err(driver_error)?;
            }
            let counts = self
                .stream
                .memcpy_dtov(&self.generic_enum_counts.slice(..generic_enum_count))
                .map_err(driver_error)?;
            Some(
                counts
                    .into_iter()
                    .map(|count| {
                        usize::try_from(count).map_err(|_| {
                            CudaError::InvalidInput(
                                "generic enum count exceeds host usize".to_owned(),
                            )
                        })
                    })
                    .collect::<Result<Vec<_>, CudaError>>()?,
            )
        };

        Ok(Some(CudaDeviceObservations {
            views,
            grouped_views,
            generic_enum_counts,
        }))
    }

    fn ensure_host_state(&mut self) -> Result<(), CudaError> {
        if !self.host_state_current {
            self.download_state_store()?;
        }
        Ok(())
    }

    fn readback_control(&self) -> Result<(Vec<u64>, Vec<u64>), CudaError> {
        let mut fired_counts = self
            .stream
            .memcpy_dtov(&self.fired_counts)
            .map_err(driver_error)?;
        fired_counts.truncate(self.layout.candidate_offsets.len());
        let mut deferred_counts = self
            .stream
            .memcpy_dtov(&self.deferred_counts)
            .map_err(driver_error)?;
        deferred_counts.truncate(self.layout.row_counts.len());
        Ok((fired_counts, deferred_counts))
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
        unsafe { args.launch(LaunchConfig::for_num_elems(count)) }.map_err(driver_error)?;
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

    fn validation_launch_config(&self, rows: u32, one: LaunchConfig) -> LaunchConfig {
        if rows == 0 {
            return one;
        }
        #[cfg(test)]
        if let Some(geometry) = self.validation_launch_override {
            return geometry.config();
        }
        LaunchConfig::for_num_elems(rows)
    }

    fn conflict_launch_config(&self, elements: u32, one: LaunchConfig) -> LaunchConfig {
        if elements == 0 {
            return one;
        }
        #[cfg(test)]
        if let Some(geometry) = self.conflict_launch_override {
            return geometry.config();
        }
        LaunchConfig::for_num_elems(elements)
    }

    fn execute_tick(&mut self) -> Result<(), CudaError> {
        self.execute_tick_batch_statuses()?
            .into_iter()
            .next()
            .unwrap_or(Ok(()))
    }

    fn execute_tick_batch_statuses(&mut self) -> Result<Vec<Result<(), CudaError>>, CudaError> {
        let one = LaunchConfig {
            grid_dim: (1, 1, 1),
            block_dim: (1, 1, 1),
            shared_mem_bytes: 0,
        };
        let aggregate_error_count = (self.layout.aggregate_max_groups + 2) as u64;
        {
            let mut args =
                fused_launch_builder(&self.stream, &self.reset_status, self.fused_batch.as_ref());
            args.arg(&mut self.status)
                .arg(&mut self.aggregate_errors)
                .arg(&aggregate_error_count);
            unsafe { args.launch(one) }.map_err(driver_error)?;
        }
        {
            let rule_count = self.layout.candidate_offsets.len() as u64;
            let mut args = fused_launch_builder(
                &self.stream,
                &self.init_validation_scratch,
                self.fused_batch.as_ref(),
            );
            args.arg(&mut self.status)
                .arg(&mut self.effect_active)
                .arg(&rule_count);
            unsafe { args.launch(one) }.map_err(driver_error)?;
        }
        // Build all tick-start aggregates without committing errors. Each
        // aggregate leaves a device error fact which the ordered validators
        // surface only when the CPU evaluator would first reach that node.
        let require_active = 0_u8;
        for aggregate_slot in self.generated.state_aggregate_indices.clone() {
            let group_table = self.generated.aggregate_group_tables[aggregate_slot];
            let aggregate_index = u32::try_from(aggregate_slot)
                .map_err(|_| CudaError::InvalidInput("aggregate count exceeds u32".to_owned()))?;
            let mut args = fused_launch_builder(
                &self.stream,
                &self.build_aggregate_partials,
                self.fused_batch.as_ref(),
            );
            args.arg(&self.state)
                .arg(&self.column_offsets)
                .arg(&self.row_counts)
                .arg(&self.inputs)
                .arg(&self.input_offsets)
                .arg(&self.input_counts)
                .arg(&self.params)
                .arg(&self.aggregates)
                .arg(&self.aggregate_facts)
                .arg(&aggregate_index)
                .arg(&self.aggregate_active)
                .arg(&require_active)
                .arg(&mut self.aggregate_partials)
                .arg(&self.aggregate_offsets)
                .arg(&mut self.aggregate_errors);
            unsafe { args.launch(one) }.map_err(driver_error)?;
            let groups = u32::try_from(self.layout.row_counts[group_table]).map_err(|_| {
                CudaError::InvalidInput("aggregate group count exceeds u32".to_owned())
            })?;
            if groups != 0 {
                let mut args = fused_launch_builder(
                    &self.stream,
                    &self.finish_aggregates,
                    self.fused_batch.as_ref(),
                );
                args.arg(&self.aggregate_partials)
                    .arg(&self.row_counts)
                    .arg(&aggregate_index)
                    .arg(&self.aggregate_active)
                    .arg(&require_active)
                    .arg(&mut self.aggregates)
                    .arg(&self.aggregate_offsets)
                    .arg(&mut self.aggregate_errors);
                unsafe { args.launch(LaunchConfig::for_num_elems(groups)) }
                    .map_err(driver_error)?;
            }
            let aggregate_identity = u64::from(aggregate_index);
            let mut args = fused_launch_builder(
                &self.stream,
                &self.record_aggregate_errors,
                self.fused_batch.as_ref(),
            );
            args.arg(&mut self.aggregate_errors)
                .arg(&aggregate_error_count)
                .arg(&aggregate_identity)
                .arg(&mut self.aggregate_facts);
            unsafe { args.launch(one) }.map_err(driver_error)?;
        }

        // Mirror stage_box: schedule, resolve, and validate winning effects
        // for one box before any expression in the following box is reached.
        for box_index in 0..self.model.model().boxes.len() {
            let transition_positions = self
                .model
                .transitions()
                .iter()
                .enumerate()
                .filter_map(|(index, transition)| {
                    (transition.box_index == box_index).then_some((index, transition))
                })
                .collect::<Vec<_>>();

            for (index, transition) in &transition_positions {
                let rule_id = transition.rule_id;
                let model_transition = &self.model.model().boxes[transition.box_index].transitions
                    [transition.transition_index];
                let table_index = self.model.model().boxes[transition.box_index]
                    .tables
                    .iter()
                    .position(|table| table.name == model_transition.table)
                    .expect("validated transition table");
                let global_table = global_table(&self.model, transition.box_index, table_index);
                let rows = u32::try_from(self.layout.row_counts[global_table]).map_err(|_| {
                    CudaError::InvalidInput(format!(
                        "rule {} row count exceeds u32 entity IDs",
                        transition.rule_id
                    ))
                })?;
                // Scalar input/aggregate checks inside the kernel still need
                // one worker when the table is empty, so zero rows keeps a
                // single-thread launch instead of a zero-block one.
                let validation_config = self.validation_launch_config(rows, one);
                for phase in 0..VALIDATION_REDUCTION_PASSES {
                    {
                        let mut args = fused_launch_builder(
                            &self.stream,
                            &self.validate_transition,
                            self.fused_batch.as_ref(),
                        );
                        args.arg(&self.state)
                            .arg(&self.column_offsets)
                            .arg(&self.row_counts)
                            .arg(&self.inputs)
                            .arg(&self.input_offsets)
                            .arg(&self.input_counts)
                            .arg(&self.params)
                            .arg(&self.aggregates)
                            .arg(&self.aggregate_facts)
                            .arg(&self.aggregate_offsets)
                            .arg(&self.candidate_offsets)
                            .arg(&rule_id)
                            .arg(&mut self.status);
                        unsafe { args.launch(validation_config) }.map_err(driver_error)?;
                    }
                    finish_validation_reduction_pass(
                        &self.stream,
                        &self.advance_validation_phase,
                        &self.commit_validation_status,
                        &mut self.status,
                        phase,
                        one,
                        self.fused_batch.as_ref(),
                    )?;
                }

                if rows == 0 {
                    continue;
                }
                let dt = self.model.model().dt;
                let tick = self.next_tick;
                let mut args = fused_launch_builder(
                    &self.stream,
                    &self.transition_functions[*index],
                    self.fused_batch.as_ref(),
                );
                args.arg(&self.state)
                    .arg(&self.column_offsets)
                    .arg(&self.row_counts)
                    .arg(&self.inputs)
                    .arg(&self.input_offsets)
                    .arg(&self.input_counts)
                    .arg(&self.params)
                    .arg(&self.aggregates)
                    .arg(&self.aggregate_offsets)
                    .arg(&self.candidate_offsets)
                    .arg(&self.seed)
                    .arg(&tick)
                    .arg(&dt)
                    .arg(&mut self.enabled)
                    .arg(&mut self.times)
                    .arg(&mut self.candidate_errors)
                    .arg(&self.status);
                unsafe { args.launch(LaunchConfig::for_num_elems(rows)) }.map_err(driver_error)?;

                let rule_index = usize::try_from(transition.rule_id).map_err(|_| {
                    CudaError::InvalidInput("rule id exceeds host index width".to_owned())
                })?;
                let candidate_begin = self.layout.candidate_offsets[rule_index];
                let candidate_count = u64::from(rows);
                for phase in 0..VALIDATION_REDUCTION_PASSES {
                    {
                        let mut args = fused_launch_builder(
                            &self.stream,
                            &self.check_errors,
                            self.fused_batch.as_ref(),
                        );
                        args.arg(&self.candidate_errors)
                            .arg(&candidate_begin)
                            .arg(&candidate_count)
                            .arg(&mut self.status);
                        unsafe { args.launch(validation_config) }.map_err(driver_error)?;
                    }
                    finish_validation_reduction_pass(
                        &self.stream,
                        &self.advance_validation_phase,
                        &self.commit_validation_status,
                        &mut self.status,
                        phase,
                        one,
                        self.fused_batch.as_ref(),
                    )?;
                }

                let claims_config = self.validation_launch_config(rows, one);
                for phase in 0..VALIDATION_REDUCTION_PASSES {
                    {
                        let mut args = fused_launch_builder(
                            &self.stream,
                            &self.validate_claims,
                            self.fused_batch.as_ref(),
                        );
                        args.arg(&self.state)
                            .arg(&self.column_offsets)
                            .arg(&self.row_counts)
                            .arg(&self.inputs)
                            .arg(&self.input_offsets)
                            .arg(&self.input_counts)
                            .arg(&self.params)
                            .arg(&self.aggregates)
                            .arg(&self.aggregate_offsets)
                            .arg(&self.candidate_offsets)
                            .arg(&rule_id)
                            .arg(&self.enabled)
                            .arg(&mut self.status);
                        unsafe { args.launch(claims_config) }.map_err(driver_error)?;
                    }
                    finish_validation_reduction_pass(
                        &self.stream,
                        &self.advance_validation_phase,
                        &self.commit_validation_status,
                        &mut self.status,
                        phase,
                        one,
                        self.fused_batch.as_ref(),
                    )?;
                }
            }

            let box_index_u32 = u32::try_from(box_index)
                .map_err(|_| CudaError::InvalidInput("box count exceeds u32".to_owned()))?;
            {
                let mut args = fused_launch_builder(
                    &self.stream,
                    &self.validate_claim_compatibility,
                    self.fused_batch.as_ref(),
                );
                args.arg(&self.state)
                    .arg(&self.column_offsets)
                    .arg(&self.row_counts)
                    .arg(&self.inputs)
                    .arg(&self.input_offsets)
                    .arg(&self.input_counts)
                    .arg(&self.params)
                    .arg(&self.aggregates)
                    .arg(&self.aggregate_offsets)
                    .arg(&self.candidate_offsets)
                    .arg(&self.enabled)
                    .arg(&box_index_u32)
                    .arg(&mut self.status);
                unsafe { args.launch(one) }.map_err(driver_error)?;
            }

            let mut candidate_begin = 0_u64;
            let mut candidate_count = 0_u64;
            let mut claim_instance_begin = 0_u64;
            let mut claim_instance_count = 0_u64;
            if let Some((_, first)) = transition_positions.first() {
                let rule_index = usize::try_from(first.rule_id).map_err(|_| {
                    CudaError::InvalidInput("rule id exceeds host index width".to_owned())
                })?;
                candidate_begin = self.layout.candidate_offsets[rule_index];
                claim_instance_begin = self.layout.claim_instance_offsets[rule_index];
                for (_, transition) in &transition_positions {
                    let model_transition = &self.model.model().boxes[transition.box_index]
                        .transitions[transition.transition_index];
                    let table_index = self.model.model().boxes[transition.box_index]
                        .tables
                        .iter()
                        .position(|table| table.name == model_transition.table)
                        .expect("validated transition table");
                    let global_table = global_table(&self.model, transition.box_index, table_index);
                    let rows = self.layout.row_counts[global_table];
                    candidate_count = candidate_count.checked_add(rows).ok_or_else(|| {
                        CudaError::InvalidInput("box candidate count overflow".to_owned())
                    })?;
                    let claims = u64::try_from(model_transition.contests.len()).map_err(|_| {
                        CudaError::InvalidInput("claim count exceeds u64".to_owned())
                    })?;
                    claim_instance_count = claim_instance_count
                        .checked_add(rows.checked_mul(claims).ok_or_else(|| {
                            CudaError::InvalidInput("box claim-instance count overflow".to_owned())
                        })?)
                        .ok_or_else(|| {
                            CudaError::InvalidInput("box claim-instance count overflow".to_owned())
                        })?;
                }
            }
            if candidate_count != 0 {
                let candidate_launch_count = u32::try_from(candidate_count).map_err(|_| {
                    CudaError::InvalidInput(
                        "box candidate count exceeds CUDA launch capacity".to_owned(),
                    )
                })?;
                let candidate_config = self.conflict_launch_config(candidate_launch_count, one);
                let resource_table_count = self.layout.row_counts.len() as u64;

                if claim_instance_count != 0 {
                    let instance_launch_count =
                        u32::try_from(claim_instance_count).map_err(|_| {
                            CudaError::InvalidInput(
                                "box claim-instance count exceeds CUDA launch capacity".to_owned(),
                            )
                        })?;
                    let resource_count =
                        u64::try_from(self.layout.resource_count).map_err(|_| {
                            CudaError::InvalidInput("resource count exceeds u64".to_owned())
                        })?;
                    let resource_launch_count = u32::try_from(resource_count).map_err(|_| {
                        CudaError::InvalidInput(
                            "resource count exceeds CUDA launch capacity".to_owned(),
                        )
                    })?;
                    let resource_config = self.conflict_launch_config(resource_launch_count, one);
                    let instance_config = self.conflict_launch_config(instance_launch_count, one);

                    let mut args = fused_launch_builder(
                        &self.stream,
                        &self.init_conflict_winners,
                        self.fused_batch.as_ref(),
                    );
                    args.arg(&resource_count)
                        .arg(&mut self.winner_keys)
                        .arg(&mut self.winner_rules)
                        .arg(&mut self.winner_entities)
                        .arg(&mut self.winner_instances);
                    unsafe { args.launch(resource_config) }.map_err(driver_error)?;

                    let mut args = fused_launch_builder(
                        &self.stream,
                        &self.build_claim_instances,
                        self.fused_batch.as_ref(),
                    );
                    args.arg(&self.state)
                        .arg(&self.column_offsets)
                        .arg(&self.row_counts)
                        .arg(&self.inputs)
                        .arg(&self.input_offsets)
                        .arg(&self.input_counts)
                        .arg(&self.params)
                        .arg(&self.aggregates)
                        .arg(&self.aggregate_offsets)
                        .arg(&self.candidate_offsets)
                        .arg(&self.claim_instance_offsets)
                        .arg(&self.resource_offsets)
                        .arg(&candidate_begin)
                        .arg(&candidate_count)
                        .arg(&self.enabled)
                        .arg(&self.times)
                        .arg(&mut self.instance_resources)
                        .arg(&mut self.instance_keys)
                        .arg(&mut self.instance_rules)
                        .arg(&mut self.instance_entities)
                        .arg(&self.status);
                    unsafe { args.launch(candidate_config) }.map_err(driver_error)?;

                    let mut args = fused_launch_builder(
                        &self.stream,
                        &self.reduce_claim_keys,
                        self.fused_batch.as_ref(),
                    );
                    args.arg(&claim_instance_begin)
                        .arg(&claim_instance_count)
                        .arg(&self.instance_resources)
                        .arg(&self.instance_keys)
                        .arg(&mut self.winner_keys);
                    unsafe { args.launch(instance_config) }.map_err(driver_error)?;

                    let mut args = fused_launch_builder(
                        &self.stream,
                        &self.reduce_claim_rules,
                        self.fused_batch.as_ref(),
                    );
                    args.arg(&claim_instance_begin)
                        .arg(&claim_instance_count)
                        .arg(&self.instance_resources)
                        .arg(&self.instance_keys)
                        .arg(&self.instance_rules)
                        .arg(&self.winner_keys)
                        .arg(&mut self.winner_rules);
                    unsafe { args.launch(instance_config) }.map_err(driver_error)?;

                    let mut args = fused_launch_builder(
                        &self.stream,
                        &self.reduce_claim_entities,
                        self.fused_batch.as_ref(),
                    );
                    args.arg(&claim_instance_begin)
                        .arg(&claim_instance_count)
                        .arg(&self.instance_resources)
                        .arg(&self.instance_keys)
                        .arg(&self.instance_rules)
                        .arg(&self.instance_entities)
                        .arg(&self.winner_keys)
                        .arg(&self.winner_rules)
                        .arg(&mut self.winner_entities);
                    unsafe { args.launch(instance_config) }.map_err(driver_error)?;

                    let mut args = fused_launch_builder(
                        &self.stream,
                        &self.reduce_claim_instances,
                        self.fused_batch.as_ref(),
                    );
                    args.arg(&claim_instance_begin)
                        .arg(&claim_instance_count)
                        .arg(&self.instance_resources)
                        .arg(&self.instance_keys)
                        .arg(&self.instance_rules)
                        .arg(&self.instance_entities)
                        .arg(&self.winner_keys)
                        .arg(&self.winner_rules)
                        .arg(&self.winner_entities)
                        .arg(&mut self.winner_instances);
                    unsafe { args.launch(instance_config) }.map_err(driver_error)?;
                }

                let mut args = fused_launch_builder(
                    &self.stream,
                    &self.resolve_conflicts,
                    self.fused_batch.as_ref(),
                );
                args.arg(&self.row_counts)
                    .arg(&self.candidate_offsets)
                    .arg(&self.claim_instance_offsets)
                    .arg(&candidate_begin)
                    .arg(&candidate_count)
                    .arg(&resource_table_count)
                    .arg(&self.enabled)
                    .arg(&self.instance_resources)
                    .arg(&self.winner_rules)
                    .arg(&self.winner_entities)
                    .arg(&mut self.wins)
                    .arg(&mut self.deferred)
                    .arg(&self.status);
                unsafe { args.launch(candidate_config) }.map_err(driver_error)?;
            }

            // Reduce each effect-bearing rule's winners into a stable
            // per-rule activity flag before the parallel effects validator
            // reads it. This preserves the serial any_winner scan without an
            // O(rows) rescan per worker.
            let mut effects_rows = 0_u32;
            for (_, transition) in &transition_positions {
                let model_transition = &self.model.model().boxes[transition.box_index].transitions
                    [transition.transition_index];
                let table_index = self.model.model().boxes[transition.box_index]
                    .tables
                    .iter()
                    .position(|table| table.name == model_transition.table)
                    .expect("validated transition table");
                let global_table = global_table(&self.model, transition.box_index, table_index);
                let rows = u32::try_from(self.layout.row_counts[global_table]).map_err(|_| {
                    CudaError::InvalidInput(format!(
                        "rule {} row count exceeds u32 entity IDs",
                        transition.rule_id
                    ))
                })?;
                effects_rows = effects_rows.max(rows);
                if model_transition.effects.is_empty() || rows == 0 {
                    continue;
                }
                let rule_index = usize::try_from(transition.rule_id).map_err(|_| {
                    CudaError::InvalidInput("rule id exceeds host index width".to_owned())
                })?;
                let candidate_begin = self.layout.candidate_offsets[rule_index];
                let rule_id = transition.rule_id;
                let candidate_count = u64::from(rows);
                let mut args = fused_launch_builder(
                    &self.stream,
                    &self.mark_effect_active,
                    self.fused_batch.as_ref(),
                );
                args.arg(&self.wins)
                    .arg(&candidate_begin)
                    .arg(&candidate_count)
                    .arg(&rule_id)
                    .arg(&mut self.effect_active);
                unsafe { args.launch(LaunchConfig::for_num_elems(rows)) }.map_err(driver_error)?;
            }
            let effects_config = self.validation_launch_config(effects_rows, one);
            for phase in 0..VALIDATION_REDUCTION_PASSES {
                {
                    let mut args = fused_launch_builder(
                        &self.stream,
                        &self.validate_effects,
                        self.fused_batch.as_ref(),
                    );
                    args.arg(&self.state)
                        .arg(&self.column_offsets)
                        .arg(&self.row_counts)
                        .arg(&self.inputs)
                        .arg(&self.input_offsets)
                        .arg(&self.input_counts)
                        .arg(&self.params)
                        .arg(&self.aggregates)
                        .arg(&self.aggregate_facts)
                        .arg(&self.aggregate_offsets)
                        .arg(&self.candidate_offsets)
                        .arg(&self.wins)
                        .arg(&self.effect_active)
                        .arg(&box_index_u32)
                        .arg(&mut self.status);
                    unsafe { args.launch(effects_config) }.map_err(driver_error)?;
                }
                finish_validation_reduction_pass(
                    &self.stream,
                    &self.advance_validation_phase,
                    &self.commit_validation_status,
                    &mut self.status,
                    phase,
                    one,
                    self.fused_batch.as_ref(),
                )?;
            }
        }
        self.stream
            .memcpy_dtod(&self.state, &mut self.next_state)
            .map_err(driver_error)?;
        let owner_count = self.layout.owner_count as u64;
        let owner_launch_count = u32::try_from(self.layout.owner_count).map_err(|_| {
            CudaError::InvalidInput("write-owner count exceeds CUDA launch capacity".to_owned())
        })?;
        if owner_launch_count != 0 {
            let mut args = fused_launch_builder(
                &self.stream,
                &self.init_effect_owners,
                self.fused_batch.as_ref(),
            );
            args.arg(&mut self.owners).arg(&owner_count);
            unsafe { args.launch(LaunchConfig::for_num_elems(owner_launch_count)) }
                .map_err(driver_error)?;
        }
        for transition in self.model.transitions() {
            let model_transition = &self.model.model().boxes[transition.box_index].transitions
                [transition.transition_index];
            if model_transition.effects.is_empty() {
                continue;
            }
            let table_index = self.model.model().boxes[transition.box_index]
                .tables
                .iter()
                .position(|table| table.name == model_transition.table)
                .expect("validated transition table");
            let global_table = global_table(&self.model, transition.box_index, table_index);
            let rows = u32::try_from(self.layout.row_counts[global_table]).map_err(|_| {
                CudaError::InvalidInput(format!(
                    "rule {} row count exceeds u32 entity IDs",
                    transition.rule_id
                ))
            })?;
            if rows == 0 {
                continue;
            }
            let rule_id = transition.rule_id;
            for phase in 0..VALIDATION_REDUCTION_PASSES {
                {
                    let mut args = fused_launch_builder(
                        &self.stream,
                        &self.prepare_effects,
                        self.fused_batch.as_ref(),
                    );
                    args.arg(&self.state)
                        .arg(&self.column_offsets)
                        .arg(&self.row_counts)
                        .arg(&self.inputs)
                        .arg(&self.input_offsets)
                        .arg(&self.input_counts)
                        .arg(&self.params)
                        .arg(&self.aggregates)
                        .arg(&self.aggregate_offsets)
                        .arg(&self.candidate_offsets)
                        .arg(&self.wins)
                        .arg(&self.write_offsets)
                        .arg(&mut self.owners)
                        .arg(&mut self.owner_values)
                        .arg(&rule_id)
                        .arg(&mut self.status);
                    unsafe { args.launch(LaunchConfig::for_num_elems(rows)) }
                        .map_err(driver_error)?;
                }
                finish_validation_reduction_pass(
                    &self.stream,
                    &self.advance_validation_phase,
                    &self.commit_validation_status,
                    &mut self.status,
                    phase,
                    one,
                    self.fused_batch.as_ref(),
                )?;
            }
        }
        if self.layout.owner_count != 0 {
            let launch_count = owner_launch_count;
            let mut args =
                fused_launch_builder(&self.stream, &self.apply_effects, self.fused_batch.as_ref());
            args.arg(&mut self.next_state)
                .arg(&self.column_offsets)
                .arg(&self.row_counts)
                .arg(&self.write_offsets)
                .arg(&self.owners)
                .arg(&self.owner_values)
                .arg(&owner_count)
                .arg(&self.status);
            unsafe { args.launch(LaunchConfig::for_num_elems(launch_count)) }
                .map_err(driver_error)?;
        }
        // Moore outputs observe prospective state, so rebuild only aggregates
        // reachable from wired output expressions against next_state.
        let require_active = 0_u8;
        for &aggregate_slot in &self.generated.output_aggregate_indices {
            let group_table = self.generated.aggregate_group_tables[aggregate_slot];
            let aggregate_index = u32::try_from(aggregate_slot)
                .map_err(|_| CudaError::InvalidInput("aggregate count exceeds u32".to_owned()))?;
            let mut args = fused_launch_builder(
                &self.stream,
                &self.build_aggregate_partials,
                self.fused_batch.as_ref(),
            );
            args.arg(&self.next_state)
                .arg(&self.column_offsets)
                .arg(&self.row_counts)
                .arg(&self.inputs)
                .arg(&self.input_offsets)
                .arg(&self.input_counts)
                .arg(&self.params)
                .arg(&self.aggregates)
                .arg(&self.aggregate_facts)
                .arg(&aggregate_index)
                .arg(&self.aggregate_active)
                .arg(&require_active)
                .arg(&mut self.aggregate_partials)
                .arg(&self.aggregate_offsets)
                .arg(&mut self.aggregate_errors);
            unsafe { args.launch(LaunchConfig::for_num_elems(1)) }.map_err(driver_error)?;
            let groups = u32::try_from(self.layout.row_counts[group_table]).map_err(|_| {
                CudaError::InvalidInput("aggregate group count exceeds u32".to_owned())
            })?;
            if groups != 0 {
                let mut args = fused_launch_builder(
                    &self.stream,
                    &self.finish_aggregates,
                    self.fused_batch.as_ref(),
                );
                args.arg(&self.aggregate_partials)
                    .arg(&self.row_counts)
                    .arg(&aggregate_index)
                    .arg(&self.aggregate_active)
                    .arg(&require_active)
                    .arg(&mut self.aggregates)
                    .arg(&self.aggregate_offsets)
                    .arg(&mut self.aggregate_errors);
                unsafe { args.launch(LaunchConfig::for_num_elems(groups)) }
                    .map_err(driver_error)?;
            }
            let aggregate_identity = u64::from(aggregate_index);
            let mut args = fused_launch_builder(
                &self.stream,
                &self.record_aggregate_errors,
                self.fused_batch.as_ref(),
            );
            args.arg(&mut self.aggregate_errors)
                .arg(&aggregate_error_count)
                .arg(&aggregate_identity)
                .arg(&mut self.aggregate_facts);
            unsafe { args.launch(one) }.map_err(driver_error)?;
        }
        {
            // The validator grid-strides each wired output's source table, so
            // the launch covers the largest wired source table; scalar
            // input/aggregate checks still get one worker when no wired
            // table has rows.
            let mut output_rows = 0_u32;
            for wire in &self.model.model().wires {
                let from_box = self
                    .model
                    .model()
                    .boxes
                    .iter()
                    .position(|entry| entry.name == wire.from.r#box)
                    .ok_or_else(|| CudaError::InvalidInput("wire source box missing".to_owned()))?;
                let output = self.model.model().boxes[from_box]
                    .outputs
                    .iter()
                    .find(|entry| entry.name == wire.from.port)
                    .ok_or_else(|| {
                        CudaError::InvalidInput("wire source output missing".to_owned())
                    })?;
                let sembla_ir::OutputBuilder::PerTable { table, .. } = &output.builder;
                let table_index = self.model.model().boxes[from_box]
                    .tables
                    .iter()
                    .position(|entry| entry.name == *table)
                    .ok_or_else(|| {
                        CudaError::InvalidInput("wire source table missing".to_owned())
                    })?;
                let global = global_table(&self.model, from_box, table_index);
                let rows = u32::try_from(self.layout.row_counts[global]).map_err(|_| {
                    CudaError::InvalidInput(
                        "wired output row count exceeds CUDA launch capacity".to_owned(),
                    )
                })?;
                output_rows = output_rows.max(rows);
            }
            let output_config = self.validation_launch_config(output_rows, one);
            for phase in 0..VALIDATION_REDUCTION_PASSES {
                {
                    let mut args = fused_launch_builder(
                        &self.stream,
                        &self.validate_outputs,
                        self.fused_batch.as_ref(),
                    );
                    args.arg(&self.next_state)
                        .arg(&self.column_offsets)
                        .arg(&self.row_counts)
                        .arg(&self.inputs)
                        .arg(&self.input_offsets)
                        .arg(&self.input_counts)
                        .arg(&self.params)
                        .arg(&self.aggregates)
                        .arg(&self.aggregate_facts)
                        .arg(&self.aggregate_offsets)
                        .arg(&mut self.status);
                    unsafe { args.launch(output_config) }.map_err(driver_error)?;
                }
                finish_validation_reduction_pass(
                    &self.stream,
                    &self.advance_validation_phase,
                    &self.commit_validation_status,
                    &mut self.status,
                    phase,
                    one,
                    self.fused_batch.as_ref(),
                )?;
            }
        }
        {
            let port_count = self.layout.ports.len() as u64;
            let field_count = self.layout.input_offsets.len() as u64;
            let error_count = field_count.saturating_mul(3).max(3);
            let mut args = fused_launch_builder(
                &self.stream,
                &self.prepare_outputs,
                self.fused_batch.as_ref(),
            );
            args.arg(&mut self.next_input_counts)
                .arg(&port_count)
                .arg(&mut self.output_errors)
                .arg(&error_count);
            unsafe { args.launch(one) }.map_err(driver_error)?;
        }
        if !self.layout.input_offsets.is_empty() {
            let field_count = u32::try_from(self.layout.input_offsets.len()).map_err(|_| {
                CudaError::InvalidInput(
                    "output field count exceeds CUDA launch capacity".to_owned(),
                )
            })?;
            let mut args = fused_launch_builder(
                &self.stream,
                &self.build_output_partials,
                self.fused_batch.as_ref(),
            );
            args.arg(&self.next_state)
                .arg(&self.column_offsets)
                .arg(&self.row_counts)
                .arg(&self.inputs)
                .arg(&self.input_offsets)
                .arg(&self.input_counts)
                .arg(&self.params)
                .arg(&self.aggregates)
                .arg(&self.aggregate_offsets)
                .arg(&mut self.output_partials)
                .arg(&mut self.output_errors)
                .arg(&self.status);
            unsafe { args.launch(LaunchConfig::for_num_elems(field_count)) }
                .map_err(driver_error)?;
            let field_count_u64 = u64::from(field_count);
            let mut args = fused_launch_builder(
                &self.stream,
                &self.finish_outputs,
                self.fused_batch.as_ref(),
            );
            args.arg(&self.output_partials)
                .arg(&field_count_u64)
                .arg(&mut self.next_inputs)
                .arg(&self.input_offsets)
                .arg(&mut self.output_errors);
            unsafe { args.launch(LaunchConfig::for_num_elems(field_count)) }
                .map_err(driver_error)?;
            let mut args = fused_launch_builder(
                &self.stream,
                &self.check_output_errors,
                self.fused_batch.as_ref(),
            );
            args.arg(&self.output_errors)
                .arg(&field_count_u64)
                .arg(&mut self.status);
            unsafe { args.launch(one) }.map_err(driver_error)?;
        }
        // These diagnostics are consumed only by the host report. Reduce them
        // while the raw control buffers remain device-resident; the terminal
        // status readback below orders all three kernels before compact D2H.
        let rule_count = u64::try_from(self.layout.candidate_offsets.len())
            .map_err(|_| CudaError::InvalidInput("rule count exceeds u64".to_owned()))?;
        let table_count = u64::try_from(self.layout.row_counts.len())
            .map_err(|_| CudaError::InvalidInput("table count exceeds u64".to_owned()))?;
        let candidate_count = u64::try_from(self.layout.candidate_count)
            .map_err(|_| CudaError::InvalidInput("candidate count exceeds u64".to_owned()))?;
        {
            let mut args = fused_launch_builder(
                &self.stream,
                &self.init_control_counts,
                self.fused_batch.as_ref(),
            );
            args.arg(&mut self.fired_counts)
                .arg(&rule_count)
                .arg(&mut self.deferred_counts)
                .arg(&table_count);
            unsafe { args.launch(control_count_launch_config(rule_count.max(table_count))) }
                .map_err(driver_error)?;
        }
        for rule_index in 0..self.layout.candidate_offsets.len() {
            let begin = self.layout.candidate_offsets[rule_index];
            let end = self
                .layout
                .candidate_offsets
                .get(rule_index + 1)
                .copied()
                .unwrap_or(candidate_count);
            if begin == end {
                continue;
            }
            let rule = u64::try_from(rule_index)
                .map_err(|_| CudaError::InvalidInput("rule index exceeds u64".to_owned()))?;
            let mut args =
                fused_launch_builder(&self.stream, &self.count_fired, self.fused_batch.as_ref());
            args.arg(&self.wins)
                .arg(&self.candidate_offsets)
                .arg(&candidate_count)
                .arg(&rule_count)
                .arg(&rule)
                .arg(&mut self.fired_counts);
            unsafe { args.launch(control_count_launch_config(end - begin)) }
                .map_err(driver_error)?;
        }
        if candidate_count != 0 {
            let config = control_count_launch_config(candidate_count);
            for table in 0..table_count {
                let mut args = fused_launch_builder(
                    &self.stream,
                    &self.count_deferred,
                    self.fused_batch.as_ref(),
                );
                args.arg(&self.deferred)
                    .arg(&candidate_count)
                    .arg(&table_count)
                    .arg(&table)
                    .arg(&mut self.deferred_counts);
                unsafe { args.launch(config) }.map_err(driver_error)?;
            }
        }

        let status = self
            .stream
            .memcpy_dtov(&self.status)
            .map_err(driver_error)?;
        let active_width = self
            .fused_batch
            .as_ref()
            .map_or(1, |batch| batch.active_width);
        let mut results = Vec::with_capacity(active_width);
        let mut deactivate = false;
        for slot in 0..active_width {
            let begin = slot * 12;
            let slot_status = &status[begin..begin + 12];
            if slot_status[0] == 0 {
                results.push(Ok(()));
            } else {
                results.push(Err(device_status(slot_status)));
                if let Some(batch) = self.fused_batch.as_mut() {
                    batch.active_host[slot] = 0;
                    deactivate = true;
                }
            }
        }
        if deactivate {
            let batch = self.fused_batch.as_mut().expect("fused batch exists");
            self.stream
                .memcpy_htod(&batch.active_host, &mut batch.active)
                .map_err(driver_error)?;
        }
        if self.fused_batch.is_none() {
            if let Some(error) = results.iter().find_map(|status| status.as_ref().err()) {
                return Err(error.clone());
            }
        }
        mem::swap(&mut self.state, &mut self.next_state);
        mem::swap(&mut self.inputs, &mut self.next_inputs);
        mem::swap(&mut self.input_counts, &mut self.next_input_counts);
        self.next_tick = self
            .next_tick
            .checked_add(1)
            .ok_or_else(|| CudaError::DeviceExecution("tick coordinate overflow".to_owned()))?;
        self.host_state_current = false;
        Ok(results)
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

fn downloaded_state_bytes(
    state: &[u8],
    inputs: &[u8],
    input_counts: &[u64],
) -> Result<CudaFinalStateDownloadedBytes, CudaError> {
    let input_count_bytes = input_counts
        .len()
        .checked_mul(mem::size_of::<u64>())
        .ok_or_else(|| CudaError::InvalidInput("input-count byte total overflow".to_owned()))?;
    let total = state
        .len()
        .checked_add(inputs.len())
        .and_then(|bytes| bytes.checked_add(input_count_bytes))
        .ok_or_else(|| CudaError::InvalidInput("final-state byte total overflow".to_owned()))?;
    Ok(CudaFinalStateDownloadedBytes {
        state: state.len(),
        inputs: inputs.len(),
        input_counts: input_count_bytes,
        total,
    })
}

fn format_cuda_driver_version(version: i32) -> String {
    format!("{}.{}", version / 1000, (version % 1000) / 10)
}

fn unpack_state_into(
    model: &ValidatedModel,
    layout: &Layout,
    bytes: &[u8],
    tables: &mut [TableInit],
) -> Result<(), CudaError> {
    let mut global_table = 0;
    let mut column = 0;
    for model_box in &model.model().boxes {
        for table in &model_box.tables {
            let rows = layout.row_counts[global_table] as usize;
            let destination = tables.get_mut(global_table).ok_or_else(|| {
                CudaError::DeviceExecution(
                    "backend state staging table count changed after construction".to_owned(),
                )
            })?;
            if destination.box_name != model_box.name
                || destination.table_name != table.name
                || destination.row_count != rows
                || destination.columns.len() != table.attrs.len()
            {
                return Err(CudaError::DeviceExecution(format!(
                    "backend state staging shape changed for box '{}', table '{}'",
                    model_box.name, table.name
                )));
            }
            for (attr, destination) in table.attrs.iter().zip(&mut destination.columns) {
                if destination.name != attr.name {
                    return Err(CudaError::DeviceExecution(format!(
                        "backend state staging shape changed for box '{}', table '{}', column '{}'",
                        model_box.name, table.name, attr.name
                    )));
                }
                read_column_into(
                    bytes,
                    layout.column_offsets[column] as usize,
                    rows,
                    &attr.ty,
                    &mut destination.data,
                )?;
                column += 1;
            }
            global_table += 1;
        }
    }
    if global_table != tables.len() {
        return Err(CudaError::DeviceExecution(
            "backend state staging table count changed after construction".to_owned(),
        ));
    }
    Ok(())
}

fn unpack_inputs(
    model: &ValidatedModel,
    layout: &Layout,
    bytes: &[u8],
    counts: &[u64],
) -> Vec<InputTable> {
    let mut tables = Vec::new();
    let mut field = 0;
    for (port_flat, (box_index, port_index)) in layout.ports.iter().copied().enumerate() {
        let model_box = &model.model().boxes[box_index];
        let port = &model_box.inputs[port_index];
        let rows = counts[port_flat] as usize;
        let columns = port
            .schema
            .iter()
            .map(|attr| {
                let data = read_column(bytes, layout.input_offsets[field] as usize, rows, &attr.ty);
                field += 1;
                data
            })
            .collect();
        tables.push(InputTable {
            box_name: model_box.name.clone(),
            port_name: port.name.clone(),
            schema: port.schema.clone(),
            row_count: rows,
            columns,
        });
    }
    tables
}

fn read_column_into(
    bytes: &[u8],
    offset: usize,
    rows: usize,
    ty: &AttrType,
    destination: &mut ColumnData,
) -> Result<(), CudaError> {
    match (ty, destination) {
        (AttrType::Real, ColumnData::Real(values)) => {
            values.clear();
            values.extend((0..rows).map(|row| {
                f64::from_bits(u64::from_le_bytes(
                    bytes[offset + row * 8..offset + row * 8 + 8]
                        .try_into()
                        .unwrap(),
                ))
            }));
        }
        (AttrType::Int, ColumnData::Int(values)) => {
            values.clear();
            values.extend((0..rows).map(|row| {
                i64::from_le_bytes(
                    bytes[offset + row * 8..offset + row * 8 + 8]
                        .try_into()
                        .unwrap(),
                )
            }));
        }
        (AttrType::Enum { .. }, ColumnData::Enum(values)) => {
            values.clear();
            values.extend((0..rows).map(|row| {
                u16::from_le_bytes(
                    bytes[offset + row * 2..offset + row * 2 + 2]
                        .try_into()
                        .unwrap(),
                )
            }));
        }
        (AttrType::Ref { .. }, ColumnData::Ref(values)) => {
            values.clear();
            values.extend((0..rows).map(|row| {
                u32::from_le_bytes(
                    bytes[offset + row * 4..offset + row * 4 + 4]
                        .try_into()
                        .unwrap(),
                )
            }));
        }
        _ => {
            return Err(CudaError::DeviceExecution(
                "backend state staging column type changed after construction".to_owned(),
            ));
        }
    }
    Ok(())
}

fn read_column(bytes: &[u8], offset: usize, rows: usize, ty: &AttrType) -> ColumnData {
    match ty {
        AttrType::Real => ColumnData::Real(
            (0..rows)
                .map(|row| {
                    f64::from_bits(u64::from_le_bytes(
                        bytes[offset + row * 8..offset + row * 8 + 8]
                            .try_into()
                            .unwrap(),
                    ))
                })
                .collect(),
        ),
        AttrType::Int => ColumnData::Int(
            (0..rows)
                .map(|row| {
                    i64::from_le_bytes(
                        bytes[offset + row * 8..offset + row * 8 + 8]
                            .try_into()
                            .unwrap(),
                    )
                })
                .collect(),
        ),
        AttrType::Enum { .. } => ColumnData::Enum(
            (0..rows)
                .map(|row| {
                    u16::from_le_bytes(
                        bytes[offset + row * 2..offset + row * 2 + 2]
                            .try_into()
                            .unwrap(),
                    )
                })
                .collect(),
        ),
        AttrType::Ref { .. } => ColumnData::Ref(
            (0..rows)
                .map(|row| {
                    u32::from_le_bytes(
                        bytes[offset + row * 4..offset + row * 4 + 4]
                            .try_into()
                            .unwrap(),
                    )
                })
                .collect(),
        ),
    }
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

fn build_layout(
    model: &ValidatedModel,
    initial_tables: &[TableInit],
    generated: &GeneratedCuda,
) -> Result<Layout, CudaError> {
    let mut row_counts = Vec::new();
    let mut column_offsets = Vec::new();
    let mut state_len = 0;
    let mut ports = Vec::new();
    let mut input_offsets = Vec::new();
    let mut input_len = 0;
    let mut write_offsets = Vec::new();
    let mut owner_count = 0_usize;

    for (box_index, model_box) in model.model().boxes.iter().enumerate() {
        for (table_index, table) in model_box.tables.iter().enumerate() {
            let initial = find_table(initial_tables, &model_box.name, &table.name)?;
            row_counts.push(initial.row_count as u64);
            for attr_index in 0..table.attrs.len() {
                state_len = align8(state_len);
                column_offsets.push(state_len as u64);
                state_len = state_len
                    .checked_add(
                        initial
                            .row_count
                            .checked_mul(type_size(&table.attrs[attr_index].ty))
                            .ok_or_else(|| {
                                CudaError::InvalidInput("state byte size overflow".to_owned())
                            })?,
                    )
                    .ok_or_else(|| {
                        CudaError::InvalidInput("state byte size overflow".to_owned())
                    })?;
                write_offsets.push(owner_count as u64);
                owner_count = owner_count.checked_add(initial.row_count).ok_or_else(|| {
                    CudaError::InvalidInput("write-owner size overflow".to_owned())
                })?;
                let _ = (box_index, table_index, attr_index);
            }
        }
        for (port_index, port) in model_box.inputs.iter().enumerate() {
            ports.push((box_index, port_index));
            for field_index in 0..port.schema.len() {
                input_len = align8(input_len);
                input_offsets.push(input_len as u64);
                // v0.1 outputs are one-row aggregate tables.
                input_len = input_len
                    .checked_add(type_size(&port.schema[field_index].ty))
                    .ok_or_else(|| {
                        CudaError::InvalidInput("input byte size overflow".to_owned())
                    })?;
            }
        }
    }
    let state_logical_len = state_len;
    let input_logical_len = input_len;
    state_len = state_len.max(1);
    input_len = input_len.max(1);

    // A resource segment is addressed directly by (global table, row). This
    // prefix layout is deterministic and gives the flat claim-instance list a
    // stable grouping without a scheduling-dependent sort.
    let mut resource_offsets = Vec::with_capacity(row_counts.len());
    let mut resource_count = 0_usize;
    for rows in &row_counts {
        resource_offsets.push(resource_count as u64);
        let rows = usize::try_from(*rows)
            .map_err(|_| CudaError::InvalidInput("resource row count exceeds usize".to_owned()))?;
        resource_count = resource_count
            .checked_add(rows)
            .ok_or_else(|| CudaError::InvalidInput("resource size overflow".to_owned()))?;
    }

    let mut candidate_offsets = Vec::new();
    let mut candidate_count = 0_usize;
    let mut claim_instance_offsets = Vec::new();
    let mut claim_instance_count = 0_usize;
    for transition in model.transitions() {
        candidate_offsets.push(candidate_count as u64);
        claim_instance_offsets.push(claim_instance_count as u64);
        let declaration =
            &model.model().boxes[transition.box_index].transitions[transition.transition_index];
        let table_index = model.model().boxes[transition.box_index]
            .tables
            .iter()
            .position(|table| table.name == declaration.table)
            .expect("validated transition table");
        let global = global_table(model, transition.box_index, table_index);
        let rows = usize::try_from(row_counts[global])
            .map_err(|_| CudaError::InvalidInput("candidate row count exceeds usize".to_owned()))?;
        candidate_count = candidate_count
            .checked_add(rows)
            .ok_or_else(|| CudaError::InvalidInput("candidate size overflow".to_owned()))?;
        claim_instance_count = claim_instance_count
            .checked_add(
                rows.checked_mul(declaration.contests.len())
                    .ok_or_else(|| {
                        CudaError::InvalidInput("claim-instance size overflow".to_owned())
                    })?,
            )
            .ok_or_else(|| CudaError::InvalidInput("claim-instance size overflow".to_owned()))?;
    }

    let mut aggregate_offsets = Vec::new();
    let mut aggregate_len = 0_usize;
    let mut aggregate_max_groups = 0_usize;
    for table in &generated.aggregate_group_tables {
        aggregate_max_groups = aggregate_max_groups.max(row_counts[*table] as usize);
        aggregate_len = align8(aggregate_len);
        aggregate_offsets.push(aggregate_len as u64);
        aggregate_len = aggregate_len
            .checked_add(
                (row_counts[*table] as usize)
                    .checked_mul(8)
                    .ok_or_else(|| CudaError::InvalidInput("aggregate size overflow".to_owned()))?,
            )
            .ok_or_else(|| CudaError::InvalidInput("aggregate size overflow".to_owned()))?;
    }

    Ok(Layout {
        row_counts,
        resource_offsets,
        resource_count,
        column_offsets,
        state_len,
        state_logical_len,
        ports,
        input_offsets,
        input_len,
        input_logical_len,
        candidate_offsets,
        candidate_count,
        claim_instance_offsets,
        claim_instance_count,
        aggregate_offsets,
        aggregate_len: aggregate_len.max(1),
        aggregate_max_groups,
        write_offsets,
        owner_count,
    })
}

fn pack_initial_state(
    model: &ValidatedModel,
    initial_tables: &[TableInit],
    layout: &Layout,
) -> Result<Vec<u8>, CudaError> {
    let mut bytes = vec![0_u8; layout.state_len];
    let mut column = 0;
    for model_box in &model.model().boxes {
        for table in &model_box.tables {
            let initial = find_table(initial_tables, &model_box.name, &table.name)?;
            for attr in &table.attrs {
                let data = initial
                    .columns
                    .iter()
                    .find(|entry| entry.name == attr.name)
                    .ok_or_else(|| {
                        CudaError::InvalidInput(format!(
                            "{}.{}.{} has no initializer",
                            model_box.name, table.name, attr.name
                        ))
                    })?;
                write_column(
                    &mut bytes,
                    layout.column_offsets[column] as usize,
                    &data.data,
                );
                column += 1;
            }
        }
    }
    Ok(bytes)
}

fn pack_params(model: &ValidatedModel, params: &ParamEnv) -> Result<Vec<u8>, CudaError> {
    let values = params.values().collect::<Vec<_>>();
    if values.len() != model.model().params.len() {
        return Err(CudaError::InvalidInput(
            "parameter environment does not match model declarations".to_owned(),
        ));
    }
    let mut bytes = vec![0_u8; values.len().max(1) * 8];
    for (index, (name, value)) in values.into_iter().enumerate() {
        if name != model.model().params[index].name {
            return Err(CudaError::InvalidInput(format!(
                "parameter environment entry {index} is '{name}', expected '{}'",
                model.model().params[index].name
            )));
        }
        let encoded = match value {
            ParamValue::Real { value } => value.to_bits().to_le_bytes(),
            ParamValue::Int { value } => value.to_le_bytes(),
        };
        bytes[index * 8..index * 8 + 8].copy_from_slice(&encoded);
    }
    Ok(bytes)
}

fn write_column(bytes: &mut [u8], offset: usize, data: &ColumnData) {
    match data {
        ColumnData::Real(values) => {
            for (row, value) in values.iter().enumerate() {
                let start = offset + row * 8;
                bytes[start..start + 8].copy_from_slice(&value.to_bits().to_le_bytes());
            }
        }
        ColumnData::Int(values) => {
            for (row, value) in values.iter().enumerate() {
                let start = offset + row * 8;
                bytes[start..start + 8].copy_from_slice(&value.to_le_bytes());
            }
        }
        ColumnData::Enum(values) => {
            for (row, value) in values.iter().enumerate() {
                let start = offset + row * 2;
                bytes[start..start + 2].copy_from_slice(&value.to_le_bytes());
            }
        }
        ColumnData::Ref(values) => {
            for (row, value) in values.iter().enumerate() {
                let start = offset + row * 4;
                bytes[start..start + 4].copy_from_slice(&value.to_le_bytes());
            }
        }
    }
}

fn hash_state(
    model: &ValidatedModel,
    layout: &Layout,
    state: &[u8],
    inputs: &[u8],
    input_counts: &[u64],
) -> [u8; 32] {
    let mut hash = Sha256::new();
    if layout.ports.is_empty() {
        hash.update(b"SEMBLA_STATE_V1\0");
    } else {
        hash.update(b"SEMBLA_STATE_V2\0");
    }
    update_u64(&mut hash, layout.row_counts.len());
    let mut global_table_index = 0;
    let mut column_index = 0;
    for model_box in &model.model().boxes {
        for table in &model_box.tables {
            update_string(&mut hash, &model_box.name);
            update_string(&mut hash, &table.name);
            let rows = layout.row_counts[global_table_index] as usize;
            update_u64(&mut hash, rows);
            update_u64(&mut hash, table.attrs.len());
            for attr in &table.attrs {
                update_string(&mut hash, &attr.name);
                update_packed_column(
                    &mut hash,
                    &attr.ty,
                    state,
                    layout.column_offsets[column_index] as usize,
                    rows,
                );
                column_index += 1;
            }
            global_table_index += 1;
        }
    }
    if !layout.ports.is_empty() {
        update_u64(&mut hash, layout.ports.len());
        let mut field = 0;
        for (port_flat, (box_index, port_index)) in layout.ports.iter().copied().enumerate() {
            let model_box = &model.model().boxes[box_index];
            let port = &model_box.inputs[port_index];
            let rows = input_counts[port_flat] as usize;
            update_string(&mut hash, &model_box.name);
            update_string(&mut hash, &port.name);
            update_u64(&mut hash, rows);
            update_u64(&mut hash, port.schema.len());
            for attr in &port.schema {
                update_string(&mut hash, &attr.name);
                update_packed_column(
                    &mut hash,
                    &attr.ty,
                    inputs,
                    layout.input_offsets[field] as usize,
                    rows,
                );
                field += 1;
            }
        }
    }
    hash.finalize().into()
}

fn update_packed_column(
    hash: &mut Sha256,
    ty: &AttrType,
    bytes: &[u8],
    offset: usize,
    rows: usize,
) {
    let (tag, width) = match ty {
        AttrType::Real => (0_u8, 8),
        AttrType::Int => (1, 8),
        AttrType::Enum { .. } => (2, 2),
        AttrType::Ref { .. } => (3, 4),
    };
    hash.update([tag]);
    update_u64(hash, rows);
    hash.update(&bytes[offset..offset + rows * width]);
}

fn update_u64(hash: &mut Sha256, value: usize) {
    hash.update((value as u64).to_le_bytes());
}

fn update_string(hash: &mut Sha256, value: &str) {
    update_u64(hash, value.len());
    hash.update(value.as_bytes());
}

fn find_table<'a>(
    initial_tables: &'a [TableInit],
    box_name: &str,
    table_name: &str,
) -> Result<&'a TableInit, CudaError> {
    initial_tables
        .iter()
        .find(|table| table.box_name == box_name && table.table_name == table_name)
        .ok_or_else(|| {
            CudaError::InvalidInput(format!(
                "box '{box_name}', table '{table_name}': missing initial data"
            ))
        })
}

fn global_table(model: &ValidatedModel, box_index: usize, table_index: usize) -> usize {
    model.model().boxes[..box_index]
        .iter()
        .map(|model_box| model_box.tables.len())
        .sum::<usize>()
        + table_index
}

fn type_size(ty: &AttrType) -> usize {
    match ty {
        AttrType::Real | AttrType::Int => 8,
        AttrType::Enum { .. } => 2,
        AttrType::Ref { .. } => 4,
    }
}

fn align8(value: usize) -> usize {
    (value + 7) & !7
}

#[cfg(test)]
mod control_reports_tests {
    use super::control_reports_from_counts;

    fn model(source: &str) -> sembla_ir::ValidatedModel {
        sembla_ir::validate(sembla_ir::parse_json(source).unwrap()).unwrap()
    }

    #[test]
    fn synthetic_counts_preserve_report_shape_order_and_qualification() {
        let model = model(
            r#"{"name":"control_reports","dt":1.0,"params":[],"boxes":[{"name":"left","tables":[{"name":"zero","size_hint":0,"attrs":[]},{"name":"kept","size_hint":0,"attrs":[]}],"transitions":[{"name":"first","table":"zero","guard":{"kind":"bool","value":true},"hazard":{"kind":"real","value":0.0},"effects":[],"contests":[]},{"name":"second","table":"zero","guard":{"kind":"bool","value":true},"hazard":{"kind":"real","value":0.0},"effects":[],"contests":[]}],"inputs":[],"outputs":[],"views":[]},{"name":"right","tables":[{"name":"also_kept","size_hint":0,"attrs":[]}],"transitions":[{"name":"third","table":"also_kept","guard":{"kind":"bool","value":true},"hazard":{"kind":"real","value":0.0},"effects":[],"contests":[]}],"inputs":[],"outputs":[],"views":[]}],"wires":[],"summaries":[]}"#,
        );
        let (fired, deferred) =
            control_reports_from_counts(&model, &[0, 7, 2], &[0, 5, 3]).unwrap();
        assert_eq!(
            fired,
            vec![
                ("left".to_owned(), vec![(0, 0), (1, 7)]),
                ("right".to_owned(), vec![(2, 2)]),
            ]
        );
        assert_eq!(
            deferred,
            vec![
                ("left.kept".to_owned(), 5),
                ("right.also_kept".to_owned(), 3)
            ]
        );
    }

    #[test]
    fn synthetic_counts_preserve_single_box_names_and_empty_domains() {
        let single = model(
            r#"{"name":"single","dt":1.0,"params":[],"boxes":[{"name":"only","tables":[{"name":"zero","size_hint":0,"attrs":[]},{"name":"kept","size_hint":0,"attrs":[]}],"transitions":[],"inputs":[],"outputs":[],"views":[]}],"wires":[],"summaries":[]}"#,
        );
        let (fired, deferred) = control_reports_from_counts(&single, &[], &[0, 4]).unwrap();
        assert_eq!(fired, vec![("only".to_owned(), Vec::new())]);
        assert_eq!(deferred, vec![("kept".to_owned(), 4)]);

        let empty = model(
            r#"{"name":"empty","dt":1.0,"params":[],"boxes":[{"name":"empty","tables":[],"transitions":[],"inputs":[],"outputs":[],"views":[]}],"wires":[],"summaries":[]}"#,
        );
        let (fired, deferred) = control_reports_from_counts(&empty, &[], &[]).unwrap();
        assert_eq!(fired, vec![("empty".to_owned(), Vec::new())]);
        assert!(deferred.is_empty());
    }
}

#[cfg(test)]
mod probe_tests {
    use super::classify_device_count;
    use crate::CudaError;
    use cudarc::driver::{sys::CUresult, DriverError};

    #[test]
    fn production_device_probe_maps_cuda_no_device() {
        assert_eq!(
            classify_device_count(Err(DriverError(CUresult::CUDA_ERROR_NO_DEVICE))),
            Err(CudaError::NoDevice)
        );
    }
}

#[cfg(test)]
mod diagnostic_equality_hardware {
    use super::{CudaBackend, HashMode, ValidationLaunchGeometry};
    use sembla_runtime::eval::ParamEnv;

    mod cases {
        include!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tests/support/diagnostic_cases.rs"
        ));
    }

    const CHILD_ENV: &str = "SEMBLA_CUDA_DIAGNOSTIC_CHILD";
    const DEADLINE: std::time::Duration = std::time::Duration::from_secs(120);

    fn run_negative_corpus() {
        assert_eq!(cases::FAILING_ROWS, [2, 5, 7]);
        for case in cases::CASES {
            assert!(!case.expected_cpu_error.is_empty(), "{}", case.name);
            let model = cases::load_model(&case);
            let params = ParamEnv::defaults(&model);
            let mut first_status = None;

            for (grid, block) in cases::GEOMETRIES {
                let mut backend = CudaBackend::new(
                    &model,
                    cases::initial_state(&case),
                    &params,
                    7,
                    HashMode::FinalOnly,
                )
                .expect("CUDA device, driver, and NVRTC are required");
                backend.validation_launch_override = Some(ValidationLaunchGeometry { grid, block });

                let error = backend.run(1).unwrap_err();
                assert!(
                    error.to_string().contains(case.expected_cuda_error),
                    "{} ({grid}x{block}): {error}",
                    case.name
                );
                let words = backend
                    .stream
                    .memcpy_dtov(&backend.status)
                    .expect("download validation status");
                let committed = [words[0], words[1], words[2], words[3]];
                assert_eq!(
                    (committed[0], committed[1]),
                    case.expected_status,
                    "{} ({grid}x{block})",
                    case.name
                );
                assert_eq!(
                    committed,
                    *first_status.get_or_insert(committed),
                    "{} changed diagnostic under geometry {grid}x{block}",
                    case.name
                );
                eprintln!(
                    "diagnostic_case={} geometry={}x{} status={:?}",
                    case.name, grid, block, committed
                );
            }
        }
    }

    #[test]
    #[ignore = "requires a CUDA GPU; run crates/sembla-cuda/scripts/run-differential-corpus.sh"]
    fn negative_corpus_matches_cpu_status_under_four_geometries() {
        if std::env::var_os(CHILD_ENV).is_some() {
            run_negative_corpus();
            return;
        }

        // Run the GPU body in a child process so a non-terminating CUDA kernel
        // becomes a bounded test failure. A thread-level timeout cannot recover
        // a process whose CUDA context is blocked in stream synchronization.
        let executable = std::env::current_exe().expect("locate current lib test binary");
        let test_name =
            "diagnostic_equality_hardware::negative_corpus_matches_cpu_status_under_four_geometries";
        let mut child = std::process::Command::new(executable)
            .arg("--exact")
            .arg(test_name)
            .arg("--ignored")
            .arg("--nocapture")
            .env(CHILD_ENV, "1")
            .stdout(std::process::Stdio::inherit())
            .stderr(std::process::Stdio::inherit())
            .spawn()
            .expect("spawn bounded CUDA diagnostic child");
        let deadline = std::time::Instant::now() + DEADLINE;
        loop {
            if let Some(status) = child.try_wait().expect("poll CUDA diagnostic child") {
                assert!(
                    status.success(),
                    "CUDA diagnostic child failed with {status}"
                );
                return;
            }
            if std::time::Instant::now() >= deadline {
                child.kill().expect("kill timed-out CUDA diagnostic child");
                let _ = child.wait();
                panic!(
                    "CUDA diagnostic corpus exceeded its {}s internal deadline; probable kernel deadlock",
                    DEADLINE.as_secs()
                );
            }
            std::thread::sleep(std::time::Duration::from_millis(100));
        }
    }
}

#[cfg(test)]
mod sweep_capacity_tests {
    use super::{
        allocate_cacheable_staging, build_layout, checked_final_state_component_bytes,
        estimate_isolated_sweep_capacity, final_state_component_bytes, generate, hash_state,
        pack_initial_state, write_column, CudaFinalStateReadbackMode,
        FinalStateAllocationInjection, SWEEP_CAPACITY_MIB,
    };
    use sembla_runtime::state::{ColumnData, ColumnInit, InputTable, StateStore, TableInit};

    fn demographic_shape(scale: usize) -> (sembla_ir::ValidatedModel, Vec<TableInit>) {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../fixtures/demographic/benchmark/demographic_slots.full.json");
        let source = std::fs::read_to_string(path).unwrap();
        let features =
            sembla_ir::FeatureSet::from([sembla_ir::GROUPED_OBSERVATIONS_FEATURE.to_owned()]);
        let model =
            sembla_ir::validate_with_features(sembla_ir::parse_json(&source).unwrap(), &features)
                .unwrap();
        let composed = model.model().boxes.len() > 1 || !model.model().wires.is_empty();
        // The estimator consumes only names and row counts. Avoid allocating a
        // 10M-row test state; constructor validation remains unchanged and the
        // production caller passes the already validated real tables.
        let tables = model
            .model()
            .boxes
            .iter()
            .flat_map(|model_box| {
                model_box.tables.iter().map(|table| {
                    let rows = if composed && table.size_hint != 0 {
                        usize::try_from(table.size_hint).unwrap()
                    } else {
                        scale
                    };
                    TableInit::new(&model_box.name, &table.name, rows, Vec::new())
                })
            })
            .collect();
        (model, tables)
    }

    #[test]
    fn isolated_lane_estimate_is_conservative_against_measured_h100_arms() {
        for (scale, observed_mib) in [
            (1_000_000, [1_033_usize, 1_613, 2_733]),
            (10_000_000, [6_025_usize, 11_597, 22_701]),
        ] {
            let (model, tables) = demographic_shape(scale);
            let generated = generate(&model).unwrap();
            let layout = build_layout(&model, &tables, &generated).unwrap();
            let parameter_bytes = model.model().params.len().max(1) * 8;
            for (workers, observed) in [1_usize, 2, 4].into_iter().zip(observed_mib) {
                let estimate = estimate_isolated_sweep_capacity(
                    &layout,
                    &generated,
                    parameter_bytes,
                    workers,
                    CudaFinalStateReadbackMode::Materialized,
                )
                .unwrap();
                assert!(
                    estimate.device_bytes >= observed * SWEEP_CAPACITY_MIB,
                    "{scale} rows/{workers} lanes: estimate {} MiB under measured {observed} MiB",
                    estimate.device_bytes / SWEEP_CAPACITY_MIB
                );
            }
        }
    }

    #[test]
    fn capacity_estimator_rejects_zero_workers() {
        let (model, tables) = demographic_shape(1);
        let generated = generate(&model).unwrap();
        let layout = build_layout(&model, &tables, &generated).unwrap();
        assert!(estimate_isolated_sweep_capacity(
            &layout,
            &generated,
            8,
            0,
            CudaFinalStateReadbackMode::PackedPinned,
        )
        .unwrap_err()
        .to_string()
        .contains("greater than zero"));
    }

    #[test]
    fn packed_pinned_accounting_is_exact_per_lane_and_zero_aware() {
        let empty_source = r#"{"name":"empty","dt":1.0,"params":[],"boxes":[{"name":"world","tables":[],"transitions":[],"inputs":[],"outputs":[],"views":[]}],"wires":[],"summaries":[]}"#;
        let empty = sembla_ir::validate(sembla_ir::parse_json(empty_source).unwrap()).unwrap();
        let generated = generate(&empty).unwrap();
        let layout = build_layout(&empty, &[], &generated).unwrap();
        let zero = checked_final_state_component_bytes(0, 0, 0).unwrap();
        assert_eq!(zero.total, 0);
        assert_eq!(zero.state, 0);
        assert_eq!(zero.inputs, 0);
        assert_eq!(zero.input_counts, 0);
        assert!(checked_final_state_component_bytes(usize::MAX, 1, 0)
            .unwrap_err()
            .to_string()
            .contains("final-state byte total overflow"));
        assert!(checked_final_state_component_bytes(0, 0, usize::MAX)
            .unwrap_err()
            .to_string()
            .contains("input-count byte total overflow"));
        let empty_bytes = final_state_component_bytes(&layout).unwrap();
        assert_eq!(empty_bytes, zero);
        let empty_materialized = StateStore::new(&empty, Vec::new()).unwrap();
        assert_eq!(
            hash_state(&empty, &layout, &[], &[], &[]),
            empty_materialized.state_hash()
        );
        let estimate = estimate_isolated_sweep_capacity(
            &layout,
            &generated,
            8,
            4,
            CudaFinalStateReadbackMode::PackedPinned,
        )
        .unwrap();
        assert_eq!(estimate.requested_buffer_set_count, 4);
        let empty_allocations_per_lane = usize::from(empty_bytes.state != 0);
        assert_eq!(
            estimate.requested_underlying_pinned_allocation_count,
            empty_allocations_per_lane * 4
        );
        assert_eq!(estimate.requested_pinned_bytes, empty_bytes.total * 4);
        assert_eq!(
            estimate.requested_cacheable_staging_bytes,
            empty_bytes.total * 4
        );

        let (model, tables) = demographic_shape(3);
        let generated = generate(&model).unwrap();
        let layout = build_layout(&model, &tables, &generated).unwrap();
        let bytes = final_state_component_bytes(&layout).unwrap();
        let workers = 2;
        let pinned = estimate_isolated_sweep_capacity(
            &layout,
            &generated,
            8,
            workers,
            CudaFinalStateReadbackMode::PackedPinned,
        )
        .unwrap();
        assert_eq!(pinned.final_state_bytes_per_lane, bytes);
        assert_eq!(pinned.requested_pinned_bytes_per_lane, bytes.total);
        assert_eq!(
            pinned.requested_cacheable_staging_bytes_per_lane,
            bytes.total
        );
        assert_eq!(pinned.requested_pinned_bytes, bytes.total * workers);
        assert_eq!(
            pinned.requested_cacheable_staging_bytes,
            bytes.total * workers
        );
        assert_eq!(pinned.requested_buffer_set_count, workers);
        let allocations_per_lane = usize::from(bytes.state != 0)
            + usize::from(bytes.inputs != 0)
            + usize::from(bytes.input_counts != 0);
        assert_eq!(
            pinned.requested_underlying_pinned_allocation_count,
            allocations_per_lane * workers
        );
        assert!(pinned.requested_underlying_pinned_allocation_count <= 3 * workers);

        assert!(estimate_isolated_sweep_capacity(
            &layout,
            &generated,
            8,
            usize::MAX,
            CudaFinalStateReadbackMode::PackedPinned,
        )
        .unwrap_err()
        .to_string()
        .contains("overflow"));

        for mode in [
            CudaFinalStateReadbackMode::Materialized,
            CudaFinalStateReadbackMode::PackedPageable,
        ] {
            let control =
                estimate_isolated_sweep_capacity(&layout, &generated, 8, workers, mode).unwrap();
            assert_eq!(control.requested_pinned_bytes, 0);
            assert_eq!(control.requested_cacheable_staging_bytes, 0);
            assert_eq!(control.requested_buffer_set_count, 0);
            assert_eq!(control.requested_underlying_pinned_allocation_count, 0);
        }
    }

    #[test]
    fn cacheable_staging_allocation_is_fallible_and_zero_safe() {
        let empty = allocate_cacheable_staging::<u64>(
            0,
            "input counts",
            FinalStateAllocationInjection::None,
        )
        .unwrap();
        assert!(empty.is_empty());
        assert_eq!(empty.capacity(), 0);

        let error = allocate_cacheable_staging::<u8>(
            17,
            "state",
            FinalStateAllocationInjection::Staging("state"),
        )
        .unwrap_err()
        .to_string();
        assert!(error.contains("injected packed-pinned cacheable staging allocation failure"));
        assert!(error.contains("requested 17 bytes"));
        assert!(error.contains("one lane"));
        assert!(!error.contains("fallback"));
        assert!(FinalStateAllocationInjection::Pinned("state").rejects_pinned("state"));
        assert!(!FinalStateAllocationInjection::Pinned("inputs").rejects_pinned("state"));
    }

    #[test]
    fn canonical_packed_hash_matches_materialized_v1_mixed_layout() {
        let source = r#"{"name":"hash_mixed","dt":1.0,"params":[],"boxes":[{"name":"world","tables":[{"name":"Target","size_hint":2,"attrs":[]},{"name":"Row","size_hint":2,"attrs":[{"name":"real","ty":{"kind":"real"}},{"name":"int","ty":{"kind":"int"}},{"name":"kind","ty":{"kind":"enum","variants":["a","b"]}},{"name":"target","ty":{"kind":"ref","table":"Target"}}]}],"transitions":[],"inputs":[],"outputs":[],"views":[]}],"wires":[],"summaries":[]}"#;
        let model = sembla_ir::validate(sembla_ir::parse_json(source).unwrap()).unwrap();
        let tables = vec![
            TableInit::new("world", "Target", 2, Vec::new()),
            TableInit::new(
                "world",
                "Row",
                2,
                vec![
                    ColumnInit::new("real", ColumnData::Real(vec![1.25, -3.5])),
                    ColumnInit::new("int", ColumnData::Int(vec![4, -9])),
                    ColumnInit::new("kind", ColumnData::Enum(vec![0, 1])),
                    ColumnInit::new("target", ColumnData::Ref(vec![1, 0])),
                ],
            ),
        ];
        let generated = generate(&model).unwrap();
        let layout = build_layout(&model, &tables, &generated).unwrap();
        let packed_state = pack_initial_state(&model, &tables, &layout).unwrap();
        let materialized = StateStore::new(&model, tables).unwrap();
        assert_eq!(
            hash_state(&model, &layout, &packed_state, &[0], &[0]),
            materialized.state_hash()
        );
    }

    #[test]
    fn canonical_packed_hash_matches_materialized_v2_inputs_and_negative_control() {
        let source = r#"{"name":"hash_inputs","dt":1.0,"params":[],"boxes":[{"name":"source","tables":[{"name":"Event","size_hint":2,"attrs":[{"name":"amount","ty":{"kind":"int"}}]}],"transitions":[],"inputs":[],"outputs":[{"name":"events","schema":[{"name":"amount","ty":{"kind":"int"}}],"builder":{"kind":"per_table","table":"Event","fields":[{"name":"amount","op":{"kind":"sum","value":{"kind":"self_attr","name":"amount"}},"filter":null}]}}],"views":[]},{"name":"sink","tables":[],"transitions":[],"inputs":[{"name":"events","schema":[{"name":"amount","ty":{"kind":"int"}}]}],"outputs":[],"views":[]}],"wires":[{"from":{"box":"source","port":"events"},"to":{"box":"sink","port":"events"}}],"summaries":[]}"#;
        let model = sembla_ir::validate(sembla_ir::parse_json(source).unwrap()).unwrap();
        let tables = vec![TableInit::new(
            "source",
            "Event",
            2,
            vec![ColumnInit::new("amount", ColumnData::Int(vec![4, 9]))],
        )];
        let generated = generate(&model).unwrap();
        let layout = build_layout(&model, &tables, &generated).unwrap();
        assert_eq!(layout.ports.len(), 1);
        let packed_state = pack_initial_state(&model, &tables, &layout).unwrap();
        let values = ColumnData::Int(vec![13]);
        let mut packed_inputs = vec![0_u8; layout.input_len.max(1)];
        write_column(
            &mut packed_inputs,
            layout.input_offsets[0] as usize,
            &values,
        );
        let input_counts = vec![1_u64];
        let input_table = InputTable {
            box_name: "sink".to_owned(),
            port_name: "events".to_owned(),
            schema: model.model().boxes[1].inputs[0].schema.clone(),
            row_count: 1,
            columns: vec![values],
        };
        let mut materialized = StateStore::new(&model, tables.clone()).unwrap();
        materialized
            .refresh_backend_snapshot(&model, &tables, vec![input_table])
            .unwrap();
        let packed = hash_state(
            &model,
            &layout,
            &packed_state,
            &packed_inputs,
            &input_counts,
        );
        assert_eq!(packed, materialized.state_hash());

        packed_inputs[layout.input_offsets[0] as usize] ^= 1;
        assert_ne!(
            hash_state(
                &model,
                &layout,
                &packed_state,
                &packed_inputs,
                &input_counts,
            ),
            packed
        );
    }
}

#[cfg(test)]
mod conflict_geometry_hardware {
    use super::{ConflictLaunchGeometry, CudaBackend, HashMode};
    use sembla_runtime::eval::ParamEnv;
    use sembla_runtime::executor::run_tick;
    use sembla_runtime::state::{ColumnData, ColumnInit, StateStore, TableInit};

    fn contested_model() -> sembla_ir::ValidatedModel {
        // Rules are deliberately ordered B, C, A. Their only enabled rows have
        // keys 1, 2, 0 and entity IDs 0, 1, 2 respectively, so key, rule, and
        // entity component minima come from different instances. The CPU
        // compare_instances lexicographic minimum is A (key 0), not a
        // component-wise synthetic tuple.
        let source = r#"{"name":"segmented_argmin_geometry","dt":1.0,"params":[],"boxes":[{"name":"world","tables":[{"name":"Worker","size_hint":1,"attrs":[]},{"name":"Applicant","size_hint":3,"attrs":[{"name":"worker","ty":{"kind":"ref","table":"Worker"}},{"name":"priority","ty":{"kind":"int"}},{"name":"role","ty":{"kind":"enum","variants":["B","C","A"]}},{"name":"outcome","ty":{"kind":"enum","variants":["Waiting","Won"]}}]}],"transitions":[{"name":"rule_b","table":"Applicant","guard":{"kind":"enum_is","attr":"role","variant":"B"},"hazard":{"kind":"real","value":1e300},"effects":[{"kind":"set_attr","attr":"outcome","value":{"kind":"enum","variant":"Won"}}],"contests":[{"resource":{"kind":"self_attr","name":"worker"},"ordering":{"kind":"key","expr":{"kind":"self_attr","name":"priority"}}}]},{"name":"rule_c","table":"Applicant","guard":{"kind":"enum_is","attr":"role","variant":"C"},"hazard":{"kind":"real","value":1e300},"effects":[{"kind":"set_attr","attr":"outcome","value":{"kind":"enum","variant":"Won"}}],"contests":[{"resource":{"kind":"self_attr","name":"worker"},"ordering":{"kind":"key","expr":{"kind":"self_attr","name":"priority"}}}]},{"name":"rule_a","table":"Applicant","guard":{"kind":"enum_is","attr":"role","variant":"A"},"hazard":{"kind":"real","value":1e300},"effects":[{"kind":"set_attr","attr":"outcome","value":{"kind":"enum","variant":"Won"}}],"contests":[{"resource":{"kind":"self_attr","name":"worker"},"ordering":{"kind":"key","expr":{"kind":"self_attr","name":"priority"}}}]}],"inputs":[],"outputs":[],"views":[]}],"wires":[],"summaries":[]}"#;
        sembla_ir::validate(sembla_ir::parse_json(source).unwrap()).unwrap()
    }

    fn contested_state() -> Vec<TableInit> {
        vec![
            TableInit::new("world", "Worker", 1, Vec::new()),
            TableInit::new(
                "world",
                "Applicant",
                3,
                vec![
                    ColumnInit::new("worker", ColumnData::Ref(vec![0, 0, 0])),
                    ColumnInit::new("priority", ColumnData::Int(vec![1, 2, 0])),
                    ColumnInit::new("role", ColumnData::Enum(vec![0, 1, 2])),
                    ColumnInit::new("outcome", ColumnData::Enum(vec![0, 0, 0])),
                ],
            ),
        ]
    }

    #[test]
    #[ignore = "requires a CUDA GPU; exercises explicit conflict launch geometries"]
    fn segmented_argmin_winner_matches_cpu_under_three_geometries() {
        let model = contested_model();
        let initial = contested_state();
        let params = ParamEnv::defaults(&model);
        let mut cpu = StateStore::new(&model, initial.clone()).unwrap();
        run_tick(&model, &mut cpu, &params, 9009, 0).unwrap();
        let expected = cpu.state_hash();

        for (grid, block) in [(1, 1), (1, 32), (3, 4)] {
            let mut backend =
                CudaBackend::new(&model, initial.clone(), &params, 9009, HashMode::EveryTick)
                    .expect("CUDA device, driver, and NVRTC are required");
            backend.conflict_launch_override = Some(ConflictLaunchGeometry { grid, block });
            let result = backend.run(1).unwrap();
            assert_eq!(result.final_state_hash, expected, "geometry {grid}x{block}");
            assert_eq!(result.per_tick_state_hashes, [expected]);
        }
    }
}
