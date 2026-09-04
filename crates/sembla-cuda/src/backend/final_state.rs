//! Retained final-state buffers and conservative sweep-capacity accounting.

use super::{
    mem, CudaError, CudaFinalStateBufferAccounting, CudaFinalStateDownloadedBytes,
    CudaFinalStateReadbackMode, CudaSlice, CudaStream, CudaSweepCapacityEstimate, DeviceRepr,
    GeneratedCuda, Layout, PinnedHostSlice, ValidAsZeroBits, GROUPED_OBSERVATION_KEY_SPACE_LIMIT,
};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(super) enum FinalStateAllocationInjection {
    #[default]
    None,
    #[cfg(test)]
    Pinned(&'static str),
    #[cfg(test)]
    Staging(&'static str),
}

impl FinalStateAllocationInjection {
    pub(super) fn rejects_pinned(self, _label: &str) -> bool {
        match self {
            Self::None => false,
            #[cfg(test)]
            Self::Pinned(injected) => injected == _label,
            #[cfg(test)]
            Self::Staging(_) => false,
        }
    }

    pub(super) fn rejects_staging(self, _label: &str) -> bool {
        match self {
            Self::None => false,
            #[cfg(test)]
            Self::Pinned(_) => false,
            #[cfg(test)]
            Self::Staging(injected) => injected == _label,
        }
    }
}

pub(super) fn allocate_cacheable_staging<T>(
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
    pub(super) fn allocate(
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

    pub(super) fn wait_until_readable(&self) -> Result<(), CudaError> {
        self.pinned.as_slice().map(|_| ()).map_err(|error| {
            CudaError::Driver(format!(
                "packed-pinned {} destination did not become host-readable: {error}",
                self.label
            ))
        })
    }

    pub(super) fn stage(&mut self) -> Result<(), CudaError> {
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
pub(super) struct PinnedFinalStateBuffers {
    // This is an Arc clone of the backend's existing default/non-blocking lane
    // stream. No treatment-specific stream is created.
    stream: std::sync::Arc<CudaStream>,
    state: Option<PinnedFinalStateComponent<u8>>,
    inputs: Option<PinnedFinalStateComponent<u8>>,
    input_counts: Option<PinnedFinalStateComponent<u64>>,
    accounting: CudaFinalStateBufferAccounting,
}

impl PinnedFinalStateBuffers {
    pub(super) fn allocate(
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

    pub(super) fn enqueue_downloads(
        &mut self,
        state: &CudaSlice<u8>,
        inputs: &CudaSlice<u8>,
        input_counts: &CudaSlice<u64>,
        bytes: CudaFinalStateDownloadedBytes,
    ) -> Result<(), CudaError> {
        if let Some(destination) = &mut self.state {
            let source = state.try_slice(0..bytes.state).ok_or_else(|| {
                CudaError::DeviceExecution(
                    "packed-pinned logical state view exceeds device slice".to_owned(),
                )
            })?;
            self.stream
                .memcpy_dtoh(&source, &mut destination.pinned)
                .map_err(|error| {
                    CudaError::Driver(format!(
                        "packed-pinned state D2H enqueue failed for {} bytes: {error}",
                        bytes.state
                    ))
                })?;
        }
        if let Some(destination) = &mut self.inputs {
            let source = inputs.try_slice(0..bytes.inputs).ok_or_else(|| {
                CudaError::DeviceExecution(
                    "packed-pinned logical input view exceeds device slice".to_owned(),
                )
            })?;
            self.stream
                .memcpy_dtoh(&source, &mut destination.pinned)
                .map_err(|error| {
                    CudaError::Driver(format!(
                        "packed-pinned input D2H enqueue failed for {} bytes: {error}",
                        bytes.inputs
                    ))
                })?;
        }
        if let Some(destination) = &mut self.input_counts {
            let count_len = bytes.input_counts / mem::size_of::<u64>();
            let source = input_counts.try_slice(0..count_len).ok_or_else(|| {
                CudaError::DeviceExecution(
                    "packed-pinned logical input-count view exceeds device slice".to_owned(),
                )
            })?;
            self.stream
                .memcpy_dtoh(&source, &mut destination.pinned)
                .map_err(|error| {
                    CudaError::Driver(format!(
                        "packed-pinned input-count D2H enqueue failed for {} bytes: {error}",
                        bytes.input_counts
                    ))
                })?;
        }
        Ok(())
    }

    pub(super) fn wait_until_readable(&self) -> Result<(), CudaError> {
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

    pub(super) fn stage(&mut self) -> Result<(), CudaError> {
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

    pub(super) fn state(&self) -> &[u8] {
        self.state
            .as_ref()
            .map_or(&[], |component| component.cacheable.as_slice())
    }

    pub(super) fn inputs(&self) -> &[u8] {
        self.inputs
            .as_ref()
            .map_or(&[], |component| component.cacheable.as_slice())
    }

    pub(super) fn input_counts(&self) -> &[u64] {
        self.input_counts
            .as_ref()
            .map_or(&[], |component| component.cacheable.as_slice())
    }

    pub(super) fn accounting(&self) -> CudaFinalStateBufferAccounting {
        self.accounting
    }
}

impl Drop for PinnedFinalStateBuffers {
    fn drop(&mut self) {
        // A failed enqueue/wait may unwind with work still pending. Synchronize
        // the retained lane stream before the pinned destinations are freed.
        self.stream.context().record_err(self.stream.synchronize());
    }
}

pub(super) fn checked_final_state_component_bytes(
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

pub(super) fn final_state_component_bytes(
    layout: &Layout,
) -> Result<CudaFinalStateDownloadedBytes, CudaError> {
    checked_final_state_component_bytes(
        layout.state_logical_len,
        layout.input_logical_len,
        layout.ports.len(),
    )
}

pub(super) fn checked_arena_len(
    per_slot: usize,
    slots: usize,
    label: &str,
) -> Result<usize, CudaError> {
    per_slot.checked_mul(slots).ok_or_else(|| {
        CudaError::InvalidInput(format!(
            "fused CUDA {label} arena size overflow for capacity {slots}"
        ))
    })
}

pub(super) const SWEEP_CAPACITY_MIB: usize = 1024 * 1024;
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

pub(super) fn estimate_isolated_sweep_capacity(
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
