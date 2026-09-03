use super::*;

pub(super) fn validate_state_initializers(
    model: &ValidatedModel,
    initial_tables: &[TableInit],
) -> Result<(), StateError> {
    validate_table_initializers(model, initial_tables)?;
    for model_box in &model.model().boxes {
        for table in &model_box.tables {
            let initial = find_table_init(initial_tables, &model_box.name, &table.name)
                .ok_or_else(|| {
                    StateError::new(format!(
                        "box '{}', table '{}': missing initial data",
                        model_box.name, table.name
                    ))
                })?;
            for attr in &table.attrs {
                let column = find_column_init(&initial.columns, &attr.name).ok_or_else(|| {
                    StateError::new(format!(
                        "box '{}', table '{}', column '{}': missing initial data",
                        model_box.name, table.name, attr.name
                    ))
                })?;
                validate_column_initializer_value(
                    model,
                    initial_tables,
                    &model_box.name,
                    &table.name,
                    attr,
                    column,
                )?;
            }
        }
    }
    Ok(())
}

fn validate_table_initializers(
    model: &ValidatedModel,
    initial_tables: &[TableInit],
) -> Result<(), StateError> {
    for (index, initial) in initial_tables.iter().enumerate() {
        if initial_tables[..index].iter().any(|previous| {
            previous.box_name == initial.box_name && previous.table_name == initial.table_name
        }) {
            return Err(StateError::new(format!(
                "box '{}', table '{}': duplicate initial data",
                initial.box_name, initial.table_name
            )));
        }
        let schema_table = model
            .model()
            .boxes
            .iter()
            .find(|model_box| model_box.name == initial.box_name)
            .and_then(|model_box| {
                model_box
                    .tables
                    .iter()
                    .find(|table| table.name == initial.table_name)
            })
            .ok_or_else(|| {
                StateError::new(format!(
                    "box '{}', table '{}': no such table in model",
                    initial.box_name, initial.table_name
                ))
            })?;
        validate_column_initializers(initial)?;
        for column in &initial.columns {
            if !schema_table
                .attrs
                .iter()
                .any(|attr| attr.name == column.name)
            {
                return Err(StateError::new(format!(
                    "box '{}', table '{}', column '{}': no such column in model",
                    initial.box_name, initial.table_name, column.name
                )));
            }
        }
    }

    for model_box in &model.model().boxes {
        for table in &model_box.tables {
            if find_table_init(initial_tables, &model_box.name, &table.name).is_none() {
                return Err(StateError::new(format!(
                    "box '{}', table '{}': missing initial data",
                    model_box.name, table.name
                )));
            }
        }
    }
    Ok(())
}

fn validate_column_initializers(initial: &TableInit) -> Result<(), StateError> {
    for (index, column) in initial.columns.iter().enumerate() {
        if initial.columns[..index]
            .iter()
            .any(|previous| previous.name == column.name)
        {
            return Err(StateError::new(format!(
                "box '{}', table '{}', column '{}': duplicate initial data",
                initial.box_name, initial.table_name, column.name
            )));
        }
        if column.data.len() != initial.row_count {
            return Err(StateError::new(format!(
                "box '{}', table '{}', column '{}': expected {} rows, found {}",
                initial.box_name,
                initial.table_name,
                column.name,
                initial.row_count,
                column.data.len()
            )));
        }
    }
    Ok(())
}

fn validate_column_initializer_value(
    model: &ValidatedModel,
    initial_tables: &[TableInit],
    box_name: &str,
    table_name: &str,
    attr: &sembla_ir::Attr,
    initial: &ColumnInit,
) -> Result<(), StateError> {
    let type_error = || {
        StateError::new(format!(
            "box '{box_name}', table '{table_name}', column '{}': expected {}, found {}",
            attr.name,
            attr_type_name(&attr.ty),
            initial.data.kind_name()
        ))
    };

    match (&attr.ty, &initial.data) {
        (AttrType::Real, ColumnData::Real(_)) | (AttrType::Int, ColumnData::Int(_)) => Ok(()),
        (AttrType::Enum { variants }, ColumnData::Enum(values)) => {
            for (row, value) in values.iter().copied().enumerate() {
                if usize::from(value) >= variants.len() {
                    return Err(StateError::new(format!(
                        "box '{box_name}', table '{table_name}', column '{}', row {row}: enum index {value} is out of bounds for {} variants",
                        attr.name,
                        variants.len()
                    )));
                }
            }
            Ok(())
        }
        (AttrType::Ref { table: target }, ColumnData::Ref(values)) => {
            let model_box = model
                .model()
                .boxes
                .iter()
                .find(|model_box| model_box.name == box_name)
                .ok_or_else(|| {
                    StateError::new(format!("box '{box_name}': no such box in model"))
                })?;
            if !model_box.tables.iter().any(|table| table.name == *target) {
                return Err(StateError::new(format!(
                    "box '{box_name}', table '{table_name}', column '{}': unknown target table '{target}'",
                    attr.name
                )));
            }
            let target_initial =
                find_table_init(initial_tables, box_name, target).ok_or_else(|| {
                    StateError::new(format!(
                        "box '{box_name}', table '{target}': missing initial data"
                    ))
                })?;
            for (row, value) in values.iter().copied().enumerate() {
                if value as usize >= target_initial.row_count {
                    return Err(StateError::new(format!(
                        "box '{box_name}', table '{table_name}', column '{}', row {row}: reference index {value} is out of bounds for target table '{target}' with {} rows",
                        attr.name, target_initial.row_count
                    )));
                }
            }
            Ok(())
        }
        _ => Err(type_error()),
    }
}

pub(super) fn build_validated_column(
    model: &ValidatedModel,
    box_table_base: usize,
    box_name: &str,
    attr: &sembla_ir::Attr,
    initial: &ColumnInit,
) -> ColumnState {
    match (&attr.ty, &initial.data) {
        (AttrType::Real, ColumnData::Real(values)) => ColumnState::Real {
            name: attr.name.clone(),
            values: values.clone(),
        },
        (AttrType::Int, ColumnData::Int(values)) => ColumnState::Int {
            name: attr.name.clone(),
            values: values.clone(),
        },
        (AttrType::Enum { variants }, ColumnData::Enum(values)) => ColumnState::Enum {
            name: attr.name.clone(),
            variant_count: variants.len(),
            values: values.clone(),
        },
        (AttrType::Ref { table: target }, ColumnData::Ref(values)) => {
            let model_box = model
                .model()
                .boxes
                .iter()
                .find(|model_box| model_box.name == box_name)
                .expect("validated state box disappeared");
            let target_offset = model_box
                .tables
                .iter()
                .position(|table| table.name == *target)
                .expect("validated reference target disappeared");
            ColumnState::Ref {
                name: attr.name.clone(),
                target_table: box_table_base + target_offset,
                values: values.clone(),
            }
        }
        _ => unreachable!("validated state column type changed"),
    }
}
