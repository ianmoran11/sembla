use std::mem;

use cudarc::driver::{
    CudaContext, CudaFunction, CudaSlice, LaunchConfig, PushKernelArg,
};
use cudarc::nvrtc::{compile_ptx_with_opts, CompileOptions};
use sembla_ir::{AttrType, ParamValue, ValidatedModel};
use sembla_runtime::eval::ParamEnv;
use sembla_runtime::state::{ColumnData, StateStore, TableInit};
use sha2::{Digest, Sha256};

use crate::{generate, CudaAvailability, CudaError, GeneratedCuda};

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

#[derive(Debug)]
struct Layout {
    row_counts: Vec<u64>,
    column_offsets: Vec<u64>,
    state_len: usize,
    ports: Vec<(usize, usize)>,
    input_fields: Vec<(usize, usize, usize)>,
    input_offsets: Vec<u64>,
    input_len: usize,
    candidate_offsets: Vec<u64>,
    candidate_count: usize,
    aggregate_offsets: Vec<u64>,
    aggregate_len: usize,
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
    build_aggregates: CudaFunction,
    check_errors: CudaFunction,
    resolve_conflicts: CudaFunction,
    apply_effects: CudaFunction,
    build_outputs: CudaFunction,
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
    aggregate_offsets: CudaSlice<u64>,
    candidate_offsets: CudaSlice<u64>,
    enabled: CudaSlice<u8>,
    times: CudaSlice<f64>,
    candidate_errors: CudaSlice<u8>,
    wins: CudaSlice<u8>,
    write_offsets: CudaSlice<u64>,
    owners: CudaSlice<i32>,
    status: CudaSlice<u64>,
    seed: u64,
    next_tick: u32,
    hash_mode: HashMode,
}

impl CudaBackend {
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
        let device_count = CudaContext::device_count()
            .map_err(|error| CudaError::Driver(error.to_string()))?;
        let nvrtc_library = unsafe { cudarc::nvrtc::sys::is_culib_present() };
        CudaAvailability {
            driver_library,
            device_count: usize::try_from(device_count).unwrap_or(0),
            nvrtc_library,
        }
        .require()?;

        // Reuse the oracle's public constructor solely to validate initial
        // schema/ranges. It is dropped before CUDA construction and never runs.
        StateStore::new(model, initial_tables.clone())
            .map_err(|error| CudaError::InvalidInput(error.to_string()))?;

        let generated = generate(model)?;
        let dump_path = generated.dump_if_requested()?;
        let context = CudaContext::new(0).map_err(|error| CudaError::Driver(error.to_string()))?;
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
        let build_aggregates = load("sembla_build_aggregates")?;
        let check_errors = load("sembla_check_candidate_errors")?;
        let resolve_conflicts = load("sembla_resolve_conflicts")?;
        let apply_effects = load("sembla_apply_effects")?;
        let build_outputs = load("sembla_build_outputs")?;

        let layout = build_layout(model, &initial_tables, &generated)?;
        let state_bytes = pack_initial_state(model, &initial_tables, &layout)?;
        let params_bytes = pack_params(model, params)?;
        let state = stream
            .memcpy_stod(&state_bytes)
            .map_err(driver_error)?;
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
        let aggregate_offsets = stream
            .memcpy_stod(&nonempty(&layout.aggregate_offsets))
            .map_err(driver_error)?;
        let candidate_offsets = stream
            .memcpy_stod(&nonempty(&layout.candidate_offsets))
            .map_err(driver_error)?;
        let candidate_len = layout.candidate_count.max(1);
        let enabled = stream.alloc_zeros::<u8>(candidate_len).map_err(driver_error)?;
        let times = stream.alloc_zeros::<f64>(candidate_len).map_err(driver_error)?;
        let candidate_errors = stream.alloc_zeros::<u8>(candidate_len).map_err(driver_error)?;
        let wins = stream.alloc_zeros::<u8>(candidate_len).map_err(driver_error)?;
        let write_offsets = stream
            .memcpy_stod(&nonempty(&layout.write_offsets))
            .map_err(driver_error)?;
        let owners = stream
            .alloc_zeros::<i32>(layout.owner_count.max(1))
            .map_err(driver_error)?;
        let status = stream.alloc_zeros::<u64>(4).map_err(driver_error)?;

        Ok(Self {
            model: model.clone(),
            generated,
            layout,
            stream,
            transition_functions,
            build_aggregates,
            check_errors,
            resolve_conflicts,
            apply_effects,
            build_outputs,
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
            aggregate_offsets,
            candidate_offsets,
            enabled,
            times,
            candidate_errors,
            wins,
            write_offsets,
            owners,
            status,
            seed,
            next_tick: 0,
            hash_mode,
        })
    }

    pub fn generated(&self) -> &GeneratedCuda {
        &self.generated
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
        {
            let mut args = self.stream.launch_builder(&self.build_aggregates);
            args.arg(&self.state)
                .arg(&self.column_offsets)
                .arg(&self.row_counts)
                .arg(&self.inputs)
                .arg(&self.input_offsets)
                .arg(&self.input_counts)
                .arg(&self.params)
                .arg(&mut self.aggregates)
                .arg(&self.aggregate_offsets)
                .arg(&mut self.status);
            unsafe { args.launch(one) }.map_err(driver_error)?;
        }
        for (index, transition) in self.model.transitions().iter().enumerate() {
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
                .launch_builder(&self.transition_functions[index]);
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
                .arg(&mut self.candidate_errors);
            unsafe { args.launch(LaunchConfig::for_num_elems(rows)) }.map_err(driver_error)?;
        }
        {
            let count = self.layout.candidate_count as u64;
            let mut args = self.stream.launch_builder(&self.check_errors);
            args.arg(&self.candidate_errors)
                .arg(&count)
                .arg(&mut self.status);
            unsafe { args.launch(one) }.map_err(driver_error)?;
        }
        {
            let count = self.layout.candidate_count as u64;
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
                .arg(&count)
                .arg(&self.enabled)
                .arg(&self.times)
                .arg(&mut self.wins)
                .arg(&mut self.status);
            unsafe { args.launch(one) }.map_err(driver_error)?;
        }
        self.stream
            .memcpy_dtod(&self.state, &mut self.next_state)
            .map_err(driver_error)?;
        {
            let owner_count = self.layout.owner_count as u64;
            let mut args = self.stream.launch_builder(&self.apply_effects);
            args.arg(&self.state)
                .arg(&mut self.next_state)
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
                .arg(&owner_count)
                .arg(&mut self.status);
            unsafe { args.launch(one) }.map_err(driver_error)?;
        }
        {
            let port_count = self.layout.ports.len() as u64;
            let mut args = self.stream.launch_builder(&self.build_outputs);
            args.arg(&self.next_state)
                .arg(&self.column_offsets)
                .arg(&self.row_counts)
                .arg(&self.inputs)
                .arg(&self.input_offsets)
                .arg(&self.input_counts)
                .arg(&self.params)
                .arg(&self.aggregates)
                .arg(&self.aggregate_offsets)
                .arg(&mut self.next_inputs)
                .arg(&self.input_offsets)
                .arg(&mut self.next_input_counts)
                .arg(&port_count)
                .arg(&mut self.status);
            unsafe { args.launch(one) }.map_err(driver_error)?;
        }
        let status = self.stream.memcpy_dtov(&self.status).map_err(driver_error)?;
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

    fn download_hash(&self) -> Result<[u8; 32], CudaError> {
        let state = self.stream.memcpy_dtov(&self.state).map_err(driver_error)?;
        let inputs = self.stream.memcpy_dtov(&self.inputs).map_err(driver_error)?;
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
    let mut input_fields = Vec::new();
    let mut input_offsets = Vec::new();
    let mut input_len = 0;
    let mut write_offsets = Vec::new();
    let mut owner_count = 0;

    for (box_index, model_box) in model.model().boxes.iter().enumerate() {
        for (table_index, table) in model_box.tables.iter().enumerate() {
            let initial = find_table(initial_tables, &model_box.name, &table.name)?;
            row_counts.push(initial.row_count as u64);
            for attr_index in 0..table.attrs.len() {
                state_len = align8(state_len);
                column_offsets.push(state_len as u64);
                state_len = state_len
                    .checked_add(initial.row_count.checked_mul(type_size(&table.attrs[attr_index].ty)).ok_or_else(|| CudaError::InvalidInput("state byte size overflow".to_owned()))?)
                    .ok_or_else(|| CudaError::InvalidInput("state byte size overflow".to_owned()))?;
                write_offsets.push(owner_count as u64);
                owner_count = owner_count
                    .checked_add(initial.row_count)
                    .ok_or_else(|| CudaError::InvalidInput("write-owner size overflow".to_owned()))?;
                let _ = (box_index, table_index, attr_index);
            }
        }
        for (port_index, port) in model_box.inputs.iter().enumerate() {
            ports.push((box_index, port_index));
            for field_index in 0..port.schema.len() {
                input_len = align8(input_len);
                input_fields.push((box_index, port_index, field_index));
                input_offsets.push(input_len as u64);
                // v0.1 outputs are one-row aggregate tables.
                input_len = input_len
                    .checked_add(type_size(&port.schema[field_index].ty))
                    .ok_or_else(|| CudaError::InvalidInput("input byte size overflow".to_owned()))?;
            }
        }
    }
    state_len = state_len.max(1);
    input_len = input_len.max(1);

    let mut candidate_offsets = Vec::new();
    let mut candidate_count = 0_usize;
    for transition in model.transitions() {
        candidate_offsets.push(candidate_count as u64);
        let declaration = &model.model().boxes[transition.box_index].transitions
            [transition.transition_index];
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
    for table in &generated.aggregate_group_tables {
        aggregate_len = align8(aggregate_len);
        aggregate_offsets.push(aggregate_len as u64);
        aggregate_len = aggregate_len
            .checked_add((row_counts[*table] as usize).checked_mul(8).ok_or_else(|| CudaError::InvalidInput("aggregate size overflow".to_owned()))?)
            .ok_or_else(|| CudaError::InvalidInput("aggregate size overflow".to_owned()))?;
    }

    Ok(Layout {
        row_counts,
        column_offsets,
        state_len,
        ports,
        input_fields,
        input_offsets,
        input_len,
        candidate_offsets,
        candidate_count,
        aggregate_offsets,
        aggregate_len: aggregate_len.max(1),
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
                    .ok_or_else(|| CudaError::InvalidInput(format!(
                        "{}.{}.{} has no initializer",
                        model_box.name, table.name, attr.name
                    )))?;
                write_column(&mut bytes, layout.column_offsets[column] as usize, &data.data);
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
