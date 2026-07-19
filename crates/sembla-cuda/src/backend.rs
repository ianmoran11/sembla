use std::mem;

use cudarc::driver::{CudaContext, CudaFunction, CudaSlice, LaunchConfig, PushKernelArg};
use cudarc::nvrtc::{compile_ptx_with_opts, CompileOptions};
use sembla_ir::{AttrType, ParamValue, ValidatedModel};
use sembla_runtime::eval::ParamEnv;
use sembla_runtime::state::{ColumnData, ColumnInit, InputTable, StateStore, TableInit};
use sha2::{Digest, Sha256};

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

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CudaDeviceIdentity {
    pub gpu_model: String,
    pub driver_version: String,
}

#[derive(Clone, Debug)]
pub struct CudaTickObservation {
    pub tick: u32,
    pub state: StateStore,
    pub fired_per_box: Vec<(String, Vec<(u32, usize)>)>,
    pub deferred_per_resource_table: Vec<(String, usize)>,
}

#[derive(Debug)]
struct Layout {
    row_counts: Vec<u64>,
    column_offsets: Vec<u64>,
    state_len: usize,
    ports: Vec<(usize, usize)>,
    input_offsets: Vec<u64>,
    input_len: usize,
    candidate_offsets: Vec<u64>,
    candidate_count: usize,
    aggregate_offsets: Vec<u64>,
    aggregate_len: usize,
    aggregate_max_groups: usize,
    write_offsets: Vec<u64>,
    owner_count: usize,
}

#[derive(Debug)]
pub struct CudaBackend {
    model: ValidatedModel,
    generated: GeneratedCuda,
    layout: Layout,
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
    resolve_conflicts: CudaFunction,
    validate_effects: CudaFunction,
    prepare_effects: CudaFunction,
    apply_effects: CudaFunction,
    validate_outputs: CudaFunction,
    prepare_outputs: CudaFunction,
    build_output_partials: CudaFunction,
    finish_outputs: CudaFunction,
    check_output_errors: CudaFunction,
    philox_vectors_kernel: CudaFunction,
    state: CudaSlice<u8>,
    next_state: CudaSlice<u8>,
    column_offsets: CudaSlice<u64>,
    row_counts: CudaSlice<u64>,
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
    enabled: CudaSlice<u8>,
    times: CudaSlice<f64>,
    candidate_errors: CudaSlice<u8>,
    wins: CudaSlice<u8>,
    deferred: CudaSlice<u8>,
    write_offsets: CudaSlice<u64>,
    owners: CudaSlice<i32>,
    owner_values: CudaSlice<u64>,
    output_partials: CudaSlice<u64>,
    output_errors: CudaSlice<u8>,
    status: CudaSlice<u64>,
    seed: u64,
    next_tick: u32,
    hash_mode: HashMode,
    device_identity: CudaDeviceIdentity,
}

impl CudaBackend {
    /// Applies the same explicit availability gate used by [`Self::new`].
    /// This seam makes no-device behavior testable without depending on the
    /// machine running the test and never constructs another backend.
    pub fn check_availability(availability: CudaAvailability) -> Result<(), CudaError> {
        availability.require()
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

        // Reuse the oracle's public constructor solely to validate initial
        // schema/ranges. It is dropped before CUDA construction and never runs.
        StateStore::new(model, initial_tables.clone())
            .map_err(|error| CudaError::InvalidInput(error.to_string()))?;

        let generated = generate(model)?;
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
        let stream = context.default_stream();

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
        let resolve_conflicts = load("sembla_resolve_conflicts")?;
        let validate_effects = load("sembla_validate_effects")?;
        let prepare_effects = load("sembla_prepare_effects")?;
        let apply_effects = load("sembla_apply_effects")?;
        let validate_outputs = load("sembla_validate_outputs")?;
        let prepare_outputs = load("sembla_prepare_outputs")?;
        let build_output_partials = load("sembla_build_output_partials")?;
        let finish_outputs = load("sembla_finish_outputs")?;
        let check_output_errors = load("sembla_check_output_errors")?;
        let philox_vectors_kernel = load("sembla_philox_vectors")?;

        let layout = build_layout(model, &initial_tables, &generated)?;
        let state_bytes = pack_initial_state(model, &initial_tables, &layout)?;
        let params_bytes = pack_params(model, params)?;
        let state = stream.memcpy_stod(&state_bytes).map_err(driver_error)?;
        let next_state = stream.clone_dtod(&state).map_err(driver_error)?;
        let column_offsets = stream
            .memcpy_stod(&nonempty(&layout.column_offsets))
            .map_err(driver_error)?;
        let row_counts = stream
            .memcpy_stod(&nonempty(&layout.row_counts))
            .map_err(driver_error)?;
        let input_zeroes = vec![0_u8; layout.input_len.max(1)];
        let inputs = stream.memcpy_stod(&input_zeroes).map_err(driver_error)?;
        let next_inputs = stream.memcpy_stod(&input_zeroes).map_err(driver_error)?;
        let input_offsets = stream
            .memcpy_stod(&nonempty(&layout.input_offsets))
            .map_err(driver_error)?;
        let input_count_zeroes = vec![0_u64; layout.ports.len().max(1)];
        let input_counts = stream
            .memcpy_stod(&input_count_zeroes)
            .map_err(driver_error)?;
        let next_input_counts = stream
            .memcpy_stod(&input_count_zeroes)
            .map_err(driver_error)?;
        let params = stream.memcpy_stod(&params_bytes).map_err(driver_error)?;
        let aggregates = stream
            .memcpy_stod(&vec![0_u8; layout.aggregate_len.max(1)])
            .map_err(driver_error)?;
        let aggregate_partials = stream
            .memcpy_stod(&vec![0_u8; layout.aggregate_len.max(1) * 2])
            .map_err(driver_error)?;
        let aggregate_errors = stream
            .alloc_zeros::<u8>((layout.aggregate_max_groups + 2).max(2))
            .map_err(driver_error)?;
        let aggregate_facts = stream
            .alloc_zeros::<u8>(generated.aggregate_group_tables.len().max(1))
            .map_err(driver_error)?;
        let aggregate_active = stream
            .alloc_zeros::<u8>(generated.aggregate_group_tables.len().max(1))
            .map_err(driver_error)?;
        let aggregate_offsets = stream
            .memcpy_stod(&nonempty(&layout.aggregate_offsets))
            .map_err(driver_error)?;
        let candidate_offsets = stream
            .memcpy_stod(&nonempty(&layout.candidate_offsets))
            .map_err(driver_error)?;
        let candidate_len = layout.candidate_count.max(1);
        let enabled = stream
            .alloc_zeros::<u8>(candidate_len)
            .map_err(driver_error)?;
        let times = stream
            .alloc_zeros::<f64>(candidate_len)
            .map_err(driver_error)?;
        let candidate_error_len = candidate_len.checked_mul(2).ok_or_else(|| {
            CudaError::InvalidInput("candidate error buffer size overflow".to_owned())
        })?;
        let candidate_errors = stream
            .alloc_zeros::<u8>(candidate_error_len)
            .map_err(driver_error)?;
        let wins = stream
            .alloc_zeros::<u8>(candidate_len)
            .map_err(driver_error)?;
        let deferred_len = candidate_len
            .checked_mul(layout.row_counts.len().max(1))
            .ok_or_else(|| CudaError::InvalidInput("deferred metadata size overflow".to_owned()))?;
        let deferred = stream
            .alloc_zeros::<u8>(deferred_len)
            .map_err(driver_error)?;
        let write_offsets = stream
            .memcpy_stod(&nonempty(&layout.write_offsets))
            .map_err(driver_error)?;
        let owners = stream
            .alloc_zeros::<i32>(layout.owner_count.max(1))
            .map_err(driver_error)?;
        let owner_values = stream
            .alloc_zeros::<u64>(layout.owner_count.max(1))
            .map_err(driver_error)?;
        let output_field_count = layout.input_offsets.len().max(1);
        let output_partials = stream
            .alloc_zeros::<u64>(output_field_count * 2)
            .map_err(driver_error)?;
        let output_errors = stream
            .alloc_zeros::<u8>(output_field_count * 3)
            .map_err(driver_error)?;
        let status = stream.alloc_zeros::<u64>(4).map_err(driver_error)?;

        Ok(Self {
            model: model.clone(),
            generated,
            layout,
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
            resolve_conflicts,
            validate_effects,
            prepare_effects,
            apply_effects,
            validate_outputs,
            prepare_outputs,
            build_output_partials,
            finish_outputs,
            check_output_errors,
            philox_vectors_kernel,
            state,
            next_state,
            column_offsets,
            row_counts,
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
            enabled,
            times,
            candidate_errors,
            wins,
            deferred,
            write_offsets,
            owners,
            owner_values,
            output_partials,
            output_errors,
            status,
            seed,
            next_tick: 0,
            hash_mode,
            device_identity,
        })
    }

    pub fn generated(&self) -> &GeneratedCuda {
        &self.generated
    }

    pub fn device_identity(&self) -> &CudaDeviceIdentity {
        &self.device_identity
    }

    /// Executes one tick on CUDA and downloads a read-only observation snapshot.
    /// State remains resident on the device for subsequent ticks.
    pub fn run_tick_observed(&mut self) -> Result<CudaTickObservation, CudaError> {
        let tick = self.next_tick;
        self.execute_tick()?;
        let wins = self.stream.memcpy_dtov(&self.wins).map_err(driver_error)?;
        let deferred = self
            .stream
            .memcpy_dtov(&self.deferred)
            .map_err(driver_error)?;
        let mut fired_per_box = Vec::with_capacity(self.model.model().boxes.len());
        for (box_index, model_box) in self.model.model().boxes.iter().enumerate() {
            let mut fired = Vec::with_capacity(model_box.transitions.len());
            for transition in self
                .model
                .transitions()
                .iter()
                .filter(|transition| transition.box_index == box_index)
            {
                let rule = transition.rule_id as usize;
                let begin = self.layout.candidate_offsets[rule] as usize;
                let end = self
                    .layout
                    .candidate_offsets
                    .get(rule + 1)
                    .copied()
                    .map(|value| value as usize)
                    .unwrap_or(self.layout.candidate_count);
                fired.push((
                    transition.rule_id,
                    wins[begin..end].iter().filter(|value| **value != 0).count(),
                ));
            }
            fired_per_box.push((model_box.name.clone(), fired));
        }
        let table_count = self.layout.row_counts.len();
        let qualify = self.model.model().boxes.len() > 1;
        let mut deferred_per_resource_table = Vec::new();
        let mut global_table = 0;
        for model_box in &self.model.model().boxes {
            for table in &model_box.tables {
                let count = (0..self.layout.candidate_count)
                    .filter(|candidate| deferred[candidate * table_count + global_table] != 0)
                    .count();
                if count != 0 {
                    let name = if qualify {
                        format!("{}.{}", model_box.name, table.name)
                    } else {
                        table.name.clone()
                    };
                    deferred_per_resource_table.push((name, count));
                }
                global_table += 1;
            }
        }
        Ok(CudaTickObservation {
            tick,
            state: self.download_state_store()?,
            fired_per_box,
            deferred_per_resource_table,
        })
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
            .map(|value| value.rule_id)
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

    fn execute_tick(&mut self) -> Result<(), CudaError> {
        let one = LaunchConfig {
            grid_dim: (1, 1, 1),
            block_dim: (1, 1, 1),
            shared_mem_bytes: 0,
        };
        let aggregate_error_count = (self.layout.aggregate_max_groups + 2) as u64;
        {
            let mut args = self.stream.launch_builder(&self.reset_status);
            args.arg(&mut self.status)
                .arg(&mut self.aggregate_errors)
                .arg(&aggregate_error_count);
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
            let mut args = self.stream.launch_builder(&self.build_aggregate_partials);
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
                let mut args = self.stream.launch_builder(&self.finish_aggregates);
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
            let mut args = self.stream.launch_builder(&self.record_aggregate_errors);
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
                {
                    let mut args = self.stream.launch_builder(&self.validate_transition);
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
                    unsafe { args.launch(one) }.map_err(driver_error)?;
                }

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
                if rows == 0 {
                    continue;
                }
                let dt = self.model.model().dt;
                let tick = self.next_tick;
                let mut args = self
                    .stream
                    .launch_builder(&self.transition_functions[*index]);
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
                let mut args = self.stream.launch_builder(&self.check_errors);
                args.arg(&self.candidate_errors)
                    .arg(&candidate_begin)
                    .arg(&candidate_count)
                    .arg(&mut self.status);
                unsafe { args.launch(one) }.map_err(driver_error)?;

                let mut args = self.stream.launch_builder(&self.validate_claims);
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
                unsafe { args.launch(one) }.map_err(driver_error)?;
            }

            let box_index_u32 = u32::try_from(box_index)
                .map_err(|_| CudaError::InvalidInput("box count exceeds u32".to_owned()))?;
            {
                let mut args = self
                    .stream
                    .launch_builder(&self.validate_claim_compatibility);
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
            if let Some((_, first)) = transition_positions.first() {
                let rule_index = usize::try_from(first.rule_id).map_err(|_| {
                    CudaError::InvalidInput("rule id exceeds host index width".to_owned())
                })?;
                candidate_begin = self.layout.candidate_offsets[rule_index];
                for (_, transition) in &transition_positions {
                    let model_transition = &self.model.model().boxes[transition.box_index]
                        .transitions[transition.transition_index];
                    let table_index = self.model.model().boxes[transition.box_index]
                        .tables
                        .iter()
                        .position(|table| table.name == model_transition.table)
                        .expect("validated transition table");
                    let global_table = global_table(&self.model, transition.box_index, table_index);
                    candidate_count = candidate_count
                        .checked_add(self.layout.row_counts[global_table])
                        .ok_or_else(|| {
                            CudaError::InvalidInput("box candidate count overflow".to_owned())
                        })?;
                }
            }
            if candidate_count != 0 {
                let launch_count = u32::try_from(candidate_count).map_err(|_| {
                    CudaError::InvalidInput(
                        "box candidate count exceeds CUDA launch capacity".to_owned(),
                    )
                })?;
                let resource_table_count = self.layout.row_counts.len() as u64;
                let mut args = self.stream.launch_builder(&self.resolve_conflicts);
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
                    .arg(&candidate_begin)
                    .arg(&candidate_count)
                    .arg(&resource_table_count)
                    .arg(&self.enabled)
                    .arg(&self.times)
                    .arg(&mut self.wins)
                    .arg(&mut self.deferred)
                    .arg(&self.status);
                unsafe { args.launch(LaunchConfig::for_num_elems(launch_count)) }
                    .map_err(driver_error)?;
            }

            let mut args = self.stream.launch_builder(&self.validate_effects);
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
                .arg(&box_index_u32)
                .arg(&mut self.status);
            unsafe { args.launch(one) }.map_err(driver_error)?;
        }
        self.stream
            .memcpy_dtod(&self.state, &mut self.next_state)
            .map_err(driver_error)?;
        {
            let owner_count = self.layout.owner_count as u64;
            let mut args = self.stream.launch_builder(&self.prepare_effects);
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
                .arg(&owner_count)
                .arg(&mut self.status);
            unsafe { args.launch(one) }.map_err(driver_error)?;
        }
        if self.layout.owner_count != 0 {
            let owner_count = self.layout.owner_count as u64;
            let launch_count = u32::try_from(self.layout.owner_count).map_err(|_| {
                CudaError::InvalidInput("write-owner count exceeds CUDA launch capacity".to_owned())
            })?;
            let mut args = self.stream.launch_builder(&self.apply_effects);
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
            let mut args = self.stream.launch_builder(&self.build_aggregate_partials);
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
                let mut args = self.stream.launch_builder(&self.finish_aggregates);
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
            let mut args = self.stream.launch_builder(&self.record_aggregate_errors);
            args.arg(&mut self.aggregate_errors)
                .arg(&aggregate_error_count)
                .arg(&aggregate_identity)
                .arg(&mut self.aggregate_facts);
            unsafe { args.launch(one) }.map_err(driver_error)?;
        }
        {
            let mut args = self.stream.launch_builder(&self.validate_outputs);
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
            unsafe { args.launch(one) }.map_err(driver_error)?;
        }
        {
            let port_count = self.layout.ports.len() as u64;
            let field_count = self.layout.input_offsets.len() as u64;
            let error_count = field_count.saturating_mul(3).max(3);
            let mut args = self.stream.launch_builder(&self.prepare_outputs);
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
            let mut args = self.stream.launch_builder(&self.build_output_partials);
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
            let mut args = self.stream.launch_builder(&self.finish_outputs);
            args.arg(&self.output_partials)
                .arg(&field_count_u64)
                .arg(&mut self.next_inputs)
                .arg(&self.input_offsets)
                .arg(&mut self.output_errors);
            unsafe { args.launch(LaunchConfig::for_num_elems(field_count)) }
                .map_err(driver_error)?;
            let mut args = self.stream.launch_builder(&self.check_output_errors);
            args.arg(&self.output_errors)
                .arg(&field_count_u64)
                .arg(&mut self.status);
            unsafe { args.launch(one) }.map_err(driver_error)?;
        }
        let status = self
            .stream
            .memcpy_dtov(&self.status)
            .map_err(driver_error)?;
        if status[0] != 0 {
            return Err(device_status(&status));
        }
        mem::swap(&mut self.state, &mut self.next_state);
        mem::swap(&mut self.inputs, &mut self.next_inputs);
        mem::swap(&mut self.input_counts, &mut self.next_input_counts);
        self.next_tick = self
            .next_tick
            .checked_add(1)
            .ok_or_else(|| CudaError::DeviceExecution("tick coordinate overflow".to_owned()))?;
        Ok(())
    }

    fn download_state_store(&self) -> Result<StateStore, CudaError> {
        let state = self.stream.memcpy_dtov(&self.state).map_err(driver_error)?;
        let inputs = self
            .stream
            .memcpy_dtov(&self.inputs)
            .map_err(driver_error)?;
        let input_counts = self
            .stream
            .memcpy_dtov(&self.input_counts)
            .map_err(driver_error)?;
        let initial = unpack_state(&self.model, &self.layout, &state);
        let mut store = StateStore::new(&self.model, initial)
            .map_err(|error| CudaError::DeviceExecution(error.to_string()))?;
        store
            .replace_backend_inputs(unpack_inputs(
                &self.model,
                &self.layout,
                &inputs,
                &input_counts,
            ))
            .map_err(|error| CudaError::DeviceExecution(error.to_string()))?;
        Ok(store)
    }

    fn download_hash(&self) -> Result<[u8; 32], CudaError> {
        let state = self.stream.memcpy_dtov(&self.state).map_err(driver_error)?;
        let inputs = self
            .stream
            .memcpy_dtov(&self.inputs)
            .map_err(driver_error)?;
        let input_counts = self
            .stream
            .memcpy_dtov(&self.input_counts)
            .map_err(driver_error)?;
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

fn unpack_state(model: &ValidatedModel, layout: &Layout, bytes: &[u8]) -> Vec<TableInit> {
    let mut tables = Vec::new();
    let mut global_table = 0;
    let mut column = 0;
    for model_box in &model.model().boxes {
        for table in &model_box.tables {
            let rows = layout.row_counts[global_table] as usize;
            let columns = table
                .attrs
                .iter()
                .map(|attr| {
                    let data = read_column(
                        bytes,
                        layout.column_offsets[column] as usize,
                        rows,
                        &attr.ty,
                    );
                    column += 1;
                    ColumnInit::new(&attr.name, data)
                })
                .collect();
            tables.push(TableInit::new(&model_box.name, &table.name, rows, columns));
            global_table += 1;
        }
    }
    tables
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
    state_len = state_len.max(1);
    input_len = input_len.max(1);

    let mut candidate_offsets = Vec::new();
    let mut candidate_count = 0_usize;
    for transition in model.transitions() {
        candidate_offsets.push(candidate_count as u64);
        let declaration =
            &model.model().boxes[transition.box_index].transitions[transition.transition_index];
        let table_index = model.model().boxes[transition.box_index]
            .tables
            .iter()
            .position(|table| table.name == declaration.table)
            .expect("validated transition table");
        let global = global_table(model, transition.box_index, table_index);
        candidate_count = candidate_count
            .checked_add(row_counts[global] as usize)
            .ok_or_else(|| CudaError::InvalidInput("candidate size overflow".to_owned()))?;
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
        column_offsets,
        state_len,
        ports,
        input_offsets,
        input_len,
        candidate_offsets,
        candidate_count,
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
