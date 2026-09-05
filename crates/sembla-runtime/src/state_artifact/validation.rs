use super::*;

pub(super) fn validate_header_structure(header: &Header) -> Result<(), StateArtifactError> {
    let mut tables = HashSet::new();
    for table in &header.tables {
        if !tables.insert((table.box_name.as_str(), table.table.as_str())) {
            return Err(StateArtifactError::DuplicateTable {
                box_name: table.box_name.clone(),
                table: table.table.clone(),
            });
        }
        let mut columns = HashSet::new();
        for column in &table.columns {
            if !columns.insert(column.name.as_str()) {
                return Err(StateArtifactError::DuplicateColumn {
                    box_name: table.box_name.clone(),
                    table: table.table.clone(),
                    column: column.name.clone(),
                });
            }
            let metadata_valid = match column.column_type {
                ColumnType::Enum => column.variant_count.is_some() && column.ref_target.is_none(),
                ColumnType::Ref => column.variant_count.is_none() && column.ref_target.is_some(),
                ColumnType::Real | ColumnType::Int => {
                    column.variant_count.is_none() && column.ref_target.is_none()
                }
            };
            if !metadata_valid {
                return Err(StateArtifactError::InvalidColumnMetadata {
                    box_name: table.box_name.clone(),
                    table: table.table.clone(),
                    column: column.name.clone(),
                    detail: format!(
                        "type '{}' requires variant_count iff enum and ref_target iff ref",
                        column.column_type.name()
                    ),
                });
            }
            if let Some(target) = &column.ref_target {
                if !header.tables.iter().any(|candidate| {
                    candidate.box_name == target.box_name && candidate.table == target.table
                }) {
                    return Err(StateArtifactError::UnresolvedRefTarget {
                        box_name: table.box_name.clone(),
                        table: table.table.clone(),
                        column: column.name.clone(),
                        target_box: target.box_name.clone(),
                        target_table: target.table.clone(),
                    });
                }
            }
        }
    }
    Ok(())
}

pub(super) fn expected_file_len(
    header: &Header,
    header_end: usize,
) -> Result<usize, StateArtifactError> {
    let mut expected = header_end;
    for table in &header.tables {
        let rows =
            usize::try_from(table.row_count).map_err(|_| StateArtifactError::RowCountTooLarge {
                box_name: table.box_name.clone(),
                table: table.table.clone(),
                row_count: table.row_count,
            })?;
        for column in &table.columns {
            let bytes = rows
                .checked_mul(column.column_type.width())
                .ok_or_else(|| StateArtifactError::PayloadSizeOverflow {
                    box_name: table.box_name.clone(),
                    table: table.table.clone(),
                    column: column.name.clone(),
                })?;
            expected = expected.checked_add(bytes).ok_or_else(|| {
                StateArtifactError::PayloadSizeOverflow {
                    box_name: table.box_name.clone(),
                    table: table.table.clone(),
                    column: column.name.clone(),
                }
            })?;
        }
    }
    Ok(expected)
}

pub(super) fn header_for_model(model: &ValidatedModel) -> Result<Header, StateArtifactError> {
    let mut tables = Vec::new();
    for model_box in &model.model().boxes {
        for table in &model_box.tables {
            tables.push(TableHeader {
                box_name: model_box.name.clone(),
                table: table.name.clone(),
                row_count: table.size_hint,
                columns: table
                    .attrs
                    .iter()
                    .map(|attr| column_header_for_attr(&model_box.name, attr))
                    .collect(),
            });
        }
    }
    let header = Header {
        schema_version: STATE_ARTIFACT_SCHEMA.to_owned(),
        tables,
    };
    validate_header_structure(&header)?;
    Ok(header)
}

pub(super) fn validate_writer_inputs(
    model: &ValidatedModel,
    inputs: &[TableInit],
) -> Result<(), StateArtifactError> {
    let mut seen = HashSet::new();
    for table in inputs {
        if !seen.insert((table.box_name.as_str(), table.table_name.as_str())) {
            return Err(StateArtifactError::DuplicateTable {
                box_name: table.box_name.clone(),
                table: table.table_name.clone(),
            });
        }
    }
    // Validate all table row counts before any column values so a Ref source
    // cannot observe a target whose row-count contract has not yet passed.
    let mut input_row_counts = BTreeMap::new();
    for model_box in &model.model().boxes {
        for table in &model_box.tables {
            let input = inputs
                .iter()
                .find(|input| input.box_name == model_box.name && input.table_name == table.name)
                .ok_or_else(|| StateArtifactError::MissingTable {
                    box_name: model_box.name.clone(),
                    table: table.name.clone(),
                })?;
            let expected_rows = usize::try_from(table.size_hint).map_err(|_| {
                StateArtifactError::RowCountTooLarge {
                    box_name: model_box.name.clone(),
                    table: table.name.clone(),
                    row_count: table.size_hint,
                }
            })?;
            if input.row_count != expected_rows {
                return Err(StateArtifactError::RowCountMismatch {
                    box_name: model_box.name.clone(),
                    table: table.name.clone(),
                    expected: expected_rows,
                    actual: input.row_count,
                });
            }
            input_row_counts.insert(
                (input.box_name.clone(), input.table_name.clone()),
                input.row_count,
            );
        }
    }

    for model_box in &model.model().boxes {
        for table in &model_box.tables {
            let input = inputs
                .iter()
                .find(|input| input.box_name == model_box.name && input.table_name == table.name)
                .ok_or_else(|| StateArtifactError::MissingTable {
                    box_name: model_box.name.clone(),
                    table: table.name.clone(),
                })?;
            let mut column_seen = HashSet::new();
            for column in &input.columns {
                if !column_seen.insert(column.name.as_str()) {
                    return Err(StateArtifactError::DuplicateColumn {
                        box_name: model_box.name.clone(),
                        table: table.name.clone(),
                        column: column.name.clone(),
                    });
                }
            }
            for attr in &table.attrs {
                let column = input
                    .columns
                    .iter()
                    .find(|column| column.name == attr.name)
                    .ok_or_else(|| StateArtifactError::MissingColumn {
                        box_name: model_box.name.clone(),
                        table: table.name.clone(),
                        column: attr.name.clone(),
                    })?;
                let header = column_header_for_attr(&model_box.name, attr);
                let ref_target_rows = header.ref_target.as_ref().and_then(|target| {
                    input_row_counts
                        .get(&(target.box_name.clone(), target.table.clone()))
                        .copied()
                });
                validate_column(
                    &model_box.name,
                    &table.name,
                    attr,
                    &header,
                    &column.data,
                    input.row_count,
                    ref_target_rows,
                )?;
            }
            for column in &input.columns {
                if !table.attrs.iter().any(|attr| attr.name == column.name) {
                    return Err(StateArtifactError::ExtraColumn {
                        box_name: model_box.name.clone(),
                        table: table.name.clone(),
                        column: column.name.clone(),
                    });
                }
            }
        }
    }
    for input in inputs {
        if !model.model().boxes.iter().any(|model_box| {
            model_box.name == input.box_name
                && model_box
                    .tables
                    .iter()
                    .any(|table| table.name == input.table_name)
        }) {
            return Err(StateArtifactError::ExtraTable {
                box_name: input.box_name.clone(),
                table: input.table_name.clone(),
            });
        }
    }
    Ok(())
}

fn column_header_for_attr(box_name: &str, attr: &sembla_ir::Attr) -> ColumnHeader {
    let (column_type, variant_count, ref_target) = match &attr.ty {
        AttrType::Real => (ColumnType::Real, None, None),
        AttrType::Int => (ColumnType::Int, None, None),
        AttrType::Enum { variants } => (ColumnType::Enum, Some(variants.len() as u64), None),
        AttrType::Ref { table } => (
            ColumnType::Ref,
            None,
            Some(RefTarget {
                box_name: box_name.to_owned(),
                table: table.clone(),
            }),
        ),
    };
    ColumnHeader {
        name: attr.name.clone(),
        column_type,
        variant_count,
        ref_target,
    }
}

pub(super) fn validate_column(
    box_name: &str,
    table_name: &str,
    attr: &sembla_ir::Attr,
    header: &ColumnHeader,
    data: &ColumnData,
    row_count: usize,
    ref_target_rows: Option<usize>,
) -> Result<(), StateArtifactError> {
    let actual_type = column_data_type(data);
    let expected_type = match &attr.ty {
        AttrType::Real => ColumnType::Real,
        AttrType::Int => ColumnType::Int,
        AttrType::Enum { .. } => ColumnType::Enum,
        AttrType::Ref { .. } => ColumnType::Ref,
    };
    if header.column_type != expected_type {
        return Err(StateArtifactError::ColumnTypeMismatch {
            box_name: box_name.to_owned(),
            table: table_name.to_owned(),
            column: attr.name.clone(),
            expected: expected_type.name().to_owned(),
            actual: header.column_type.name().to_owned(),
        });
    }
    if actual_type != expected_type {
        return Err(StateArtifactError::ColumnTypeMismatch {
            box_name: box_name.to_owned(),
            table: table_name.to_owned(),
            column: attr.name.clone(),
            expected: expected_type.name().to_owned(),
            actual: actual_type.name().to_owned(),
        });
    }
    let actual_len = data.len();
    if actual_len != row_count {
        return Err(StateArtifactError::ColumnLengthMismatch {
            box_name: box_name.to_owned(),
            table: table_name.to_owned(),
            column: attr.name.clone(),
            expected: row_count,
            actual: actual_len,
        });
    }
    match (&attr.ty, data) {
        (AttrType::Enum { variants }, ColumnData::Enum(values)) => {
            let actual = header.variant_count.expect("validated enum metadata");
            if actual != variants.len() as u64 {
                return Err(StateArtifactError::VariantCountMismatch {
                    box_name: box_name.to_owned(),
                    table: table_name.to_owned(),
                    column: attr.name.clone(),
                    expected: variants.len(),
                    actual,
                });
            }
            if let Some((row, &value)) = values
                .iter()
                .enumerate()
                .find(|(_, value)| u64::from(**value) >= actual)
            {
                return Err(StateArtifactError::EnumValueOutOfRange {
                    box_name: box_name.to_owned(),
                    table: table_name.to_owned(),
                    column: attr.name.clone(),
                    row,
                    value,
                    variant_count: actual,
                });
            }
        }
        (
            AttrType::Ref {
                table: target_table,
            },
            ColumnData::Ref(values),
        ) => {
            let target = header.ref_target.as_ref().ok_or_else(|| {
                StateArtifactError::InvalidColumnMetadata {
                    box_name: box_name.to_owned(),
                    table: table_name.to_owned(),
                    column: attr.name.clone(),
                    detail: "ref column requires ref_target".to_owned(),
                }
            })?;
            let expected = qualified(box_name, target_table);
            let actual = qualified(&target.box_name, &target.table);
            if actual != expected {
                return Err(StateArtifactError::RefTargetMismatch {
                    box_name: box_name.to_owned(),
                    table: table_name.to_owned(),
                    column: attr.name.clone(),
                    expected,
                    actual,
                });
            }
            let target_rows =
                ref_target_rows.ok_or_else(|| StateArtifactError::UnresolvedRefTarget {
                    box_name: box_name.to_owned(),
                    table: table_name.to_owned(),
                    column: attr.name.clone(),
                    target_box: target.box_name.clone(),
                    target_table: target.table.clone(),
                })?;
            if let Some((row, &value)) = values.iter().enumerate().find(|(_, value)| {
                usize::try_from(**value).map_or(true, |value| value >= target_rows)
            }) {
                return Err(StateArtifactError::RefValueOutOfRange {
                    box_name: box_name.to_owned(),
                    table: table_name.to_owned(),
                    column: attr.name.clone(),
                    row,
                    value,
                    target: qualified(box_name, target_table),
                    target_rows,
                });
            }
        }
        _ => {}
    }
    Ok(())
}

pub(super) fn qualified(box_name: &str, table: &str) -> String {
    format!("{box_name}.{table}")
}

fn column_data_type(data: &ColumnData) -> ColumnType {
    match data {
        ColumnData::Real(_) => ColumnType::Real,
        ColumnData::Int(_) => ColumnType::Int,
        ColumnData::Enum(_) => ColumnType::Enum,
        ColumnData::Ref(_) => ColumnType::Ref,
    }
}
