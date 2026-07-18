use std::collections::BTreeMap;
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
        "sembla-observation-{label}-{}-{nonce}",
        std::process::id()
    ));
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

fn run(model: &str, population: &Path, out: &Path) -> Output {
    Command::new(env!("CARGO_BIN_EXE_sembla"))
        .arg("run")
        .arg(repository_path(model))
        .arg("--population")
        .arg(population)
        .args(["--seed", "9", "--ticks", "3", "--out"])
        .arg(out)
        .output()
        .unwrap()
}

fn summaries_path(output: &Path) -> PathBuf {
    PathBuf::from(format!("{}.summaries.csv", output.display()))
}

#[test]
fn migrated_sir_and_sweep_preserve_legacy_while_policy_reports_every_rule() {
    let temp = temp_dir("legacy-goldens");
    let population = temp.join("pop.bin");
    synth_population(&population);

    for (model, fixture) in [
        (
            "examples/sir.json",
            include_bytes!("fixtures/sir_legacy.csv").as_slice(),
        ),
        (
            "examples/sir_policy.json",
            include_bytes!("fixtures/sir_policy_all_rules.csv").as_slice(),
        ),
    ] {
        let out = temp.join(format!(
            "{}.csv",
            Path::new(model).file_stem().unwrap().to_string_lossy()
        ));
        let output = run(model, &population, &out);
        assert_success(&output);
        assert_eq!(std::fs::read(&out).unwrap(), fixture, "{model}");
    }

    let sweep = temp.join("sweep");
    let output = Command::new(env!("CARGO_BIN_EXE_sembla"))
        .arg("sweep")
        .arg(repository_path("examples/sir.json"))
        .arg("--population")
        .arg(&population)
        .args(["--seed", "9", "--draws", "2", "--ticks", "3", "--out"])
        .arg(&sweep)
        .output()
        .unwrap();
    assert_success(&output);
    for (name, expected) in [
        (
            "draw_0.csv",
            include_bytes!("fixtures/sweep_draw_0_legacy.csv").as_slice(),
        ),
        (
            "draw_1.csv",
            include_bytes!("fixtures/sweep_draw_1_legacy.csv").as_slice(),
        ),
        (
            "manifest.csv",
            include_bytes!("fixtures/sweep_manifest_legacy.csv").as_slice(),
        ),
        (
            "summary.csv",
            include_bytes!("fixtures/sweep_summary_legacy.csv").as_slice(),
        ),
    ] {
        assert_eq!(std::fs::read(sweep.join(name)).unwrap(), expected, "{name}");
    }
    std::fs::remove_dir_all(temp).unwrap();
}

#[test]
fn summaries_match_hand_computed_peak_and_earliest_tick() {
    let temp = temp_dir("summaries");
    let population = temp.join("pop.bin");
    synth_population(&population);
    let out = temp.join("sir.csv");
    let output = run("examples/sir.json", &population, &out);
    assert_success(&output);
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("observation_sha256="), "{stdout}");

    let csv = std::fs::read_to_string(&out).unwrap();
    let infected = csv
        .lines()
        .filter(|line| !line.starts_with('#'))
        .skip(1)
        .map(|line| line.split(',').nth(2).unwrap().parse::<i64>().unwrap())
        .collect::<Vec<_>>();
    let peak = *infected.iter().max().unwrap();
    let peak_tick = infected.iter().position(|value| *value == peak).unwrap() as i64;

    let summaries = std::fs::read_to_string(summaries_path(&out)).unwrap();
    let values = summaries
        .lines()
        .skip(1)
        .map(|line| {
            let mut fields = line.split(',');
            (
                fields.next().unwrap().to_owned(),
                fields.next().unwrap().parse::<i64>().unwrap(),
            )
        })
        .collect::<BTreeMap<_, _>>();
    assert_eq!(values["peak_I"], peak);
    assert_eq!(values["peak_tick"], peak_tick);
    std::fs::remove_dir_all(temp).unwrap();
}

#[test]
fn views_free_sweep_is_deterministic() {
    let temp = temp_dir("generic-sweep");
    let first = temp.join("first");
    let second = temp.join("second");
    for out in [&first, &second] {
        let output = Command::new(env!("CARGO_BIN_EXE_sembla"))
            .arg("sweep")
            .arg(repository_path("examples/reversible_ctmc.json"))
            .args([
                "--population",
                "100",
                "--seed",
                "7",
                "--draws",
                "2",
                "--ticks",
                "3",
                "--out",
            ])
            .arg(out)
            .output()
            .unwrap();
        assert_success(&output);
    }
    for name in [
        "draw_0.csv",
        "draw_1.csv",
        "manifest.csv",
        "summary.csv",
        "run-manifest.json",
    ] {
        assert_eq!(
            std::fs::read(first.join(name)).unwrap(),
            std::fs::read(second.join(name)).unwrap(),
            "{name}"
        );
    }
    let header = std::fs::read_to_string(first.join("summary.csv")).unwrap();
    assert!(header.starts_with("tick,count:chain.particle.phase=A_p05"));
    std::fs::remove_dir_all(temp).unwrap();
}

#[test]
fn cli_source_has_no_model_named_observation_branch() {
    let source = include_str!("../src/main.rs");
    let production = source.split("#[cfg(test)]").next().unwrap();
    for deleted in [
        concat!("optional_sir_box_", "name"),
        concat!("sir_box_", "name"),
        concat!("run_sir_results_", "csv"),
        concat!("sir_", "counts"),
    ] {
        assert!(
            !source.contains(deleted),
            "deleted symbol survives: {deleted}"
        );
    }
    for model_name in ["\"sir\"", "\"population\""] {
        assert!(
            !production.contains(model_name),
            "model box literal survives in CLI production source: {model_name}"
        );
    }
}
