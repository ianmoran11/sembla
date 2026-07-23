use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::sync::atomic::{AtomicU64, Ordering};

use sembla_runtime::eval::ParamEnv;
use sembla_runtime::executor::{run, run_tick, TickError};
use sembla_runtime::state::{ColumnData, ColumnInit, StateStore, TableInit};
use sembla_runtime::state_artifact::{read, to_table_inits, write};

const ROWS: usize = 100;
const SEED: u64 = 57;
static TEMP_COUNTER: AtomicU64 = AtomicU64::new(0);

fn repository_path(relative: impl AsRef<Path>) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join(relative)
}

fn fixture_model_path() -> PathBuf {
    repository_path("crates/sembla-cli/tests/fixtures/contest_competing_exits.json")
}

fn temp_dir(label: &str) -> PathBuf {
    let sequence = TEMP_COUNTER.fetch_add(1, Ordering::Relaxed);
    let path = std::env::temp_dir().join(format!(
        "sembla-contest-{label}-{}-{sequence}",
        std::process::id()
    ));
    if path.exists() {
        std::fs::remove_dir_all(&path).unwrap();
    }
    std::fs::create_dir_all(&path).unwrap();
    path
}

fn validated_model() -> sembla_ir::ValidatedModel {
    let source = std::fs::read_to_string(fixture_model_path()).unwrap();
    sembla_ir::validate(sembla_ir::parse_json(&source).unwrap()).unwrap()
}

fn identity_ref_tables() -> Vec<TableInit> {
    vec![
        TableInit::new("World", "slot_resource", ROWS, Vec::new()),
        TableInit::new(
            "World",
            "slot",
            ROWS,
            vec![
                ColumnInit::new("occupancy", ColumnData::Enum(vec![0; ROWS])),
                ColumnInit::new("cause", ColumnData::Enum(vec![0; ROWS])),
                ColumnInit::new("slot_resource", ColumnData::Ref((0..ROWS as u32).collect())),
            ],
        ),
    ]
}

fn assert_success(output: &Output) {
    assert!(
        output.status.success(),
        "stdout={}\nstderr={}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

fn cli_run(initial_state: &Path, output: &Path, exported_state: &Path) -> Output {
    Command::new(env!("CARGO_BIN_EXE_sembla"))
        .arg("run")
        .arg(fixture_model_path())
        .arg("--population")
        .arg(initial_state)
        .args(["--seed", &SEED.to_string(), "--ticks", "5", "--out"])
        .arg(output)
        .arg("--export-state")
        .arg(exported_state)
        .output()
        .unwrap()
}

fn csv_rows(csv: &str) -> (Vec<String>, Vec<BTreeMap<String, usize>>) {
    let mut lines = csv.lines().filter(|line| !line.starts_with('#'));
    let headers = lines
        .next()
        .unwrap()
        .split(',')
        .map(str::to_owned)
        .collect::<Vec<_>>();
    let rows = lines
        .map(|line| {
            headers
                .iter()
                .zip(line.split(','))
                .map(|(name, value)| (name.clone(), value.parse::<usize>().unwrap()))
                .collect()
        })
        .collect();
    (headers, rows)
}

#[test]
fn lean_surface_competing_exits_are_reproducible_and_single_cause() {
    let temp = temp_dir("end-to-end");
    let model = validated_model();
    let initial_state = temp.join("initial.state");
    write(&initial_state, &model, &identity_ref_tables()).unwrap();

    let first_csv = temp.join("first.csv");
    let second_csv = temp.join("second.csv");
    let first_state = temp.join("first.state");
    let second_state = temp.join("second.state");
    let first = cli_run(&initial_state, &first_csv, &first_state);
    let second = cli_run(&initial_state, &second_csv, &second_state);
    assert_success(&first);
    assert_success(&second);

    assert_eq!(
        first.stdout, second.stdout,
        "run hashes must be reproducible"
    );
    assert_eq!(
        std::fs::read(&first_csv).unwrap(),
        std::fs::read(&second_csv).unwrap(),
        "view traces must be bitwise reproducible"
    );
    assert_eq!(
        std::fs::read(&first_state).unwrap(),
        std::fs::read(&second_state).unwrap(),
        "final state artifacts must be bitwise reproducible"
    );

    let csv = std::fs::read_to_string(&first_csv).unwrap();
    let (headers, rows) = csv_rows(&csv);
    assert!(headers.iter().any(|name| name == "deferred_total"));
    assert_eq!(rows.len(), 5);
    for row in &rows {
        assert_eq!(row["cause_a"] + row["cause_b"] + row["present"], ROWS);
    }
    assert!(rows[0]["cause_a"] > 0, "fixed seed must exercise exit_a");
    assert!(rows[0]["cause_b"] > 0, "fixed seed must exercise exit_b");
    assert_eq!(rows[0]["deferred_total"], ROWS);
    assert!(rows[1..].iter().all(|row| row["deferred_total"] == 0));

    std::fs::remove_dir_all(temp).unwrap();
}

#[test]
fn deferred_losers_reach_the_existing_saturation_report() {
    let temp = temp_dir("saturation");
    let model = validated_model();
    let artifact_path = temp.join("initial.state");
    write(&artifact_path, &model, &identity_ref_tables()).unwrap();
    let artifact = read(&artifact_path).unwrap();
    let mut state = StateStore::new(&model, to_table_inits(&artifact, &model).unwrap()).unwrap();

    let report = run(&model, &mut state, &ParamEnv::defaults(&model), SEED, 1).unwrap();
    assert_eq!(
        report.ticks[0].deferred_per_resource_table,
        vec![("slot_resource".to_owned(), ROWS)]
    );
    assert_eq!(report.warnings.len(), 1);
    let warning = &report.warnings[0];
    assert_eq!(warning.table, "slot_resource");
    assert_eq!((warning.deferred_count, warning.fired_count), (ROWS, ROWS));

    std::fs::remove_dir_all(temp).unwrap();
}

#[test]
fn removing_surface_claims_exposes_the_runtime_double_write_defense() {
    let model = validated_model();
    let mut claimless_model = model.clone().into_model();
    for model_box in &mut claimless_model.boxes {
        for transition in &mut model_box.transitions {
            transition.contests.clear();
        }
    }
    let claimless = sembla_ir::validate(claimless_model).unwrap();
    let mut state = StateStore::new(&claimless, identity_ref_tables()).unwrap();

    let error = run_tick(
        &claimless,
        &mut state,
        &ParamEnv::defaults(&claimless),
        SEED,
        0,
    )
    .unwrap_err();
    assert!(matches!(error, TickError::DoubleWrite { .. }));
    let message = error.to_string();
    assert!(message.contains("'exit_a' (rule 0)"));
    assert!(message.contains("'exit_b' (rule 1)"));
}

#[test]
#[ignore = "fixture regeneration requires the pinned Lean toolchain; run explicitly"]
fn contest_fixture_matches_the_canonical_lean_export() {
    let temp = temp_dir("regenerate");
    let exporter = temp.join("ExportContest.lean");
    let generated = temp.join("contest_competing_exits.json");
    std::fs::write(
        &exporter,
        r#"import Sembla.ContestTests

open Sembla.ContestTests

def main (args : List String) : IO UInt32 := do
  match args with
  | [path] => IO.FS.writeFile path competingExitsModelJson; pure 0
  | _ => pure 2
"#,
    )
    .unwrap();
    let frontend = repository_path("frontend");
    let build = Command::new("lake")
        .args(["build", "Sembla.ContestTests"])
        .current_dir(&frontend)
        .output()
        .unwrap();
    assert_success(&build);
    let export = Command::new("lake")
        .args(["env", "lean", "--run"])
        .arg(&exporter)
        .arg(&generated)
        .current_dir(frontend)
        .output()
        .unwrap();
    assert_success(&export);
    assert_eq!(
        std::fs::read(generated).unwrap(),
        std::fs::read(fixture_model_path()).unwrap()
    );
    std::fs::remove_dir_all(temp).unwrap();
}
