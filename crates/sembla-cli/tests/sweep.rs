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
    let path = std::env::temp_dir().join(format!("sembla-{label}-{}-{nonce}", std::process::id()));
    std::fs::create_dir_all(&path).unwrap();
    path
}

fn command(arguments: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_sembla"))
        .args(arguments)
        .output()
        .unwrap()
}

fn synth(path: &Path, persons: usize, employers: usize, infected: usize) {
    let output = Command::new(env!("CARGO_BIN_EXE_sembla"))
        .args([
            "synth-pop",
            "--persons",
            &persons.to_string(),
            "--employers",
            &employers.to_string(),
            "--initial-infected",
            &infected.to_string(),
            "--seed",
            "123",
            "--out",
        ])
        .arg(path)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
}

fn sweep(population: &Path, out: &Path, seed: u64, draws: u32, ticks: u32, params: Option<&Path>) {
    let mut process = Command::new(env!("CARGO_BIN_EXE_sembla"));
    process
        .arg("sweep")
        .arg(repository_path("examples/sir.json"))
        .arg("--population")
        .arg(population)
        .args([
            "--seed",
            &seed.to_string(),
            "--draws",
            &draws.to_string(),
            "--ticks",
            &ticks.to_string(),
            "--out",
        ])
        .arg(out);
    if let Some(params) = params {
        process.arg("--params").arg(params);
    }
    let output = process.output().unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("manifest_sha256="), "{stdout}");
    assert!(stdout.contains("summary_sha256="), "{stdout}");
}

fn custom_sweep(
    population: &Path,
    out: &Path,
    seed: u64,
    draws: Option<u32>,
    ticks: u32,
    noise: Option<&str>,
    theta_file: Option<&Path>,
) -> Output {
    let mut process = Command::new(env!("CARGO_BIN_EXE_sembla"));
    process
        .arg("sweep")
        .arg(repository_path("examples/sir.json"))
        .arg("--population")
        .arg(population)
        .arg("--seed")
        .arg(seed.to_string())
        .arg("--ticks")
        .arg(ticks.to_string())
        .arg("--out")
        .arg(out);
    if let Some(draws) = draws {
        process.arg("--draws").arg(draws.to_string());
    }
    if let Some(noise) = noise {
        process.arg("--noise").arg(noise);
    }
    if let Some(theta_file) = theta_file {
        process.arg("--theta-file").arg(theta_file);
    }
    process.output().unwrap()
}

struct PairsSweepOptions<'a> {
    seed: u64,
    draws: Option<u32>,
    noise: &'a str,
    theta_file: Option<&'a Path>,
}

fn pairs_sweep(
    model: &Path,
    population: &Path,
    out: &Path,
    pairs: &Path,
    options: PairsSweepOptions<'_>,
) -> Output {
    let mut process = Command::new(env!("CARGO_BIN_EXE_sembla"));
    process
        .arg("sweep")
        .arg(model)
        .arg("--population")
        .arg(population)
        .arg("--seed")
        .arg(options.seed.to_string())
        .arg("--ticks")
        .arg("8")
        .arg("--noise")
        .arg(options.noise)
        .arg("--out")
        .arg(out)
        .arg("--export-pairs")
        .arg(pairs);
    if let Some(draws) = options.draws {
        process.arg("--draws").arg(draws.to_string());
    }
    if let Some(theta_file) = options.theta_file {
        process.arg("--theta-file").arg(theta_file);
    }
    process.output().unwrap()
}

fn pairs_meta_path(pairs: &Path) -> PathBuf {
    PathBuf::from(format!("{}.meta.json", pairs.display()))
}

fn assert_pairs_match_sweep_outputs(pairs: &Path, out: &Path) {
    let pairs_csv = std::fs::read_to_string(pairs).unwrap();
    let mut pair_lines = pairs_csv.lines();
    assert_eq!(pair_lines.next(), Some("k,beta,gamma,peak_tick,peak_I"));

    let theta_csv = String::from_utf8(file(out, "manifest.csv")).unwrap();
    let mut theta_lines = theta_csv.lines().filter(|line| !line.starts_with('#'));
    let theta_header = theta_lines.next().unwrap().split(',').collect::<Vec<_>>();
    let beta_index = theta_header
        .iter()
        .position(|name| *name == "beta")
        .unwrap();
    let gamma_index = theta_header
        .iter()
        .position(|name| *name == "gamma")
        .unwrap();

    let theta_rows = theta_lines.collect::<Vec<_>>();
    let pair_rows = pair_lines.collect::<Vec<_>>();
    assert_eq!(pair_rows.len(), theta_rows.len());
    for (draw, (pair_row, theta_row)) in pair_rows.iter().zip(theta_rows).enumerate() {
        let pair_values = pair_row.split(',').collect::<Vec<_>>();
        let theta_values = theta_row.split(',').collect::<Vec<_>>();
        assert_eq!(pair_values[0], draw.to_string());
        assert_eq!(pair_values[1], theta_values[beta_index]);
        assert_eq!(pair_values[2], theta_values[gamma_index]);

        let summaries =
            std::fs::read_to_string(out.join(format!("draw_{draw}.csv.summaries.csv"))).unwrap();
        let summary_values = summaries
            .lines()
            .skip(1)
            .map(|row| row.split_once(',').unwrap())
            .collect::<std::collections::BTreeMap<_, _>>();
        assert_eq!(pair_values[3], summary_values["peak_tick"]);
        assert_eq!(pair_values[4], summary_values["peak_I"]);
    }
}

fn assert_success(output: &Output) {
    assert!(
        output.status.success(),
        "stdout={}\nstderr={}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

fn file(path: &Path, name: &str) -> Vec<u8> {
    std::fs::read(path.join(name)).unwrap()
}

fn run_manifest(path: &Path) -> serde_json::Value {
    serde_json::from_slice(&file(path, "run-manifest.json")).unwrap()
}

#[test]
fn sweep_is_reproducible_seed_sensitive_pinnable_and_uses_crn() {
    let temp = temp_dir("sweep-contract");
    let population = temp.join("population.bin");
    synth(&population, 2_000, 40, 20);

    let first = temp.join("first");
    let repeat = temp.join("repeat");
    let changed = temp.join("changed");
    sweep(&population, &first, 77, 4, 6, None);
    sweep(&population, &repeat, 77, 4, 6, None);
    sweep(&population, &changed, 78, 4, 6, None);
    for name in [
        "manifest.csv",
        "summary.csv",
        "draw_0.csv",
        "draw_1.csv",
        "draw_2.csv",
        "draw_3.csv",
    ] {
        assert_eq!(file(&first, name), file(&repeat, name), "{name}");
        assert_ne!(file(&first, name), file(&changed, name), "{name}");
    }

    let gamma_pin = temp.join("gamma.json");
    std::fs::write(&gamma_pin, r#"{"gamma":0.125}"#).unwrap();
    let pinned = temp.join("pinned");
    sweep(&population, &pinned, 77, 5, 2, Some(&gamma_pin));
    let manifest = String::from_utf8(file(&pinned, "manifest.csv")).unwrap();
    assert!(manifest.starts_with("# parameter_status,beta=sampled,gamma=pinned\n"));
    let rows = manifest.lines().skip(2).collect::<Vec<_>>();
    let beta = rows
        .iter()
        .map(|row| row.split(',').nth(1).unwrap())
        .collect::<Vec<_>>();
    assert!(beta.windows(2).any(|pair| pair[0] != pair[1]));
    assert!(rows
        .iter()
        .all(|row| row.split(',').nth(2) == Some("0.125")));

    // Artificially identical theta under different k must reuse exactly the
    // same simulation shocks because sweep runs use the unchanged seed.
    let all_pins = temp.join("all.json");
    std::fs::write(&all_pins, r#"{"beta":0.7,"gamma":0.12}"#).unwrap();
    let paired = temp.join("paired");
    sweep(&population, &paired, 81, 2, 8, Some(&all_pins));
    assert_eq!(file(&paired, "draw_0.csv"), file(&paired, "draw_1.csv"));

    std::fs::remove_dir_all(temp).unwrap();
}

#[test]
fn twenty_draw_hundred_thousand_person_summary_has_monotone_bands() {
    let temp = temp_dir("sweep-100k");
    let population = temp.join("population.bin");
    synth(&population, 100_000, 500, 100);
    let out = temp.join("sweep");
    sweep(&population, &out, 2025, 20, 50, None);

    let summary = String::from_utf8(file(&out, "summary.csv")).unwrap();
    let mut lines = summary.lines();
    let header = lines.next().unwrap().split(',').collect::<Vec<_>>();
    assert_eq!(lines.clone().count(), 50);
    for row in lines {
        let values = row
            .split(',')
            .map(|value| value.parse::<usize>().unwrap())
            .collect::<Vec<_>>();
        for column in 0..6 {
            let start = 1 + column * 5;
            assert!(
                values[start] <= values[start + 2] && values[start + 2] <= values[start + 4],
                "non-monotone {} bands in {row}",
                header[start].trim_end_matches("_p05")
            );
        }
    }
    assert_eq!(
        std::fs::read_dir(&out)
            .unwrap()
            .filter_map(Result::ok)
            .filter(|entry| entry.file_name().to_string_lossy().starts_with("draw_"))
            .count(),
        20
    );
    std::fs::remove_dir_all(temp).unwrap();
}

#[test]
fn independent_noise_is_k_stable_and_preserves_prior_theta() {
    let temp = temp_dir("independent-stability");
    let population = temp.join("population.bin");
    synth(&population, 2_000, 40, 20);

    let five = temp.join("five");
    let fifty = temp.join("fifty");
    let crn = temp.join("crn");
    assert_success(&custom_sweep(
        &population,
        &five,
        99,
        Some(5),
        8,
        Some("independent"),
        None,
    ));
    assert_success(&custom_sweep(
        &population,
        &fifty,
        99,
        Some(50),
        8,
        Some("independent"),
        None,
    ));
    assert_success(&custom_sweep(
        &population,
        &crn,
        99,
        Some(5),
        8,
        Some("crn"),
        None,
    ));

    let five_manifest = run_manifest(&five);
    let fifty_manifest = run_manifest(&fifty);
    let five_k3 = &five_manifest["executions"][3];
    let fifty_k3 = &fifty_manifest["executions"][3];
    assert_eq!(five_k3["seed"], fifty_k3["seed"]);
    assert_eq!(five_k3["resolved_theta"], fifty_k3["resolved_theta"]);
    assert_eq!(file(&five, "draw_3.csv"), file(&fifty, "draw_3.csv"));
    assert_eq!(five_manifest["noise_mode"], "independent");
    assert_eq!(five_manifest["theta_source"]["kind"], "prior");
    assert_eq!(five_manifest["theta_source"]["algorithm"], "sha256");
    assert_eq!(
        five_manifest["theta_source"]["sha256"],
        five_manifest["ir_hash"]
    );

    // Prior coordinates never include the simulation-noise mode.
    assert_eq!(file(&five, "manifest.csv"), file(&crn, "manifest.csv"));
    for draw in 0..5 {
        assert_eq!(
            five_manifest["executions"][draw]["resolved_theta"],
            run_manifest(&crn)["executions"][draw]["resolved_theta"]
        );
    }

    std::fs::remove_dir_all(temp).unwrap();
}

#[test]
fn identical_theta_is_crn_paired_and_independent_noise_is_distinct() {
    let temp = temp_dir("independent-identical-theta");
    let population = temp.join("population.bin");
    synth(&population, 2_000, 40, 20);
    let theta = temp.join("theta.json");
    let theta_bytes = b"[{\"beta\":0.7,\"gamma\":0.12},{\"beta\":0.7,\"gamma\":0.12}]\n";
    std::fs::write(&theta, theta_bytes).unwrap();

    let crn = temp.join("crn");
    let independent = temp.join("independent");
    let crn_output = custom_sweep(&population, &crn, 81, None, 12, Some("crn"), Some(&theta));
    assert_success(&crn_output);
    let independent_output = custom_sweep(
        &population,
        &independent,
        81,
        None,
        12,
        Some("independent"),
        Some(&theta),
    );
    assert_success(&independent_output);

    assert_eq!(file(&crn, "draw_0.csv"), file(&crn, "draw_1.csv"));
    assert_ne!(
        file(&independent, "draw_0.csv"),
        file(&independent, "draw_1.csv")
    );
    assert_eq!(
        file(&crn, "manifest.csv"),
        file(&independent, "manifest.csv")
    );
    assert!(String::from_utf8(file(&crn, "manifest.csv"))
        .unwrap()
        .starts_with("# theta_source=file\n# parameter_status,beta=file,gamma=file\n"));

    let expected_hash = format!("{:x}", Sha256::digest(theta_bytes));
    let stdout = String::from_utf8(independent_output.stdout).unwrap();
    assert!(
        stdout.contains(&format!("theta_file_sha256={expected_hash}")),
        "{stdout}"
    );
    let manifest = run_manifest(&independent);
    assert_eq!(manifest["theta_source"]["kind"], "file");
    assert_eq!(manifest["theta_source"]["sha256"], expected_hash);
    assert_ne!(
        manifest["executions"][0]["seed"],
        manifest["executions"][1]["seed"]
    );

    std::fs::remove_dir_all(temp).unwrap();
}

#[test]
fn prior_manifest_theta_round_trips_through_theta_file() {
    let temp = temp_dir("theta-round-trip");
    let population = temp.join("population.bin");
    synth(&population, 2_000, 40, 20);
    let prior = temp.join("prior");
    assert_success(&custom_sweep(
        &population,
        &prior,
        37,
        Some(4),
        8,
        None,
        None,
    ));

    let csv = String::from_utf8(file(&prior, "manifest.csv")).unwrap();
    let mut lines = csv.lines();
    assert!(lines.next().unwrap().starts_with("# parameter_status"));
    let names = lines
        .next()
        .unwrap()
        .split(',')
        .skip(1)
        .map(str::to_owned)
        .collect::<Vec<_>>();
    let assignments = lines
        .map(|row| {
            let values = row.split(',').skip(1);
            names
                .iter()
                .zip(values)
                .map(|(name, value)| {
                    (
                        name.clone(),
                        serde_json::Value::from(value.parse::<f64>().unwrap()),
                    )
                })
                .collect::<serde_json::Map<_, _>>()
        })
        .map(serde_json::Value::Object)
        .collect::<Vec<_>>();
    let theta = temp.join("theta.json");
    std::fs::write(&theta, serde_json::to_vec(&assignments).unwrap()).unwrap();

    let replay = temp.join("replay");
    assert_success(&custom_sweep(
        &population,
        &replay,
        37,
        None,
        8,
        Some("crn"),
        Some(&theta),
    ));
    for draw in 0..4 {
        assert_eq!(
            file(&prior, &format!("draw_{draw}.csv")),
            file(&replay, &format!("draw_{draw}.csv"))
        );
    }

    std::fs::remove_dir_all(temp).unwrap();
}

#[test]
fn theta_file_reports_missing_unknown_and_draw_conflict() {
    let temp = temp_dir("theta-errors");
    let missing = temp.join("missing.json");
    let unknown = temp.join("unknown.json");
    std::fs::write(&missing, r#"[{"beta":0.7}]"#).unwrap();
    std::fs::write(&unknown, r#"[{"beta":0.7,"gamma":0.12,"mystery":1.0}]"#).unwrap();

    let missing_output = custom_sweep(
        Path::new("1"),
        &temp.join("missing-out"),
        1,
        None,
        1,
        None,
        Some(&missing),
    );
    assert_eq!(missing_output.status.code(), Some(1));
    let missing_stderr = String::from_utf8_lossy(&missing_output.stderr);
    assert_eq!(
        missing_stderr.trim_end(),
        format!(
            "{}: theta assignment 0 is missing prior-bearing parameter 'gamma'",
            missing.display()
        )
    );

    let unknown_output = custom_sweep(
        Path::new("1"),
        &temp.join("unknown-out"),
        1,
        None,
        1,
        None,
        Some(&unknown),
    );
    assert_eq!(unknown_output.status.code(), Some(1));
    let unknown_stderr = String::from_utf8_lossy(&unknown_output.stderr);
    assert_eq!(
        unknown_stderr.trim_end(),
        format!(
            "{}: theta assignment 0 has unknown parameter 'mystery'",
            unknown.display()
        )
    );

    let conflict = custom_sweep(
        Path::new("1"),
        &temp.join("conflict-out"),
        1,
        Some(2),
        1,
        None,
        Some(&missing),
    );
    assert_eq!(conflict.status.code(), Some(2));
    let conflict_stderr = String::from_utf8_lossy(&conflict.stderr);
    assert!(
        conflict_stderr.starts_with("'--theta-file' cannot be combined with '--draws'\nusage:"),
        "{conflict_stderr}"
    );

    std::fs::remove_dir_all(temp).unwrap();
}

#[test]
fn pairs_export_is_deterministic_canonical_and_matches_theta_and_summaries() {
    let temp = temp_dir("pairs-export");
    let population = temp.join("population.bin");
    synth(&population, 2_000, 40, 20);
    let model = repository_path("crates/sembla-cli/tests/fixtures/pairs_model.json");
    let first_out = temp.join("first-sweep");
    let repeat_out = temp.join("repeat-sweep");
    let first_pairs = temp.join("first-pairs.csv");
    let repeat_pairs = temp.join("repeat-pairs.csv");

    let first = pairs_sweep(
        &model,
        &population,
        &first_out,
        &first_pairs,
        PairsSweepOptions {
            seed: 91,
            draws: Some(3),
            noise: "independent",
            theta_file: None,
        },
    );
    assert_success(&first);
    assert!(
        first.stderr.is_empty(),
        "{}",
        String::from_utf8_lossy(&first.stderr)
    );
    let repeat = pairs_sweep(
        &model,
        &population,
        &repeat_out,
        &repeat_pairs,
        PairsSweepOptions {
            seed: 91,
            draws: Some(3),
            noise: "independent",
            theta_file: None,
        },
    );
    assert_success(&repeat);

    assert_eq!(
        std::fs::read(&first_pairs).unwrap(),
        std::fs::read(&repeat_pairs).unwrap()
    );
    assert_eq!(
        std::fs::read(pairs_meta_path(&first_pairs)).unwrap(),
        std::fs::read(pairs_meta_path(&repeat_pairs)).unwrap()
    );
    assert_pairs_match_sweep_outputs(&first_pairs, &first_out);

    let pairs_bytes = std::fs::read(&first_pairs).unwrap();
    let metadata_bytes = std::fs::read(pairs_meta_path(&first_pairs)).unwrap();
    assert_eq!(metadata_bytes.last(), Some(&b'\n'));
    let metadata: serde_json::Value = serde_json::from_slice(&metadata_bytes).unwrap();
    assert_eq!(metadata["schema_versions"], serde_json::json!({"pairs": 1}));
    assert_eq!(metadata["model"], "pairs_fixture");
    assert_eq!(metadata["seed"], 91);
    assert_eq!(metadata["noise_mode"], "independent");
    assert_eq!(metadata["draws"], 3);
    assert_eq!(metadata["ticks"], 8);
    assert_eq!(metadata["dt"], 0.25);
    assert_eq!(metadata["determinism_level"], "A");
    assert_eq!(metadata["ir_hash_algorithm"], "sha256");
    assert_eq!(metadata["pairs_hash_algorithm"], "sha256");
    assert_eq!(metadata["theta_source"]["kind"], "prior");
    assert_eq!(metadata["theta_source"]["algorithm"], "sha256");
    assert_eq!(
        metadata["parameter_columns"],
        serde_json::json!(["beta", "gamma"])
    );
    assert_eq!(
        metadata["summary_columns"],
        serde_json::json!(["peak_tick", "peak_I"])
    );
    assert_eq!(
        metadata["pairs_sha256"],
        format!("{:x}", Sha256::digest(pairs_bytes))
    );
    assert_eq!(
        metadata["component_versions"]["sembla-cli"],
        env!("CARGO_PKG_VERSION")
    );
    assert!(metadata["component_versions"]["sembla-ir"].is_string());
    assert!(metadata["component_versions"]["sembla-runtime"].is_string());

    // Canonical PRD-0001 JSON recursively sorts object keys and ends in one newline.
    let reparsed = serde_json::to_string(&metadata).unwrap() + "\n";
    assert_eq!(metadata_bytes, reparsed.as_bytes());

    std::fs::remove_dir_all(temp).unwrap();
}

#[test]
fn pairs_export_accepts_theta_file_warns_for_crn_and_rejects_missing_summaries() {
    let temp = temp_dir("pairs-export-modes");
    let population = temp.join("population.bin");
    synth(&population, 2_000, 40, 20);
    let model = repository_path("crates/sembla-cli/tests/fixtures/pairs_model.json");
    let theta = temp.join("theta.json");
    let theta_bytes = b"[{\"gamma\":0.12,\"beta\":0.7},{\"gamma\":0.1,\"beta\":0.8},{\"gamma\":0.09,\"beta\":0.9}]\n";
    std::fs::write(&theta, theta_bytes).unwrap();
    let out = temp.join("theta-sweep");
    let pairs = temp.join("theta-pairs.csv");

    let crn = pairs_sweep(
        &model,
        &population,
        &out,
        &pairs,
        PairsSweepOptions {
            seed: 41,
            draws: None,
            noise: "crn",
            theta_file: Some(&theta),
        },
    );
    assert_success(&crn);
    assert_eq!(
        String::from_utf8(crn.stderr).unwrap().trim_end(),
        "warning: --export-pairs with --noise crn is unsuitable for NPE training (DECISIONS.md §G5); use --noise independent"
    );
    assert_pairs_match_sweep_outputs(&pairs, &out);
    let metadata: serde_json::Value =
        serde_json::from_slice(&std::fs::read(pairs_meta_path(&pairs)).unwrap()).unwrap();
    assert_eq!(metadata["noise_mode"], "crn");
    assert_eq!(metadata["theta_source"]["kind"], "file");
    assert_eq!(
        metadata["theta_source"]["sha256"],
        format!("{:x}", Sha256::digest(theta_bytes))
    );

    let no_summaries = temp.join("no-summaries.json");
    let mut no_summary_model: serde_json::Value =
        serde_json::from_slice(&std::fs::read(&model).unwrap()).unwrap();
    no_summary_model["name"] = serde_json::Value::String("no_summaries_fixture".to_owned());
    no_summary_model["summaries"] = serde_json::json!([]);
    std::fs::write(
        &no_summaries,
        serde_json::to_vec(&no_summary_model).unwrap(),
    )
    .unwrap();
    let rejected_pairs = temp.join("rejected.csv");
    let rejected = pairs_sweep(
        &no_summaries,
        Path::new("1"),
        &temp.join("rejected-sweep"),
        &rejected_pairs,
        PairsSweepOptions {
            seed: 1,
            draws: Some(1),
            noise: "independent",
            theta_file: None,
        },
    );
    assert_eq!(rejected.status.code(), Some(1));
    assert_eq!(
        String::from_utf8(rejected.stderr).unwrap().trim_end(),
        "model 'no_summaries_fixture' declares no summaries; --export-pairs requires declared summaries (DESIGN.md §4.6)"
    );
    assert!(!rejected_pairs.exists());
    assert!(!pairs_meta_path(&rejected_pairs).exists());

    std::fs::remove_dir_all(temp).unwrap();
}

#[test]
fn sweep_rejects_zero_draws() {
    let output = command(&[
        "sweep",
        repository_path("examples/sir.json").to_str().unwrap(),
        "--population",
        "1",
        "--seed",
        "1",
        "--draws",
        "0",
        "--ticks",
        "1",
        "--out",
        "/tmp/sembla-unused-zero-draw-sweep",
    ]);
    assert_eq!(output.status.code(), Some(2));
    assert!(String::from_utf8_lossy(&output.stderr).contains("must be greater than zero"));
}
