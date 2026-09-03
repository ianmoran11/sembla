//! Host-side CUDA state layout, packing, reconstruction, and canonical hashing.

use super::*;

pub(super) fn downloaded_state_bytes(
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

pub(super) fn unpack_state_into(
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

pub(super) fn unpack_inputs(
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

pub(super) fn build_layout(
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

pub(super) fn pack_initial_state(
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

pub(super) fn pack_params(model: &ValidatedModel, params: &ParamEnv) -> Result<Vec<u8>, CudaError> {
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

pub(super) fn write_column(bytes: &mut [u8], offset: usize, data: &ColumnData) {
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

pub(super) fn hash_state(
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

pub(super) fn global_table(model: &ValidatedModel, box_index: usize, table_index: usize) -> usize {
    model.model().boxes[..box_index]
        .iter()
        .map(|model_box| model_box.tables.len())
        .sum::<usize>()
        + table_index
}

pub(super) fn type_size(ty: &AttrType) -> usize {
    match ty {
        AttrType::Real | AttrType::Int => 8,
        AttrType::Enum { .. } => 2,
        AttrType::Ref { .. } => 4,
    }
}

pub(super) fn align8(value: usize) -> usize {
    (value + 7) & !7
}
