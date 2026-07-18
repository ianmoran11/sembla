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
        "sembla-manifest-{label}-{}-{nonce}",
        std::process::id()
    ));
    std::fs::create_dir_all(&path).unwrap();
    path
}

fn command(arguments: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_sembla"))
        .args(arguments)
        .output()
        .unwrap()
}

fn assert_success(output: &Output) {
    assert!(
        output.status.success(),
        "stdout={}\nstderr={}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

fn synth_population(path: &Path) {
    let output = Command::new(env!("CARGO_BIN_EXE_sembla"))
        .args([
            "synth-pop",
            "--persons",
            "80",
            "--employers",
            "8",
            "--initial-infected",
            "4",
            "--seed",
            "123",
            "--out",
        ])
        .arg(path)
        .output()
        .unwrap();
    assert_success(&output);
}

fn sidecar(output: &Path) -> PathBuf {
    PathBuf::from(format!("{}.manifest.json", output.display()))
}

#[test]
fn run_sweep_and_compare_manifests_are_byte_deterministic() {
    let temp = temp_dir("deterministic");
    let population = temp.join("population.bin");
    synth_population(&population);
    let sir = repository_path("examples/sir.json");
    let policy = repository_path("examples/sir_policy.json");

    let first_run = temp.join("first.csv");
    let second_run = temp.join("second.csv");
    for out in [&first_run, &second_run] {
        let output = Command::new(env!("CARGO_BIN_EXE_sembla"))
            .arg("run")
            .arg(&sir)
            .arg("--population")
            .arg(&population)
            .args(["--seed", "9", "--ticks", "3", "--out"])
            .arg(out)
            .output()
            .unwrap();
        assert_success(&output);
    }
    assert_eq!(
        std::fs::read(sidecar(&first_run)).unwrap(),
        std::fs::read(sidecar(&second_run)).unwrap()
    );
    let run_manifest: serde_json::Value =
        serde_json::from_slice(&std::fs::read(sidecar(&first_run)).unwrap()).unwrap();
    assert_eq!(
        run_manifest["schema_versions"],
        serde_json::json!({"backend_identity": 1, "manifest": 1})
    );
    assert_eq!(run_manifest["ir_hash_algorithm"], "sha256");
    assert_eq!(run_manifest["population_hash_algorithm"], "sha256");
    assert_eq!(run_manifest["results_hash_algorithm"], "sha256");
    assert_eq!(run_manifest["final_state_hash_algorithm"], "sha256");
    assert_eq!(
        run_manifest["backend_identity"],
        serde_json::json!({"backend": "cpu-oracle", "precision": "f64", "fell_back": false})
    );
    assert_eq!(run_manifest["determinism_level"], "A");
    assert_eq!(run_manifest["enabled_flags"], serde_json::json!([]));
    assert_eq!(run_manifest["population_source"], "population.bin");
    assert!(run_manifest["component_versions"]["sembla-cli"].is_string());
    assert!(run_manifest["component_versions"]["sembla-ir"].is_string());
    assert!(run_manifest["component_versions"]["sembla-runtime"].is_string());

    let first_sweep = temp.join("first-sweep");
    let second_sweep = temp.join("second-sweep");
    for out in [&first_sweep, &second_sweep] {
        let output = Command::new(env!("CARGO_BIN_EXE_sembla"))
            .arg("sweep")
            .arg(&sir)
            .arg("--population")
            .arg(&population)
            .args(["--seed", "9", "--draws", "2", "--ticks", "3", "--out"])
            .arg(out)
            .output()
            .unwrap();
        assert_success(&output);
    }
    assert_eq!(
        std::fs::read(first_sweep.join("run-manifest.json")).unwrap(),
        std::fs::read(second_sweep.join("run-manifest.json")).unwrap()
    );

    let first_compare = temp.join("first-compare.csv");
    let second_compare = temp.join("second-compare.csv");
    for out in [&first_compare, &second_compare] {
        let output = Command::new(env!("CARGO_BIN_EXE_sembla"))
            .arg("compare")
            .arg(&sir)
            .arg(&policy)
            .arg("--population")
            .arg(&population)
            .args(["--seed", "9", "--ticks", "3", "--out"])
            .arg(out)
            .output()
            .unwrap();
        assert_success(&output);
    }
    assert_eq!(
        std::fs::read(sidecar(&first_compare)).unwrap(),
        std::fs::read(sidecar(&second_compare)).unwrap()
    );

    for path in [
        sidecar(&first_run),
        first_sweep.join("run-manifest.json"),
        sidecar(&first_compare),
    ] {
        let bytes = std::fs::read(path).unwrap();
        assert!(bytes.ends_with(b"\n"));
        assert!(!bytes[..bytes.len() - 1].contains(&b'\n'));
    }
    std::fs::remove_dir_all(temp).unwrap();
}

#[test]
fn verify_run_round_trips_sir_generic_and_one_sweep_draw() {
    let temp = temp_dir("round-trip");
    let population = temp.join("population.bin");
    synth_population(&population);
    let sir = repository_path("examples/sir.json");
    let sir_results = temp.join("sir.csv");
    let run_output = Command::new(env!("CARGO_BIN_EXE_sembla"))
        .arg("run")
        .arg(&sir)
        .arg("--population")
        .arg(&population)
        .args(["--seed", "17", "--ticks", "4", "--dt", "0.5", "--out"])
        .arg(&sir_results)
        .output()
        .unwrap();
    assert_success(&run_output);
    let verify = Command::new(env!("CARGO_BIN_EXE_sembla"))
        .arg("verify-run")
        .arg(sidecar(&sir_results))
        .arg(&sir)
        .arg("--population")
        .arg(&population)
        .output()
        .unwrap();
    assert_success(&verify);
    assert!(String::from_utf8_lossy(&verify.stdout).contains("verified 1 execution"));

    let generic = repository_path("examples/reversible_ctmc.json");
    let generic_results = temp.join("generic.csv");
    let generic_run = Command::new(env!("CARGO_BIN_EXE_sembla"))
        .arg("run")
        .arg(&generic)
        .args(["--population", "25", "--seed", "5", "--ticks", "3", "--out"])
        .arg(&generic_results)
        .output()
        .unwrap();
    assert_success(&generic_run);
    let generic_verify = command(&[
        "verify-run",
        sidecar(&generic_results).to_str().unwrap(),
        generic.to_str().unwrap(),
        "--population",
        "25",
    ]);
    assert_success(&generic_verify);

    let sweep = temp.join("sweep");
    let sweep_output = Command::new(env!("CARGO_BIN_EXE_sembla"))
        .arg("sweep")
        .arg(&sir)
        .arg("--population")
        .arg(&population)
        .args(["--seed", "23", "--draws", "3", "--ticks", "3", "--out"])
        .arg(&sweep)
        .output()
        .unwrap();
    assert_success(&sweep_output);
    let sweep_verify = Command::new(env!("CARGO_BIN_EXE_sembla"))
        .arg("verify-run")
        .arg(sweep.join("run-manifest.json"))
        .arg(&sir)
        .arg("--population")
        .arg(&population)
        .args(["--draw", "1"])
        .output()
        .unwrap();
    assert_success(&sweep_verify);

    std::fs::remove_dir_all(temp).unwrap();
}

#[test]
fn verify_run_reports_tampered_seed_and_partial_backend_tuple() {
    let temp = temp_dir("tamper");
    let model = repository_path("examples/reversible_ctmc.json");
    let results = temp.join("results.csv");
    let output = Command::new(env!("CARGO_BIN_EXE_sembla"))
        .arg("run")
        .arg(&model)
        .args(["--population", "40", "--seed", "7", "--ticks", "4", "--out"])
        .arg(&results)
        .output()
        .unwrap();
    assert_success(&output);

    let original = sidecar(&results);
    let mut value: serde_json::Value =
        serde_json::from_slice(&std::fs::read(&original).unwrap()).unwrap();
    value["seed"] = serde_json::json!(8);
    let tampered = temp.join("tampered.json");
    std::fs::write(&tampered, serde_json::to_vec(&value).unwrap()).unwrap();
    let verify = command(&[
        "verify-run",
        tampered.to_str().unwrap(),
        model.to_str().unwrap(),
        "--population",
        "40",
    ]);
    assert_eq!(verify.status.code(), Some(1));
    let stderr = String::from_utf8_lossy(&verify.stderr);
    assert!(stderr.contains("verification mismatch"), "{stderr}");
    assert!(stderr.contains("results_sha256"), "{stderr}");

    let mut partial: serde_json::Value =
        serde_json::from_slice(&std::fs::read(&original).unwrap()).unwrap();
    partial["backend_identity"]
        .as_object_mut()
        .unwrap()
        .remove("fell_back");
    let partial_path = temp.join("partial-backend.json");
    std::fs::write(&partial_path, serde_json::to_vec(&partial).unwrap()).unwrap();
    let rejected = command(&[
        "verify-run",
        partial_path.to_str().unwrap(),
        model.to_str().unwrap(),
        "--population",
        "40",
    ]);
    assert_eq!(rejected.status.code(), Some(1));
    let stderr = String::from_utf8_lossy(&rejected.stderr);
    assert!(stderr.contains("backend_identity tuple"), "{stderr}");
    assert!(stderr.contains("fell_back"), "{stderr}");

    std::fs::remove_dir_all(temp).unwrap();
}
