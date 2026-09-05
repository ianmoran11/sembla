//! Sweep timing documents and final-state accounting.

use super::*;

#[derive(Clone, Serialize)]
pub(crate) struct SweepFinalStateDownloadedBytesTiming {
    pub(super) state: usize,
    pub(super) inputs: usize,
    pub(super) input_counts: usize,
    pub(super) total: usize,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub(crate) struct SweepFinalStateBufferAccountingTiming {
    pub(super) buffer_set_count: usize,
    pub(super) underlying_pinned_allocation_count: usize,
    pub(super) pinned_bytes: usize,
    pub(super) cacheable_staging_bytes: usize,
}

pub(crate) const FINAL_STATE_TIMER_TOLERANCE: Duration = Duration::from_micros(1);
pub(crate) const FINAL_STATE_TIMER_TOLERANCE_MS: f64 = 0.001;

#[derive(Clone, Serialize)]
pub(crate) struct SweepFinalStateTiming {
    pub(super) schema: &'static str,
    pub(super) mode: &'static str,
    pub(super) one_time_allocation_ms: f64,
    pub(super) pageable_dtoh_host_api_ms: Option<f64>,
    pub(super) pinned_dtoh_enqueue_api_ms: Option<f64>,
    pub(super) wait_to_pinned_host_readable_ms: Option<f64>,
    pub(super) pinned_to_cacheable_staging_copy_ms: Option<f64>,
    pub(super) host_state_reconstruction_ms: Option<f64>,
    pub(super) cpu_sha256_ms: f64,
    pub(super) attributed_phase_sum_ms: f64,
    pub(super) unattributed_timer_overhead_ms: f64,
    pub(super) final_state_seam_total_ms: f64,
    pub(super) final_state_seam_total_excludes_one_time_allocation: bool,
    pub(super) timer_tolerance_ms: f64,
    pub(super) phases_reconcile: bool,
    pub(super) allocation_plus_seam_reconciles_with_draw_wall: bool,
    pub(super) downloaded_bytes: SweepFinalStateDownloadedBytesTiming,
    pub(super) buffer_accounting: SweepFinalStateBufferAccountingTiming,
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
    pub(super) k: u32,
    pub(super) wall_time_ms: f64,
    pub(super) final_state: Option<SweepFinalStateTiming>,
}

#[derive(Serialize)]
pub(crate) struct SweepTimingDocument {
    pub(super) schema: &'static str,
    pub(super) backend: &'static str,
    pub(super) draws: u32,
    pub(super) ticks_per_draw: u32,
    pub(super) setup_wall_time_ms: f64,
    pub(super) draw_zero_including_setup_wall_time_ms: f64,
    pub(super) final_state_buffer_accounting: SweepFinalStateBufferAccountingDocument,
    pub(super) draw_timings: Vec<SweepTimingDraw>,
    pub(super) whole_sweep_wall_time_ms: f64,
    pub(super) repository_commit: String,
    pub(super) binary_sha256: String,
}

#[derive(Serialize)]
pub(crate) struct SweepFusedSpikeTimingChunk {
    pub(super) chunk_index: usize,
    pub(super) first_k: u32,
    pub(super) active_slots: usize,
    pub(super) capacity: usize,
    pub(super) start_offset_ms: f64,
    pub(super) finish_offset_ms: f64,
    pub(super) shared_chunk_wall_time_ms: f64,
}

#[derive(Serialize)]
pub(crate) struct SweepFusedSpikeTimingDocument {
    pub(super) schema: &'static str,
    pub(super) backend: &'static str,
    pub(super) draws: u32,
    pub(super) ticks_per_draw: u32,
    pub(super) requested_capacity: usize,
    pub(super) maximum_active_slots: usize,
    pub(super) setup_wall_time_ms: f64,
    pub(super) execution_window_wall_time_ms: f64,
    pub(super) publication_wall_time_ms: f64,
    pub(super) chunks: Vec<SweepFusedSpikeTimingChunk>,
    pub(super) whole_sweep_wall_time_ms: f64,
    pub(super) repository_commit: String,
    pub(super) binary_sha256: String,
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
    pub(super) schema: &'static str,
    pub(super) backend: &'static str,
    pub(super) draws: u32,
    pub(super) ticks_per_draw: u32,
    pub(super) requested_draw_workers: usize,
    pub(super) effective_draw_workers: usize,
    pub(super) maximum_pending_results: usize,
    pub(super) execution_mode: &'static str,
    pub(super) setup_wall_time_ms: f64,
    pub(super) execution_window_wall_time_ms: f64,
    pub(super) publication_wall_time_ms: f64,
    pub(super) final_state_buffer_accounting: SweepFinalStateBufferAccountingDocument,
    pub(super) draw_timings: Vec<SweepConcurrencySpikeTimingDraw>,
    pub(super) whole_sweep_wall_time_ms: f64,
    pub(super) repository_commit: String,
    pub(super) binary_sha256: String,
}
