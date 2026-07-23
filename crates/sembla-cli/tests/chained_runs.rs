use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::time::{SystemTime, UNIX_EPOCH};

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
        "sembla-chained-runs-{label}-{}-{nonce}",
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

fn write_chain_model(directory: &Path) -> PathBuf {
    let mut model: serde_json::Value = serde_json::from_slice(
        &std::fs::read(repository_path("fixtures/state/models/refs_small.json")).unwrap(),
    )
    .unwrap();
    model["params"] = serde_json::json!([
        {
            "name": "close_rate",
            "ty": "real",
            "default": {"kind": "real", "value": 0.4},
            "prior": null
        },
        {
            "name": "open_rate",
            "ty": "real",
            "default": {"kind": "real", "value": 0.2},
            "prior": null
        }
    ]);
    model["boxes"][0]["transitions"][0]["hazard"] =
        serde_json::json!({"kind": "param", "name": "close_rate"});
    model["boxes"][0]["transitions"]
        .as_array_mut()
        .unwrap()
        .push(serde_json::json!({
            "name": "open",
            "table": "Person",
            "guard": {"kind": "enum_is", "attr": "status", "variant": "Closed"},
            "hazard": {"kind": "param", "name": "open_rate"},
            "effects": [
                {"kind": "set_attr", "attr": "status", "value": {"kind": "enum", "variant": "Open"}}
            ],
            "contests": []
        }));
    let path = directory.join("chain-model.json");
    std::fs::write(&path, serde_json::to_vec(&model).unwrap()).unwrap();
    path
}

fn write_plan_state(plan: &Path, state: &Path) {
    let source = std::fs::read_to_string(plan).unwrap();
    let sembla_ir::ParsedInput::Plan(plan) = sembla_ir::parse_input(&source).unwrap() else {
        panic!("expected a plan envelope")
    };
    let model = sembla_ir::validate_plan(&plan)
        .unwrap()
        .model_with_rule_words();
    let tables = model
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
                            sembla_ir::AttrType::Real => {
                                sembla_runtime::state::ColumnData::Real(vec![0.0; rows])
                            }
                            sembla_ir::AttrType::Int => {
                                sembla_runtime::state::ColumnData::Int(vec![0; rows])
                            }
                            sembla_ir::AttrType::Enum { .. } => {
                                sembla_runtime::state::ColumnData::Enum(vec![0; rows])
                            }
                            sembla_ir::AttrType::Ref { .. } => {
                                sembla_runtime::state::ColumnData::Ref(vec![0; rows])
                            }
                        };
                        sembla_runtime::state::ColumnInit::new(&attr.name, data)
                    })
                    .collect();
                sembla_runtime::state::TableInit::new(&model_box.name, &table.name, rows, columns)
            })
        })
        .collect::<Vec<_>>();
    sembla_runtime::state_artifact::write(state, &model, &tables).unwrap();
}

fn write_params(directory: &Path, label: &str, close_rate: f64, open_rate: f64) -> PathBuf {
    let path = directory.join(format!("{label}.params.json"));
    std::fs::write(
        &path,
        serde_json::to_vec(&serde_json::json!({
            "close_rate": close_rate,
            "open_rate": open_rate
        }))
        .unwrap(),
    )
    .unwrap();
    path
}

fn run_window(
    model: &Path,
    population: &Path,
    params: &Path,
    seed: u64,
    ticks: u32,
    output: &Path,
    exported_state: Option<&Path>,
) -> Output {
    let mut command = Command::new(env!("CARGO_BIN_EXE_sembla"));
    command
        .arg("run")
        .arg(model)
        .arg("--population")
        .arg(population)
        .arg("--seed")
        .arg(seed.to_string())
        .arg("--ticks")
        .arg(ticks.to_string())
        .arg("--params")
        .arg(params)
        .arg("--out")
        .arg(output);
    if let Some(path) = exported_state {
        command.arg("--export-state").arg(path);
    }
    command.output().unwrap()
}

fn manifest_path(output: &Path) -> PathBuf {
    PathBuf::from(format!("{}.manifest.json", output.display()))
}

fn summaries_path(output: &Path) -> PathBuf {
    PathBuf::from(format!("{}.summaries.csv", output.display()))
}

fn read_manifest(output: &Path) -> serde_json::Value {
    serde_json::from_slice(&std::fs::read(manifest_path(output)).unwrap()).unwrap()
}

fn assert_reproducible_run(
    left_output: &Path,
    left_state: &Path,
    right_output: &Path,
    right_state: &Path,
) {
    for (left, right) in [
        (left_output.to_path_buf(), right_output.to_path_buf()),
        (summaries_path(left_output), summaries_path(right_output)),
        (manifest_path(left_output), manifest_path(right_output)),
        (left_state.to_path_buf(), right_state.to_path_buf()),
    ] {
        assert_eq!(
            std::fs::read(&left).unwrap(),
            std::fs::read(&right).unwrap(),
            "{} and {} differ",
            left.display(),
            right.display()
        );
    }
}

fn data_rows(path: &Path) -> Vec<Vec<String>> {
    let source = std::fs::read_to_string(path).unwrap();
    let mut lines = source.lines().filter(|line| !line.starts_with('#'));
    let _header = lines.next().unwrap();
    lines
        .map(|line| line.split(',').map(str::to_owned).collect())
        .collect()
}

#[test]
fn chain_hash_identity_and_each_window_are_bitwise_reproducible() {
    let temp = temp_dir("identity");
    let model = write_chain_model(&temp);
    let params_a = write_params(&temp, "a", 0.4, 0.2);
    let params_b = write_params(&temp, "b", 0.15, 0.6);
    let original = repository_path("fixtures/state/refs_small.state");

    let a_outputs = [temp.join("a-first.csv"), temp.join("a-second.csv")];
    let a_states = [temp.join("a-first.state"), temp.join("a-second.state")];
    for (output, state) in a_outputs.iter().zip(&a_states) {
        assert_success(&run_window(
            &model,
            &original,
            &params_a,
            7,
            12,
            output,
            Some(state),
        ));
    }
    assert_reproducible_run(&a_outputs[0], &a_states[0], &a_outputs[1], &a_states[1]);

    let b_outputs = [temp.join("b-first.csv"), temp.join("b-second.csv")];
    let b_states = [temp.join("b-first.state"), temp.join("b-second.state")];
    for (output, state) in b_outputs.iter().zip(&b_states) {
        assert_success(&run_window(
            &model,
            &a_states[0],
            &params_b,
            19,
            12,
            output,
            Some(state),
        ));
    }
    assert_reproducible_run(&b_outputs[0], &b_states[0], &b_outputs[1], &b_states[1]);

    let manifest_a = read_manifest(&a_outputs[0]);
    let manifest_b = read_manifest(&b_outputs[0]);
    assert_eq!(
        manifest_a["exported_state"]["hash"],
        manifest_b["initial_state"]["hash"]
    );
    assert_eq!(manifest_a["exported_state"]["format"], "sembla.state/v1");
    assert_eq!(manifest_b["initial_state"]["format"], "sembla.state/v1");

    let reloaded_output = temp.join("a-reloaded-at-zero-ticks.csv");
    assert_success(&run_window(
        &model,
        &a_states[0],
        &params_a,
        999,
        0,
        &reloaded_output,
        None,
    ));
    let reloaded_manifest = read_manifest(&reloaded_output);
    assert_eq!(
        manifest_a["final_state_sha256"], reloaded_manifest["final_state_sha256"],
        "the exported tables must reconstruct the final committed state"
    );

    std::fs::remove_dir_all(temp).unwrap();
}

#[test]
fn chained_windows_are_honestly_not_a_continuous_run() {
    let temp = temp_dir("non-equivalence");
    let model = write_chain_model(&temp);
    let params = write_params(&temp, "same", 0.4, 0.2);
    let original = repository_path("fixtures/state/refs_small.state");
    let a_output = temp.join("a.csv");
    let a_state = temp.join("a.state");
    let b_output = temp.join("b.csv");
    let c_output = temp.join("continuous.csv");

    assert_success(&run_window(
        &model,
        &original,
        &params,
        7,
        12,
        &a_output,
        Some(&a_state),
    ));
    assert_success(&run_window(
        &model, &a_state, &params, 7, 12, &b_output, None,
    ));
    assert_success(&run_window(
        &model, &original, &params, 7, 24, &c_output, None,
    ));

    let a_rows = data_rows(&a_output);
    let mut b_rows = data_rows(&b_output);
    let c_rows = data_rows(&c_output);
    assert_eq!(a_rows, c_rows[..12]);
    for row in &mut b_rows {
        row[0] = (row[0].parse::<u32>().unwrap() + 12).to_string();
    }
    let chained_rows = a_rows.into_iter().chain(b_rows).collect::<Vec<_>>();
    // DECISIONS §K4: run B restarts tick coordinates at zero, so its Philox
    // draws differ by design even with the same seed and theta. A+B must not be
    // represented as bitwise-equivalent to one continuous 24-tick execution.
    assert_ne!(chained_rows, c_rows);

    std::fs::remove_dir_all(temp).unwrap();
}

#[test]
fn export_is_pure_deterministic_and_refuses_to_overwrite() {
    let temp = temp_dir("purity");
    let model = write_chain_model(&temp);
    let params = write_params(&temp, "params", 0.4, 0.2);
    let original = repository_path("fixtures/state/refs_small.state");
    let plain_output = temp.join("plain.csv");
    let exported_output = temp.join("exported.csv");
    let repeated_output = temp.join("repeated.csv");
    let first_state = temp.join("first.state");
    let second_state = temp.join("second.state");

    let plain = run_window(&model, &original, &params, 31, 12, &plain_output, None);
    let exported = run_window(
        &model,
        &original,
        &params,
        31,
        12,
        &exported_output,
        Some(&first_state),
    );
    let repeated = run_window(
        &model,
        &original,
        &params,
        31,
        12,
        &repeated_output,
        Some(&second_state),
    );
    assert_success(&plain);
    assert_success(&exported);
    assert_success(&repeated);
    assert_eq!(plain.stdout, exported.stdout);
    assert_eq!(plain.stdout, repeated.stdout);
    assert_eq!(
        std::fs::read(&plain_output).unwrap(),
        std::fs::read(&exported_output).unwrap()
    );
    assert_eq!(
        std::fs::read(summaries_path(&plain_output)).unwrap(),
        std::fs::read(summaries_path(&exported_output)).unwrap()
    );
    assert_eq!(
        std::fs::read(&first_state).unwrap(),
        std::fs::read(&second_state).unwrap()
    );
    let plain_manifest = read_manifest(&plain_output);
    let exported_manifest = read_manifest(&exported_output);
    for field in ["results_sha256", "final_state_sha256", "observation_sha256"] {
        assert_eq!(plain_manifest[field], exported_manifest[field], "{field}");
    }
    assert!(plain_manifest.get("exported_state").is_none());
    assert!(plain_manifest.get("initial_state").is_some());

    let original_export = std::fs::read(&first_state).unwrap();
    let rejected_output = temp.join("rejected.csv");
    let rejected = run_window(
        &model,
        &original,
        &params,
        31,
        12,
        &rejected_output,
        Some(&first_state),
    );
    assert!(!rejected.status.success());
    assert_eq!(
        String::from_utf8(rejected.stderr).unwrap(),
        format!(
            "refusing to overwrite existing state artifact '{}'\n",
            first_state.display()
        )
    );
    assert!(!rejected_output.exists());
    assert_eq!(std::fs::read(&first_state).unwrap(), original_export);

    let collision = temp.join("collision");
    let rejected_collision = run_window(
        &model,
        &original,
        &params,
        31,
        12,
        &collision,
        Some(&collision),
    );
    assert!(!rejected_collision.status.success());
    assert!(String::from_utf8(rejected_collision.stderr)
        .unwrap()
        .contains("conflicts with run output path"));
    assert!(!collision.exists());

    std::fs::remove_dir_all(temp).unwrap();
}

#[test]
fn chained_manifest_records_each_windows_theta_and_accepts_verify_run() {
    let temp = temp_dir("theta");
    let model = write_chain_model(&temp);
    let params_a = write_params(&temp, "a", 0.4, 0.2);
    let params_b = write_params(&temp, "b", 0.1, 0.7);
    let original = repository_path("fixtures/state/refs_small.state");
    let a_output = temp.join("a.csv");
    let a_state = temp.join("a.state");
    let b_output = temp.join("b.csv");
    let b_state = temp.join("b.state");

    assert_success(&run_window(
        &model,
        &original,
        &params_a,
        5,
        12,
        &a_output,
        Some(&a_state),
    ));
    assert_success(&run_window(
        &model,
        &a_state,
        &params_b,
        6,
        12,
        &b_output,
        Some(&b_state),
    ));

    let manifest_a = read_manifest(&a_output);
    let manifest_b = read_manifest(&b_output);
    assert_eq!(manifest_a["resolved_theta"]["close_rate"], 0.4);
    assert_eq!(manifest_a["resolved_theta"]["open_rate"], 0.2);
    assert_eq!(manifest_b["resolved_theta"]["close_rate"], 0.1);
    assert_eq!(manifest_b["resolved_theta"]["open_rate"], 0.7);
    assert_eq!(
        manifest_a["exported_state"]["hash"],
        manifest_b["initial_state"]["hash"]
    );
    assert!(manifest_b.get("exported_state").is_some());

    let verified = Command::new(env!("CARGO_BIN_EXE_sembla"))
        .arg("verify-run")
        .arg(manifest_path(&b_output))
        .arg(&model)
        .arg("--population")
        .arg(&a_state)
        .arg("--params")
        .arg(&params_b)
        .output()
        .unwrap();
    assert_success(&verified);
    assert_eq!(verified.stdout, b"verified 1 execution(s)\n");

    std::fs::remove_dir_all(temp).unwrap();
}

#[test]
fn export_supports_plan_envelopes_and_numeric_legacy_manifests_remain_tuple_free() {
    let temp = temp_dir("plan-and-legacy");
    let plan_path = repository_path("fixtures/plans/two_box.plan.json");
    let plan_input = temp.join("plan-input.state");
    write_plan_state(&plan_path, &plan_input);
    let plan_output = temp.join("plan.csv");
    let plan_state = temp.join("plan.state");
    let plan = Command::new(env!("CARGO_BIN_EXE_sembla"))
        .arg("run")
        .arg(&plan_path)
        .arg("--population")
        .arg(&plan_input)
        .args(["--seed", "55", "--ticks", "2", "--out"])
        .arg(&plan_output)
        .arg("--export-state")
        .arg(&plan_state)
        .output()
        .unwrap();
    assert_success(&plan);
    assert!(plan_state.is_file());
    let plan_manifest = read_manifest(&plan_output);
    assert!(plan_manifest.get("initial_state").is_some());
    assert!(plan_manifest.get("exported_state").is_some());

    let legacy_output = temp.join("legacy.csv");
    let legacy = Command::new(env!("CARGO_BIN_EXE_sembla"))
        .arg("run")
        .arg(repository_path("examples/two_state.json"))
        .args(["--population", "10", "--seed", "9", "--ticks", "2", "--out"])
        .arg(&legacy_output)
        .output()
        .unwrap();
    assert_success(&legacy);
    let legacy_manifest = read_manifest(&legacy_output);
    assert!(legacy_manifest.get("initial_state").is_none());
    assert!(legacy_manifest.get("exported_state").is_none());

    std::fs::remove_dir_all(temp).unwrap();
}

#[test]
fn export_without_results_works_and_other_commands_reject_the_run_only_flag() {
    let temp = temp_dir("flag-scope");
    let model = write_chain_model(&temp);
    let params = write_params(&temp, "params", 0.4, 0.2);
    let original = repository_path("fixtures/state/refs_small.state");
    let state = temp.join("standalone.state");
    let run = Command::new(env!("CARGO_BIN_EXE_sembla"))
        .arg("run")
        .arg(&model)
        .arg("--population")
        .arg(&original)
        .args(["--seed", "3", "--ticks", "2", "--params"])
        .arg(&params)
        .arg("--export-state")
        .arg(&state)
        .output()
        .unwrap();
    assert_success(&run);
    assert!(state.is_file());

    let sweep = Command::new(env!("CARGO_BIN_EXE_sembla"))
        .arg("sweep")
        .arg(&model)
        .arg("--population")
        .arg(&original)
        .args(["--seed", "3", "--draws", "1", "--ticks", "1", "--out"])
        .arg(temp.join("sweep"))
        .arg("--export-state")
        .arg(temp.join("forbidden.state"))
        .output()
        .unwrap();
    assert!(!sweep.status.success());
    assert!(String::from_utf8(sweep.stderr)
        .unwrap()
        .contains("unknown sweep flag '--export-state'"));

    let compare = Command::new(env!("CARGO_BIN_EXE_sembla"))
        .arg("compare")
        .arg(&model)
        .arg(&model)
        .arg("--population")
        .arg(&original)
        .args(["--seed", "3", "--ticks", "1", "--out"])
        .arg(temp.join("compare.csv"))
        .arg("--export-state")
        .arg(temp.join("forbidden.state"))
        .output()
        .unwrap();
    assert!(!compare.status.success());
    assert!(String::from_utf8(compare.stderr)
        .unwrap()
        .contains("unknown compare flag '--export-state'"));

    std::fs::remove_dir_all(temp).unwrap();
}
