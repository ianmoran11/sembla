use std::fmt::Write as _;
use std::path::{Path, PathBuf};
use std::process::Command;

use sembla_ir::{
    domain_digest, parse_input, to_canonical_string, validate_plan, ParsedInput, PlanOrigin,
};

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
fn every_top_level_plan_fixture_is_valid_and_canonical() {
    let directory = repository_path("fixtures/plans");
    let mut plans = std::fs::read_dir(directory)
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .filter(|path| {
            path.file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| name.ends_with(".plan.json"))
        })
        .collect::<Vec<_>>();
    plans.sort();
    assert!(!plans.is_empty());

    for path in plans {
        let output = Command::new(env!("CARGO_BIN_EXE_sembla"))
            .arg("validate")
            .arg(&path)
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}: {}",
            path.display(),
            String::from_utf8_lossy(&output.stderr)
        );

        let source = std::fs::read_to_string(&path).unwrap();
        let ParsedInput::Plan(plan) = parse_input(&source).unwrap() else {
            panic!("{} dispatched as a legacy model", path.display());
        };
        validate_plan(&plan).unwrap();
        assert_eq!(
            to_canonical_string(&plan).unwrap(),
            source,
            "{} is not byte-canonical",
            path.display()
        );
    }
}

#[test]
fn every_linked_plan_is_valid_canonical_and_pins_its_source_hash() {
    let directory = repository_path("fixtures/plans/linked");
    let mut plans = std::fs::read_dir(directory)
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .filter(|path| {
            path.file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| name.ends_with(".plan.json"))
        })
        .collect::<Vec<_>>();
    plans.sort();
    assert_eq!(plans.len(), 8);

    for path in plans {
        let output = Command::new(env!("CARGO_BIN_EXE_sembla"))
            .arg("validate")
            .arg(&path)
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}: {}",
            path.display(),
            String::from_utf8_lossy(&output.stderr)
        );

        let plan_bytes = std::fs::read(&path).unwrap();
        let plan_source = std::str::from_utf8(&plan_bytes).unwrap();
        let ParsedInput::Plan(plan) = parse_input(plan_source).unwrap() else {
            panic!("{} dispatched as a legacy model", path.display());
        };
        validate_plan(&plan).unwrap();
        assert_eq!(plan.origin, PlanOrigin::Linked);
        assert_eq!(to_canonical_string(&plan).unwrap().as_bytes(), plan_bytes);

        let fixture = path
            .file_name()
            .unwrap()
            .to_str()
            .unwrap()
            .strip_suffix(".plan.json")
            .unwrap();
        let source_bytes = std::fs::read(repository_path(format!(
            "fixtures/composition-source/{fixture}.source.json"
        )))
        .unwrap();
        let digest = domain_digest("sembla.source-artifact/v1", &source_bytes)
            .iter()
            .fold(String::with_capacity(64), |mut output, byte| {
                write!(&mut output, "{byte:02x}").unwrap();
                output
            });
        let provenance = plan.linked_provenance.as_ref().unwrap();
        assert_eq!(provenance.source_hash.algorithm, "sha256");
        assert_eq!(provenance.source_hash.domain, "sembla.source-artifact/v1");
        assert_eq!(provenance.source_hash.digest, digest);
        assert_eq!(
            provenance.source_map["schema_version"],
            "sembla.source-map/v1"
        );
        let boundary = provenance.source_map["boundary"].as_array().unwrap();
        let hidden = provenance.source_map["hidden"].as_array().unwrap();
        if fixture == "regional_response" {
            assert_eq!(boundary.len(), 1);
            assert_eq!(boundary[0]["outer"], "port:regional_infection_count");
            assert_eq!(boundary[0]["leaf"], "occ:epidemic/population");
            assert_eq!(boundary[0]["port"], "infection_count");
            assert_eq!(
                boundary[0]["path"],
                serde_json::json!(["expose:regional_infection_count", "expose:infection_count"])
            );
            assert_eq!(hidden.len(), 1);
            assert_eq!(hidden[0]["instance"], "inst:epidemic");
            assert_eq!(hidden[0]["port"], "port:restriction_modifier");
        } else {
            assert!(boundary.is_empty(), "{fixture}");
            assert!(hidden.is_empty(), "{fixture}");
        }
        let source_leaves = provenance.source_map["leaves"].as_array().unwrap();
        assert_eq!(source_leaves.len(), plan.identity.leaves.len());
        for leaf in source_leaves {
            assert!(leaf["occurrence"].as_str().is_some());
            assert!(leaf["definition"].as_str().is_some());
            assert!(leaf["instance_path"].as_array().is_some());
            assert!(leaf["display_path"].as_str().is_some());
        }
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
fn sir_plan_run_matches_the_checked_csv_golden_bitwise() {
    let temp = std::env::temp_dir().join(format!(
        "sembla-sir-plan-golden-{}-{}",
        std::process::id(),
        std::thread::current().name().unwrap_or("test")
    ));
    std::fs::create_dir_all(&temp).unwrap();
    let expected_csv = std::fs::read(repository_path(
        "fixtures/plans/goldens/sir.seed55.ticks8.csv",
    ))
    .unwrap();
    let expected_summaries = std::fs::read(repository_path(
        "fixtures/plans/goldens/sir.seed55.ticks8.csv.summaries.csv",
    ))
    .unwrap();

    for repeat in ["first", "second"] {
        let output_path = temp.join(format!("{repeat}.csv"));
        let output = Command::new(env!("CARGO_BIN_EXE_sembla"))
            .arg("run")
            .arg(repository_path("fixtures/plans/sir.plan.json"))
            .args([
                "--population",
                "16",
                "--seed",
                "55",
                "--ticks",
                "8",
                "--out",
            ])
            .arg(&output_path)
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
                "results_sha256=1f0069cbbd25894fefb3b9434a218d8f48fd2fdd2f574fbc81884ca478a19a4e ",
                "final_state_sha256=ae3e613c5ac01cc72580ca4dbd7bb4a475b4e78c632eb1dc008b41015ca610f8 ",
                "observation_sha256=d37275caa988bca0bf1a70c253e36edb6577c0150f5024fabcb4e606bec81ded\n"
            )
        );
        assert_eq!(std::fs::read(&output_path).unwrap(), expected_csv);
        assert_eq!(
            std::fs::read(format!("{}.summaries.csv", output_path.display())).unwrap(),
            expected_summaries
        );
    }

    std::fs::remove_dir_all(temp).unwrap();
}

#[test]
fn run_accepts_plan_envelopes_on_cpu_and_reaches_cuda_backend_without_dt() {
    let plan = repository_path("fixtures/plans/two_box.plan.json");
    let output = Command::new(env!("CARGO_BIN_EXE_sembla"))
        .arg("run")
        .arg(&plan)
        .args(["--seed", "1", "--ticks", "1", "--population", "1"])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );

    let cuda = Command::new(env!("CARGO_BIN_EXE_sembla"))
        .arg("run")
        .arg(&plan)
        .args(["--seed", "1", "--ticks", "1", "--population", "1"])
        .args(["--backend", "cuda"])
        .output()
        .unwrap();
    assert_eq!(cuda.status.code(), Some(1));
    let stderr = String::from_utf8(cuda.stderr).unwrap();
    assert!(stderr.contains("cuda backend unavailable"), "{stderr}");
    assert!(!stderr.contains("cpu backend only"), "{stderr}");

    let dt = Command::new(env!("CARGO_BIN_EXE_sembla"))
        .arg("run")
        .arg(&plan)
        .args(["--seed", "1", "--ticks", "1", "--population", "1"])
        .args(["--dt", "0.5"])
        .output()
        .unwrap();
    assert_eq!(dt.status.code(), Some(1));
    assert!(
        String::from_utf8_lossy(&dt.stderr)
            .contains("plan envelopes do not support --dt overrides"),
        "{}",
        String::from_utf8_lossy(&dt.stderr)
    );
}

#[test]
fn diff_backends_accepts_plans_rejects_plan_dt_and_checks_corpus_exclusivity() {
    let plan = repository_path("fixtures/plans/two_box.plan.json");
    let output = Command::new(env!("CARGO_BIN_EXE_sembla"))
        .current_dir(repository_path("."))
        .arg("diff-backends")
        .arg(&plan)
        .args(["--population", "1", "--seed", "1", "--ticks", "1"])
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(1));
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(stderr.contains("cuda backend unavailable"), "{stderr}");
    assert!(!stderr.contains("not yet runnable"), "{stderr}");

    let dt = Command::new(env!("CARGO_BIN_EXE_sembla"))
        .current_dir(repository_path("."))
        .arg("diff-backends")
        .arg(&plan)
        .args(["--population", "1", "--seed", "1", "--ticks", "1"])
        .args(["--dt", "0.5"])
        .output()
        .unwrap();
    assert_eq!(dt.status.code(), Some(1));
    assert!(
        String::from_utf8_lossy(&dt.stderr)
            .contains("plan envelopes do not support --dt overrides"),
        "{}",
        String::from_utf8_lossy(&dt.stderr)
    );

    let corpus = Command::new(env!("CARGO_BIN_EXE_sembla"))
        .current_dir(repository_path("."))
        .args([
            "diff-backends",
            "--all-plan-fixtures",
            "--population",
            "1",
            "--seed",
            "1",
            "--ticks",
            "1",
        ])
        .output()
        .unwrap();
    assert_eq!(corpus.status.code(), Some(1));
    let stderr = String::from_utf8(corpus.stderr).unwrap();
    // Grouped plans remain explicitly gated by the manifest-recorded runtime flag.
    assert!(
        stderr.contains("requires --enable grouped-observations"),
        "{stderr}"
    );

    for arguments in [
        vec!["--all-plan-fixtures", "--all-examples"],
        vec!["--all-plan-fixtures", "fixtures/plans/two_box.plan.json"],
        vec!["fixtures/plans/two_box.plan.json", "--all-plan-fixtures"],
    ] {
        let output = Command::new(env!("CARGO_BIN_EXE_sembla"))
            .current_dir(repository_path("."))
            .arg("diff-backends")
            .args(arguments)
            .output()
            .unwrap();
        assert_eq!(output.status.code(), Some(1));
        assert!(
            String::from_utf8_lossy(&output.stderr).contains("cannot be combined"),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
}
