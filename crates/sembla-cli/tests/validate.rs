use std::path::{Path, PathBuf};
use std::process::Command;

fn repository_path(relative: impl AsRef<Path>) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join(relative)
}

#[test]
fn validate_subcommand_accepts_the_golden_fixture() {
    let output = Command::new(env!("CARGO_BIN_EXE_sembla"))
        .arg("validate")
        .arg(repository_path("examples/two_state.json"))
        .output()
        .unwrap();

    assert!(output.status.success());
    assert!(output.stderr.is_empty());
}

#[test]
fn validate_subcommand_rejects_every_invalid_fixture() {
    let fixtures = [
        ("unresolved_param.json", "hazard.name"),
        ("duplicate_param.json", "params[1].name"),
        ("bad_prior_arity.json", "prior.args"),
        ("wrong_guard_type.json", "transitions[0].guard"),
        ("unknown_enum_variant.json", "guard.variant"),
        ("unknown_effect_attr.json", "effects[0].attr"),
    ];

    for (fixture, offending_path) in fixtures {
        let output = Command::new(env!("CARGO_BIN_EXE_sembla"))
            .arg("validate")
            .arg(repository_path(format!("examples/invalid/{fixture}")))
            .output()
            .unwrap();
        let stderr = String::from_utf8(output.stderr).unwrap();

        assert_eq!(output.status.code(), Some(1), "{fixture}: {stderr}");
        assert!(stderr.contains(offending_path), "{fixture}: {stderr}");
    }
}

#[test]
fn diff_ir_compares_validated_canonical_models() {
    let fixture = repository_path("examples/reversible_ctmc.json");
    let normalized_copy = std::env::temp_dir().join(format!(
        "sembla-diff-ir-{}-{}.json",
        std::process::id(),
        std::thread::current().name().unwrap_or("test")
    ));
    let source = std::fs::read_to_string(&fixture).unwrap();
    let mut legacy_shape: serde_json::Value = serde_json::from_str(&source).unwrap();
    legacy_shape.as_object_mut().unwrap().remove("summaries");
    for model_box in legacy_shape["boxes"].as_array_mut().unwrap() {
        model_box.as_object_mut().unwrap().remove("views");
    }
    std::fs::write(
        &normalized_copy,
        serde_json::to_vec_pretty(&legacy_shape).unwrap(),
    )
    .unwrap();

    let identical = Command::new(env!("CARGO_BIN_EXE_sembla"))
        .args(["diff-ir"])
        .arg(&fixture)
        .arg(&normalized_copy)
        .output()
        .unwrap();
    assert!(identical.status.success());
    assert!(String::from_utf8(identical.stdout)
        .unwrap()
        .contains("semantically identical"));

    let different = Command::new(env!("CARGO_BIN_EXE_sembla"))
        .args(["diff-ir"])
        .arg(&fixture)
        .arg(repository_path("examples/sir_policy.json"))
        .output()
        .unwrap();
    assert_eq!(different.status.code(), Some(1));
    assert!(String::from_utf8(different.stderr)
        .unwrap()
        .contains("canonical normalization"));

    let observations = repository_path("examples/observations.json");
    let changed_observation = std::env::temp_dir().join(format!(
        "sembla-diff-ir-observation-{}-{}.json",
        std::process::id(),
        std::thread::current().name().unwrap_or("test")
    ));
    let mut changed: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&observations).unwrap()).unwrap();
    changed["summaries"][0]["reduce"] = serde_json::json!("max");
    std::fs::write(&changed_observation, serde_json::to_vec(&changed).unwrap()).unwrap();
    let observation_difference = Command::new(env!("CARGO_BIN_EXE_sembla"))
        .args(["diff-ir"])
        .arg(&observations)
        .arg(&changed_observation)
        .output()
        .unwrap();
    assert_eq!(observation_difference.status.code(), Some(1));
    assert!(String::from_utf8(observation_difference.stderr)
        .unwrap()
        .contains("canonical normalization"));

    let invalid = Command::new(env!("CARGO_BIN_EXE_sembla"))
        .args(["diff-ir"])
        .arg(repository_path("examples/invalid/wrong_guard_type.json"))
        .arg(&fixture)
        .output()
        .unwrap();
    assert_eq!(invalid.status.code(), Some(1));
    assert!(String::from_utf8(invalid.stderr)
        .unwrap()
        .contains("transitions[0].guard"));

    std::fs::remove_file(normalized_copy).unwrap();
    std::fs::remove_file(changed_observation).unwrap();
}

#[test]
fn validate_accepts_legacy_two_box_and_canonical_plan() {
    for path in ["examples/two_box.json", "fixtures/plans/two_box.plan.json"] {
        let output = Command::new(env!("CARGO_BIN_EXE_sembla"))
            .arg("validate")
            .arg(repository_path(path))
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{path}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(output.stdout.is_empty(), "{path}");
        assert!(output.stderr.is_empty(), "{path}");
    }
}

#[test]
fn plan_hash_prints_the_two_frozen_records() {
    let output = Command::new(env!("CARGO_BIN_EXE_sembla"))
        .arg("plan-hash")
        .arg(repository_path("fixtures/plans/two_box.plan.json"))
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(output.stderr.is_empty());
    assert_eq!(
        String::from_utf8(output.stdout).unwrap(),
        concat!(
            "semantic sha256 sembla.plan-core/v1 0524e9403ce2e945a6a98bd5cc7db646779d565c963e83e9a881e86b3459cc9c\n",
            "envelope sha256 sembla.plan-envelope/v1 135c303af99e524e9260751891e38f8724e65cf9e6906ff0d441d77fe63a0028\n"
        )
    );
}

#[test]
fn validate_rejects_noncanonical_plan_bytes() {
    let output = Command::new(env!("CARGO_BIN_EXE_sembla"))
        .arg("validate")
        .arg(repository_path(
            "fixtures/plans/invalid/noncanonical.plan.json",
        ))
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(1));
    assert!(String::from_utf8(output.stderr)
        .unwrap()
        .contains("plan file is not canonical"));
}

#[test]
fn run_rejects_plan_envelopes_until_prd_0004() {
    let output = Command::new(env!("CARGO_BIN_EXE_sembla"))
        .arg("run")
        .arg(repository_path("fixtures/plans/two_box.plan.json"))
        .args(["--seed", "1", "--ticks", "1", "--population", "1"])
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(1));
    assert!(String::from_utf8(output.stderr)
        .unwrap()
        .contains("plan envelopes are not yet runnable; see PRD 0004"));
}

#[test]
fn every_other_runtime_entrypoint_rejects_plan_envelopes() {
    let plan = repository_path("fixtures/plans/two_box.plan.json");
    let population = std::env::temp_dir().join(format!(
        "sembla-plan-rejection-population-{}.bin",
        std::process::id()
    ));
    std::fs::write(&population, []).unwrap();
    let output_path = std::env::temp_dir().join(format!(
        "sembla-plan-rejection-output-{}",
        std::process::id()
    ));

    let mut commands = Vec::new();

    let mut sweep = Command::new(env!("CARGO_BIN_EXE_sembla"));
    sweep
        .arg("sweep")
        .arg(&plan)
        .args(["--population", "1", "--seed", "1", "--draws", "1"])
        .args(["--ticks", "1", "--out"])
        .arg(&output_path);
    commands.push(("sweep", sweep));

    let mut verify = Command::new(env!("CARGO_BIN_EXE_sembla"));
    verify
        .arg("verify-run")
        .arg("manifest-does-not-need-to-exist.json")
        .arg(&plan)
        .args(["--population", "1"]);
    commands.push(("verify-run", verify));

    let mut compare = Command::new(env!("CARGO_BIN_EXE_sembla"));
    compare
        .arg("compare")
        .arg(&plan)
        .arg(repository_path("examples/two_box.json"))
        .arg("--population")
        .arg(&population)
        .args(["--seed", "1", "--ticks", "1", "--out"])
        .arg(&output_path);
    commands.push(("compare", compare));

    let mut differential = Command::new(env!("CARGO_BIN_EXE_sembla"));
    differential.arg("diff-backends").arg(&plan).args([
        "--population",
        "1",
        "--seed",
        "1",
        "--ticks",
        "1",
    ]);
    commands.push(("diff-backends", differential));

    for (name, mut command) in commands {
        let output = command.output().unwrap();
        let stderr = String::from_utf8(output.stderr).unwrap();
        assert_eq!(output.status.code(), Some(1), "{name}: {stderr}");
        assert!(
            stderr.contains("plan envelopes are not yet runnable; see PRD 0004"),
            "{name}: {stderr}"
        );
    }

    std::fs::remove_file(population).unwrap();
}
