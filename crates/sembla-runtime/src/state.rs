//! Ordered, fixed-population columnar state with read-old/write-new buffering.

use std::error::Error;
use std::fmt;

use sembla_ir::{Attr, AttrType, ValidatedModel};
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

/// One owned table delivered to a box input port for the current tick.
///
/// Values are copied when wires are delivered; no column aliases model state or
/// another port.
#[derive(Clone, Debug, PartialEq)]
pub struct InputTable {
    pub box_name: String,
    pub port_name: String,
    pub schema: Vec<Attr>,
    pub row_count: usize,
    pub columns: Vec<ColumnData>,
}

impl InputTable {
    pub(crate) fn empty(box_name: &str, port_name: &str, schema: &[Attr]) -> Self {
        Self {
            box_name: box_name.to_owned(),
            port_name: port_name.to_owned(),
            schema: schema.to_vec(),
            row_count: 0,
            columns: schema.iter().map(|attr| empty_column(&attr.ty)).collect(),
        }
    }

    pub fn column(&self, name: &str) -> Option<&ColumnData> {
        self.schema
            .iter()
            .position(|attr| attr.name == name)
            .and_then(|index| self.columns.get(index))
    }
}

fn empty_column(ty: &AttrType) -> ColumnData {
    match ty {
        AttrType::Real => ColumnData::Real(Vec::new()),
        AttrType::Int => ColumnData::Int(Vec::new()),
        AttrType::Enum { .. } => ColumnData::Enum(Vec::new()),
        AttrType::Ref { .. } => ColumnData::Ref(Vec::new()),
    }
}

fn column_matches_type(column: &ColumnData, ty: &AttrType) -> bool {
    matches!(
        (column, ty),
        (ColumnData::Real(_), AttrType::Real)
            | (ColumnData::Int(_), AttrType::Int)
            | (ColumnData::Enum(_), AttrType::Enum { .. })
            | (ColumnData::Ref(_), AttrType::Ref { .. })
    )
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
    inputs: Vec<InputTable>,
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
        let inputs = model
            .model()
            .boxes
            .iter()
            .flat_map(|model_box| {
                model_box
                    .inputs
                    .iter()
                    .map(|input| InputTable::empty(&model_box.name, &input.name, &input.schema))
            })
            .collect();
        Ok(Self {
            next: current.clone(),
            current,
            inputs,
            write_prepared: false,
        })
    }

    /// Returns a read-only view of the committed tick-start state.
    pub fn snapshot(&self) -> Snapshot<'_> {
        Snapshot {
            state: &self.current,
            inputs: &self.inputs,
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
                inputs: &self.inputs,
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

    /// Returns a read-only view of the fully prepared prospective state.
    ///
    /// Executors use this to build fallible Moore-machine outputs before making
    /// either state writes or newly delivered inputs observable.
    pub(crate) fn prepared_snapshot(&self) -> Result<Snapshot<'_>, StateError> {
        if !self.write_prepared {
            return Err(StateError::new(
                "cannot snapshot prepared state: no write buffer has been prepared",
            ));
        }
        Ok(Snapshot {
            state: &self.next,
            inputs: &self.inputs,
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

    /// Returns the canonical hash of committed state and in-flight input tables.
    pub fn state_hash(&self) -> [u8; 32] {
        self.snapshot().state_hash()
    }

    pub(crate) fn replace_inputs(&mut self, inputs: Vec<InputTable>) {
        self.inputs = inputs;
    }

    /// Replaces input tables while reconstructing a read-only snapshot from an
    /// external execution backend. The schema and declaration order must match
    /// the validated model-derived inputs already owned by this store.
    pub fn replace_backend_inputs(&mut self, inputs: Vec<InputTable>) -> Result<(), StateError> {
        if inputs.len() != self.inputs.len() {
            return Err(StateError::new(format!(
                "backend snapshot supplied {} input tables, expected {}",
                inputs.len(),
                self.inputs.len()
            )));
        }
        for (expected, actual) in self.inputs.iter().zip(&inputs) {
            if expected.box_name != actual.box_name
                || expected.port_name != actual.port_name
                || expected.schema != actual.schema
                || actual.columns.len() != actual.schema.len()
                || actual
                    .columns
                    .iter()
                    .zip(&actual.schema)
                    .any(|(column, attr)| {
                        column.len() != actual.row_count
                            || !column_matches_type(column, &attr.ty)
                            || matches!(
                                (column, &attr.ty),
                                (ColumnData::Enum(values), AttrType::Enum { variants })
                                    if values.iter().any(|value| usize::from(*value) >= variants.len())
                            )
                            || matches!(
                                (column, &attr.ty),
                                (ColumnData::Ref(values), AttrType::Ref { table })
                                    if self.current.tables.iter()
                                        .find(|target| target.box_name == actual.box_name && target.name == *table)
                                        .map_or(true, |target| values.iter().any(|value| usize::try_from(*value).map_or(true, |value| value >= target.row_count)))
                            )
                    })
            {
                return Err(StateError::new(format!(
                    "backend input snapshot for '{}.{}' does not match the validated schema",
                    actual.box_name, actual.port_name
                )));
            }
        }
        self.inputs = inputs;
        Ok(())
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
    inputs: &'a [InputTable],
}

impl Snapshot<'_> {
    /// Returns the table delivered to `box_name.port_name` for this tick.
    /// Every declared input exists; all have zero rows at tick 0.
    pub fn input_table(&self, box_name: &str, port_name: &str) -> Result<&InputTable, StateError> {
        self.inputs
            .iter()
            .find(|table| table.box_name == box_name && table.port_name == port_name)
            .ok_or_else(|| StateError::new(format!("unknown input port '{box_name}.{port_name}'")))
    }

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

    /// Borrows a complete Enum column in canonical row order.
    pub fn enum_values(
        &self,
        box_name: &str,
        table_name: &str,
        column_name: &str,
    ) -> Result<&[u16], StateError> {
        let table = find_table(self.state, box_name, table_name)?;
        let column = table
            .columns
            .iter()
            .find(|column| column.name() == column_name)
            .ok_or_else(|| {
                StateError::new(format!(
                    "box '{box_name}', table '{table_name}': unknown column '{column_name}'"
                ))
            })?;
        match column {
            ColumnState::Enum { values, .. } => Ok(values),
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
    /// Models without input ports retain PRD 0004's frozen serialization, in order:
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
    ///
    /// Composition uses domain `SEMBLA_STATE_V2\0`, followed by the same
    /// box-major state encoding and then input tables in box/port declaration
    /// order. Each input encodes its box and port names, row and column counts,
    /// then named typed columns in schema order. Zero-row tick-0 inputs are
    /// therefore explicit and in-flight wire values affect the digest.
    pub fn state_hash(&self) -> [u8; 32] {
        let mut hash = Sha256::new();
        if self.inputs.is_empty() {
            // Preserve PRD 0004's frozen digest for models without composition.
            hash.update(b"SEMBLA_STATE_V1\0");
            update_state_tables(&mut hash, &self.state.tables, true);
        } else {
            hash.update(b"SEMBLA_STATE_V2\0");
            update_state_tables(&mut hash, &self.state.tables, true);
            update_u64(&mut hash, self.inputs.len());
            for input in self.inputs {
                update_string(&mut hash, &input.box_name);
                update_string(&mut hash, &input.port_name);
                update_u64(&mut hash, input.row_count);
                update_u64(&mut hash, input.schema.len());
                for (attr, column) in input.schema.iter().zip(&input.columns) {
                    update_string(&mut hash, &attr.name);
                    update_column_data(&mut hash, column);
                }
            }
        }
        hash.finalize().into()
    }

    /// Canonical hash of one table's values, independent of its containing box.
    pub fn table_hash(&self, box_name: &str, table_name: &str) -> Result<[u8; 32], StateError> {
        let table = find_table(self.state, box_name, table_name)?;
        let mut hash = Sha256::new();
        hash.update(b"SEMBLA_TABLE_V1\0");
        update_string(&mut hash, &table.name);
        update_u64(&mut hash, table.row_count);
        update_u64(&mut hash, table.columns.len());
        for column in &table.columns {
            update_state_column(&mut hash, column);
        }
        Ok(hash.finalize().into())
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

fn update_state_tables(hash: &mut Sha256, tables: &[TableState], include_box: bool) {
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

fn update_state_column(hash: &mut Sha256, column: &ColumnState) {
    update_string(hash, column.name());
    match column {
        ColumnState::Real { values, .. } => {
            update_column_data(hash, &ColumnData::Real(values.clone()))
        }
        ColumnState::Int { values, .. } => {
            update_column_data(hash, &ColumnData::Int(values.clone()))
        }
        ColumnState::Enum { values, .. } => {
            update_column_data(hash, &ColumnData::Enum(values.clone()))
        }
        ColumnState::Ref { values, .. } => {
            update_column_data(hash, &ColumnData::Ref(values.clone()))
        }
    }
}

fn update_column_data(hash: &mut Sha256, column: &ColumnData) {
    match column {
        ColumnData::Real(values) => {
            hash.update([0]);
            update_u64(hash, values.len());
            for value in values {
                hash.update(value.to_bits().to_le_bytes());
            }
        }
        ColumnData::Int(values) => {
            hash.update([1]);
            update_u64(hash, values.len());
            for value in values {
                hash.update(value.to_le_bytes());
            }
        }
        ColumnData::Enum(values) => {
            hash.update([2]);
            update_u64(hash, values.len());
            for value in values {
                hash.update(value.to_le_bytes());
            }
        }
        ColumnData::Ref(values) => {
            hash.update([3]);
            update_u64(hash, values.len());
            for value in values {
                hash.update(value.to_le_bytes());
            }
        }
    }
}

fn update_u64(hash: &mut Sha256, value: usize) {
    hash.update((value as u64).to_le_bytes());
}

fn update_string(hash: &mut Sha256, value: &str) {
    update_u64(hash, value.len());
    hash.update(value.as_bytes());
}
