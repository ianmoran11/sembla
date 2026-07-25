#![cfg(feature = "cuda")]

use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::time::{SystemTime, UNIX_EPOCH};

fn repository_path(relative: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join(relative)
}

fn temp_dir(label: &str) -> PathBuf {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let path = std::env::temp_dir().join(format!("sembla-gpu-{label}-{nonce}"));
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
}

const GROUPED_DIAGNOSTIC: &str = "--enable grouped-observations is not yet supported for diff-backends; see the grouped-observations backend follow-up PRD";

fn plan_fixture_paths() -> Vec<PathBuf> {
    let mut paths = Vec::new();
    for directory in ["fixtures/plans", "fixtures/plans/linked"] {
        paths.extend(
            std::fs::read_dir(repository_path(directory))
                .unwrap()
                .map(|entry| entry.unwrap().path())
                .filter(|path| {
                    path.is_file()
                        && path
                            .file_name()
                            .and_then(|name| name.to_str())
                            .is_some_and(|name| name.ends_with(".plan.json"))
                }),
        );
    }
    paths.sort();
    paths
}

fn plan_uses_grouped_views(path: &Path) -> bool {
    let source = std::fs::read_to_string(path).unwrap();
    let sembla_ir::ParsedInput::Plan(plan) = sembla_ir::parse_input(&source).unwrap() else {
        panic!("{} is not a plan", path.display());
    };
    plan.model
        .boxes
        .iter()
        .any(|model_box| !model_box.grouped_views.is_empty())
}

#[test]
#[ignore = "requires a CUDA GPU; run crates/sembla-cuda/scripts/run-differential-corpus.sh"]
fn differential_corpus_passes() {
    let output = Command::new(env!("CARGO_BIN_EXE_sembla"))
        .current_dir(repository_path("."))
        .args([
            "diff-backends",
            "--all-examples",
            "--population",
            "100",
            "--seed",
            "7",
            "--ticks",
            "20",
        ])
        .output()
        .unwrap();
    assert_success(&output);
}

#[test]
fn composition_plan_differential_corpus_rejects_grouped() {
    let output = Command::new(env!("CARGO_BIN_EXE_sembla"))
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
    assert_eq!(output.status.code(), Some(1));
    assert!(output.stdout.is_empty());
    assert_eq!(
        String::from_utf8(output.stderr).unwrap().trim_end(),
        GROUPED_DIAGNOSTIC
    );
}

#[test]
#[ignore = "requires a CUDA GPU; run crates/sembla-cuda/scripts/run-differential-corpus.sh"]
fn supported_composition_plan_differential_corpus_passes() {
    let mut grouped = 0;
    let mut supported = 0;
    for plan in plan_fixture_paths() {
        if plan_uses_grouped_views(&plan) {
            grouped += 1;
            continue;
        }
        let output = Command::new(env!("CARGO_BIN_EXE_sembla"))
            .current_dir(repository_path("."))
            .arg("diff-backends")
            .arg(&plan)
            .args(["--population", "1000", "--seed", "7", "--ticks", "20"])
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "plan={}\nstdout={}\nstderr={}",
            plan.display(),
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        supported += 1;
    }
    assert!(
        grouped > 0,
        "plan corpus must retain its grouped rejection case"
    );
    assert!(
        supported > 0,
        "plan corpus must retain CUDA-supported members"
    );
}

#[test]
#[ignore = "requires a CUDA GPU; run crates/sembla-cuda/scripts/run-differential-corpus.sh"]
fn cuda_manifest_verify_and_level_a_bytes_round_trip() {
    let temp = temp_dir("manifest");
    let model = repository_path("examples/two_state.json");
    let mut outputs = Vec::new();
    for name in ["first.csv", "second.csv"] {
        let out = temp.join(name);
        let output = Command::new(env!("CARGO_BIN_EXE_sembla"))
            .arg("run")
            .arg(&model)
            .args([
                "--population",
                "100",
                "--seed",
                "11",
                "--ticks",
                "20",
                "--backend",
                "cuda",
                "--out",
            ])
            .arg(&out)
            .output()
            .unwrap();
        assert_success(&output);
        outputs.push(out);
    }
    assert_eq!(
        std::fs::read(&outputs[0]).unwrap(),
        std::fs::read(&outputs[1]).unwrap()
    );
    assert_eq!(
        std::fs::read(format!("{}.summaries.csv", outputs[0].display())).unwrap(),
        std::fs::read(format!("{}.summaries.csv", outputs[1].display())).unwrap()
    );
    assert_eq!(
        std::fs::read(format!("{}.manifest.json", outputs[0].display())).unwrap(),
        std::fs::read(format!("{}.manifest.json", outputs[1].display())).unwrap()
    );
    let manifest_path = format!("{}.manifest.json", outputs[0].display());
    let manifest: serde_json::Value =
        serde_json::from_slice(&std::fs::read(&manifest_path).unwrap()).unwrap();
    assert_eq!(manifest["backend_identity"]["backend"], "cuda-native-f64");
    assert_eq!(manifest["backend_identity"]["precision"], "f64");
    assert_eq!(manifest["backend_identity"]["fell_back"], false);
    assert!(manifest["backend_identity"]["gpu_model"]
        .as_str()
        .is_some_and(|value| !value.is_empty()));
    assert!(manifest["backend_identity"]["driver_version"]
        .as_str()
        .is_some_and(|value| !value.is_empty()));
    let verify = Command::new(env!("CARGO_BIN_EXE_sembla"))
        .arg("verify-run")
        .arg(&manifest_path)
        .arg(&model)
        .args(["--population", "100"])
        .output()
        .unwrap();
    assert_success(&verify);
    std::fs::remove_dir_all(temp).unwrap();
}
