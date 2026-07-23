//! Frozen `sembla.state/v1` initial-state artifacts.

use std::collections::{BTreeMap, HashSet};
use std::error::Error;
use std::fmt;
use std::fs;
use std::io::Write as _;
use std::path::{Path, PathBuf};

use sembla_ir::{domain_digest, to_canonical_string, AttrType, HashRecordV1, ValidatedModel};

use crate::state::{ColumnData, ColumnInit, StateError, StateStore, TableInit};

pub const STATE_ARTIFACT_SCHEMA: &str = "sembla.state/v1";
pub const STATE_ARTIFACT_HASH_DOMAIN: &str = "sembla.state-artifact/v1";
pub const STATE_MAGIC: &[u8; 12] = b"SEMBLA_STATE";
const POPULATION_MAGIC: &[u8; 12] = b"SEMBLA_POP\0\0";
const PREFIX_LEN: usize = 16;

/// Binary input kind identified solely by its frozen 12-byte magic.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StateKind {
    SemblaPop,
    SemblaState,
    Unknown,
}

/// A deterministic state-artifact failure.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum StateArtifactError {
    Io {
        path: PathBuf,
        message: String,
    },
    InvalidMagic,
    TruncatedHeaderLength {
        actual: usize,
    },
    TruncatedHeader {
        expected: usize,
        actual: usize,
    },
    InvalidHeaderUtf8,
    InvalidHeaderJson(String),
    UnsupportedSchemaVersion(String),
    NonCanonicalHeader,
    DuplicateTable {
        box_name: String,
        table: String,
    },
    DuplicateColumn {
        box_name: String,
        table: String,
        column: String,
    },
    InvalidColumnMetadata {
        box_name: String,
        table: String,
        column: String,
        detail: String,
    },
    UnresolvedRefTarget {
        box_name: String,
        table: String,
        column: String,
        target_box: String,
        target_table: String,
    },
    PayloadSizeOverflow {
        box_name: String,
        table: String,
        column: String,
    },
    TruncatedPayload {
        expected: usize,
        actual: usize,
    },
    TrailingBytes {
        expected: usize,
        actual: usize,
    },
    MissingTable {
        box_name: String,
        table: String,
    },
    ExtraTable {
        box_name: String,
        table: String,
    },
    TableOrderMismatch {
        position: usize,
        expected: String,
        actual: String,
    },
    RowCountTooLarge {
        box_name: String,
        table: String,
        row_count: u64,
    },
    RowCountMismatch {
        box_name: String,
        table: String,
        expected: usize,
        actual: usize,
    },
    MissingColumn {
        box_name: String,
        table: String,
        column: String,
    },
    ExtraColumn {
        box_name: String,
        table: String,
        column: String,
    },
    ColumnOrderMismatch {
        box_name: String,
        table: String,
        position: usize,
        expected: String,
        actual: String,
    },
    ColumnTypeMismatch {
        box_name: String,
        table: String,
        column: String,
        expected: String,
        actual: String,
    },
    ColumnLengthMismatch {
        box_name: String,
        table: String,
        column: String,
        expected: usize,
        actual: usize,
    },
    VariantCountMismatch {
        box_name: String,
        table: String,
        column: String,
        expected: usize,
        actual: u64,
    },
    EnumValueOutOfRange {
        box_name: String,
        table: String,
        column: String,
        row: usize,
        value: u16,
        variant_count: u64,
    },
    RefTargetMismatch {
        box_name: String,
        table: String,
        column: String,
        expected: String,
        actual: String,
    },
    RefValueOutOfRange {
        box_name: String,
        table: String,
        column: String,
        row: usize,
        value: u32,
        target: String,
        target_rows: usize,
    },
    HeaderTooLarge {
        length: usize,
    },
    RefuseOverwrite {
        path: PathBuf,
    },
}

impl fmt::Display for StateArtifactError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        use StateArtifactError::*;
        match self {
            Io { path, message } => write!(f, "state artifact '{}': {message}", path.display()),
            InvalidMagic => write!(f, "invalid state artifact magic; expected SEMBLA_STATE"),
            TruncatedHeaderLength { actual } => write!(f, "truncated state header length: expected 16 prefix bytes, found {actual}"),
            TruncatedHeader { expected, actual } => write!(f, "truncated state header: expected {expected} bytes through header, found {actual}"),
            InvalidHeaderUtf8 => write!(f, "state artifact header is not valid UTF-8"),
            InvalidHeaderJson(message) => write!(f, "invalid state artifact header JSON: {message}"),
            UnsupportedSchemaVersion(version) => write!(f, "unsupported state schema_version '{version}'; expected '{STATE_ARTIFACT_SCHEMA}'"),
            NonCanonicalHeader => write!(f, "state artifact header is not canonical JSON"),
            DuplicateTable { box_name, table } => write!(f, "duplicate state table '{box_name}.{table}'"),
            DuplicateColumn { box_name, table, column } => write!(f, "duplicate state column '{box_name}.{table}.{column}'"),
            InvalidColumnMetadata { box_name, table, column, detail } => write!(f, "invalid state column metadata for '{box_name}.{table}.{column}': {detail}"),
            UnresolvedRefTarget { box_name, table, column, target_box, target_table } => write!(f, "state ref column '{box_name}.{table}.{column}' names missing target '{target_box}.{target_table}'"),
            PayloadSizeOverflow { box_name, table, column } => write!(f, "state payload size overflows for '{box_name}.{table}.{column}'"),
            TruncatedPayload { expected, actual } => write!(f, "truncated state payload: expected {expected} file bytes, found {actual}"),
            TrailingBytes { expected, actual } => write!(f, "state artifact has trailing bytes: expected {expected} file bytes, found {actual}"),
            MissingTable { box_name, table } => write!(f, "state artifact is missing table '{box_name}.{table}'"),
            ExtraTable { box_name, table } => write!(f, "state artifact has extra table '{box_name}.{table}'"),
            TableOrderMismatch { position, expected, actual } => write!(f, "state table order mismatch at position {position}: expected '{expected}', found '{actual}'"),
            RowCountTooLarge { box_name, table, row_count } => write!(f, "state table '{box_name}.{table}' row_count {row_count} is not representable on this platform"),
            RowCountMismatch { box_name, table, expected, actual } => write!(f, "state row_count mismatch for table '{box_name}.{table}': model declares {expected}, artifact has {actual}"),
            MissingColumn { box_name, table, column } => write!(f, "state artifact table '{box_name}.{table}' is missing column '{column}'"),
            ExtraColumn { box_name, table, column } => write!(f, "state artifact table '{box_name}.{table}' has extra column '{column}'"),
            ColumnOrderMismatch { box_name, table, position, expected, actual } => write!(f, "state column order mismatch for table '{box_name}.{table}' at position {position}: expected '{expected}', found '{actual}'"),
            ColumnTypeMismatch { box_name, table, column, expected, actual } => write!(f, "state type mismatch for column '{box_name}.{table}.{column}': expected {expected}, found {actual}"),
            ColumnLengthMismatch { box_name, table, column, expected, actual } => write!(f, "state length mismatch for column '{box_name}.{table}.{column}': expected {expected}, found {actual}"),
            VariantCountMismatch { box_name, table, column, expected, actual } => write!(f, "state variant_count mismatch for column '{box_name}.{table}.{column}': expected {expected}, found {actual}"),
            EnumValueOutOfRange { box_name, table, column, row, value, variant_count } => write!(f, "state enum value out of range for column '{box_name}.{table}.{column}' at row {row}: {value} >= variant_count {variant_count}"),
            RefTargetMismatch { box_name, table, column, expected, actual } => write!(f, "state ref_target mismatch for column '{box_name}.{table}.{column}': expected '{expected}', found '{actual}'"),
            RefValueOutOfRange { box_name, table, column, row, value, target, target_rows } => write!(f, "state ref value out of range for column '{box_name}.{table}.{column}' at row {row}: {value} >= '{target}' row_count {target_rows}"),
            HeaderTooLarge { length } => write!(f, "state artifact canonical header is too large for u32 length: {length} bytes"),
            RefuseOverwrite { path } => write!(f, "refusing to overwrite existing state artifact '{}'", path.display()),
        }
    }
}

impl Error for StateArtifactError {}

#[derive(Clone, Debug)]
struct Header {
    schema_version: String,
    tables: Vec<TableHeader>,
}

#[derive(Clone, Debug)]
struct TableHeader {
    box_name: String,
    table: String,
    row_count: u64,
    columns: Vec<ColumnHeader>,
}

#[derive(Clone, Debug)]
struct ColumnHeader {
    name: String,
    column_type: ColumnType,
    variant_count: Option<u64>,
    ref_target: Option<RefTarget>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ColumnType {
    Real,
    Int,
    Enum,
    Ref,
}

impl ColumnType {
    fn name(self) -> &'static str {
        match self {
            Self::Real => "real",
            Self::Int => "int",
            Self::Enum => "enum",
            Self::Ref => "ref",
        }
    }

    fn width(self) -> usize {
        match self {
            Self::Real | Self::Int => 8,
            Self::Enum => 2,
            Self::Ref => 4,
        }
    }
}

#[derive(Clone, Debug)]
struct RefTarget {
    box_name: String,
    table: String,
}

/// A structurally valid, canonical `sembla.state/v1` artifact.
#[derive(Clone, Debug)]
pub struct StateArtifact {
    header: Header,
    columns: Vec<Vec<ColumnData>>,
}

/// Writes canonical deterministic state-artifact bytes.
pub fn write(
    path: impl AsRef<Path>,
    model: &ValidatedModel,
    tables: &[TableInit],
) -> Result<(), StateArtifactError> {
    let bytes = artifact_bytes(model, tables)?;
    fs::write(path.as_ref(), bytes).map_err(|error| io_error(path.as_ref(), error))
}

/// Writes a canonical artifact while atomically refusing to replace an existing path.
pub fn write_new(
    path: impl AsRef<Path>,
    model: &ValidatedModel,
    tables: &[TableInit],
) -> Result<(), StateArtifactError> {
    let path = path.as_ref();
    let bytes = artifact_bytes(model, tables)?;
    let mut file = match fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
    {
        Ok(file) => file,
        Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
            return Err(StateArtifactError::RefuseOverwrite {
                path: path.to_owned(),
            });
        }
        Err(error) => return Err(io_error(path, error)),
    };
    if let Err(error) = file.write_all(&bytes) {
        drop(file);
        let _ = fs::remove_file(path);
        return Err(io_error(path, error));
    }
    Ok(())
}

/// Copies every committed model table from a state store into writer-ready inits.
pub fn committed_table_inits(
    model: &ValidatedModel,
    state: &StateStore,
) -> Result<Vec<TableInit>, StateError> {
    let snapshot = state.snapshot();
    let mut tables = Vec::new();
    for model_box in &model.model().boxes {
        for table in &model_box.tables {
            let row_count = snapshot.row_count(&model_box.name, &table.name)?;
            let mut columns = Vec::with_capacity(table.attrs.len());
            for attr in &table.attrs {
                let data = match &attr.ty {
                    AttrType::Real => ColumnData::Real(
                        (0..row_count)
                            .map(|row| snapshot.real(&model_box.name, &table.name, &attr.name, row))
                            .collect::<Result<Vec<_>, _>>()?,
                    ),
                    AttrType::Int => ColumnData::Int(
                        (0..row_count)
                            .map(|row| snapshot.int(&model_box.name, &table.name, &attr.name, row))
                            .collect::<Result<Vec<_>, _>>()?,
                    ),
                    AttrType::Enum { .. } => ColumnData::Enum(
                        (0..row_count)
                            .map(|row| {
                                snapshot.enum_index(&model_box.name, &table.name, &attr.name, row)
                            })
                            .collect::<Result<Vec<_>, _>>()?,
                    ),
                    AttrType::Ref { .. } => ColumnData::Ref(
                        (0..row_count)
                            .map(|row| {
                                snapshot.reference(&model_box.name, &table.name, &attr.name, row)
                            })
                            .collect::<Result<Vec<_>, _>>()?,
                    ),
                };
                columns.push(ColumnInit::new(&attr.name, data));
            }
            tables.push(TableInit::new(
                &model_box.name,
                &table.name,
                row_count,
                columns,
            ));
        }
    }
    Ok(tables)
}

fn artifact_bytes(
    model: &ValidatedModel,
    tables: &[TableInit],
) -> Result<Vec<u8>, StateArtifactError> {
    let normalized = normalize_writer_inputs(model, tables)?;
    let header = header_for_model(model)?;
    let header_json = canonical_header(&header)?;
    let header_len =
        u32::try_from(header_json.len()).map_err(|_| StateArtifactError::HeaderTooLarge {
            length: header_json.len(),
        })?;

    let mut payload_len = 0usize;
    for table in &normalized {
        for column in &table.columns {
            let byte_len = column_data_len(&column.data)
                .checked_mul(column_data_type(&column.data).width())
                .ok_or_else(|| StateArtifactError::PayloadSizeOverflow {
                    box_name: table.box_name.clone(),
                    table: table.table_name.clone(),
                    column: column.name.clone(),
                })?;
            payload_len = payload_len.checked_add(byte_len).ok_or_else(|| {
                StateArtifactError::PayloadSizeOverflow {
                    box_name: table.box_name.clone(),
                    table: table.table_name.clone(),
                    column: column.name.clone(),
                }
            })?;
        }
    }
    let capacity = PREFIX_LEN
        .checked_add(header_json.len())
        .and_then(|size| size.checked_add(payload_len))
        .ok_or_else(|| StateArtifactError::PayloadSizeOverflow {
            box_name: "<writer>".to_owned(),
            table: "<writer>".to_owned(),
            column: "<file>".to_owned(),
        })?;
    let mut bytes = Vec::with_capacity(capacity);
    bytes.extend_from_slice(STATE_MAGIC);
    bytes.extend_from_slice(&header_len.to_le_bytes());
    bytes.extend_from_slice(header_json.as_bytes());
    for table in &normalized {
        for column in &table.columns {
            append_column_bytes(&mut bytes, &column.data);
        }
    }
    Ok(bytes)
}

/// Reads and structurally validates a canonical state artifact.
pub fn read(path: impl AsRef<Path>) -> Result<StateArtifact, StateArtifactError> {
    let bytes = fs::read(path.as_ref()).map_err(|error| io_error(path.as_ref(), error))?;
    read_bytes(&bytes)
}

fn read_bytes(bytes: &[u8]) -> Result<StateArtifact, StateArtifactError> {
    if bytes.len() < STATE_MAGIC.len() || &bytes[..STATE_MAGIC.len()] != STATE_MAGIC {
        return Err(StateArtifactError::InvalidMagic);
    }
    if bytes.len() < PREFIX_LEN {
        return Err(StateArtifactError::TruncatedHeaderLength {
            actual: bytes.len(),
        });
    }
    let header_len =
        u32::from_le_bytes(bytes[12..16].try_into().expect("four-byte header length")) as usize;
    let header_end =
        PREFIX_LEN
            .checked_add(header_len)
            .ok_or(StateArtifactError::TruncatedHeader {
                expected: usize::MAX,
                actual: bytes.len(),
            })?;
    if bytes.len() < header_end {
        return Err(StateArtifactError::TruncatedHeader {
            expected: header_end,
            actual: bytes.len(),
        });
    }
    let header_bytes = &bytes[PREFIX_LEN..header_end];
    let header_text =
        std::str::from_utf8(header_bytes).map_err(|_| StateArtifactError::InvalidHeaderUtf8)?;
    let header = parse_header(header_text)?;
    if header.schema_version != STATE_ARTIFACT_SCHEMA {
        return Err(StateArtifactError::UnsupportedSchemaVersion(
            header.schema_version,
        ));
    }
    let canonical = canonical_header(&header)?;
    if canonical.as_bytes() != header_bytes {
        return Err(StateArtifactError::NonCanonicalHeader);
    }
    validate_header_structure(&header)?;

    let expected = expected_file_len(&header, header_end)?;
    if bytes.len() < expected {
        return Err(StateArtifactError::TruncatedPayload {
            expected,
            actual: bytes.len(),
        });
    }
    if bytes.len() > expected {
        return Err(StateArtifactError::TrailingBytes {
            expected,
            actual: bytes.len(),
        });
    }

    let mut offset = header_end;
    let mut tables = Vec::with_capacity(header.tables.len());
    for table in &header.tables {
        let row_count =
            usize::try_from(table.row_count).map_err(|_| StateArtifactError::RowCountTooLarge {
                box_name: table.box_name.clone(),
                table: table.table.clone(),
                row_count: table.row_count,
            })?;
        let mut columns = Vec::with_capacity(table.columns.len());
        for column in &table.columns {
            let width = column.column_type.width();
            let byte_len = row_count.checked_mul(width).ok_or_else(|| {
                StateArtifactError::PayloadSizeOverflow {
                    box_name: table.box_name.clone(),
                    table: table.table.clone(),
                    column: column.name.clone(),
                }
            })?;
            let end = offset + byte_len;
            columns.push(decode_column(column.column_type, &bytes[offset..end]));
            offset = end;
        }
        tables.push(columns);
    }
    debug_assert_eq!(offset, bytes.len());
    Ok(StateArtifact {
        header,
        columns: tables,
    })
}

/// Validates an artifact as an exact initializer for `model` and returns
/// declaration-ordered loader inputs.
pub fn to_table_inits(
    artifact: &StateArtifact,
    model: &ValidatedModel,
) -> Result<Vec<TableInit>, StateArtifactError> {
    let model_tables: Vec<_> = model
        .model()
        .boxes
        .iter()
        .flat_map(|model_box| model_box.tables.iter().map(move |table| (model_box, table)))
        .collect();

    for (model_box, table) in &model_tables {
        if !artifact
            .header
            .tables
            .iter()
            .any(|entry| entry.box_name == model_box.name && entry.table == table.name)
        {
            return Err(StateArtifactError::MissingTable {
                box_name: model_box.name.clone(),
                table: table.name.clone(),
            });
        }
    }
    for entry in &artifact.header.tables {
        if !model_tables
            .iter()
            .any(|(model_box, table)| entry.box_name == model_box.name && entry.table == table.name)
        {
            return Err(StateArtifactError::ExtraTable {
                box_name: entry.box_name.clone(),
                table: entry.table.clone(),
            });
        }
    }
    for (position, ((model_box, table), entry)) in
        model_tables.iter().zip(&artifact.header.tables).enumerate()
    {
        let expected = qualified(&model_box.name, &table.name);
        let actual = qualified(&entry.box_name, &entry.table);
        if expected != actual {
            return Err(StateArtifactError::TableOrderMismatch {
                position,
                expected,
                actual,
            });
        }
    }

    // Establish every model/artifact row-count contract before validating any
    // column values. Ref sources may precede their targets in declaration order.
    let mut artifact_row_counts = BTreeMap::new();
    for ((model_box, table), entry) in model_tables.iter().zip(&artifact.header.tables) {
        let row_count =
            usize::try_from(entry.row_count).map_err(|_| StateArtifactError::RowCountTooLarge {
                box_name: entry.box_name.clone(),
                table: entry.table.clone(),
                row_count: entry.row_count,
            })?;
        let expected_rows =
            usize::try_from(table.size_hint).map_err(|_| StateArtifactError::RowCountTooLarge {
                box_name: model_box.name.clone(),
                table: table.name.clone(),
                row_count: table.size_hint,
            })?;
        if row_count != expected_rows {
            return Err(StateArtifactError::RowCountMismatch {
                box_name: model_box.name.clone(),
                table: table.name.clone(),
                expected: expected_rows,
                actual: row_count,
            });
        }
        artifact_row_counts.insert((entry.box_name.clone(), entry.table.clone()), row_count);
    }

    let mut result = Vec::with_capacity(model_tables.len());
    for (table_index, ((model_box, table), entry)) in
        model_tables.iter().zip(&artifact.header.tables).enumerate()
    {
        let row_count = artifact_row_counts[&(entry.box_name.clone(), entry.table.clone())];

        for attr in &table.attrs {
            if !entry.columns.iter().any(|column| column.name == attr.name) {
                return Err(StateArtifactError::MissingColumn {
                    box_name: model_box.name.clone(),
                    table: table.name.clone(),
                    column: attr.name.clone(),
                });
            }
        }
        for column in &entry.columns {
            if !table.attrs.iter().any(|attr| attr.name == column.name) {
                return Err(StateArtifactError::ExtraColumn {
                    box_name: model_box.name.clone(),
                    table: table.name.clone(),
                    column: column.name.clone(),
                });
            }
        }

        let mut columns = Vec::with_capacity(table.attrs.len());
        for (position, ((attr, column), data)) in table
            .attrs
            .iter()
            .zip(&entry.columns)
            .zip(&artifact.columns[table_index])
            .enumerate()
        {
            if attr.name != column.name {
                return Err(StateArtifactError::ColumnOrderMismatch {
                    box_name: model_box.name.clone(),
                    table: table.name.clone(),
                    position,
                    expected: attr.name.clone(),
                    actual: column.name.clone(),
                });
            }
            let ref_target_rows = column.ref_target.as_ref().and_then(|target| {
                artifact_row_counts
                    .get(&(target.box_name.clone(), target.table.clone()))
                    .copied()
            });
            validate_column(
                &model_box.name,
                &table.name,
                attr,
                column,
                data,
                row_count,
                ref_target_rows,
            )?;
            columns.push(ColumnInit::new(attr.name.clone(), data.clone()));
        }
        result.push(TableInit::new(
            model_box.name.clone(),
            table.name.clone(),
            row_count,
            columns,
        ));
    }
    Ok(result)
}

/// Identifies legacy and generic state files without extension-based routing.
pub fn sniff_magic(path: impl AsRef<Path>) -> Result<StateKind, StateArtifactError> {
    let bytes = fs::read(path.as_ref()).map_err(|error| io_error(path.as_ref(), error))?;
    if bytes.starts_with(POPULATION_MAGIC) {
        Ok(StateKind::SemblaPop)
    } else if bytes.starts_with(STATE_MAGIC) {
        Ok(StateKind::SemblaState)
    } else {
        Ok(StateKind::Unknown)
    }
}

/// Returns the frozen domain-separated hash record for exact artifact bytes.
pub fn state_artifact_hash(path: impl AsRef<Path>) -> Result<HashRecordV1, StateArtifactError> {
    let bytes = fs::read(path.as_ref()).map_err(|error| io_error(path.as_ref(), error))?;
    read_bytes(&bytes)?;
    let digest = domain_digest(STATE_ARTIFACT_HASH_DOMAIN, &bytes);
    let mut digest_hex = String::with_capacity(digest.len() * 2);
    for byte in digest {
        use fmt::Write as _;
        write!(&mut digest_hex, "{byte:02x}").expect("writing to String cannot fail");
    }
    Ok(HashRecordV1 {
        algorithm: "sha256".to_owned(),
        domain: STATE_ARTIFACT_HASH_DOMAIN.to_owned(),
        digest: digest_hex,
    })
}

// The header has a closed schema, so its lexically sorted key order is fixed.
// Every variable JSON string still goes through the IR's canonical serializer;
// the only numbers are unsigned decimal counts. This keeps the runtime's
// dependency surface narrow while sharing the canonical string/escape rules.
fn canonical_header(header: &Header) -> Result<String, StateArtifactError> {
    use fmt::Write as _;

    let mut output = String::new();
    output.push_str("{\"schema_version\":");
    output.push_str(&canonical_json_string(&header.schema_version)?);
    output.push_str(",\"tables\":[");
    for (table_index, table) in header.tables.iter().enumerate() {
        if table_index != 0 {
            output.push(',');
        }
        output.push_str("{\"box\":");
        output.push_str(&canonical_json_string(&table.box_name)?);
        output.push_str(",\"columns\":[");
        for (column_index, column) in table.columns.iter().enumerate() {
            if column_index != 0 {
                output.push(',');
            }
            output.push_str("{\"name\":");
            output.push_str(&canonical_json_string(&column.name)?);
            if let Some(target) = &column.ref_target {
                output.push_str(",\"ref_target\":{\"box\":");
                output.push_str(&canonical_json_string(&target.box_name)?);
                output.push_str(",\"table\":");
                output.push_str(&canonical_json_string(&target.table)?);
                output.push('}');
            }
            output.push_str(",\"type\":");
            output.push_str(&canonical_json_string(column.column_type.name())?);
            if let Some(variant_count) = column.variant_count {
                write!(&mut output, ",\"variant_count\":{variant_count}")
                    .expect("writing to String cannot fail");
            }
            output.push('}');
        }
        write!(
            &mut output,
            "],\"row_count\":{},\"table\":",
            table.row_count
        )
        .expect("writing to String cannot fail");
        output.push_str(&canonical_json_string(&table.table)?);
        output.push('}');
    }
    output.push_str("]}");
    Ok(output)
}

fn canonical_json_string(value: &str) -> Result<String, StateArtifactError> {
    to_canonical_string(&value).map_err(StateArtifactError::InvalidHeaderJson)
}

#[derive(Clone, Debug)]
enum JsonValue {
    Object(Vec<(String, JsonValue)>),
    Array(Vec<JsonValue>),
    String(String),
    Number(u64),
    Bool,
    Null,
}

// Strict parser for the closed header schema. It accepts semantically valid
// non-canonical JSON so the caller can distinguish it from malformed JSON by
// re-emitting and comparing the exact canonical header bytes.
struct JsonParser<'a> {
    source: &'a str,
    offset: usize,
}

impl<'a> JsonParser<'a> {
    fn new(source: &'a str) -> Self {
        Self { source, offset: 0 }
    }

    fn parse(mut self) -> Result<JsonValue, StateArtifactError> {
        let value = self.parse_value()?;
        self.skip_whitespace();
        if self.offset != self.source.len() {
            return self.error("trailing characters after header value");
        }
        Ok(value)
    }

    fn parse_value(&mut self) -> Result<JsonValue, StateArtifactError> {
        self.skip_whitespace();
        match self.peek() {
            Some(b'{') => self.parse_object(),
            Some(b'[') => self.parse_array(),
            Some(b'\"') => self.parse_string().map(JsonValue::String),
            Some(b'0'..=b'9') => self.parse_number().map(JsonValue::Number),
            Some(b't') => {
                self.literal("true")?;
                Ok(JsonValue::Bool)
            }
            Some(b'f') => {
                self.literal("false")?;
                Ok(JsonValue::Bool)
            }
            Some(b'n') => {
                self.literal("null")?;
                Ok(JsonValue::Null)
            }
            Some(other) => self.error(&format!("unexpected byte 0x{other:02x}")),
            None => self.error("unexpected end of header"),
        }
    }

    fn parse_object(&mut self) -> Result<JsonValue, StateArtifactError> {
        self.offset += 1;
        self.skip_whitespace();
        let mut fields = Vec::new();
        if self.consume(b'}') {
            return Ok(JsonValue::Object(fields));
        }
        loop {
            self.skip_whitespace();
            if self.peek() != Some(b'\"') {
                return self.error("object key must be a string");
            }
            let key = self.parse_string()?;
            self.skip_whitespace();
            if !self.consume(b':') {
                return self.error("object key must be followed by ':'");
            }
            fields.push((key, self.parse_value()?));
            self.skip_whitespace();
            if self.consume(b'}') {
                break;
            }
            if !self.consume(b',') {
                return self.error("object entries must be separated by ','");
            }
        }
        Ok(JsonValue::Object(fields))
    }

    fn parse_array(&mut self) -> Result<JsonValue, StateArtifactError> {
        self.offset += 1;
        self.skip_whitespace();
        let mut values = Vec::new();
        if self.consume(b']') {
            return Ok(JsonValue::Array(values));
        }
        loop {
            values.push(self.parse_value()?);
            self.skip_whitespace();
            if self.consume(b']') {
                break;
            }
            if !self.consume(b',') {
                return self.error("array entries must be separated by ','");
            }
        }
        Ok(JsonValue::Array(values))
    }

    fn parse_string(&mut self) -> Result<String, StateArtifactError> {
        debug_assert_eq!(self.peek(), Some(b'\"'));
        self.offset += 1;
        let mut output = String::new();
        loop {
            let byte = self
                .peek()
                .ok_or_else(|| header_json_error(self.offset, "unterminated string"))?;
            match byte {
                b'\"' => {
                    self.offset += 1;
                    return Ok(output);
                }
                b'\\' => {
                    self.offset += 1;
                    let escape = self
                        .peek()
                        .ok_or_else(|| header_json_error(self.offset, "unterminated escape"))?;
                    self.offset += 1;
                    match escape {
                        b'\"' => output.push('\"'),
                        b'\\' => output.push('\\'),
                        b'/' => output.push('/'),
                        b'b' => output.push('\u{0008}'),
                        b'f' => output.push('\u{000c}'),
                        b'n' => output.push('\n'),
                        b'r' => output.push('\r'),
                        b't' => output.push('\t'),
                        b'u' => output.push(self.parse_unicode_escape()?),
                        _ => return self.error("invalid string escape"),
                    }
                }
                0x00..=0x1f => return self.error("unescaped control character in string"),
                0x20..=0x7f => {
                    output.push(char::from(byte));
                    self.offset += 1;
                }
                _ => {
                    let value = self.source[self.offset..]
                        .chars()
                        .next()
                        .ok_or_else(|| header_json_error(self.offset, "invalid UTF-8 string"))?;
                    output.push(value);
                    self.offset += value.len_utf8();
                }
            }
        }
    }

    fn parse_unicode_escape(&mut self) -> Result<char, StateArtifactError> {
        let first = self.parse_hex_quad()?;
        if (0xd800..=0xdbff).contains(&first) {
            if !self.consume(b'\\') || !self.consume(b'u') {
                return self.error("high surrogate must be followed by a low surrogate");
            }
            let second = self.parse_hex_quad()?;
            if !(0xdc00..=0xdfff).contains(&second) {
                return self.error("invalid low surrogate");
            }
            let scalar =
                0x10000 + ((u32::from(first) - 0xd800) << 10) + (u32::from(second) - 0xdc00);
            char::from_u32(scalar)
                .ok_or_else(|| header_json_error(self.offset, "invalid Unicode scalar"))
        } else if (0xdc00..=0xdfff).contains(&first) {
            self.error("unpaired low surrogate")
        } else {
            char::from_u32(u32::from(first))
                .ok_or_else(|| header_json_error(self.offset, "invalid Unicode scalar"))
        }
    }

    fn parse_hex_quad(&mut self) -> Result<u16, StateArtifactError> {
        let mut value = 0u16;
        for _ in 0..4 {
            let byte = self
                .peek()
                .ok_or_else(|| header_json_error(self.offset, "truncated Unicode escape"))?;
            self.offset += 1;
            let digit = match byte {
                b'0'..=b'9' => u16::from(byte - b'0'),
                b'a'..=b'f' => u16::from(byte - b'a' + 10),
                b'A'..=b'F' => u16::from(byte - b'A' + 10),
                _ => return self.error("invalid hexadecimal Unicode escape"),
            };
            value = (value << 4) | digit;
        }
        Ok(value)
    }

    fn parse_number(&mut self) -> Result<u64, StateArtifactError> {
        let start = self.offset;
        if self.consume(b'0') {
            if matches!(self.peek(), Some(b'0'..=b'9')) {
                return self.error("number has a leading zero");
            }
        } else {
            while matches!(self.peek(), Some(b'0'..=b'9')) {
                self.offset += 1;
            }
        }
        self.source[start..self.offset]
            .parse::<u64>()
            .map_err(|_| header_json_error(start, "number is outside u64 range"))
    }

    fn literal(&mut self, literal: &str) -> Result<(), StateArtifactError> {
        if self.source[self.offset..].starts_with(literal) {
            self.offset += literal.len();
            Ok(())
        } else {
            self.error("invalid JSON literal")
        }
    }

    fn skip_whitespace(&mut self) {
        while matches!(self.peek(), Some(b' ' | b'\n' | b'\r' | b'\t')) {
            self.offset += 1;
        }
    }

    fn peek(&self) -> Option<u8> {
        self.source.as_bytes().get(self.offset).copied()
    }

    fn consume(&mut self, expected: u8) -> bool {
        if self.peek() == Some(expected) {
            self.offset += 1;
            true
        } else {
            false
        }
    }

    fn error<T>(&self, message: &str) -> Result<T, StateArtifactError> {
        Err(header_json_error(self.offset, message))
    }
}

fn header_json_error(offset: usize, message: &str) -> StateArtifactError {
    StateArtifactError::InvalidHeaderJson(format!("byte {offset}: {message}"))
}

fn parse_header(source: &str) -> Result<Header, StateArtifactError> {
    let mut fields = object_fields(JsonParser::new(source).parse()?, "header")?;
    let schema_version = take_string(
        required(&mut fields, "schema_version", "header")?,
        "header.schema_version",
    )?;
    let tables = take_array(required(&mut fields, "tables", "header")?, "header.tables")?
        .into_iter()
        .enumerate()
        .map(|(index, table)| parse_table(table, index))
        .collect::<Result<Vec<_>, _>>()?;
    reject_extra_fields(&fields, "header")?;
    Ok(Header {
        schema_version,
        tables,
    })
}

fn parse_table(value: JsonValue, index: usize) -> Result<TableHeader, StateArtifactError> {
    let context = format!("header.tables[{index}]");
    let mut fields = object_fields(value, &context)?;
    let box_name = take_string(
        required(&mut fields, "box", &context)?,
        &format!("{context}.box"),
    )?;
    let table = take_string(
        required(&mut fields, "table", &context)?,
        &format!("{context}.table"),
    )?;
    let row_count = take_number(
        required(&mut fields, "row_count", &context)?,
        &format!("{context}.row_count"),
    )?;
    let columns = take_array(
        required(&mut fields, "columns", &context)?,
        &format!("{context}.columns"),
    )?
    .into_iter()
    .enumerate()
    .map(|(column_index, column)| parse_column(column, index, column_index))
    .collect::<Result<Vec<_>, _>>()?;
    reject_extra_fields(&fields, &context)?;
    Ok(TableHeader {
        box_name,
        table,
        row_count,
        columns,
    })
}

fn parse_column(
    value: JsonValue,
    table_index: usize,
    column_index: usize,
) -> Result<ColumnHeader, StateArtifactError> {
    let context = format!("header.tables[{table_index}].columns[{column_index}]");
    let mut fields = object_fields(value, &context)?;
    let name = take_string(
        required(&mut fields, "name", &context)?,
        &format!("{context}.name"),
    )?;
    let type_name = take_string(
        required(&mut fields, "type", &context)?,
        &format!("{context}.type"),
    )?;
    let column_type = match type_name.as_str() {
        "real" => ColumnType::Real,
        "int" => ColumnType::Int,
        "enum" => ColumnType::Enum,
        "ref" => ColumnType::Ref,
        _ => {
            return Err(header_json_error(
                0,
                &format!("{context}.type has unsupported value '{type_name}'"),
            ))
        }
    };
    let variant_count = fields
        .remove("variant_count")
        .map(|value| take_number(value, &format!("{context}.variant_count")))
        .transpose()?;
    let ref_target = fields
        .remove("ref_target")
        .map(|value| parse_ref_target(value, &format!("{context}.ref_target")))
        .transpose()?;
    reject_extra_fields(&fields, &context)?;
    Ok(ColumnHeader {
        name,
        column_type,
        variant_count,
        ref_target,
    })
}

fn parse_ref_target(value: JsonValue, context: &str) -> Result<RefTarget, StateArtifactError> {
    let mut fields = object_fields(value, context)?;
    let box_name = take_string(
        required(&mut fields, "box", context)?,
        &format!("{context}.box"),
    )?;
    let table = take_string(
        required(&mut fields, "table", context)?,
        &format!("{context}.table"),
    )?;
    reject_extra_fields(&fields, context)?;
    Ok(RefTarget { box_name, table })
}

fn object_fields(
    value: JsonValue,
    context: &str,
) -> Result<BTreeMap<String, JsonValue>, StateArtifactError> {
    let JsonValue::Object(entries) = value else {
        return Err(header_json_error(
            0,
            &format!("{context} must be an object"),
        ));
    };
    let mut fields = BTreeMap::new();
    for (name, value) in entries {
        if fields.insert(name.clone(), value).is_some() {
            return Err(header_json_error(
                0,
                &format!("{context} has duplicate field '{name}'"),
            ));
        }
    }
    Ok(fields)
}

fn required(
    fields: &mut BTreeMap<String, JsonValue>,
    name: &str,
    context: &str,
) -> Result<JsonValue, StateArtifactError> {
    fields
        .remove(name)
        .ok_or_else(|| header_json_error(0, &format!("{context} is missing field '{name}'")))
}

fn reject_extra_fields(
    fields: &BTreeMap<String, JsonValue>,
    context: &str,
) -> Result<(), StateArtifactError> {
    if let Some(name) = fields.keys().next() {
        Err(header_json_error(
            0,
            &format!("{context} has unknown field '{name}'"),
        ))
    } else {
        Ok(())
    }
}

fn take_string(value: JsonValue, context: &str) -> Result<String, StateArtifactError> {
    if let JsonValue::String(value) = value {
        Ok(value)
    } else {
        Err(header_json_error(0, &format!("{context} must be a string")))
    }
}

fn take_number(value: JsonValue, context: &str) -> Result<u64, StateArtifactError> {
    if let JsonValue::Number(value) = value {
        Ok(value)
    } else {
        Err(header_json_error(
            0,
            &format!("{context} must be an unsigned integer"),
        ))
    }
}

fn take_array(value: JsonValue, context: &str) -> Result<Vec<JsonValue>, StateArtifactError> {
    if let JsonValue::Array(value) = value {
        Ok(value)
    } else {
        Err(header_json_error(0, &format!("{context} must be an array")))
    }
}

fn validate_header_structure(header: &Header) -> Result<(), StateArtifactError> {
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

fn expected_file_len(header: &Header, header_end: usize) -> Result<usize, StateArtifactError> {
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

fn header_for_model(model: &ValidatedModel) -> Result<Header, StateArtifactError> {
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
                    .map(|attr| {
                        let (column_type, variant_count, ref_target) = match &attr.ty {
                            AttrType::Real => (ColumnType::Real, None, None),
                            AttrType::Int => (ColumnType::Int, None, None),
                            AttrType::Enum { variants } => {
                                (ColumnType::Enum, Some(variants.len() as u64), None)
                            }
                            AttrType::Ref { table } => (
                                ColumnType::Ref,
                                None,
                                Some(RefTarget {
                                    box_name: model_box.name.clone(),
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
                    })
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

fn normalize_writer_inputs(
    model: &ValidatedModel,
    inputs: &[TableInit],
) -> Result<Vec<TableInit>, StateArtifactError> {
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

    let mut normalized = Vec::new();
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
            let mut columns = Vec::new();
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
                columns.push(column.clone());
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
            normalized.push(TableInit::new(
                model_box.name.clone(),
                table.name.clone(),
                input.row_count,
                columns,
            ));
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
    Ok(normalized)
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

fn validate_column(
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
    let actual_len = column_data_len(data);
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

fn qualified(box_name: &str, table: &str) -> String {
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

fn column_data_len(data: &ColumnData) -> usize {
    match data {
        ColumnData::Real(values) => values.len(),
        ColumnData::Int(values) => values.len(),
        ColumnData::Enum(values) => values.len(),
        ColumnData::Ref(values) => values.len(),
    }
}

fn append_column_bytes(output: &mut Vec<u8>, data: &ColumnData) {
    match data {
        ColumnData::Real(values) => values
            .iter()
            .for_each(|value| output.extend_from_slice(&value.to_bits().to_le_bytes())),
        ColumnData::Int(values) => values
            .iter()
            .for_each(|value| output.extend_from_slice(&value.to_le_bytes())),
        ColumnData::Enum(values) => values
            .iter()
            .for_each(|value| output.extend_from_slice(&value.to_le_bytes())),
        ColumnData::Ref(values) => values
            .iter()
            .for_each(|value| output.extend_from_slice(&value.to_le_bytes())),
    }
}

fn decode_column(column_type: ColumnType, bytes: &[u8]) -> ColumnData {
    match column_type {
        ColumnType::Real => ColumnData::Real(
            bytes
                .chunks_exact(8)
                .map(|chunk| {
                    f64::from_bits(u64::from_le_bytes(
                        chunk.try_into().expect("eight-byte real"),
                    ))
                })
                .collect(),
        ),
        ColumnType::Int => ColumnData::Int(
            bytes
                .chunks_exact(8)
                .map(|chunk| i64::from_le_bytes(chunk.try_into().expect("eight-byte int")))
                .collect(),
        ),
        ColumnType::Enum => ColumnData::Enum(
            bytes
                .chunks_exact(2)
                .map(|chunk| u16::from_le_bytes(chunk.try_into().expect("two-byte enum")))
                .collect(),
        ),
        ColumnType::Ref => ColumnData::Ref(
            bytes
                .chunks_exact(4)
                .map(|chunk| u32::from_le_bytes(chunk.try_into().expect("four-byte ref")))
                .collect(),
        ),
    }
}

fn io_error(path: &Path, error: std::io::Error) -> StateArtifactError {
    StateArtifactError::Io {
        path: path.to_path_buf(),
        message: error.to_string(),
    }
}
