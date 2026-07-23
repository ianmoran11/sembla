use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::time::{SystemTime, UNIX_EPOCH};

use sembla_runtime::population::SyntheticPopulation;
use sembla_runtime::state::{ColumnData, ColumnInit, TableInit};
use sembla_runtime::state_artifact::write;

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
        "sembla-cli-state-{label}-{}-{nonce}",
        std::process::id()
    ));
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

fn validated_input(relative: &str) -> sembla_ir::ValidatedModel {
    let source = std::fs::read_to_string(repository_path(relative)).unwrap();
    match sembla_ir::parse_input(&source).unwrap() {
        sembla_ir::ParsedInput::LegacyModel(model) => sembla_ir::validate(model).unwrap(),
        sembla_ir::ParsedInput::Plan(plan) => sembla_ir::validate_plan(&plan)
            .unwrap()
            .model_with_rule_words(),
    }
}

fn zero_tables(model: &sembla_ir::ValidatedModel) -> Vec<TableInit> {
    model
        .model()
        .boxes
        .iter()
        .flat_map(|model_box| {
            model_box.tables.iter().map(move |table| {
                let rows = usize::try_from(table.size_hint).unwrap();
                let columns = table
                    .attrs
                    .iter()
                    .map(|attr| {
                        let data = match &attr.ty {
                            sembla_ir::AttrType::Real => ColumnData::Real(vec![0.0; rows]),
                            sembla_ir::AttrType::Int => ColumnData::Int(vec![0; rows]),
                            sembla_ir::AttrType::Enum { .. } => ColumnData::Enum(vec![0; rows]),
                            sembla_ir::AttrType::Ref { .. } => ColumnData::Ref(vec![0; rows]),
                        };
                        ColumnInit::new(&attr.name, data)
                    })
                    .collect();
                TableInit::new(&model_box.name, &table.name, rows, columns)
            })
        })
        .collect()
}

fn state_for_input(relative: &str, path: &Path) {
    let model = validated_input(relative);
    write(path, &model, &zero_tables(&model)).unwrap();
}

#[test]
fn state_hash_prints_frozen_one_line_record() {
    let output = Command::new(env!("CARGO_BIN_EXE_sembla"))
        .arg("state-hash")
        .arg(repository_path("fixtures/state/refs_small.state"))
        .output()
        .unwrap();
    assert_success(&output);
    assert_eq!(
        String::from_utf8(output.stdout).unwrap(),
        "state sha256 sembla.state-artifact/v1 bdf363c0b12a157f9adffcca3591b33026e4907d99ec9a871623857935140cdc\n"
    );
}

#[test]
fn legacy_model_state_run_is_byte_deterministic_and_matches_golden() {
    let temp = temp_dir("legacy-run");
    let model = repository_path("fixtures/state/models/refs_small.json");
    let state = repository_path("fixtures/state/refs_small.state");
    let outputs = [temp.join("first.csv"), temp.join("second.csv")];
    let run_once = |out: &Path| {
        Command::new(env!("CARGO_BIN_EXE_sembla"))
            .arg("run")
            .arg(&model)
            .arg("--population")
            .arg(&state)
            .args(["--seed", "7", "--ticks", "4", "--out"])
            .arg(out)
            .output()
            .unwrap()
    };
    let first = run_once(&outputs[0]);
    let second = run_once(&outputs[1]);
    assert_success(&first);
    assert_success(&second);
    assert_eq!(first.stdout, second.stdout);
    assert_eq!(
        String::from_utf8(first.stdout).unwrap(),
        concat!(
            "results_sha256=dfe14fb1e043d656512ec11ed30f7b5d71daa307e0dbbedc19b798e9f3b83925 ",
            "final_state_sha256=080fea1e10fdf889eca204624e358d54ddd24cc56359b09e63798ca1b4b65217 ",
            "observation_sha256=2c0664afac18f766ff9366681be3f5bc1e309f78889484d6152722b78c7a88bf\n"
        )
    );
    let golden = std::fs::read(repository_path(
        "fixtures/state/goldens/refs_small.seed7.ticks4.csv",
    ))
    .unwrap();
    assert_eq!(std::fs::read(&outputs[0]).unwrap(), golden);
    assert_eq!(std::fs::read(&outputs[1]).unwrap(), golden);
    std::fs::remove_dir_all(temp).unwrap();
}

#[test]
fn plan_envelope_accepts_a_declaration_ordered_state_artifact() {
    let temp = temp_dir("plan-run");
    let state = temp.join("plan.state");
    let output_path = temp.join("plan.csv");
    state_for_input("fixtures/plans/two_box.plan.json", &state);
    let output = Command::new(env!("CARGO_BIN_EXE_sembla"))
        .arg("run")
        .arg(repository_path("fixtures/plans/two_box.plan.json"))
        .arg("--population")
        .arg(&state)
        .args(["--seed", "55", "--ticks", "2", "--out"])
        .arg(&output_path)
        .output()
        .unwrap();
    assert_success(&output);
    assert!(output_path.is_file());
    std::fs::remove_dir_all(temp).unwrap();
}

#[test]
fn state_artifacts_dispatch_through_sweep_and_verify_run() {
    let temp = temp_dir("sweep-verify");
    let model = repository_path("fixtures/state/models/refs_small.json");
    let state = repository_path("fixtures/state/refs_small.state");
    let sweep_dir = temp.join("sweep");
    let sweep = Command::new(env!("CARGO_BIN_EXE_sembla"))
        .arg("sweep")
        .arg(&model)
        .arg("--population")
        .arg(&state)
        .args(["--seed", "7", "--draws", "2", "--ticks", "2", "--out"])
        .arg(&sweep_dir)
        .output()
        .unwrap();
    assert_success(&sweep);

    let run_csv = temp.join("run.csv");
    let run = Command::new(env!("CARGO_BIN_EXE_sembla"))
        .arg("run")
        .arg(&model)
        .arg("--population")
        .arg(&state)
        .args(["--seed", "7", "--ticks", "2", "--out"])
        .arg(&run_csv)
        .output()
        .unwrap();
    assert_success(&run);
    let verify = Command::new(env!("CARGO_BIN_EXE_sembla"))
        .arg("verify-run")
        .arg(format!("{}.manifest.json", run_csv.display()))
        .arg(&model)
        .arg("--population")
        .arg(&state)
        .output()
        .unwrap();
    assert_success(&verify);
    assert_eq!(verify.stdout, b"verified 1 execution(s)\n");
    std::fs::remove_dir_all(temp).unwrap();
}

#[test]
fn state_artifacts_dispatch_through_compare() {
    let temp = temp_dir("compare");
    let state = temp.join("sir.state");
    state_for_input("examples/sir.json", &state);
    let out = temp.join("compare.csv");
    let output = Command::new(env!("CARGO_BIN_EXE_sembla"))
        .arg("compare")
        .arg(repository_path("examples/sir.json"))
        .arg(repository_path("examples/sir.json"))
        .arg("--population")
        .arg(&state)
        .args(["--seed", "3", "--ticks", "1", "--out"])
        .arg(&out)
        .output()
        .unwrap();
    assert_success(&output);
    assert!(out.is_file());
    std::fs::remove_dir_all(temp).unwrap();
}

#[test]
fn diff_backends_loads_state_before_gpu_dispatch() {
    let output = Command::new(env!("CARGO_BIN_EXE_sembla"))
        .arg("diff-backends")
        .arg(repository_path("fixtures/state/models/refs_small.json"))
        .arg("--population")
        .arg(repository_path("fixtures/state/refs_small.state"))
        .args(["--seed", "7", "--ticks", "1"])
        .output()
        .unwrap();
    if !output.status.success() {
        let stderr = String::from_utf8(output.stderr).unwrap();
        assert!(stderr.contains("cuda backend unavailable"), "{stderr}");
        assert!(!stderr.contains("state artifact"), "{stderr}");
    }
}

#[test]
fn legacy_sembla_pop_still_routes_to_the_frozen_loader() {
    let temp = temp_dir("legacy-pop");
    let population = temp.join("legacy.pop.bin");
    SyntheticPopulation::generate(20, 4, 2, 9)
        .unwrap()
        .write(&population)
        .unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_sembla"))
        .arg("run")
        .arg(repository_path("examples/sir.json"))
        .arg("--population")
        .arg(&population)
        .args(["--seed", "9", "--ticks", "1"])
        .output()
        .unwrap();
    assert_success(&output);
    std::fs::remove_dir_all(temp).unwrap();
}

#[test]
fn unknown_population_magic_names_both_supported_formats() {
    let temp = temp_dir("unknown");
    let population = temp.join("unknown.bin");
    std::fs::write(&population, b"UNKNOWN_MAGIC").unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_sembla"))
        .arg("run")
        .arg(repository_path("fixtures/state/models/refs_small.json"))
        .arg("--population")
        .arg(&population)
        .args(["--seed", "1", "--ticks", "1"])
        .output()
        .unwrap();
    assert!(!output.status.success());
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(
        stderr.contains("supported formats: SEMBLA_POP, SEMBLA_STATE"),
        "{stderr}"
    );
    std::fs::remove_dir_all(temp).unwrap();
}
