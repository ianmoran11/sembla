use std::path::{Path, PathBuf};
use std::process::Command;
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
    let path = std::env::temp_dir().join(format!("sembla-{label}-{}-{nonce}", std::process::id()));
    std::fs::create_dir_all(&path).unwrap();
    path
}

#[test]
fn run_two_state_prints_deterministic_per_rule_counts() {
    let output = Command::new(env!("CARGO_BIN_EXE_sembla"))
        .arg("run")
        .arg(repository_path("examples/two_state.json"))
        .args(["--seed", "42", "--ticks", "3", "--population", "1000"])
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
            "tick=0 box=population rule_id=0 fired=16\n",
            "tick=0 box=population rule_id=1 fired=0\n",
            "tick=1 box=population rule_id=0 fired=20\n",
            "tick=1 box=population rule_id=1 fired=0\n",
            "tick=2 box=population rule_id=0 fired=13\n",
            "tick=2 box=population rule_id=1 fired=1\n",
        )
    );
}

#[test]
fn run_multi_box_reports_counts_per_box() {
    let output = Command::new(env!("CARGO_BIN_EXE_sembla"))
        .arg("run")
        .arg(repository_path("examples/two_box.json"))
        .args(["--seed", "9", "--ticks", "2", "--population", "16"])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        String::from_utf8(output.stdout).unwrap(),
        concat!(
            "tick=0 box=population rule_id=0 fired=0\n",
            "tick=0 box=controller rule_id=1 fired=1\n",
            "tick=1 box=population rule_id=0 fired=16\n",
            "tick=1 box=controller rule_id=1 fired=0\n",
        )
    );
}

fn reported_hashes(output: &std::process::Output) -> (String, String) {
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout.clone()).unwrap();
    let mut fields = stdout.trim().split_ascii_whitespace();
    let results = fields
        .next()
        .and_then(|field| field.strip_prefix("results_sha256="))
        .expect("results hash field")
        .to_owned();
    let state = fields
        .next()
        .and_then(|field| field.strip_prefix("final_state_sha256="))
        .expect("final state hash field")
        .to_owned();
    assert!(fields.next().is_none(), "unexpected stdout: {stdout}");
    assert_eq!(results.len(), 64);
    assert_eq!(state.len(), 64);
    (results, state)
}

#[test]
fn hundred_thousand_cli_pipeline_is_deterministic_across_fresh_processes() {
    let temp = temp_dir("sir-cli-100k");
    let first_population = temp.join("population-first.bin");
    let second_population = temp.join("population-second.bin");
    for population in [&first_population, &second_population] {
        let synth = Command::new(env!("CARGO_BIN_EXE_sembla"))
            .arg("synth-pop")
            .args([
                "--persons",
                "100000",
                "--employers",
                "500",
                "--initial-infected",
                "100",
                "--seed",
                "12",
                "--out",
            ])
            .arg(population)
            .output()
            .unwrap();
        assert!(
            synth.status.success(),
            "{}",
            String::from_utf8_lossy(&synth.stderr)
        );
    }
    assert_eq!(
        std::fs::read(&first_population).unwrap(),
        std::fs::read(&second_population).unwrap(),
        "fresh synth-pop processes must produce identical population bytes"
    );

    let params = temp.join("params.json");
    let different_params = temp.join("different-params.json");
    std::fs::write(&params, r#"{"beta":0.7,"gamma":0.12}"#).unwrap();
    std::fs::write(&different_params, r#"{"beta":0.5,"gamma":0.12}"#).unwrap();
    let run_once = |population: &Path, seed: &str, params: &Path, out: &Path| {
        Command::new(env!("CARGO_BIN_EXE_sembla"))
            .arg("run")
            .arg(repository_path("examples/sir.json"))
            .arg("--population")
            .arg(population)
            .args(["--seed", seed, "--ticks", "100", "--params"])
            .arg(params)
            .arg("--out")
            .arg(out)
            .output()
            .unwrap()
    };

    let first_path = temp.join("first.csv");
    let second_path = temp.join("second.csv");
    let different_seed_path = temp.join("different-seed.csv");
    let different_theta_path = temp.join("different-theta.csv");
    let first = run_once(&first_population, "55", &params, &first_path);
    let second = run_once(&second_population, "55", &params, &second_path);
    let different_seed = run_once(&first_population, "56", &params, &different_seed_path);
    let different_theta = run_once(
        &first_population,
        "55",
        &different_params,
        &different_theta_path,
    );

    let first_hashes = reported_hashes(&first);
    let second_hashes = reported_hashes(&second);
    let different_seed_hashes = reported_hashes(&different_seed);
    let different_theta_hashes = reported_hashes(&different_theta);
    assert_eq!(first_hashes, second_hashes);
    assert_eq!(
        std::fs::read(&first_path).unwrap(),
        std::fs::read(&second_path).unwrap(),
        "fresh CLI runs must produce exact-equal CSV bytes"
    );
    assert_ne!(first_hashes.0, different_seed_hashes.0);
    assert_ne!(first_hashes.1, different_seed_hashes.1);
    assert_ne!(first_hashes.0, different_theta_hashes.0);
    assert_ne!(first_hashes.1, different_theta_hashes.1);

    let csv = std::fs::read_to_string(first_path).unwrap();
    assert!(csv.starts_with(
        "# params={\"beta\":0.7,\"gamma\":0.12}\n# dt=0.25\ntick,S,I,R,fired_infect,fired_recover,deferred_total\n"
    ));
    assert_eq!(csv.lines().count(), 103);
    std::fs::remove_dir_all(temp).unwrap();
}

#[test]
fn parameter_override_errors_name_the_parameter() {
    let temp = temp_dir("sir-param-errors");
    let population = temp.join("population.bin");
    let synth = Command::new(env!("CARGO_BIN_EXE_sembla"))
        .arg("synth-pop")
        .args([
            "--persons",
            "10",
            "--employers",
            "2",
            "--initial-infected",
            "1",
            "--seed",
            "1",
            "--out",
        ])
        .arg(&population)
        .output()
        .unwrap();
    assert!(synth.status.success());
    for (file, body, parameter) in [
        ("unknown.json", r#"{"delta":1.0}"#, "delta"),
        ("wrong.json", r#"{"beta":"fast"}"#, "beta"),
    ] {
        let params = temp.join(file);
        std::fs::write(&params, body).unwrap();
        let output = Command::new(env!("CARGO_BIN_EXE_sembla"))
            .arg("run")
            .arg(repository_path("examples/sir.json"))
            .arg("--population")
            .arg(&population)
            .args(["--seed", "2", "--ticks", "1", "--params"])
            .arg(&params)
            .arg("--out")
            .arg(temp.join("results.csv"))
            .output()
            .unwrap();
        assert_eq!(output.status.code(), Some(1));
        assert!(String::from_utf8(output.stderr)
            .unwrap()
            .contains(parameter));
    }
    std::fs::remove_dir_all(temp).unwrap();
}

#[test]
fn run_rejects_missing_duplicate_and_malformed_flags_with_usage_exit() {
    let path = repository_path("examples/two_state.json");
    let cases: &[&[&str]] = &[
        &["--seed", "42", "--ticks", "3"],
        &["--seed", "x", "--ticks", "3", "--population", "10"],
        &["--seed", "42", "--ticks", "x", "--population", "10"],
        &["--seed", "42", "--ticks", "3", "--population", "x"],
        &[
            "--seed",
            "42",
            "--seed",
            "43",
            "--ticks",
            "3",
            "--population",
            "10",
        ],
    ];
    for flags in cases {
        let output = Command::new(env!("CARGO_BIN_EXE_sembla"))
            .arg("run")
            .arg(&path)
            .args(*flags)
            .output()
            .unwrap();
        assert_eq!(output.status.code(), Some(2), "flags: {flags:?}");
        assert!(String::from_utf8(output.stderr)
            .unwrap()
            .contains("usage: sembla"));
    }
}
