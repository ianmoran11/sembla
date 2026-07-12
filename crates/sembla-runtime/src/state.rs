//! Ordered, fixed-population columnar state with read-old/write-new buffering.

use std::error::Error;
use std::fmt;

use sembla_ir::{AttrType, ValidatedModel};
use sha2::{Digest, Sha256};

/// Initial values for one typed attribute column.
#[derive(Clone, Debug, PartialEq)]
pub enum ColumnData {
    Real(Vec<f64>),
    Int(Vec<i64>),
    Enum(Vec<u16>),
    Ref(Vec<u32>),
}

impl ColumnData {
    fn len(&self) -> usize {
        match self {
            Self::Real(values) => values.len(),
            Self::Int(values) => values.len(),
            Self::Enum(values) => values.len(),
            Self::Ref(values) => values.len(),
        }
    }

    fn kind_name(&self) -> &'static str {
        match self {
            Self::Real(_) => "Real",
            Self::Int(_) => "Int",
            Self::Enum(_) => "Enum",
            Self::Ref(_) => "Ref",
        }
    }
}

/// Initial data for one named attribute.
#[derive(Clone, Debug, PartialEq)]
pub struct ColumnInit {
    pub name: String,
    pub data: ColumnData,
}

impl ColumnInit {
    pub fn new(name: impl Into<String>, data: ColumnData) -> Self {
        Self {
            name: name.into(),
            data,
        }
    }
}

/// Fixed row count and initial columns for one box-qualified table.
#[derive(Clone, Debug, PartialEq)]
pub struct TableInit {
    pub box_name: String,
    pub table_name: String,
    pub row_count: usize,
    pub columns: Vec<ColumnInit>,
}

impl TableInit {
    pub fn new(
        box_name: impl Into<String>,
        table_name: impl Into<String>,
        row_count: usize,
        columns: Vec<ColumnInit>,
    ) -> Self {
        Self {
            box_name: box_name.into(),
            table_name: table_name.into(),
            row_count,
            columns,
        }
    }
}

/// A deterministic state construction or access failure.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StateError {
    message: String,
}

impl StateError {
    fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }
}

impl fmt::Display for StateError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.message)
    }
}

impl Error for StateError {}

#[derive(Clone, Debug, PartialEq)]
struct StateData {
    tables: Vec<TableState>,
}

#[derive(Clone, Debug, PartialEq)]
struct TableState {
    box_name: String,
    name: String,
    row_count: usize,
    columns: Vec<ColumnState>,
}

#[derive(Clone, Debug, PartialEq)]
enum ColumnState {
    Real {
        name: String,
        values: Vec<f64>,
    },
    Int {
        name: String,
        values: Vec<i64>,
    },
    Enum {
        name: String,
        variant_count: usize,
        values: Vec<u16>,
    },
    Ref {
        name: String,
        target_table: usize,
        values: Vec<u32>,
    },
}

impl ColumnState {
    fn name(&self) -> &str {
        match self {
            Self::Real { name, .. }
            | Self::Int { name, .. }
            | Self::Enum { name, .. }
            | Self::Ref { name, .. } => name,
        }
    }

    fn kind_name(&self) -> &'static str {
        match self {
            Self::Real { .. } => "Real",
            Self::Int { .. } => "Int",
            Self::Enum { .. } => "Enum",
            Self::Ref { .. } => "Ref",
        }
    }
}

/// A fixed-population, double-buffered state store.
///
/// Tables are retained in box-major IR declaration order and columns in
/// attribute declaration order. Names are resolved by deterministic linear
/// search; mutable vectors are not exposed, so populations cannot grow or
/// shrink after construction.
#[derive(Clone, Debug)]
pub struct StateStore {
    current: StateData,
    next: StateData,
    write_prepared: bool,
}

impl StateStore {
    /// Builds state from a validated IR schema and box-qualified initial data.
    pub fn new(model: &ValidatedModel, initial_tables: Vec<TableInit>) -> Result<Self, StateError> {
        validate_table_initializers(model, &initial_tables)?;

        let mut tables = Vec::new();
        for model_box in &model.model().boxes {
            let box_table_base = tables.len();
            for table in &model_box.tables {
                let initial = find_table_init(&initial_tables, &model_box.name, &table.name)
                    .ok_or_else(|| {
                        StateError::new(format!(
                            "box '{}', table '{}': missing initial data",
                            model_box.name, table.name
                        ))
                    })?;
                let mut columns = Vec::with_capacity(table.attrs.len());
                for attr in &table.attrs {
                    let initial_column = find_column_init(&initial.columns, &attr.name)
                        .ok_or_else(|| {
                            StateError::new(format!(
                                "box '{}', table '{}', column '{}': missing initial data",
                                model_box.name, table.name, attr.name
                            ))
                        })?;
                    columns.push(build_column(
                        model,
                        &initial_tables,
                        box_table_base,
                        &model_box.name,
                        &table.name,
                        attr,
                        initial_column,
                    )?);
                }
                tables.push(TableState {
                    box_name: model_box.name.clone(),
                    name: table.name.clone(),
                    row_count: initial.row_count,
                    columns,
                });
            }
        }

        let current = StateData { tables };
        Ok(Self {
            next: current.clone(),
            current,
            write_prepared: false,
        })
    }

    /// Returns a read-only view of the committed tick-start state.
    pub fn snapshot(&self) -> Snapshot<'_> {
        Snapshot {
            state: &self.current,
        }
    }

    /// Eagerly copies old state and returns simultaneous read-old/write-new views.
    ///
    /// A second write buffer cannot be prepared until [`Self::commit`] swaps
    /// the current and next buffers.
    pub fn buffers(&mut self) -> Result<(Snapshot<'_>, WriteBuffer<'_>), StateError> {
        self.prepare_next()?;
        Ok((
            Snapshot {
                state: &self.current,
            },
            WriteBuffer {
                state: &mut self.next,
            },
        ))
    }

    /// Eagerly copies old state and returns the new-state write buffer.
    pub fn write_buffer(&mut self) -> Result<WriteBuffer<'_>, StateError> {
        self.prepare_next()?;
        Ok(WriteBuffer {
            state: &mut self.next,
        })
    }

    /// Makes the prepared write buffer the next committed snapshot.
    pub fn commit(&mut self) -> Result<(), StateError> {
        if !self.write_prepared {
            return Err(StateError::new(
                "cannot commit state: no write buffer has been prepared",
            ));
        }
        std::mem::swap(&mut self.current, &mut self.next);
        self.write_prepared = false;
        Ok(())
    }

    /// Returns the canonical hash of the currently committed state.
    pub fn state_hash(&self) -> [u8; 32] {
        self.snapshot().state_hash()
    }

    fn prepare_next(&mut self) -> Result<(), StateError> {
        if self.write_prepared {
            return Err(StateError::new(
                "cannot prepare state writes: a write buffer is already pending",
            ));
        }
        self.next.clone_from(&self.current);
        self.write_prepared = true;
        Ok(())
    }

    /// Abandons an executor-prepared next buffer after a staged write fails.
    pub(crate) fn discard_writes(&mut self) {
        self.write_prepared = false;
    }
}

/// A read-only view of one committed tick-start state.
#[derive(Clone, Copy, Debug)]
pub struct Snapshot<'a> {
    state: &'a StateData,
}

impl Snapshot<'_> {
    pub fn row_count(&self, box_name: &str, table_name: &str) -> Result<usize, StateError> {
        Ok(find_table(self.state, box_name, table_name)?.row_count)
    }

    pub fn real(
        &self,
        box_name: &str,
        table_name: &str,
        column_name: &str,
        row: usize,
    ) -> Result<f64, StateError> {
        let (table, column) = find_cell(self.state, box_name, table_name, column_name, row)?;
        match column {
            ColumnState::Real { values, .. } => Ok(values[row]),
            _ => Err(wrong_column_type(table, column, "Real")),
        }
    }

    pub fn int(
        &self,
        box_name: &str,
        table_name: &str,
        column_name: &str,
        row: usize,
    ) -> Result<i64, StateError> {
        let (table, column) = find_cell(self.state, box_name, table_name, column_name, row)?;
        match column {
            ColumnState::Int { values, .. } => Ok(values[row]),
            _ => Err(wrong_column_type(table, column, "Int")),
        }
    }

    pub fn enum_index(
        &self,
        box_name: &str,
        table_name: &str,
        column_name: &str,
        row: usize,
    ) -> Result<u16, StateError> {
        let (table, column) = find_cell(self.state, box_name, table_name, column_name, row)?;
        match column {
            ColumnState::Enum { values, .. } => Ok(values[row]),
            _ => Err(wrong_column_type(table, column, "Enum")),
        }
    }

    pub fn reference(
        &self,
        box_name: &str,
        table_name: &str,
        column_name: &str,
        row: usize,
    ) -> Result<u32, StateError> {
        let (table, column) = find_cell(self.state, box_name, table_name, column_name, row)?;
        match column {
            ColumnState::Ref { values, .. } => Ok(values[row]),
            _ => Err(wrong_column_type(table, column, "Ref")),
        }
    }

    /// Computes SHA-256 over the canonical state byte layout.
    ///
    /// The frozen serialization is, in order:
    ///
    /// 1. the domain bytes `SEMBLA_STATE_V1\0`;
    /// 2. the table count as `u64` little-endian;
    /// 3. each table in box-major IR declaration order, encoded as the box-name
    ///    UTF-8 byte length (`u64` little-endian), box-name bytes, table-name
    ///    length (`u64` little-endian), table-name bytes, row count (`u64`
    ///    little-endian), and column count (`u64` little-endian);
    /// 4. each column in attribute declaration order, encoded as its name length
    ///    (`u64` little-endian), name UTF-8 bytes, one type byte (`0` Real, `1`
    ///    Int, `2` Enum, `3` Ref), value count (`u64` little-endian), then values
    ///    in row order. Real values are their [`f64::to_bits`] as `u64`
    ///    little-endian (including signed zero and NaN payloads), Int values are
    ///    `i64` little-endian, Enum values are `u16` little-endian, and Ref values
    ///    are `u32` little-endian.
    ///
    /// Names are unnormalized UTF-8. Lengths, counts, names, type tags, and box
    /// qualification make the byte stream unambiguous. Pending same-tick writes
    /// are excluded because a snapshot always refers to committed old state.
    pub fn state_hash(&self) -> [u8; 32] {
        let mut hash = Sha256::new();
        hash.update(b"SEMBLA_STATE_V1\0");
        update_u64(&mut hash, self.state.tables.len());

        for table in &self.state.tables {
            update_string(&mut hash, &table.box_name);
            update_string(&mut hash, &table.name);
            update_u64(&mut hash, table.row_count);
            update_u64(&mut hash, table.columns.len());

            for column in &table.columns {
                update_string(&mut hash, column.name());
                match column {
                    ColumnState::Real { values, .. } => {
                        hash.update([0]);
                        update_u64(&mut hash, values.len());
                        for value in values {
                            hash.update(value.to_bits().to_le_bytes());
                        }
                    }
                    ColumnState::Int { values, .. } => {
                        hash.update([1]);
                        update_u64(&mut hash, values.len());
                        for value in values {
                            hash.update(value.to_le_bytes());
                        }
                    }
                    ColumnState::Enum { values, .. } => {
                        hash.update([2]);
                        update_u64(&mut hash, values.len());
                        for value in values {
                            hash.update(value.to_le_bytes());
                        }
                    }
                    ColumnState::Ref { values, .. } => {
                        hash.update([3]);
                        update_u64(&mut hash, values.len());
                        for value in values {
                            hash.update(value.to_le_bytes());
                        }
                    }
                }
            }
        }

        hash.finalize().into()
    }
}

/// A mutable new-state buffer that cannot affect its paired old snapshot.
#[derive(Debug)]
pub struct WriteBuffer<'a> {
    state: &'a mut StateData,
}

impl WriteBuffer<'_> {
    pub fn set_real(
        &mut self,
        box_name: &str,
        table_name: &str,
        column_name: &str,
        row: usize,
        value: f64,
    ) -> Result<(), StateError> {
        let (table_index, column_index) =
            locate_writable_cell(self.state, box_name, table_name, column_name, row, "Real")?;
        if let ColumnState::Real { values, .. } =
            &mut self.state.tables[table_index].columns[column_index]
        {
            values[row] = value;
            Ok(())
        } else {
            unreachable!("column type checked before mutable access")
        }
    }

    pub fn set_int(
        &mut self,
        box_name: &str,
        table_name: &str,
        column_name: &str,
        row: usize,
        value: i64,
    ) -> Result<(), StateError> {
        let (table_index, column_index) =
            locate_writable_cell(self.state, box_name, table_name, column_name, row, "Int")?;
        if let ColumnState::Int { values, .. } =
            &mut self.state.tables[table_index].columns[column_index]
        {
            values[row] = value;
            Ok(())
        } else {
            unreachable!("column type checked before mutable access")
        }
    }

    pub fn set_enum(
        &mut self,
        box_name: &str,
        table_name: &str,
        column_name: &str,
        row: usize,
        value: u16,
    ) -> Result<(), StateError> {
        let (table_index, column_index) =
            locate_writable_cell(self.state, box_name, table_name, column_name, row, "Enum")?;
        let variant_count = match &self.state.tables[table_index].columns[column_index] {
            ColumnState::Enum { variant_count, .. } => *variant_count,
            _ => unreachable!("column type checked before range validation"),
        };
        if usize::from(value) >= variant_count {
            return Err(cell_error(
                &self.state.tables[table_index],
                column_name,
                row,
                format!("enum index {value} is out of bounds for {variant_count} variants"),
            ));
        }
        if let ColumnState::Enum { values, .. } =
            &mut self.state.tables[table_index].columns[column_index]
        {
            values[row] = value;
            Ok(())
        } else {
            unreachable!("column type checked before mutable access")
        }
    }

    pub fn set_ref(
        &mut self,
        box_name: &str,
        table_name: &str,
        column_name: &str,
        row: usize,
        value: u32,
    ) -> Result<(), StateError> {
        let (table_index, column_index) =
            locate_writable_cell(self.state, box_name, table_name, column_name, row, "Ref")?;
        let target_table = match &self.state.tables[table_index].columns[column_index] {
            ColumnState::Ref { target_table, .. } => *target_table,
            _ => unreachable!("column type checked before range validation"),
        };
        let target = &self.state.tables[target_table];
        if value as usize >= target.row_count {
            return Err(cell_error(
                &self.state.tables[table_index],
                column_name,
                row,
                format!(
                    "reference index {value} is out of bounds for target table '{}' with {} rows",
                    target.name, target.row_count
                ),
            ));
        }
        if let ColumnState::Ref { values, .. } =
            &mut self.state.tables[table_index].columns[column_index]
        {
            values[row] = value;
            Ok(())
        } else {
            unreachable!("column type checked before mutable access")
        }
    }
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

fn build_column(
    model: &ValidatedModel,
    initial_tables: &[TableInit],
    box_table_base: usize,
    box_name: &str,
    table_name: &str,
    attr: &sembla_ir::Attr,
    initial: &ColumnInit,
) -> Result<ColumnState, StateError> {
    let type_error = || {
        StateError::new(format!(
            "box '{box_name}', table '{table_name}', column '{}': expected {}, found {}",
            attr.name,
            attr_type_name(&attr.ty),
            initial.data.kind_name()
        ))
    };

    match (&attr.ty, &initial.data) {
        (AttrType::Real, ColumnData::Real(values)) => Ok(ColumnState::Real {
            name: attr.name.clone(),
            values: values.clone(),
        }),
        (AttrType::Int, ColumnData::Int(values)) => Ok(ColumnState::Int {
            name: attr.name.clone(),
            values: values.clone(),
        }),
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
            Ok(ColumnState::Enum {
                name: attr.name.clone(),
                variant_count: variants.len(),
                values: values.clone(),
            })
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
            let target_offset = model_box
                .tables
                .iter()
                .position(|table| table.name == *target)
                .ok_or_else(|| {
                    StateError::new(format!(
                        "box '{box_name}', table '{table_name}', column '{}': unknown target table '{target}'",
                        attr.name
                    ))
                })?;
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
            Ok(ColumnState::Ref {
                name: attr.name.clone(),
                target_table: box_table_base + target_offset,
                values: values.clone(),
            })
        }
        _ => Err(type_error()),
    }
}

fn find_table_init<'a>(
    initial_tables: &'a [TableInit],
    box_name: &str,
    table_name: &str,
) -> Option<&'a TableInit> {
    initial_tables
        .iter()
        .find(|table| table.box_name == box_name && table.table_name == table_name)
}

fn find_column_init<'a>(columns: &'a [ColumnInit], name: &str) -> Option<&'a ColumnInit> {
    columns.iter().find(|column| column.name == name)
}

fn find_table<'a>(
    state: &'a StateData,
    box_name: &str,
    table_name: &str,
) -> Result<&'a TableState, StateError> {
    state
        .tables
        .iter()
        .find(|table| table.box_name == box_name && table.name == table_name)
        .ok_or_else(|| {
            StateError::new(format!(
                "box '{box_name}', table '{table_name}': no such state table"
            ))
        })
}

fn find_cell<'a>(
    state: &'a StateData,
    box_name: &str,
    table_name: &str,
    column_name: &str,
    row: usize,
) -> Result<(&'a TableState, &'a ColumnState), StateError> {
    let table = find_table(state, box_name, table_name)?;
    let column = table
        .columns
        .iter()
        .find(|column| column.name() == column_name)
        .ok_or_else(|| {
            StateError::new(format!(
                "box '{box_name}', table '{table_name}', column '{column_name}': no such state column"
            ))
        })?;
    if row >= table.row_count {
        return Err(cell_error(
            table,
            column_name,
            row,
            format!("row index is out of bounds for {} rows", table.row_count),
        ));
    }
    Ok((table, column))
}

fn locate_writable_cell(
    state: &StateData,
    box_name: &str,
    table_name: &str,
    column_name: &str,
    row: usize,
    expected_type: &str,
) -> Result<(usize, usize), StateError> {
    let table_index = state
        .tables
        .iter()
        .position(|table| table.box_name == box_name && table.name == table_name)
        .ok_or_else(|| {
            StateError::new(format!(
                "box '{box_name}', table '{table_name}': no such state table"
            ))
        })?;
    let table = &state.tables[table_index];
    let column_index = table
        .columns
        .iter()
        .position(|column| column.name() == column_name)
        .ok_or_else(|| {
            StateError::new(format!(
                "box '{box_name}', table '{table_name}', column '{column_name}': no such state column"
            ))
        })?;
    let column = &table.columns[column_index];
    if row >= table.row_count {
        return Err(cell_error(
            table,
            column_name,
            row,
            format!("row index is out of bounds for {} rows", table.row_count),
        ));
    }
    if column.kind_name() != expected_type {
        return Err(wrong_column_type(table, column, expected_type));
    }
    Ok((table_index, column_index))
}

fn wrong_column_type(table: &TableState, column: &ColumnState, expected_type: &str) -> StateError {
    StateError::new(format!(
        "box '{}', table '{}', column '{}': expected {expected_type}, found {}",
        table.box_name,
        table.name,
        column.name(),
        column.kind_name()
    ))
}

fn cell_error(
    table: &TableState,
    column_name: &str,
    row: usize,
    message: impl fmt::Display,
) -> StateError {
    StateError::new(format!(
        "box '{}', table '{}', column '{column_name}', row {row}: {message}",
        table.box_name, table.name
    ))
}

fn attr_type_name(attr_type: &AttrType) -> &'static str {
    match attr_type {
        AttrType::Real => "Real",
        AttrType::Int => "Int",
        AttrType::Enum { .. } => "Enum",
        AttrType::Ref { .. } => "Ref",
    }
}

fn update_u64(hash: &mut Sha256, value: usize) {
    hash.update((value as u64).to_le_bytes());
}

fn update_string(hash: &mut Sha256, value: &str) {
    update_u64(hash, value.len());
    hash.update(value.as_bytes());
}
