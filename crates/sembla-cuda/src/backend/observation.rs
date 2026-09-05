//! CUDA observation and host readback orchestration.

use super::{
    control_reports_from_counts, decode_grouped_histogram, driver_error, fused_launch_builder,
    grouped_observation_layout, host_observation_fallback, mem, CudaBackend,
    CudaDeviceObservations, CudaError, CudaTickObservation, DeviceObservationEligibility,
    FusedBuffer, FusedReusedCudaTickObservations, GroupedObservationAxisLayout,
    GroupedObservationLayout, GroupedViewValue, LaunchConfig, ObservationValue,
    ReusedCudaTickObservation, StateStore, TimedReusedCudaTickObservation, ViewValue,
};
use crate::codegen::GeneratedGroupedObservation;

impl CudaBackend {
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
        let batch = self.fused_batch.as_ref().expect("fused batch exists");
        let capacity = batch.capacity;
        let strides = batch.strides_host.clone();
        let mut slot_errors = (0..active_width)
            .map(|_| None)
            .collect::<Vec<Option<CudaError>>>();

        let mut slot_views = self.observe_scalar_views_batch(
            active_width,
            strides[FusedBuffer::ObservationValues as usize],
        )?;
        let (grouped_specs, layouts) = self.prepare_grouped_layouts_batch(
            active_width,
            capacity,
            strides[FusedBuffer::GroupedExtrema as usize],
            strides[FusedBuffer::GroupedAxisMins as usize],
            &mut slot_errors,
        )?;
        let mut slot_grouped = self.observe_grouped_views_batch(
            active_width,
            strides[FusedBuffer::GroupedHistogram as usize],
            &grouped_specs,
            &layouts,
            &mut slot_errors,
        )?;
        let mut generic_by_slot = self.observe_generic_counts_batch(
            active_width,
            strides[FusedBuffer::GenericEnumCounts as usize],
            &mut slot_errors,
        )?;

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
        let views = self.observe_scalar_views()?;
        let (grouped_specs, layouts) = self.prepare_grouped_layouts()?;
        let grouped_views = self.observe_grouped_views(tick, &grouped_specs, &layouts)?;
        let generic_enum_counts = self.observe_generic_counts()?;
        Ok(Some(CudaDeviceObservations {
            views,
            grouped_views,
            generic_enum_counts,
        }))
    }

    fn observe_scalar_views_batch(
        &mut self,
        active_width: usize,
        scalar_stride: usize,
    ) -> Result<Vec<Vec<ViewValue>>, CudaError> {
        let scalar_count = self.launch_scalar_observations()?;
        if scalar_count == 0 {
            return Ok(vec![Vec::new(); active_width]);
        }
        let scalar_arena = self
            .stream
            .memcpy_dtov(&self.observation_values)
            .map_err(driver_error)?;
        Ok((0..active_width)
            .map(|slot| {
                let begin = slot * scalar_stride;
                self.scalar_view_values(&scalar_arena[begin..begin + scalar_count])
            })
            .collect())
    }

    fn observe_scalar_views(&mut self) -> Result<Vec<ViewValue>, CudaError> {
        let scalar_count = self.launch_scalar_observations()?;
        let values = if scalar_count == 0 {
            Vec::new()
        } else {
            self.stream
                .memcpy_dtov(&self.observation_values.slice(..scalar_count))
                .map_err(driver_error)?
        };
        Ok(self.scalar_view_values(&values))
    }

    fn launch_scalar_observations(&mut self) -> Result<usize, CudaError> {
        let scalar_count = self.generated.observation_view_tables.len();
        let scalar_count_u32 = u32::try_from(scalar_count).map_err(|_| {
            CudaError::InvalidInput("observation view count exceeds u32".to_owned())
        })?;
        if scalar_count_u32 == 0 {
            return Ok(0);
        }
        let mut init = fused_launch_builder(
            &self.stream,
            &self.init_observations,
            self.fused_batch.as_ref(),
        );
        init.arg(&mut self.observation_values)
            .arg(&scalar_count_u32);
        init.launch_generated(LaunchConfig::for_num_elems(scalar_count_u32))
            .map_err(driver_error)?;

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
            observe.launch_generated(config).map_err(driver_error)?;
        }
        Ok(scalar_count)
    }

    fn scalar_view_values(&self, values: &[i64]) -> Vec<ViewValue> {
        self.model
            .model()
            .boxes
            .iter()
            .flat_map(|model_box| {
                model_box
                    .views
                    .iter()
                    .map(move |view| (model_box.name.clone(), view.name.clone()))
            })
            .zip(values.iter().copied())
            .map(|((box_name, name), value)| ViewValue {
                box_name,
                name,
                value: ObservationValue::Int(value),
            })
            .collect()
    }

    fn prepare_grouped_layouts_batch(
        &mut self,
        active_width: usize,
        capacity: usize,
        extrema_stride: usize,
        axis_stride: usize,
        slot_errors: &mut [Option<CudaError>],
    ) -> Result<
        (
            Vec<GeneratedGroupedObservation>,
            Vec<Vec<GroupedObservationLayout>>,
        ),
        CudaError,
    > {
        let grouped_specs = self.generated.grouped_observation_views.clone();
        let band_count = self.launch_grouped_extrema(&grouped_specs)?;
        let band_arena = if band_count == 0 {
            vec![0_i64; extrema_stride * active_width]
        } else {
            self.stream
                .memcpy_dtov(&self.grouped_extrema)
                .map_err(driver_error)?
        };
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
                            .map(Self::empty_grouped_layout)
                            .collect(),
                    );
                    self.deactivate_fused_slot(slot)?;
                }
            }
        }
        self.upload_grouped_axes(layouts.iter().map(Vec::as_slice), axis_stride, capacity)?;
        Ok((grouped_specs, layouts))
    }

    fn prepare_grouped_layouts(
        &mut self,
    ) -> Result<
        (
            Vec<GeneratedGroupedObservation>,
            Vec<GroupedObservationLayout>,
        ),
        CudaError,
    > {
        let grouped_specs = self.generated.grouped_observation_views.clone();
        let band_count = self.launch_grouped_extrema(&grouped_specs)?;
        let band_extrema = if band_count == 0 {
            Vec::new()
        } else {
            self.stream
                .memcpy_dtov(&self.grouped_extrema.slice(..band_count * 2))
                .map_err(driver_error)?
        };
        let layouts = grouped_specs
            .iter()
            .map(|view| grouped_observation_layout(view, &self.layout.row_counts, &band_extrema))
            .collect::<Result<Vec<_>, CudaError>>()?;
        let axis_count = layouts.iter().map(|layout| layout.axes.len()).sum();
        self.upload_grouped_axes(std::iter::once(layouts.as_slice()), axis_count, 1)?;
        Ok((grouped_specs, layouts))
    }

    fn launch_grouped_extrema(
        &mut self,
        grouped_specs: &[GeneratedGroupedObservation],
    ) -> Result<usize, CudaError> {
        let band_count = self.generated.grouped_observation_band_axes;
        let band_count_u32 = u32::try_from(band_count).map_err(|_| {
            CudaError::InvalidInput("grouped band axis count exceeds u32".to_owned())
        })?;
        if band_count_u32 == 0 {
            return Ok(0);
        }
        let mut init = fused_launch_builder(
            &self.stream,
            &self.init_grouped_extrema,
            self.fused_batch.as_ref(),
        );
        init.arg(&mut self.grouped_extrema).arg(&band_count_u32);
        init.launch_generated(LaunchConfig::for_num_elems(band_count_u32))
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
            bound.launch_generated(config).map_err(driver_error)?;
        }
        Ok(band_count)
    }

    fn upload_grouped_axes<'a>(
        &mut self,
        layouts_by_slot: impl IntoIterator<Item = &'a [GroupedObservationLayout]>,
        axis_stride: usize,
        capacity: usize,
    ) -> Result<(), CudaError> {
        if axis_stride == 0 {
            return Ok(());
        }
        let mut axis_mins = vec![0_i64; axis_stride * capacity];
        let mut axis_cardinalities = vec![0_u64; axis_stride * capacity];
        for (slot, layouts) in layouts_by_slot.into_iter().enumerate() {
            let mut index = slot * axis_stride;
            for axis in layouts.iter().flat_map(|layout| &layout.axes) {
                axis_mins[index] = axis.minimum;
                axis_cardinalities[index] = axis.cardinality;
                index += 1;
            }
        }
        self.stream
            .memcpy_htod(&axis_mins, &mut self.grouped_axis_mins)
            .map_err(driver_error)?;
        self.stream
            .memcpy_htod(&axis_cardinalities, &mut self.grouped_axis_cardinalities)
            .map_err(driver_error)?;
        Ok(())
    }

    fn empty_grouped_layout(view: &GeneratedGroupedObservation) -> GroupedObservationLayout {
        GroupedObservationLayout {
            axes: view
                .axes
                .iter()
                .map(|_| GroupedObservationAxisLayout {
                    minimum: 0,
                    cardinality: 0,
                })
                .collect(),
            key_space_size: 0,
        }
    }

    fn observe_grouped_views_batch(
        &mut self,
        active_width: usize,
        histogram_stride: usize,
        grouped_specs: &[GeneratedGroupedObservation],
        layouts: &[Vec<GroupedObservationLayout>],
        slot_errors: &mut [Option<CudaError>],
    ) -> Result<Vec<Vec<GroupedViewValue>>, CudaError> {
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
            self.launch_grouped_histogram(
                view_index,
                view,
                max_key_space,
                "grouped observation index exceeds u32",
            )?;
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
        Ok(slot_grouped)
    }

    fn observe_grouped_views(
        &mut self,
        tick: u32,
        grouped_specs: &[GeneratedGroupedObservation],
        layouts: &[GroupedObservationLayout],
    ) -> Result<Vec<GroupedViewValue>, CudaError> {
        let mut grouped_views = Vec::new();
        for (view_index, (view, layout)) in grouped_specs.iter().zip(layouts).enumerate() {
            if layout.key_space_size == 0 {
                eprintln!(
                    "cuda_device_grouped_observation tick={tick} box={:?} view={:?} key_space_size=0 occupied_groups=0 emitted_groups=0",
                    view.box_name, view.name
                );
                continue;
            }
            self.launch_grouped_histogram(
                view_index,
                view,
                layout.key_space_size,
                "grouped observation view index exceeds u32",
            )?;
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
        Ok(grouped_views)
    }

    fn launch_grouped_histogram(
        &mut self,
        view_index: usize,
        view: &GeneratedGroupedObservation,
        key_space_size: usize,
        index_error: &'static str,
    ) -> Result<(), CudaError> {
        let key_space = u32::try_from(key_space_size)
            .map_err(|_| CudaError::InvalidInput("grouped key space exceeds u32".to_owned()))?;
        let key_space_u64 = u64::from(key_space);
        let mut init = fused_launch_builder(
            &self.stream,
            &self.init_grouped_histogram,
            self.fused_batch.as_ref(),
        );
        init.arg(&mut self.grouped_histogram).arg(&key_space_u64);
        init.launch_generated(LaunchConfig::for_num_elems(key_space))
            .map_err(driver_error)?;

        let rows = u32::try_from(self.layout.row_counts[view.table]).map_err(|_| {
            CudaError::InvalidInput("grouped observation row count exceeds u32".to_owned())
        })?;
        if rows == 0 {
            return Ok(());
        }
        let view_index = u32::try_from(view_index)
            .map_err(|_| CudaError::InvalidInput(index_error.to_owned()))?;
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
        observe.launch_generated(config).map_err(driver_error)?;
        Ok(())
    }

    fn observe_generic_counts_batch(
        &mut self,
        active_width: usize,
        stride: usize,
        slot_errors: &mut [Option<CudaError>],
    ) -> Result<Vec<Option<Vec<usize>>>, CudaError> {
        let generic_count = self.launch_generic_enum_observations(
            "generic enum row count exceeds u32",
            "generic enum index exceeds u32",
            false,
        )?;
        if generic_count == 0 {
            let value = (self.generated.observation_view_tables.is_empty()
                && !self.generated.grouped_observation_views.is_empty())
            .then(Vec::new);
            return Ok(vec![value; active_width]);
        }
        let counts = self
            .stream
            .memcpy_dtov(&self.generic_enum_counts)
            .map_err(driver_error)?;
        let mut by_slot = vec![None; active_width];
        for slot in 0..active_width {
            if slot_errors[slot].is_some() {
                continue;
            }
            let begin = slot * stride;
            match Self::host_generic_enum_counts(&counts[begin..begin + generic_count]) {
                Ok(counts) => by_slot[slot] = Some(counts),
                Err(error) => {
                    slot_errors[slot] = Some(error);
                    self.deactivate_fused_slot(slot)?;
                }
            }
        }
        Ok(by_slot)
    }

    fn observe_generic_counts(&mut self) -> Result<Option<Vec<usize>>, CudaError> {
        let generic_count = self.launch_generic_enum_observations(
            "generic enum observation row count exceeds u32",
            "generic enum observation index exceeds u32",
            true,
        )?;
        if generic_count == 0 {
            return Ok((self.generated.observation_view_tables.is_empty()
                && !self.generated.grouped_observation_views.is_empty())
            .then(Vec::new));
        }
        let counts = self
            .stream
            .memcpy_dtov(&self.generic_enum_counts.slice(..generic_count))
            .map_err(driver_error)?;
        Self::host_generic_enum_counts(&counts).map(Some)
    }

    fn launch_generic_enum_observations(
        &mut self,
        row_error: &'static str,
        index_error: &'static str,
        cap_grid: bool,
    ) -> Result<usize, CudaError> {
        let generic_count = self.generated.generic_enum_count;
        let count_u32 = u32::try_from(generic_count).map_err(|_| {
            CudaError::InvalidInput("generic enum observation count exceeds u32".to_owned())
        })?;
        if count_u32 == 0 {
            return Ok(0);
        }
        let count_u64 = u64::from(count_u32);
        let mut init = fused_launch_builder(
            &self.stream,
            &self.init_generic_enum_counts,
            self.fused_batch.as_ref(),
        );
        init.arg(&mut self.generic_enum_counts).arg(&count_u64);
        init.launch_generated(LaunchConfig::for_num_elems(count_u32))
            .map_err(driver_error)?;

        let observations = self.generated.generic_enum_observations.clone();
        for (index, observation) in observations.iter().enumerate() {
            let rows = u32::try_from(self.layout.row_counts[observation.table])
                .map_err(|_| CudaError::InvalidInput(row_error.to_owned()))?;
            if rows == 0 {
                continue;
            }
            let index = u32::try_from(index)
                .map_err(|_| CudaError::InvalidInput(index_error.to_owned()))?;
            let mut config = LaunchConfig::for_num_elems(rows);
            if cap_grid {
                config.grid_dim.0 = config.grid_dim.0.min(1024);
            }
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
            observe.launch_generated(config).map_err(driver_error)?;
        }
        Ok(generic_count)
    }

    fn host_generic_enum_counts(counts: &[u64]) -> Result<Vec<usize>, CudaError> {
        counts
            .iter()
            .copied()
            .map(|count| {
                usize::try_from(count).map_err(|_| {
                    CudaError::InvalidInput("generic enum count exceeds host usize".to_owned())
                })
            })
            .collect()
    }

    pub(super) fn ensure_host_state(&mut self) -> Result<(), CudaError> {
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
}
