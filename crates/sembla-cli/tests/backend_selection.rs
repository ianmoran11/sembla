use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

fn repository_path(relative: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join(relative)
}

fn temp_dir() -> PathBuf {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let path = std::env::temp_dir().join(format!("sembla-backend-selection-{nonce}"));
    std::fs::create_dir_all(&path).unwrap();
    path
}

#[test]
fn default_backend_is_cpu_and_manifest_tuple_is_exact() {
    let temp = temp_dir();
    let default_out = temp.join("default.csv");
    let explicit_out = temp.join("explicit.csv");
    for (out, explicit) in [(&default_out, false), (&explicit_out, true)] {
        let mut command = Command::new(env!("CARGO_BIN_EXE_sembla"));
        command
            .arg("run")
            .arg(repository_path("examples/two_state.json"))
            .args(["--population", "20", "--seed", "1", "--ticks", "2"]);
        if explicit {
            command.args(["--backend", "cpu"]);
        }
        let status = command.arg("--out").arg(out).status().unwrap();
        assert!(status.success());
    }
    assert_eq!(
        std::fs::read(&default_out).unwrap(),
        std::fs::read(&explicit_out).unwrap()
    );
    assert_eq!(
        std::fs::read(format!("{}.manifest.json", default_out.display())).unwrap(),
        std::fs::read(format!("{}.manifest.json", explicit_out.display())).unwrap()
    );
    let manifest: serde_json::Value = serde_json::from_slice(
        &std::fs::read(format!("{}.manifest.json", default_out.display())).unwrap(),
    )
    .unwrap();
    assert_eq!(
        manifest["backend_identity"],
        serde_json::json!({"backend":"cpu-oracle","precision":"f64","fell_back":false})
    );
    std::fs::remove_dir_all(temp).unwrap();
}

#[test]
#[cfg(not(feature = "cuda"))]
fn unavailable_cuda_request_is_nonzero_and_never_falls_back() {
    let output = Command::new(env!("CARGO_BIN_EXE_sembla"))
        .arg("run")
        .arg(repository_path("examples/two_state.json"))
        .args([
            "--population",
            "20",
            "--seed",
            "1",
            "--ticks",
            "1",
            "--backend",
            "cuda",
        ])
        .output()
        .unwrap();
    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("cuda backend unavailable"), "{stderr}");
    assert!(!stderr.contains("fell back"), "{stderr}");
}
