use std::{fs, path::Path, process::Command};

fn run(model: &Path, out: &Path, timing: Option<&Path>) -> std::process::Output {
    let mut command = Command::new(env!("CARGO_BIN_EXE_sembla"));
    command
        .arg("run")
        .arg(model)
        .args([
            "--population",
            "17",
            "--seed",
            "19",
            "--ticks",
            "2",
            "--backend",
            "cpu",
            "--out",
        ])
        .arg(out);
    if let Some(timing) = timing {
        command.arg("--lifecycle-timing-json").arg(timing);
    }
    command.output().unwrap()
}

#[test]
fn lifecycle_report_reconciles_without_changing_results_or_overwriting_inputs() {
    let root = std::env::temp_dir().join(format!(
        "sembla-lifecycle-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    fs::create_dir(&root).unwrap();
    let source = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples/sir.json");
    let model = root.join("model.json");
    fs::copy(source, &model).unwrap();
    let plain = root.join("plain.csv");
    let timed = root.join("timed.csv");
    let report = root.join("lifecycle.json");
    let baseline = run(&model, &plain, None);
    assert!(
        baseline.status.success(),
        "{}",
        String::from_utf8_lossy(&baseline.stderr)
    );
    let observed = run(&model, &timed, Some(&report));
    assert!(
        observed.status.success(),
        "{}",
        String::from_utf8_lossy(&observed.stderr)
    );
    assert_eq!(baseline.stdout, observed.stdout);
    assert_eq!(fs::read(&plain).unwrap(), fs::read(&timed).unwrap());
    assert_eq!(
        fs::read(root.join("plain.csv.summaries.csv")).unwrap(),
        fs::read(root.join("timed.csv.summaries.csv")).unwrap()
    );
    let doc: serde_json::Value = serde_json::from_slice(&fs::read(&report).unwrap()).unwrap();
    assert_eq!(doc["schema"], "sembla-run-lifecycle-timing-v1");
    assert_eq!(doc["backend"], "cpu");
    assert!(doc["cuda_construction_ms"].is_null());
    let total = doc["total_ms"].as_f64().unwrap();
    let sum: f64 = doc["phases_ms"]
        .as_object()
        .unwrap()
        .values()
        .map(|v| {
            let ms = v.as_f64().unwrap();
            assert!(ms >= 0.0);
            ms
        })
        .sum();
    assert!(total > 0.0 && (total - sum).abs() < 0.001);

    let original = fs::read(&model).unwrap();
    let rejected = run(&model, &timed, Some(&model));
    assert!(!rejected.status.success());
    assert!(String::from_utf8_lossy(&rejected.stderr).contains("conflicts with"));
    assert_eq!(fs::read(&model).unwrap(), original);
    let alias = root.join("alias.json");
    fs::hard_link(&model, &alias).unwrap();
    assert!(!run(&model, &timed, Some(&alias)).status.success());
    assert_eq!(fs::read(&model).unwrap(), original);
    fs::remove_dir_all(root).unwrap();
}
