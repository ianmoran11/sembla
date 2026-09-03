use super::*;

pub(super) fn update_state_tables(hash: &mut Sha256, tables: &[TableState], include_box: bool) {
    update_u64(hash, tables.len());
    for table in tables {
        if include_box {
            update_string(hash, &table.box_name);
        }
        update_string(hash, &table.name);
        update_u64(hash, table.row_count);
        update_u64(hash, table.columns.len());
        for column in &table.columns {
            update_state_column(hash, column);
        }
    }
}

pub(super) fn update_state_column(hash: &mut Sha256, column: &ColumnState) {
    update_string(hash, column.name());
    match column {
        ColumnState::Real { values, .. } => {
            update_values(hash, 0, values, |value| value.to_bits().to_le_bytes())
        }
        ColumnState::Int { values, .. } => {
            update_values(hash, 1, values, |value| value.to_le_bytes())
        }
        ColumnState::Enum { values, .. } => {
            update_values(hash, 2, values, |value| value.to_le_bytes())
        }
        ColumnState::Ref { values, .. } => {
            update_values(hash, 3, values, |value| value.to_le_bytes())
        }
    }
}

pub(super) fn update_column_data(hash: &mut Sha256, column: &ColumnData) {
    match column {
        ColumnData::Real(values) => {
            update_values(hash, 0, values, |value| value.to_bits().to_le_bytes())
        }
        ColumnData::Int(values) => update_values(hash, 1, values, |value| value.to_le_bytes()),
        ColumnData::Enum(values) => update_values(hash, 2, values, |value| value.to_le_bytes()),
        ColumnData::Ref(values) => update_values(hash, 3, values, |value| value.to_le_bytes()),
    }
}

fn update_values<T, const WIDTH: usize>(
    hash: &mut Sha256,
    tag: u8,
    values: &[T],
    encode: impl Fn(&T) -> [u8; WIDTH],
) {
    hash.update([tag]);
    update_u64(hash, values.len());
    for value in values {
        hash.update(encode(value));
    }
}

pub(super) fn update_u64(hash: &mut Sha256, value: usize) {
    hash.update((value as u64).to_le_bytes());
}

pub(super) fn update_string(hash: &mut Sha256, value: &str) {
    update_u64(hash, value.len());
    hash.update(value.as_bytes());
}
