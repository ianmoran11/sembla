use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use sembla_runtime::state::{ColumnData, ColumnInit, TableInit};
use sembla_runtime::state_artifact::{
    read, sniff_magic, state_artifact_hash, to_table_inits, write, write_new, StateArtifactError,
    StateKind, STATE_ARTIFACT_HASH_DOMAIN, STATE_MAGIC,
};

fn repository_path(relative: impl AsRef<Path>) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join(relative)
}

fn temp_dir(label: &str) -> PathBuf {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let path = std::env::temp_dir().join(format!(
        "sembla-state-{label}-{}-{nonce}",
        std::process::id()
    ));
    std::fs::create_dir_all(&path).unwrap();
    path
}

fn load_model(relative: &str) -> sembla_ir::ValidatedModel {
    let source = std::fs::read_to_string(repository_path(relative)).unwrap();
    sembla_ir::validate(sembla_ir::parse_json(&source).unwrap()).unwrap()
}

fn refs_model() -> sembla_ir::ValidatedModel {
    load_model("fixtures/state/models/refs_small.json")
}

fn refs_tables() -> Vec<TableInit> {
    vec![
        TableInit::new(
            "world",
            "Area",
            2,
            vec![ColumnInit::new("rate", ColumnData::Real(vec![1.0, 1.5]))],
        ),
        TableInit::new(
            "world",
            "Person",
            3,
            vec![
                ColumnInit::new("age", ColumnData::Int(vec![10, 20, 30])),
                ColumnInit::new("status", ColumnData::Enum(vec![0, 1, 0])),
                ColumnInit::new("area", ColumnData::Ref(vec![0, 1, 0])),
            ],
        ),
    ]
}

fn two_box_tables() -> Vec<TableInit> {
    vec![
        TableInit::new(
            "population",
            "Person",
            16,
            vec![
                ColumnInit::new("health", ColumnData::Enum(vec![0; 16])),
                ColumnInit::new("group", ColumnData::Ref(vec![0; 16])),
            ],
        ),
        TableInit::new("population", "Group", 1, vec![]),
        TableInit::new(
            "controller",
            "Controller",
            1,
            vec![
                ColumnInit::new("modifier", ColumnData::Real(vec![0.0])),
                ColumnInit::new("group", ColumnData::Ref(vec![0])),
            ],
        ),
        TableInit::new("controller", "Group", 1, vec![]),
    ]
}

fn write_bytes(model: &sembla_ir::ValidatedModel, tables: &[TableInit]) -> Vec<u8> {
    let temp = temp_dir("bytes");
    let path = temp.join("artifact.state");
    write(&path, model, tables).unwrap();
    let bytes = std::fs::read(&path).unwrap();
    std::fs::remove_dir_all(temp).unwrap();
    bytes
}

fn two_box_source_before_target_mismatch_bytes() -> Vec<u8> {
    let mut bytes = std::fs::read(repository_path("fixtures/state/two_box_small.state")).unwrap();
    let header_len = u32::from_le_bytes(bytes[12..16].try_into().unwrap()) as usize;
    let header_end = 16 + header_len;
    let header = std::str::from_utf8(&bytes[16..header_end]).unwrap();
    let changed = header.replacen(
        "\"row_count\":1,\"table\":\"Group\"",
        "\"row_count\":2,\"table\":\"Group\"",
        1,
    );
    assert_ne!(changed, header);
    assert_eq!(changed.len(), header_len);
    bytes[16..header_end].copy_from_slice(changed.as_bytes());

    // population.Person.health occupies the first 16 u16 values. Its group
    // Ref column follows and targets the later population.Group table.
    let first_group_ref = header_end + 16 * std::mem::size_of::<u16>();
    bytes[first_group_ref..first_group_ref + 4].copy_from_slice(&1u32.to_le_bytes());
    bytes
}

const AREA_HEADER: &str = "{\"box\":\"world\",\"columns\":[{\"name\":\"rate\",\"type\":\"real\"}],\"row_count\":2,\"table\":\"Area\"}";
const PERSON_HEADER: &str = "{\"box\":\"world\",\"columns\":[{\"name\":\"age\",\"type\":\"int\"},{\"name\":\"status\",\"type\":\"enum\",\"variant_count\":2},{\"name\":\"area\",\"ref_target\":{\"box\":\"world\",\"table\":\"Area\"},\"type\":\"ref\"}],\"row_count\":3,\"table\":\"Person\"}";
const REFS_HEADER: &str = "{\"schema_version\":\"sembla.state/v1\",\"tables\":[{\"box\":\"world\",\"columns\":[{\"name\":\"rate\",\"type\":\"real\"}],\"row_count\":2,\"table\":\"Area\"},{\"box\":\"world\",\"columns\":[{\"name\":\"age\",\"type\":\"int\"},{\"name\":\"status\",\"type\":\"enum\",\"variant_count\":2},{\"name\":\"area\",\"ref_target\":{\"box\":\"world\",\"table\":\"Area\"},\"type\":\"ref\"}],\"row_count\":3,\"table\":\"Person\"}]}";

#[derive(Clone)]
struct Parts {
    header: String,
    columns: Vec<Vec<Vec<u8>>>,
}

fn parse_parts(bytes: &[u8]) -> Parts {
    assert_eq!(&bytes[..12], STATE_MAGIC);
    let header_len = u32::from_le_bytes(bytes[12..16].try_into().unwrap()) as usize;
    let header = std::str::from_utf8(&bytes[16..16 + header_len])
        .unwrap()
        .to_owned();
    assert_eq!(header, REFS_HEADER);
    let payload = &bytes[16 + header_len..];
    assert_eq!(payload.len(), 58);
    Parts {
        header,
        columns: vec![
            vec![payload[0..16].to_vec()],
            vec![
                payload[16..40].to_vec(),
                payload[40..46].to_vec(),
                payload[46..58].to_vec(),
            ],
        ],
    }
}

fn assemble(parts: &Parts) -> Vec<u8> {
    assemble_with_header(parts.header.as_bytes(), &parts.columns)
}

fn assemble_with_header(header: &[u8], columns: &[Vec<Vec<u8>>]) -> Vec<u8> {
    let mut bytes = Vec::new();
    bytes.extend_from_slice(STATE_MAGIC);
    bytes.extend_from_slice(&(header.len() as u32).to_le_bytes());
    bytes.extend_from_slice(header);
    for table in columns {
        for column in table {
            bytes.extend_from_slice(column);
        }
    }
    bytes
}

fn header_with_tables(tables: &str) -> String {
    format!("{{\"schema_version\":\"sembla.state/v1\",\"tables\":[{tables}]}}")
}

fn invalid_fixture_bytes() -> BTreeMap<&'static str, Vec<u8>> {
    let valid = write_bytes(&refs_model(), &refs_tables());
    let base = parse_parts(&valid);
    let mut fixtures = BTreeMap::new();

    let mut wrong_magic = valid.clone();
    wrong_magic[0] = b'X';
    fixtures.insert("wrong_magic.state", wrong_magic);

    let mut wrong_schema = base.clone();
    wrong_schema.header = wrong_schema
        .header
        .replace("sembla.state/v1", "sembla.state/v999");
    fixtures.insert("wrong_schema_version.state", assemble(&wrong_schema));

    let mut missing_table = base.clone();
    missing_table.header = header_with_tables(AREA_HEADER);
    missing_table.columns.pop();
    fixtures.insert("missing_table.state", assemble(&missing_table));

    let mut extra_table = base.clone();
    extra_table.header = header_with_tables(&format!(
        "{AREA_HEADER},{PERSON_HEADER},{{\"box\":\"world\",\"columns\":[],\"row_count\":0,\"table\":\"Extra\"}}"
    ));
    extra_table.columns.push(vec![]);
    fixtures.insert("extra_table.state", assemble(&extra_table));

    let mut wrong_rows = base.clone();
    wrong_rows.header = wrong_rows.header.replace(
        "\"row_count\":3,\"table\":\"Person\"",
        "\"row_count\":4,\"table\":\"Person\"",
    );
    for (column, width) in wrong_rows.columns[1].iter_mut().zip([8, 2, 4]) {
        column.resize(column.len() + width, 0);
    }
    fixtures.insert("row_count_mismatch.state", assemble(&wrong_rows));

    let mut missing_column = base.clone();
    missing_column.header = missing_column
        .header
        .replace("{\"name\":\"age\",\"type\":\"int\"},", "");
    missing_column.columns[1].remove(0);
    fixtures.insert("missing_column.state", assemble(&missing_column));

    let mut type_mismatch = base.clone();
    type_mismatch.header = type_mismatch.header.replace(
        "{\"name\":\"age\",\"type\":\"int\"}",
        "{\"name\":\"age\",\"type\":\"real\"}",
    );
    fixtures.insert("type_mismatch.state", assemble(&type_mismatch));

    let mut variant_count = base.clone();
    variant_count.header = variant_count
        .header
        .replace("\"variant_count\":2", "\"variant_count\":3");
    fixtures.insert("variant_count_mismatch.state", assemble(&variant_count));

    let mut enum_range = base.clone();
    enum_range.columns[1][1][0..2].copy_from_slice(&2u16.to_le_bytes());
    fixtures.insert("enum_out_of_range.state", assemble(&enum_range));

    let mut ref_range = base.clone();
    ref_range.columns[1][2][0..4].copy_from_slice(&2u32.to_le_bytes());
    fixtures.insert("ref_out_of_range.state", assemble(&ref_range));

    let mut truncated = valid.clone();
    truncated.pop();
    fixtures.insert("truncated_blob.state", truncated);

    let mut trailing = valid.clone();
    trailing.push(0);
    fixtures.insert("trailing_bytes.state", trailing);

    let spaced_header = base.header.replace('{', "{ ").replace('}', " }");
    fixtures.insert(
        "non_canonical_header.state",
        assemble_with_header(spaced_header.as_bytes(), &base.columns),
    );
    fixtures
}

fn invalid_path(name: &str) -> PathBuf {
    repository_path("fixtures/state/invalid").join(name)
}

fn model_error(name: &str) -> StateArtifactError {
    let artifact = read(invalid_path(name)).unwrap();
    to_table_inits(&artifact, &refs_model()).unwrap_err()
}

fn read_bytes_error(label: &str, bytes: &[u8]) -> StateArtifactError {
    let temp = temp_dir(label);
    let path = temp.join("invalid.state");
    std::fs::write(&path, bytes).unwrap();
    let error = read(&path).unwrap_err();
    std::fs::remove_dir_all(temp).unwrap();
    error
}

fn table_init_error(label: &str, parts: &Parts) -> StateArtifactError {
    let temp = temp_dir(label);
    let path = temp.join("invalid.state");
    std::fs::write(&path, assemble(parts)).unwrap();
    let artifact = read(&path).unwrap();
    let error = to_table_inits(&artifact, &refs_model()).unwrap_err();
    std::fs::remove_dir_all(temp).unwrap();
    error
}

fn table_init_error_for_bytes(
    label: &str,
    bytes: &[u8],
    model: &sembla_ir::ValidatedModel,
) -> StateArtifactError {
    let temp = temp_dir(label);
    let path = temp.join("invalid.state");
    std::fs::write(&path, bytes).unwrap();
    let artifact = read(&path).unwrap();
    let error = to_table_inits(&artifact, model).unwrap_err();
    std::fs::remove_dir_all(temp).unwrap();
    error
}

#[test]
fn refs_fixture_has_frozen_prefix_and_canonical_header() {
    let bytes = std::fs::read(repository_path("fixtures/state/refs_small.state")).unwrap();
    assert_eq!(&bytes[..12], b"SEMBLA_STATE");
    let header_len = u32::from_le_bytes(bytes[12..16].try_into().unwrap()) as usize;
    let header = std::str::from_utf8(&bytes[16..16 + header_len]).unwrap();
    assert_eq!(
        header,
        "{\"schema_version\":\"sembla.state/v1\",\"tables\":[{\"box\":\"world\",\"columns\":[{\"name\":\"rate\",\"type\":\"real\"}],\"row_count\":2,\"table\":\"Area\"},{\"box\":\"world\",\"columns\":[{\"name\":\"age\",\"type\":\"int\"},{\"name\":\"status\",\"type\":\"enum\",\"variant_count\":2},{\"name\":\"area\",\"ref_target\":{\"box\":\"world\",\"table\":\"Area\"},\"type\":\"ref\"}],\"row_count\":3,\"table\":\"Person\"}]}"
    );
}

#[test]
fn valid_fixtures_are_generated_bytes_and_load_exactly() {
    let refs = write_bytes(&refs_model(), &refs_tables());
    assert_eq!(
        std::fs::read(repository_path("fixtures/state/refs_small.state")).unwrap(),
        refs
    );
    let artifact = read(repository_path("fixtures/state/refs_small.state")).unwrap();
    assert_eq!(
        to_table_inits(&artifact, &refs_model()).unwrap(),
        refs_tables()
    );

    let two_box_model = load_model("examples/two_box.json");
    let two_box = write_bytes(&two_box_model, &two_box_tables());
    assert_eq!(
        std::fs::read(repository_path("fixtures/state/two_box_small.state")).unwrap(),
        two_box
    );
    let artifact = read(repository_path("fixtures/state/two_box_small.state")).unwrap();
    assert_eq!(
        to_table_inits(&artifact, &two_box_model).unwrap(),
        two_box_tables()
    );
}

#[test]
fn round_trip_and_repeated_writes_are_byte_identical() {
    let model = refs_model();
    let temp = temp_dir("round-trip");
    let first = temp.join("first.state");
    let second = temp.join("second.state");
    let third = temp.join("third.state");
    write(&first, &model, &refs_tables()).unwrap();
    write(&second, &model, &refs_tables()).unwrap();
    let loaded = to_table_inits(&read(&first).unwrap(), &model).unwrap();
    write(&third, &model, &loaded).unwrap();
    assert_eq!(
        std::fs::read(&first).unwrap(),
        std::fs::read(&second).unwrap()
    );
    assert_eq!(
        std::fs::read(&first).unwrap(),
        std::fs::read(&third).unwrap()
    );
    std::fs::remove_dir_all(temp).unwrap();
}

#[test]
fn create_new_writer_refuses_to_replace_a_chain_link() {
    let model = refs_model();
    let temp = temp_dir("create-new");
    let path = temp.join("link.state");
    write_new(&path, &model, &refs_tables()).unwrap();
    let original = std::fs::read(&path).unwrap();
    assert_eq!(
        write_new(&path, &model, &refs_tables()).unwrap_err(),
        StateArtifactError::RefuseOverwrite { path: path.clone() }
    );
    assert_eq!(std::fs::read(&path).unwrap(), original);
    std::fs::remove_dir_all(temp).unwrap();
}

#[test]
fn writer_uses_model_order_even_when_inputs_are_permuted() {
    let model = refs_model();
    let expected = write_bytes(&model, &refs_tables());
    let mut tables = refs_tables();
    tables.reverse();
    for table in &mut tables {
        table.columns.reverse();
    }
    assert_eq!(write_bytes(&model, &tables), expected);
}

#[test]
fn reader_checks_later_target_row_count_before_source_ref_values() {
    let error = table_init_error_for_bytes(
        "source-before-target-reader",
        &two_box_source_before_target_mismatch_bytes(),
        &load_model("examples/two_box.json"),
    );
    assert_eq!(
        error,
        StateArtifactError::RowCountMismatch {
            box_name: "population".to_owned(),
            table: "Group".to_owned(),
            expected: 1,
            actual: 2,
        }
    );
}

#[test]
fn writer_checks_later_target_row_count_before_source_ref_values() {
    let model = load_model("examples/two_box.json");
    let mut tables = two_box_tables();
    let ColumnData::Ref(values) = &mut tables[0].columns[1].data else {
        panic!("population.Person.group must be a Ref column");
    };
    values[0] = 1;
    tables[1].row_count = 2;

    let temp = temp_dir("source-before-target-writer");
    let error = write(temp.join("invalid.state"), &model, &tables).unwrap_err();
    std::fs::remove_dir_all(temp).unwrap();
    assert_eq!(
        error,
        StateArtifactError::RowCountMismatch {
            box_name: "population".to_owned(),
            table: "Group".to_owned(),
            expected: 1,
            actual: 2,
        }
    );
}

#[test]
fn strict_header_shape_errors_are_distinct_and_deterministic() {
    let base = parse_parts(&write_bytes(&refs_model(), &refs_tables()));

    let mut unknown = base.clone();
    unknown.header =
        unknown
            .header
            .replacen("{\"schema_version\"", "{\"extra\":0,\"schema_version\"", 1);
    assert!(matches!(
        read_bytes_error("unknown-field", &assemble(&unknown)),
        StateArtifactError::InvalidHeaderJson(message) if message.contains("unknown field 'extra'")
    ));

    let mut duplicate = base.clone();
    duplicate.header = duplicate.header.replacen(
        "{\"schema_version\":\"sembla.state/v1\"",
        "{\"schema_version\":\"sembla.state/v1\",\"schema_version\":\"sembla.state/v1\"",
        1,
    );
    assert!(matches!(
        read_bytes_error("duplicate-field", &assemble(&duplicate)),
        StateArtifactError::InvalidHeaderJson(message) if message.contains("duplicate field 'schema_version'")
    ));

    let mut metadata = base.clone();
    metadata.header = metadata.header.replace(
        "{\"name\":\"rate\",\"type\":\"real\"}",
        "{\"name\":\"rate\",\"type\":\"real\",\"variant_count\":1}",
    );
    assert!(matches!(
        read_bytes_error("metadata", &assemble(&metadata)),
        StateArtifactError::InvalidColumnMetadata { column, .. } if column == "rate"
    ));

    let mut unresolved = base.clone();
    unresolved.header = unresolved.header.replace(
        "\"ref_target\":{\"box\":\"world\",\"table\":\"Area\"}",
        "\"ref_target\":{\"box\":\"world\",\"table\":\"Missing\"}",
    );
    assert!(matches!(
        read_bytes_error("unresolved", &assemble(&unresolved)),
        StateArtifactError::UnresolvedRefTarget { column, target_table, .. }
            if column == "area" && target_table == "Missing"
    ));
}

#[test]
fn declaration_order_extra_column_and_ref_target_are_enforced() {
    let base = parse_parts(&write_bytes(&refs_model(), &refs_tables()));

    let mut table_order = base.clone();
    table_order.header = header_with_tables(&format!("{PERSON_HEADER},{AREA_HEADER}"));
    table_order.columns.swap(0, 1);
    assert!(matches!(
        table_init_error("table-order", &table_order),
        StateArtifactError::TableOrderMismatch { position: 0, .. }
    ));

    let mut extra_column = base.clone();
    extra_column.header = extra_column.header.replace(
        "{\"name\":\"area\",\"ref_target\":{\"box\":\"world\",\"table\":\"Area\"},\"type\":\"ref\"}",
        "{\"name\":\"area\",\"ref_target\":{\"box\":\"world\",\"table\":\"Area\"},\"type\":\"ref\"},{\"name\":\"extra\",\"type\":\"int\"}",
    );
    extra_column.columns[1].push(vec![0; 24]);
    assert!(matches!(
        table_init_error("extra-column", &extra_column),
        StateArtifactError::ExtraColumn { column, .. } if column == "extra"
    ));

    let mut target = base.clone();
    target.header = target.header.replace(
        "\"ref_target\":{\"box\":\"world\",\"table\":\"Area\"}",
        "\"ref_target\":{\"box\":\"world\",\"table\":\"Person\"}",
    );
    assert!(matches!(
        table_init_error("target", &target),
        StateArtifactError::RefTargetMismatch { column, expected, actual, .. }
            if column == "area" && expected == "world.Area" && actual == "world.Person"
    ));
}

#[test]
fn writer_rejects_invalid_initializer_values_without_repair() {
    let model = refs_model();
    let temp = temp_dir("writer-errors");
    let path = temp.join("state.bin");

    let mut wrong_rows = refs_tables();
    wrong_rows[1].row_count = 4;
    assert!(matches!(
        write(&path, &model, &wrong_rows).unwrap_err(),
        StateArtifactError::RowCountMismatch {
            expected: 3,
            actual: 4,
            ..
        }
    ));

    let mut wrong_enum = refs_tables();
    wrong_enum[1].columns[1].data = ColumnData::Enum(vec![0, 2, 0]);
    assert!(matches!(
        write(&path, &model, &wrong_enum).unwrap_err(),
        StateArtifactError::EnumValueOutOfRange {
            row: 1,
            value: 2,
            ..
        }
    ));

    let mut wrong_ref = refs_tables();
    wrong_ref[1].columns[2].data = ColumnData::Ref(vec![0, 2, 0]);
    assert!(matches!(
        write(&path, &model, &wrong_ref).unwrap_err(),
        StateArtifactError::RefValueOutOfRange {
            row: 1,
            value: 2,
            target_rows: 2,
            ..
        }
    ));
    std::fs::remove_dir_all(temp).unwrap();
}

#[test]
fn state_hash_has_frozen_domain_and_exact_byte_payload() {
    let path = repository_path("fixtures/state/refs_small.state");
    let bytes = std::fs::read(&path).unwrap();
    let record = state_artifact_hash(&path).unwrap();
    assert_eq!(record.algorithm, "sha256");
    assert_eq!(record.domain, STATE_ARTIFACT_HASH_DOMAIN);
    assert_eq!(record.digest.len(), 64);
    let mut expected = String::new();
    for byte in sembla_ir::domain_digest(STATE_ARTIFACT_HASH_DOMAIN, &bytes) {
        use std::fmt::Write as _;
        write!(&mut expected, "{byte:02x}").unwrap();
    }
    assert_eq!(record.digest, expected);
}

#[test]
fn sniff_magic_distinguishes_state_legacy_and_unknown() {
    let temp = temp_dir("sniff");
    let legacy = temp.join("legacy.bin");
    let unknown = temp.join("unknown.bin");
    std::fs::write(&legacy, b"SEMBLA_POP\0\0payload").unwrap();
    std::fs::write(&unknown, b"not-a-format").unwrap();
    assert_eq!(
        sniff_magic(repository_path("fixtures/state/refs_small.state")).unwrap(),
        StateKind::SemblaState
    );
    assert_eq!(sniff_magic(&legacy).unwrap(), StateKind::SemblaPop);
    assert_eq!(sniff_magic(&unknown).unwrap(), StateKind::Unknown);
    std::fs::remove_dir_all(temp).unwrap();
}

#[test]
fn invalid_fixtures_match_deterministic_regeneration_bytes() {
    for (name, expected) in invalid_fixture_bytes() {
        assert_eq!(
            std::fs::read(invalid_path(name)).unwrap(),
            expected,
            "{name}"
        );
    }
}

#[test]
fn wrong_magic_fixture_has_specific_error() {
    assert_eq!(
        read(invalid_path("wrong_magic.state")).unwrap_err(),
        StateArtifactError::InvalidMagic
    );
}

#[test]
fn wrong_schema_fixture_has_specific_error() {
    assert_eq!(
        read(invalid_path("wrong_schema_version.state")).unwrap_err(),
        StateArtifactError::UnsupportedSchemaVersion("sembla.state/v999".to_owned())
    );
}

#[test]
fn missing_table_fixture_has_specific_error() {
    assert!(
        matches!(model_error("missing_table.state"), StateArtifactError::MissingTable { table, .. } if table == "Person")
    );
}

#[test]
fn extra_table_fixture_has_specific_error() {
    assert!(
        matches!(model_error("extra_table.state"), StateArtifactError::ExtraTable { table, .. } if table == "Extra")
    );
}

#[test]
fn row_count_fixture_has_specific_error() {
    assert!(
        matches!(model_error("row_count_mismatch.state"), StateArtifactError::RowCountMismatch { table, expected: 3, actual: 4, .. } if table == "Person")
    );
}

#[test]
fn missing_column_fixture_has_specific_error() {
    assert!(
        matches!(model_error("missing_column.state"), StateArtifactError::MissingColumn { column, .. } if column == "age")
    );
}

#[test]
fn type_mismatch_fixture_has_specific_error() {
    assert!(
        matches!(model_error("type_mismatch.state"), StateArtifactError::ColumnTypeMismatch { column, expected, actual, .. } if column == "age" && expected == "int" && actual == "real")
    );
}

#[test]
fn variant_count_fixture_has_specific_error() {
    assert!(
        matches!(model_error("variant_count_mismatch.state"), StateArtifactError::VariantCountMismatch { column, expected: 2, actual: 3, .. } if column == "status")
    );
}

#[test]
fn enum_range_fixture_has_specific_error() {
    assert!(
        matches!(model_error("enum_out_of_range.state"), StateArtifactError::EnumValueOutOfRange { column, row: 0, value: 2, variant_count: 2, .. } if column == "status")
    );
}

#[test]
fn ref_range_fixture_has_specific_error() {
    assert!(
        matches!(model_error("ref_out_of_range.state"), StateArtifactError::RefValueOutOfRange { column, row: 0, value: 2, target_rows: 2, .. } if column == "area")
    );
}

#[test]
fn truncated_blob_fixture_has_specific_error() {
    assert!(matches!(
        read(invalid_path("truncated_blob.state")).unwrap_err(),
        StateArtifactError::TruncatedPayload { .. }
    ));
}

#[test]
fn trailing_bytes_fixture_has_specific_error() {
    assert!(matches!(
        read(invalid_path("trailing_bytes.state")).unwrap_err(),
        StateArtifactError::TrailingBytes { .. }
    ));
}

#[test]
fn noncanonical_header_fixture_has_specific_error() {
    assert_eq!(
        read(invalid_path("non_canonical_header.state")).unwrap_err(),
        StateArtifactError::NonCanonicalHeader
    );
}

#[test]
#[ignore = "explicit state-fixture regeneration only"]
fn regenerate_state_fixtures() {
    let root = repository_path("fixtures/state");
    std::fs::create_dir_all(root.join("invalid")).unwrap();
    std::fs::write(
        root.join("refs_small.state"),
        write_bytes(&refs_model(), &refs_tables()),
    )
    .unwrap();
    let two_box_model = load_model("examples/two_box.json");
    std::fs::write(
        root.join("two_box_small.state"),
        write_bytes(&two_box_model, &two_box_tables()),
    )
    .unwrap();
    for (name, bytes) in invalid_fixture_bytes() {
        std::fs::write(root.join("invalid").join(name), bytes).unwrap();
    }
}
