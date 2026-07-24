use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::sync::atomic::{AtomicU64, Ordering};

use sembla_ir::{FeatureSet, ParsedInput, GROUPED_OBSERVATIONS_FEATURE};
use sembla_runtime::state::ColumnData;
use sembla_runtime::state_artifact::{read, to_table_inits};

static TEMP_COUNTER: AtomicU64 = AtomicU64::new(0);

const MODEL: &str = "fixtures/demographic/demographic_slots.json";
const PLAN: &str = "fixtures/demographic/demographic_slots.plan.json";
const FULL: &str = "fixtures/demographic/benchmark/demographic_slots.full.json";
const NO_AGEING: &str = "fixtures/demographic/benchmark/demographic_slots.no-ageing.json";
const NO_GROUPED: &str = "fixtures/demographic/benchmark/demographic_slots.no-grouped.json";

fn repository_path(relative: impl AsRef<Path>) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join(relative)
}

fn temp_dir(label: &str) -> PathBuf {
    let sequence = TEMP_COUNTER.fetch_add(1, Ordering::Relaxed);
    let path = std::env::temp_dir().join(format!(
        "sembla-synth-state-{label}-{}-{sequence}",
        std::process::id()
    ));
    if path.exists() {
        std::fs::remove_dir_all(&path).unwrap();
    }
    std::fs::create_dir_all(&path).unwrap();
    path
}

fn assert_success(output: &Output) {
    assert!(
        output.status.success(),
        "stdout={}\nstderr={}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(output.stderr.is_empty());
}

fn synth(input: &str, out: &Path) -> Output {
    Command::new(env!("CARGO_BIN_EXE_sembla"))
        .arg("synth-state")
        .arg("--model")
        .arg(repository_path(input))
        .arg("--slots")
        .arg("10000")
        .arg("--areas")
        .arg("4")
        .arg("--present-fraction")
        .arg("0.8")
        .arg("--streams")
        .arg("birth:600,overseas:250,internal:150")
        .arg("--seed")
        .arg("9009")
        .arg("--out")
        .arg(out)
        .output()
        .unwrap()
}

fn companion(out: &Path) -> PathBuf {
    PathBuf::from(format!("{}.model.json", out.display()))
}

fn legacy_model(path: impl AsRef<Path>) -> sembla_ir::Model {
    let source = std::fs::read_to_string(path).unwrap();
    match sembla_ir::parse_input(&source).unwrap() {
        ParsedInput::LegacyModel(model) => model,
        ParsedInput::Plan(_) => panic!("expected legacy benchmark model"),
    }
}

#[test]
fn synth_state_is_deterministic_valid_and_loadable_at_10k() {
    let temp = temp_dir("deterministic");
    let first = temp.join("first.state");
    let second = temp.join("second.state");
    assert_success(&synth(MODEL, &first));
    assert_success(&synth(MODEL, &second));
    assert_eq!(
        std::fs::read(&first).unwrap(),
        std::fs::read(&second).unwrap()
    );
    assert_eq!(
        std::fs::read(companion(&first)).unwrap(),
        std::fs::read(companion(&second)).unwrap()
    );

    let resized = legacy_model(companion(&first));
    let demographic = resized
        .boxes
        .iter()
        .find(|model_box| model_box.name == "demographic")
        .unwrap();
    assert_eq!(
        demographic
            .tables
            .iter()
            .find(|table| table.name == "area")
            .unwrap()
            .size_hint,
        4
    );
    for table in demographic
        .tables
        .iter()
        .filter(|table| table.name != "area")
    {
        assert_eq!(table.size_hint, 10_000);
    }

    let features = FeatureSet::from([GROUPED_OBSERVATIONS_FEATURE.to_owned()]);
    let validated = sembla_ir::validate_with_features(resized, &features).unwrap();
    let artifact = read(&first).unwrap();
    let tables = to_table_inits(&artifact, &validated).unwrap();
    let person = tables
        .iter()
        .find(|table| table.table_name == "person_slot")
        .unwrap();
    assert_eq!(person.row_count, 10_000);
    let occupancy = person
        .columns
        .iter()
        .find(|column| column.name == "occupancy")
        .unwrap();
    let ColumnData::Enum(values) = &occupancy.data else {
        panic!("occupancy must be enum")
    };
    assert_eq!(values.iter().filter(|value| **value == 1).count(), 8_000);
    let entry_stream = person
        .columns
        .iter()
        .find(|column| column.name == "entry_stream")
        .unwrap();
    let ColumnData::Enum(streams) = &entry_stream.data else {
        panic!("entry_stream must be enum")
    };
    let mut vacant_stream_counts = [0_usize; 3];
    for (occupancy, stream) in values.iter().zip(streams) {
        if *occupancy == 0 {
            vacant_stream_counts[*stream as usize] += 1;
        }
    }
    assert_eq!(vacant_stream_counts, [1_200, 500, 300]);

    // The canonical model has reductions that reject an empty run. The
    // benchmark's load-only working copy removes summaries (which do not
    // participate in the state schema) so --ticks 0 measures a real load
    // without executing a transition.
    let mut load_only = legacy_model(companion(&first));
    load_only.summaries.clear();
    let load_model = temp.join("load-only.json");
    std::fs::write(
        &load_model,
        sembla_ir::to_canonical_json(&load_only).unwrap(),
    )
    .unwrap();
    let run = Command::new(env!("CARGO_BIN_EXE_sembla"))
        .arg("run")
        .arg(&load_model)
        .arg("--seed")
        .arg("9009")
        .arg("--ticks")
        .arg("0")
        .arg("--population")
        .arg(&first)
        .arg("--enable")
        .arg(GROUPED_OBSERVATIONS_FEATURE)
        .output()
        .unwrap();
    assert_success(&run);
}

#[test]
fn synth_state_accepts_a_plan_and_emits_a_legacy_companion() {
    let temp = temp_dir("plan");
    let out = temp.join("plan.state");
    assert_success(&synth(PLAN, &out));
    assert!(matches!(
        sembla_ir::parse_input(&std::fs::read_to_string(companion(&out)).unwrap()).unwrap(),
        ParsedInput::LegacyModel(_)
    ));
}

#[test]
fn benchmark_variants_are_exact_canonical_subtractions() {
    let canonical = legacy_model(repository_path(MODEL));
    assert_eq!(legacy_model(repository_path(FULL)), canonical);

    let mut expected_no_ageing = canonical.clone();
    for model_box in &mut expected_no_ageing.boxes {
        model_box
            .transitions
            .retain(|transition| transition.name != "age_monthly");
    }
    assert_eq!(legacy_model(repository_path(NO_AGEING)), expected_no_ageing);

    let mut expected_no_grouped = canonical;
    for model_box in &mut expected_no_grouped.boxes {
        model_box.grouped_views.clear();
    }
    assert_eq!(
        legacy_model(repository_path(NO_GROUPED)),
        expected_no_grouped
    );
}

#[test]
fn synth_state_rejects_models_without_the_documented_demographic_roles() {
    let temp = temp_dir("reject");
    let output = synth("examples/sir.json", &temp.join("sir.state"));
    assert_eq!(output.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&output.stderr)
        .contains("requires exactly one box named 'demographic'"));
}
