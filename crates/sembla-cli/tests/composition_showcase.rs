use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::time::{SystemTime, UNIX_EPOCH};

const DEMOS: [&str; 4] = [
    "demo_counterfactual_outbreak",
    "demo_coordinated_regions",
    "demo_regional_surveillance",
    "demo_national_network",
];

fn repository_path(path: impl AsRef<Path>) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join(path)
}

fn temp_dir() -> PathBuf {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let path = std::env::temp_dir().join(format!(
        "sembla-composition-showcase-{}-{nonce}",
        std::process::id()
    ));
    std::fs::create_dir_all(&path).unwrap();
    path
}

fn assert_success(label: &str, output: &Output) {
    assert!(
        output.status.success(),
        "{label}\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn showcase_bundles_verify_and_runs_are_replayable() {
    let temp = temp_dir();
    let population = temp.join("population.bin");
    let synth = Command::new(env!("CARGO_BIN_EXE_sembla"))
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
    assert_success("synth-pop", &synth);

    for demo in DEMOS {
        let bundle = repository_path(format!("fixtures/demos/composition/{demo}"));
        let plan = bundle.join("executable-plan.json");
        let verified = Command::new(env!("CARGO_BIN_EXE_sembla"))
            .arg("bundle-verify")
            .arg(&bundle)
            .output()
            .unwrap();
        assert_success(&format!("bundle-verify {demo}"), &verified);

        let outputs = [
            temp.join(format!("{demo}.first.csv")),
            temp.join(format!("{demo}.second.csv")),
        ];
        let mut run_stdout = Vec::new();
        for output_path in &outputs {
            let run = Command::new(env!("CARGO_BIN_EXE_sembla"))
                .arg("run")
                .arg(&plan)
                .arg("--population")
                .arg(&population)
                .args(["--seed", "55", "--ticks", "8", "--out"])
                .arg(output_path)
                .output()
                .unwrap();
            assert_success(&format!("run {demo}"), &run);
            run_stdout.push(run.stdout);
        }
        assert_eq!(run_stdout[0], run_stdout[1], "{demo} hashes changed");
        let golden_dir = repository_path("fixtures/demos/composition/goldens");
        let hashes: serde_json::Value = serde_json::from_slice(
            &std::fs::read(golden_dir.join(format!("{demo}.seed55.ticks8.hashes.json"))).unwrap(),
        )
        .unwrap();
        let expected_stdout = format!(
            "results_sha256={} final_state_sha256={} observation_sha256={}\n",
            hashes["results_sha256"].as_str().unwrap(),
            hashes["final_state_sha256"].as_str().unwrap(),
            hashes["observation_sha256"].as_str().unwrap(),
        );
        assert_eq!(run_stdout[0], expected_stdout.as_bytes(), "{demo}");
        for suffix in ["", ".summaries.csv", ".manifest.json"] {
            assert_eq!(
                std::fs::read(format!("{}{suffix}", outputs[0].display())).unwrap(),
                std::fs::read(format!("{}{suffix}", outputs[1].display())).unwrap(),
                "{demo} artifact '{suffix}' was not deterministic"
            );
        }

        assert_eq!(
            std::fs::read(format!("{}.summaries.csv", outputs[0].display())).unwrap(),
            std::fs::read(golden_dir.join(format!("{demo}.seed55.ticks8.summaries.csv"))).unwrap(),
            "{demo} summaries changed"
        );

        let manifest_path = PathBuf::from(format!("{}.manifest.json", outputs[0].display()));
        let manifest: serde_json::Value =
            serde_json::from_slice(&std::fs::read(&manifest_path).unwrap()).unwrap();
        assert_eq!(manifest["plan"]["origin"], "linked", "{demo}");
        assert_eq!(manifest["plan"]["enabled_features"], serde_json::json!([]));
        assert_eq!(
            manifest["linked_source"]["source_hash"]["domain"], "sembla.source-artifact/v1",
            "{demo}"
        );

        let replay = Command::new(env!("CARGO_BIN_EXE_sembla"))
            .arg("verify-run")
            .arg(&manifest_path)
            .arg(&plan)
            .arg("--population")
            .arg(&population)
            .output()
            .unwrap();
        assert_success(&format!("verify-run {demo}"), &replay);
        assert_eq!(replay.stdout, b"verified 1 execution(s)\n", "{demo}");
    }

    std::fs::remove_dir_all(temp).unwrap();
}
