use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::sync::atomic::{AtomicU64, Ordering};

use sembla_ir::{FeatureSet, ParsedInput, GROUPED_OBSERVATIONS_FEATURE};
use sembla_runtime::state::{ColumnData, ColumnInit, TableInit};
use sembla_runtime::state_artifact::write;

static TEMP_COUNTER: AtomicU64 = AtomicU64::new(0);

fn repository_path(relative: impl AsRef<Path>) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join(relative)
}

fn fixture_model_path() -> PathBuf {
    repository_path("crates/sembla-cli/tests/fixtures/grouped_observation.json")
}

fn fixture_plan_path() -> PathBuf {
    repository_path("fixtures/plans/grouped_observation.plan.json")
}

fn temp_dir(label: &str) -> PathBuf {
    let sequence = TEMP_COUNTER.fetch_add(1, Ordering::Relaxed);
    let path = std::env::temp_dir().join(format!(
        "sembla-grouped-{label}-{}-{sequence}",
        std::process::id()
    ));
    if path.exists() {
        std::fs::remove_dir_all(&path).unwrap();
    }
    std::fs::create_dir_all(&path).unwrap();
    path
}

fn raw_model() -> sembla_ir::Model {
    let source = std::fs::read_to_string(fixture_model_path()).unwrap();
    sembla_ir::parse_json(&source).unwrap()
}

fn enabled() -> FeatureSet {
    FeatureSet::from([GROUPED_OBSERVATIONS_FEATURE.to_owned()])
}

fn validated_model() -> sembla_ir::ValidatedModel {
    sembla_ir::validate_with_features(raw_model(), &enabled()).unwrap()
}

fn initial_tables() -> Vec<TableInit> {
    vec![
        TableInit::new("world", "area", 12, Vec::new()),
        TableInit::new(
            "world",
            "person_slot",
            5,
            vec![
                ColumnInit::new("sex", ColumnData::Enum(vec![0, 0, 1, 1, 0])),
                ColumnInit::new("area", ColumnData::Ref(vec![10, 2, 10, 2, 2])),
                ColumnInit::new("age_months", ColumnData::Int(vec![-1, 0, 59, 60, 120])),
                ColumnInit::new("occupancy", ColumnData::Enum(vec![0; 5])),
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

fn run_cli(model: &Path, state: &Path, out: &Path, exported: &Path, enabled: bool) -> Output {
    let mut command = Command::new(env!("CARGO_BIN_EXE_sembla"));
    command
        .arg("run")
        .arg(model)
        .args(["--seed", "81", "--ticks", "3", "--population"])
        .arg(state)
        .arg("--out")
        .arg(out)
        .arg("--export-state")
        .arg(exported);
    if enabled {
        // Repeatability plus manifest normalization: duplicate enables collapse.
        command.args([
            "--enable",
            GROUPED_OBSERVATIONS_FEATURE,
            "--enable",
            GROUPED_OBSERVATIONS_FEATURE,
        ]);
    }
    command.output().unwrap()
}

fn manifest(path: &Path) -> serde_json::Value {
    serde_json::from_slice(&std::fs::read(format!("{}.manifest.json", path.display())).unwrap())
        .unwrap()
}

fn verify_cli(manifest: &Path, model: &Path, state: &Path) -> Output {
    Command::new(env!("CARGO_BIN_EXE_sembla"))
        .arg("verify-run")
        .arg(manifest)
        .arg(model)
        .arg("--population")
        .arg(state)
        .output()
        .unwrap()
}

fn scalar_population_by_tick(csv: &str) -> BTreeMap<i64, i64> {
    let mut lines = csv.lines().filter(|line| !line.starts_with('#'));
    let headers = lines.next().unwrap().split(',').collect::<Vec<_>>();
    let tick = headers.iter().position(|name| *name == "tick").unwrap();
    let population = headers
        .iter()
        .position(|name| *name == "population")
        .unwrap();
    lines
        .map(|line| {
            let fields = line.split(',').collect::<Vec<_>>();
            (
                fields[tick].parse().unwrap(),
                fields[population].parse().unwrap(),
            )
        })
        .collect()
}

fn grouped_totals_by_tick(csv: &str) -> BTreeMap<i64, i64> {
    let mut totals = BTreeMap::new();
    for line in csv.lines().skip(1) {
        let fields = line.split(',').collect::<Vec<_>>();
        *totals.entry(fields[0].parse().unwrap()).or_default() +=
            fields.last().unwrap().parse::<i64>().unwrap();
    }
    totals
}

#[test]
fn grouped_observation_is_a_deterministic_sink_and_records_hashes() {
    let temp = temp_dir("sink");
    let grouped_model = validated_model();
    let state = temp.join("initial.state");
    write(&state, &grouped_model, &initial_tables()).unwrap();

    let mut base = raw_model();
    base.boxes[0].grouped_views.clear();
    let base_path = temp.join("base.json");
    std::fs::write(&base_path, sembla_ir::to_canonical_json(&base).unwrap()).unwrap();

    let grouped_out = temp.join("grouped.csv");
    let repeated_out = temp.join("repeated.csv");
    let base_out = temp.join("base.csv");
    let grouped_state = temp.join("grouped.state");
    let repeated_state = temp.join("repeated.state");
    let base_state = temp.join("base.state");
    let first = run_cli(
        &fixture_model_path(),
        &state,
        &grouped_out,
        &grouped_state,
        true,
    );
    let second = run_cli(
        &fixture_model_path(),
        &state,
        &repeated_out,
        &repeated_state,
        true,
    );
    let baseline = run_cli(&base_path, &state, &base_out, &base_state, true);
    assert_success(&first);
    assert_success(&second);
    assert_success(&baseline);

    assert_eq!(
        std::fs::read(&grouped_out).unwrap(),
        std::fs::read(&base_out).unwrap()
    );
    assert_eq!(
        std::fs::read(&grouped_state).unwrap(),
        std::fs::read(&base_state).unwrap()
    );
    assert_eq!(
        std::fs::read(&grouped_state).unwrap(),
        std::fs::read(&repeated_state).unwrap()
    );

    let grouped_csv = temp.join("grouped.grouped.population_cells.csv");
    let repeated_csv = temp.join("repeated.grouped.population_cells.csv");
    assert_eq!(
        std::fs::read(&grouped_csv).unwrap(),
        std::fs::read(repeated_csv).unwrap()
    );
    let grouped_text = std::fs::read_to_string(&grouped_csv).unwrap();
    assert_eq!(
        grouped_text.lines().next().unwrap(),
        "tick,sex,area,age_months,count"
    );
    assert!(grouped_text
        .lines()
        .any(|line| line.split(',').nth(3) == Some("-1")));
    assert_eq!(
        grouped_totals_by_tick(&grouped_text),
        scalar_population_by_tick(&std::fs::read_to_string(&grouped_out).unwrap())
    );

    let numeric_rows = grouped_text
        .lines()
        .skip(1)
        .filter(|line| line.starts_with("0,"))
        .map(|line| {
            let fields = line.split(',').collect::<Vec<_>>();
            let sex = match fields[1] {
                "male" => 0_i64,
                "female" => 1,
                other => panic!("unexpected sex {other}"),
            };
            (
                sex,
                fields[2].parse::<i64>().unwrap(),
                fields[3].parse::<i64>().unwrap(),
            )
        })
        .collect::<Vec<_>>();
    assert!(numeric_rows.windows(2).all(|pair| pair[0] <= pair[1]));

    let grouped_manifest = manifest(&grouped_out);
    let base_manifest = manifest(&base_out);
    assert_eq!(
        grouped_manifest["results_sha256"],
        base_manifest["results_sha256"]
    );
    assert_eq!(
        grouped_manifest["final_state_sha256"],
        base_manifest["final_state_sha256"]
    );
    assert_eq!(
        grouped_manifest["enabled_features"],
        serde_json::json!([GROUPED_OBSERVATIONS_FEATURE])
    );
    let record = &grouped_manifest["grouped_outputs"][0];
    assert_eq!(record["view"], "population_cells");
    assert_eq!(record["algorithm"], "sha256");
    assert_eq!(record["sha256"].as_str().unwrap().len(), 64);
    std::fs::remove_dir_all(temp).unwrap();
}

#[test]
fn flags_plan_artifacts_cuda_and_sweep_are_gated() {
    let temp = temp_dir("gates");
    let model = validated_model();
    let state = temp.join("initial.state");
    write(&state, &model, &initial_tables()).unwrap();

    let off = run_cli(
        &fixture_model_path(),
        &state,
        &temp.join("off.csv"),
        &temp.join("off.state"),
        false,
    );
    assert!(!off.status.success());
    assert!(String::from_utf8_lossy(&off.stderr).contains("requires --enable grouped-observations"));

    let unknown = Command::new(env!("CARGO_BIN_EXE_sembla"))
        .args([
            "run",
            fixture_model_path().to_str().unwrap(),
            "--seed",
            "1",
            "--ticks",
            "1",
            "--population",
        ])
        .arg(&state)
        .args(["--enable", "future-feature"])
        .output()
        .unwrap();
    assert!(!unknown.status.success());
    let unknown_error = String::from_utf8_lossy(&unknown.stderr);
    assert!(unknown_error.contains("unknown feature 'future-feature'"));
    assert!(unknown_error.contains("known features: grouped-observations"));

    let validate_plan = Command::new(env!("CARGO_BIN_EXE_sembla"))
        .args(["validate", fixture_plan_path().to_str().unwrap()])
        .output()
        .unwrap();
    assert_success(&validate_plan);
    let plan_off = run_cli(
        &fixture_plan_path(),
        &state,
        &temp.join("plan-off.csv"),
        &temp.join("plan-off.state"),
        false,
    );
    assert!(!plan_off.status.success());
    assert!(String::from_utf8_lossy(&plan_off.stderr)
        .contains("requires --enable grouped-observations"));
    let plan_on = run_cli(
        &fixture_plan_path(),
        &state,
        &temp.join("plan-on.csv"),
        &temp.join("plan-on.state"),
        true,
    );
    assert_success(&plan_on);

    let cuda = Command::new(env!("CARGO_BIN_EXE_sembla"))
        .args([
            "run",
            fixture_model_path().to_str().unwrap(),
            "--seed",
            "1",
            "--ticks",
            "1",
            "--population",
        ])
        .arg(&state)
        .args([
            "--backend",
            "cuda",
            "--enable",
            GROUPED_OBSERVATIONS_FEATURE,
        ])
        .output()
        .unwrap();
    assert!(!cuda.status.success());
    assert!(String::from_utf8_lossy(&cuda.stderr)
        .contains("grouped observations run on the cpu backend only for now"));

    let sweep_off = Command::new(env!("CARGO_BIN_EXE_sembla"))
        .args([
            "sweep",
            fixture_model_path().to_str().unwrap(),
            "--population",
        ])
        .arg(&state)
        .args(["--seed", "3", "--draws", "1", "--ticks", "1", "--out"])
        .arg(temp.join("sweep-off"))
        .output()
        .unwrap();
    assert!(!sweep_off.status.success());
    assert!(String::from_utf8_lossy(&sweep_off.stderr)
        .contains("requires --enable grouped-observations"));

    let sweep_dir = temp.join("sweep");
    let sweep = Command::new(env!("CARGO_BIN_EXE_sembla"))
        .args([
            "sweep",
            fixture_model_path().to_str().unwrap(),
            "--population",
        ])
        .arg(&state)
        .args(["--seed", "3", "--draws", "1", "--ticks", "1", "--out"])
        .arg(&sweep_dir)
        .args(["--enable", GROUPED_OBSERVATIONS_FEATURE])
        .output()
        .unwrap();
    assert_success(&sweep);
    let sweep_manifest: serde_json::Value =
        serde_json::from_slice(&std::fs::read(sweep_dir.join("run-manifest.json")).unwrap())
            .unwrap();
    assert_eq!(
        sweep_manifest["enabled_features"],
        serde_json::json!([GROUPED_OBSERVATIONS_FEATURE])
    );
    assert!(sweep_dir
        .join("draw_0.grouped.population_cells.csv")
        .is_file());

    let params_a = temp.join("params-a.json");
    let params_b = temp.join("params-b.json");
    std::fs::write(&params_a, "{}\n").unwrap();
    std::fs::write(&params_b, "{}\n").unwrap();
    let compare = |out: &Path, enabled: bool, backend: Option<&str>| {
        let mut command = Command::new(env!("CARGO_BIN_EXE_sembla"));
        command
            .arg("compare")
            .arg(fixture_model_path())
            .arg("--population")
            .arg(&state)
            .args(["--seed", "1", "--ticks", "1", "--params-a"])
            .arg(&params_a)
            .arg("--params-b")
            .arg(&params_b)
            .arg("--out")
            .arg(out);
        if let Some(backend) = backend {
            command.arg("--backend").arg(backend);
        }
        if enabled {
            command.arg("--enable").arg(GROUPED_OBSERVATIONS_FEATURE);
        }
        command.output().unwrap()
    };
    let compare_off = compare(&temp.join("compare-off.csv"), false, None);
    assert!(!compare_off.status.success());
    assert!(String::from_utf8_lossy(&compare_off.stderr)
        .contains("requires --enable grouped-observations"));
    let compare_unknown = Command::new(env!("CARGO_BIN_EXE_sembla"))
        .arg("compare")
        .arg(fixture_model_path())
        .arg("--population")
        .arg(&state)
        .args(["--seed", "1", "--ticks", "1", "--params-a"])
        .arg(&params_a)
        .arg("--params-b")
        .arg(&params_b)
        .arg("--out")
        .arg(temp.join("compare-unknown.csv"))
        .args(["--enable", "future-feature"])
        .output()
        .unwrap();
    assert!(!compare_unknown.status.success());
    assert!(String::from_utf8_lossy(&compare_unknown.stderr)
        .contains("unknown feature 'future-feature'"));
    let compare_out = temp.join("compare.csv");
    let compare_on = compare(&compare_out, true, None);
    assert_success(&compare_on);
    let compare_manifest: serde_json::Value = serde_json::from_slice(
        &std::fs::read(format!("{}.manifest.json", compare_out.display())).unwrap(),
    )
    .unwrap();
    assert_eq!(
        compare_manifest["enabled_features"],
        serde_json::json!([GROUPED_OBSERVATIONS_FEATURE])
    );
    let model_contrast = Command::new(env!("CARGO_BIN_EXE_sembla"))
        .arg("compare")
        .arg(fixture_model_path())
        .arg(fixture_model_path())
        .arg("--population")
        .arg(&state)
        .args(["--seed", "1", "--ticks", "1", "--out"])
        .arg(temp.join("model-contrast.csv"))
        .args(["--enable", GROUPED_OBSERVATIONS_FEATURE])
        .output()
        .unwrap();
    assert!(!model_contrast.status.success());
    assert!(String::from_utf8_lossy(&model_contrast.stderr)
        .contains("feature-aware compare is limited to same-model parameter contrasts"));
    let compare_cuda = compare(&temp.join("compare-cuda.csv"), true, Some("cuda"));
    assert!(!compare_cuda.status.success());
    assert!(String::from_utf8_lossy(&compare_cuda.stderr)
        .contains("grouped observations run on the cpu backend only for now"));

    let diff = Command::new(env!("CARGO_BIN_EXE_sembla"))
        .arg("diff-backends")
        .arg(fixture_model_path())
        .arg("--population")
        .arg(&state)
        .args(["--enable", GROUPED_OBSERVATIONS_FEATURE])
        .output()
        .unwrap();
    assert!(!diff.status.success());
    assert!(String::from_utf8_lossy(&diff.stderr)
        .contains("grouped-observations backend follow-up PRD"));
    std::fs::remove_dir_all(temp).unwrap();
}

#[test]
fn verify_run_replays_manifest_features_and_grouped_hashes() {
    let temp = temp_dir("verify");
    let model = validated_model();
    let state = temp.join("initial.state");
    write(&state, &model, &initial_tables()).unwrap();

    let legacy_out = temp.join("legacy.csv");
    let legacy_run = run_cli(
        &fixture_model_path(),
        &state,
        &legacy_out,
        &temp.join("legacy.state"),
        true,
    );
    assert_success(&legacy_run);
    let legacy_manifest = PathBuf::from(format!("{}.manifest.json", legacy_out.display()));
    assert_success(&verify_cli(&legacy_manifest, &fixture_model_path(), &state));

    let plan_out = temp.join("plan.csv");
    let plan_run = run_cli(
        &fixture_plan_path(),
        &state,
        &plan_out,
        &temp.join("plan.state"),
        true,
    );
    assert_success(&plan_run);
    let plan_manifest = PathBuf::from(format!("{}.manifest.json", plan_out.display()));
    assert_success(&verify_cli(&plan_manifest, &fixture_plan_path(), &state));

    let sweep_dir = temp.join("sweep");
    let sweep = Command::new(env!("CARGO_BIN_EXE_sembla"))
        .arg("sweep")
        .arg(fixture_plan_path())
        .arg("--population")
        .arg(&state)
        .args(["--seed", "3", "--draws", "1", "--ticks", "1", "--out"])
        .arg(&sweep_dir)
        .args(["--enable", GROUPED_OBSERVATIONS_FEATURE])
        .output()
        .unwrap();
    assert_success(&sweep);
    let sweep_manifest = sweep_dir.join("run-manifest.json");
    assert_success(&verify_cli(&sweep_manifest, &fixture_plan_path(), &state));

    let mut tampered_run: serde_json::Value =
        serde_json::from_slice(&std::fs::read(&legacy_manifest).unwrap()).unwrap();
    tampered_run["grouped_outputs"][0]["sha256"] = serde_json::json!("0".repeat(64));
    let tampered_run_path = temp.join("tampered-run.json");
    std::fs::write(
        &tampered_run_path,
        serde_json::to_vec(&tampered_run).unwrap(),
    )
    .unwrap();
    let tampered = verify_cli(&tampered_run_path, &fixture_model_path(), &state);
    assert!(!tampered.status.success());
    assert!(String::from_utf8_lossy(&tampered.stderr).contains("grouped_outputs"));

    let mut tampered_sweep: serde_json::Value =
        serde_json::from_slice(&std::fs::read(&sweep_manifest).unwrap()).unwrap();
    tampered_sweep["executions"][0]["grouped_outputs"][0]["sha256"] =
        serde_json::json!("0".repeat(64));
    let tampered_sweep_path = temp.join("tampered-sweep.json");
    std::fs::write(
        &tampered_sweep_path,
        serde_json::to_vec(&tampered_sweep).unwrap(),
    )
    .unwrap();
    let tampered = verify_cli(&tampered_sweep_path, &fixture_plan_path(), &state);
    assert!(!tampered.status.success());
    assert!(String::from_utf8_lossy(&tampered.stderr).contains("executions[0].grouped_outputs"));

    std::fs::remove_dir_all(temp).unwrap();
}

#[test]
fn grouped_plan_fixture_is_canonical_and_bidirectionally_gated() {
    let source = std::fs::read_to_string(fixture_plan_path()).unwrap();
    let ParsedInput::Plan(plan) = sembla_ir::parse_input(&source).unwrap() else {
        panic!("fixture must be a plan")
    };
    sembla_ir::validate_plan(&plan).unwrap();
    let canonical = sembla_ir::to_canonical_string(&plan).unwrap();
    assert_eq!(source, canonical, "Lean plan bytes must be Rust-canonical");

    let mut missing = plan.clone();
    missing.identity.enabled_features.clear();
    let error = sembla_ir::validate_plan(&missing).unwrap_err();
    assert!(error
        .message
        .contains("grouped_views require enabled feature"));

    let mut inert = plan;
    inert.model.boxes[0].grouped_views.clear();
    let error = sembla_ir::validate_plan(&inert).unwrap_err();
    assert!(error
        .message
        .contains("is inert because the model has no grouped_views"));
}

#[test]
#[ignore = "fixture regeneration requires the pinned Lean toolchain; run explicitly"]
fn grouped_fixtures_match_lean_exports() {
    let temp = temp_dir("regenerate");
    let exporter = temp.join("ExportGrouped.lean");
    let generated_model = temp.join("model.json");
    let generated_plan = temp.join("plan.json");
    std::fs::write(
        &exporter,
        r#"import Sembla.GroupedObservationTests
open Sembla.GroupedObservationTests
unsafe def main (args : List String) : IO UInt32 := do
  match args with
  | [modelPath, planPath] =>
      IO.FS.writeFile modelPath groupedObservationModelJson
      IO.FS.writeFile planPath groupedObservationPlanJson
      pure 0
  | _ => pure 2
"#,
    )
    .unwrap();
    let frontend = repository_path("frontend");
    let output = Command::new("lake")
        .args(["env", "lean", "--run"])
        .arg(exporter)
        .arg(&generated_model)
        .arg(&generated_plan)
        .current_dir(frontend)
        .output()
        .unwrap();
    assert_success(&output);
    assert_eq!(
        std::fs::read(generated_model).unwrap(),
        std::fs::read(fixture_model_path()).unwrap()
    );
    assert_eq!(
        std::fs::read(generated_plan).unwrap(),
        std::fs::read(fixture_plan_path()).unwrap()
    );
    std::fs::remove_dir_all(temp).unwrap();
}
