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

fn file(path: &Path, name: &str) -> Vec<u8> {
    std::fs::read(path.join(name)).unwrap()
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
fn sweep_rejects_generic_models_before_writing_outputs() {
    let temp = temp_dir("generic-sweep-rejection");
    let out = temp.join("sweep");
    let output = command(&[
        "sweep",
        repository_path("examples/reversible_ctmc.json")
            .to_str()
            .unwrap(),
        "--population",
        "100",
        "--seed",
        "1",
        "--draws",
        "2",
        "--ticks",
        "2",
        "--out",
        out.to_str().unwrap(),
    ]);
    assert_eq!(output.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&output.stderr).contains(
        "sweep summary currently requires exactly one SIR person/employer box; use sembla run for generic models"
    ));
    assert!(!out.exists());
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
