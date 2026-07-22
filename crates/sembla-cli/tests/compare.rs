use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::time::{SystemTime, UNIX_EPOCH};

use sha2::{Digest, Sha256};

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
        "sembla-plan-compare-{label}-{}-{nonce}",
        std::process::id()
    ));
    std::fs::create_dir_all(&path).unwrap();
    path
}

fn synth_population(temp: &Path) -> PathBuf {
    let population = temp.join("population.bin");
    let output = Command::new(env!("CARGO_BIN_EXE_sembla"))
        .arg("synth-pop")
        .args([
            "--persons",
            "1000",
            "--employers",
            "50",
            "--initial-infected",
            "600",
            "--seed",
            "123",
            "--out",
        ])
        .arg(&population)
        .output()
        .unwrap();
    assert_success("synth-pop", &output);
    population
}

fn assert_success(label: &str, output: &Output) {
    assert!(
        output.status.success(),
        "{label}\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(output.stderr.is_empty(), "{label}");
}

fn compare_two_plans(population: &Path, out: &Path) -> Output {
    Command::new(env!("CARGO_BIN_EXE_sembla"))
        .arg("compare")
        .arg(repository_path(
            "fixtures/plans/linked/solo_population.plan.json",
        ))
        .arg(repository_path(
            "fixtures/plans/linked/independent_epidemic_policy.plan.json",
        ))
        .arg("--population")
        .arg(population)
        .args(["--seed", "55", "--ticks", "8", "--out"])
        .arg(out)
        .output()
        .unwrap()
}

fn compare_params(
    plan: &Path,
    population: &Path,
    params_a: &Path,
    params_b: &Path,
    ticks: &str,
    out: &Path,
) -> Output {
    Command::new(env!("CARGO_BIN_EXE_sembla"))
        .arg("compare")
        .arg(plan)
        .arg("--population")
        .arg(population)
        .args(["--seed", "55", "--ticks", ticks, "--params-a"])
        .arg(params_a)
        .arg("--params-b")
        .arg(params_b)
        .arg("--out")
        .arg(out)
        .output()
        .unwrap()
}

fn csv_rows(path: &Path) -> Vec<Vec<String>> {
    std::fs::read_to_string(path)
        .unwrap()
        .lines()
        .filter(|line| !line.starts_with('#'))
        .skip(1)
        .map(|line| line.split(',').map(ToOwned::to_owned).collect())
        .collect()
}

fn assert_population_columns_equal(row: &[String]) {
    assert_eq!(row.len(), 16, "unexpected compare row: {row:?}");
    for (arm_a, arm_b) in [(1, 4), (2, 5), (3, 6), (10, 13), (11, 14), (12, 15)] {
        assert_eq!(row[arm_a], row[arm_b], "compare row: {row:?}");
    }
    assert_eq!(&row[7..10], ["0", "0", "0"], "compare row: {row:?}");
}

#[test]
fn shared_population_is_exactly_crn_paired_across_different_linked_plans() {
    let solo: serde_json::Value = serde_json::from_slice(
        &std::fs::read(repository_path(
            "fixtures/plans/linked/solo_population.plan.json",
        ))
        .unwrap(),
    )
    .unwrap();
    let product: serde_json::Value = serde_json::from_slice(
        &std::fs::read(repository_path(
            "fixtures/plans/linked/independent_epidemic_policy.plan.json",
        ))
        .unwrap(),
    )
    .unwrap();
    let population_transitions = |plan: &serde_json::Value| {
        plan["identity"]["transitions"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|transition| transition["box"] == "population")
            .map(|transition| {
                (
                    transition["identity"].as_str().unwrap().to_owned(),
                    transition["rule_word"].as_u64().unwrap(),
                )
            })
            .collect::<Vec<_>>()
    };
    assert_eq!(
        population_transitions(&solo),
        population_transitions(&product)
    );

    let temp = temp_dir("noninterference");
    let population = synth_population(&temp);
    let outputs = [temp.join("first.csv"), temp.join("second.csv")];
    for out in &outputs {
        assert_success("plan-vs-plan compare", &compare_two_plans(&population, out));
    }
    let first = std::fs::read(&outputs[0]).unwrap();
    assert_eq!(first, std::fs::read(&outputs[1]).unwrap());
    assert_eq!(
        first,
        std::fs::read(repository_path(
            "crates/sembla-cli/tests/fixtures/compare_solo_independent.csv",
        ))
        .unwrap()
    );
    let rows = csv_rows(&outputs[0]);
    assert_eq!(rows.len(), 8);
    for row in &rows {
        assert_population_columns_equal(row);
    }

    std::fs::remove_dir_all(temp).unwrap();
}

#[test]
fn wired_policy_parameter_diverges_after_exactly_two_wire_ticks() {
    let plan =
        repository_path("crates/sembla-cli/tests/fixtures/epidemic_policy_threshold.plan.json");
    let source = std::fs::read(repository_path(
        "crates/sembla-cli/tests/fixtures/epidemic_policy_threshold.source.json",
    ))
    .unwrap();
    let envelope: serde_json::Value =
        serde_json::from_slice(&std::fs::read(&plan).unwrap()).unwrap();
    let source_digest = format!(
        "{:x}",
        Sha256::digest([b"sembla.source-artifact/v1\0".as_slice(), source.as_slice()].concat())
    );
    assert_eq!(
        envelope["linked_provenance"]["source_hash"]["digest"],
        source_digest
    );

    let temp = temp_dir("wired-delay");
    let population = synth_population(&temp);
    let params_a = temp.join("params-a.json");
    let params_b = temp.join("params-b.json");
    std::fs::write(&params_a, r#"{"restriction_threshold":500}"#).unwrap();
    std::fs::write(&params_b, r#"{"restriction_threshold":1000}"#).unwrap();
    let outputs = [temp.join("first.csv"), temp.join("second.csv")];
    for out in &outputs {
        assert_success(
            "wired parameter compare",
            &compare_params(&plan, &population, &params_a, &params_b, "8", out),
        );
    }
    let first = std::fs::read(&outputs[0]).unwrap();
    assert_eq!(first, std::fs::read(&outputs[1]).unwrap());
    assert_eq!(
        first,
        std::fs::read(repository_path(
            "crates/sembla-cli/tests/fixtures/compare_policy_threshold.csv",
        ))
        .unwrap()
    );

    let rows = csv_rows(&outputs[0]);
    let first_differing_tick = rows
        .iter()
        .find(|row| {
            [(1, 4), (2, 5), (3, 6), (10, 13), (11, 14), (12, 15)]
                .iter()
                .any(|(arm_a, arm_b)| row[*arm_a] != row[*arm_b])
        })
        .map(|row| row[0].parse::<usize>().unwrap());
    assert_eq!(first_differing_tick, Some(2));
    assert_population_columns_equal(&rows[0]);
    assert_population_columns_equal(&rows[1]);
    assert_ne!(&rows[2][7..10], ["0", "0", "0"]);

    std::fs::remove_dir_all(temp).unwrap();
}

#[test]
fn mixed_legacy_and_plan_compare_rejects_before_creating_output() {
    let temp = temp_dir("mixed-identity");
    let population = temp.join("population.bin");
    std::fs::write(&population, []).unwrap();
    let legacy = repository_path("examples/two_box.json");
    let plan = repository_path("fixtures/plans/two_box.plan.json");
    let out = temp.join("compare.csv");
    let output = Command::new(env!("CARGO_BIN_EXE_sembla"))
        .arg("compare")
        .arg(&legacy)
        .arg(&plan)
        .arg("--population")
        .arg(&population)
        .args(["--seed", "1", "--ticks", "1", "--out"])
        .arg(&out)
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(1));
    assert!(output.stdout.is_empty());
    assert_eq!(
        String::from_utf8(output.stderr).unwrap(),
        format!(
            "compare requires both inputs to use the same identity scheme; got legacy model '{}' and plan envelope '{}'\n",
            legacy.display(),
            plan.display()
        )
    );
    assert!(!out.exists());
    assert!(!PathBuf::from(format!("{}.manifest.json", out.display())).exists());

    std::fs::remove_dir_all(temp).unwrap();
}

#[test]
fn counterfactual_demo_plan_parameter_compare_matches_golden_and_is_deterministic() {
    let temp = temp_dir("demo-counterfactual");
    let population = synth_population(&temp);
    let plan = repository_path(
        "fixtures/demos/composition/demo_counterfactual_outbreak/executable-plan.json",
    );
    let params_a = temp.join("control.json");
    let params_b = temp.join("high-contact.json");
    std::fs::write(&params_a, r#"{"control_beta":0.45}"#).unwrap();
    std::fs::write(&params_b, r#"{"control_beta":0.9}"#).unwrap();
    let outputs = [temp.join("first.csv"), temp.join("second.csv")];
    for out in &outputs {
        assert_success(
            "demo parameter compare",
            &compare_params(&plan, &population, &params_a, &params_b, "8", out),
        );
    }
    let first = std::fs::read(&outputs[0]).unwrap();
    assert_eq!(first, std::fs::read(&outputs[1]).unwrap());
    assert_eq!(
        first,
        std::fs::read(repository_path(
            "crates/sembla-cli/tests/fixtures/compare_demo_counterfactual.csv",
        ))
        .unwrap()
    );

    std::fs::remove_dir_all(temp).unwrap();
}
