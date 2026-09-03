//! Frozen `sembla.state/v1` initial-state artifacts.

use std::collections::{BTreeMap, HashSet};
use std::error::Error;
use std::fmt;
use std::fs;
use std::io::{BufWriter, Write as _};
use std::path::{Path, PathBuf};

use sembla_ir::{domain_digest, to_canonical_string, AttrType, HashRecordV1, ValidatedModel};

use crate::state::{ColumnData, ColumnInit, StateError, StateStore, TableInit};

mod header_json;
mod payload;
mod validation;

use header_json::*;
use payload::*;
use validation::*;

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
///
/// The header and column payloads are streamed directly to the destination.
/// Only a bounded encoding buffer is allocated in addition to the caller's
/// column vectors, so large synthetic artifacts do not acquire a second
/// artifact-sized in-memory copy.
pub fn write(
    path: impl AsRef<Path>,
    model: &ValidatedModel,
    tables: &[TableInit],
) -> Result<(), StateArtifactError> {
    let path = path.as_ref();
    let (header_json, header_len) = prepare_writer(model, tables)?;
    let file = fs::File::create(path).map_err(|error| io_error(path, error))?;
    let mut writer = BufWriter::new(file);
    write_prepared(&mut writer, model, tables, &header_json, header_len)
        .and_then(|()| writer.flush())
        .map_err(|error| io_error(path, error))
}

/// Writes a canonical artifact while atomically refusing to replace an existing path.
pub fn write_new(
    path: impl AsRef<Path>,
    model: &ValidatedModel,
    tables: &[TableInit],
) -> Result<(), StateArtifactError> {
    let path = path.as_ref();
    let (header_json, header_len) = prepare_writer(model, tables)?;
    let file = match fs::OpenOptions::new()
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
    let mut writer = BufWriter::new(file);
    if let Err(error) = write_prepared(&mut writer, model, tables, &header_json, header_len)
        .and_then(|()| writer.flush())
    {
        drop(writer);
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

fn prepare_writer(
    model: &ValidatedModel,
    tables: &[TableInit],
) -> Result<(String, u32), StateArtifactError> {
    validate_writer_inputs(model, tables)?;
    let header = header_for_model(model)?;
    let header_json = canonical_header(&header)?;
    let header_len =
        u32::try_from(header_json.len()).map_err(|_| StateArtifactError::HeaderTooLarge {
            length: header_json.len(),
        })?;
    expected_file_len(&header, PREFIX_LEN + header_json.len())?;
    Ok((header_json, header_len))
}

fn write_prepared(
    writer: &mut impl std::io::Write,
    model: &ValidatedModel,
    tables: &[TableInit],
    header_json: &str,
    header_len: u32,
) -> std::io::Result<()> {
    writer.write_all(STATE_MAGIC)?;
    writer.write_all(&header_len.to_le_bytes())?;
    writer.write_all(header_json.as_bytes())?;
    for model_box in &model.model().boxes {
        for table in &model_box.tables {
            let input = tables
                .iter()
                .find(|input| input.box_name == model_box.name && input.table_name == table.name)
                .expect("writer inputs were validated");
            for attr in &table.attrs {
                let column = input
                    .columns
                    .iter()
                    .find(|column| column.name == attr.name)
                    .expect("writer columns were validated");
                write_column_bytes(writer, &column.data)?;
            }
        }
    }
    Ok(())
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

fn io_error(path: &Path, error: std::io::Error) -> StateArtifactError {
    StateArtifactError::Io {
        path: path.to_path_buf(),
        message: error.to_string(),
    }
}
