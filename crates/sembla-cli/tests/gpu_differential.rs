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
