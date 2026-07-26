use std::collections::BTreeSet;
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

fn run_fixed(out: &Path, export: &Path, timing: Option<&Path>) -> Output {
    let mut command = Command::new(env!("CARGO_BIN_EXE_sembla"));
    command
        .arg("run")
        .arg(repository_path("examples/two_state.json"))
        .args(["--seed", "42", "--ticks", "3", "--population", "1000"])
        .arg("--out")
        .arg(out)
        .arg("--export-state")
        .arg(export);
    if let Some(timing) = timing {
        command.arg("--timing-json").arg(timing);
    }
    command.output().unwrap()
}

fn assert_success(output: &Output) {
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn timing_json_is_inert_and_reports_reconciled_cpu_phases() {
    let temp = temp_dir("execution-timing");
    let off_csv = temp.join("off.csv");
    let on_csv = temp.join("on.csv");
    let off_state = temp.join("off.state");
    let on_state = temp.join("on.state");
    let off_timing = temp.join("off-timing.json");
    let on_timing = temp.join("on-timing.json");

    let off = run_fixed(&off_csv, &off_state, None);
    assert_success(&off);
    let off_files = std::fs::read_dir(&temp)
        .unwrap()
        .map(|entry| entry.unwrap().file_name().into_string().unwrap())
        .collect::<BTreeSet<_>>();
    assert_eq!(
        off_files,
        BTreeSet::from([
            "off.csv".to_owned(),
            "off.csv.manifest.json".to_owned(),
            "off.csv.summaries.csv".to_owned(),
            "off.state".to_owned(),
        ]),
        "disabled timing must write no extra file"
    );
    assert!(!off_timing.exists(), "disabled timing must write no file");

    let on = run_fixed(&on_csv, &on_state, Some(&on_timing));
    assert_success(&on);
    assert_eq!(off.stdout, on.stdout, "timing must not change stdout");
    assert_eq!(off.stderr, on.stderr, "timing must not change stderr");
    assert!(on_timing.is_file(), "enabled timing must write one file");

    for (off_path, on_path, label) in [
        (&off_csv, &on_csv, "results CSV"),
        (
            &PathBuf::from(format!("{}.summaries.csv", off_csv.display())),
            &PathBuf::from(format!("{}.summaries.csv", on_csv.display())),
            "summaries CSV",
        ),
        (
            &PathBuf::from(format!("{}.manifest.json", off_csv.display())),
            &PathBuf::from(format!("{}.manifest.json", on_csv.display())),
            "manifest",
        ),
        (&off_state, &on_state, "exported state"),
    ] {
        assert_eq!(
            std::fs::read(off_path).unwrap(),
            std::fs::read(on_path).unwrap(),
            "timing changed {label} bytes"
        );
    }

    let off_manifest: serde_json::Value = serde_json::from_slice(
        &std::fs::read(format!("{}.manifest.json", off_csv.display())).unwrap(),
    )
    .unwrap();
    let on_manifest: serde_json::Value = serde_json::from_slice(
        &std::fs::read(format!("{}.manifest.json", on_csv.display())).unwrap(),
    )
    .unwrap();
    assert_eq!(
        off_manifest["final_state_sha256"], on_manifest["final_state_sha256"],
        "timing changed the final state digest"
    );

    let timing: serde_json::Value =
        serde_json::from_slice(&std::fs::read(&on_timing).unwrap()).unwrap();
    assert_eq!(timing["schema"], "sembla-execution-timing-v1");
    assert_eq!(timing["session"]["backend"], "cpu");
    assert_eq!(timing["session"]["scale"], 1000);
    assert_eq!(timing["session"]["ticks"], 3);
    assert_eq!(timing["session"]["seed"], 42);
    assert_eq!(
        timing["session"]["repository_commit"]
            .as_str()
            .unwrap()
            .len(),
        40
    );
    assert_eq!(
        timing["session"]["binary_sha256"].as_str().unwrap().len(),
        64
    );
    assert_eq!(timing["kernel_sync_inserted"], false);
    assert_eq!(timing["timer"]["clock"], "std::time::Instant");
    assert_eq!(timing["timer"]["resolution"], "nanoseconds");
    assert_eq!(timing["timer"]["reported_unit"], "milliseconds");

    let expected_phases = BTreeSet::from([
        "execute_tick",
        "state_hash",
        "observe_views",
        "report",
        "other",
    ]);
    let rows = timing["ticks"].as_array().unwrap();
    assert_eq!(rows.len(), 3);
    for (tick, row) in rows.iter().enumerate() {
        assert_eq!(row["tick"], tick as u64);
        let phases = row["phases_ms"].as_object().unwrap();
        assert_eq!(
            phases.keys().map(String::as_str).collect::<BTreeSet<_>>(),
            expected_phases
        );
        for value in phases.values() {
            let value = value.as_f64().unwrap();
            assert!(value.is_finite() && value >= 0.0);
        }
        let wall = row["wall_time_ms"].as_f64().unwrap();
        let sum = row["phase_sum_ms"].as_f64().unwrap();
        let tolerance = timing["self_check"]["tolerance_ms"].as_f64().unwrap();
        assert!((wall - sum).abs() <= tolerance);
        assert_eq!(row["within_tolerance"], true);
    }
    assert_eq!(timing["self_check"]["all_ticks_reconciled"], true);
    assert_eq!(timing["self_check"]["other_non_negative"], true);
    let totals = timing["totals"].as_object().unwrap();
    assert!(totals["wall_time_ms"].as_f64().unwrap() >= 0.0);
    assert_eq!(
        totals["phases_ms"]
            .as_object()
            .unwrap()
            .keys()
            .map(String::as_str)
            .collect::<BTreeSet<_>>(),
        expected_phases
    );

    std::fs::remove_dir_all(temp).unwrap();
}

fn run_with_timing_paths(out: &Path, timing: &Path) -> Output {
    Command::new(env!("CARGO_BIN_EXE_sembla"))
        .arg("run")
        .arg(repository_path("examples/two_state.json"))
        .args(["--seed", "42", "--ticks", "1", "--population", "64"])
        .arg("--out")
        .arg(out)
        .arg("--timing-json")
        .arg(timing)
        .output()
        .unwrap()
}

fn assert_timing_collision(output: &Output) {
    assert_eq!(output.status.code(), Some(1));
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("--timing-json path"),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(String::from_utf8_lossy(&output.stderr).contains("conflicts with run output path"));
}

#[test]
fn timing_json_rejects_direct_symlink_and_hard_link_output_aliases() {
    let temp = temp_dir("execution-timing-collisions");

    let direct = temp.join("direct.csv");
    assert_timing_collision(&run_with_timing_paths(&direct, &direct));
    assert!(!direct.exists());

    let derived_output = temp.join("derived.csv");
    for timing in [
        PathBuf::from(format!("{}.summaries.csv", derived_output.display())),
        PathBuf::from(format!("{}.manifest.json", derived_output.display())),
    ] {
        assert_timing_collision(&run_with_timing_paths(&derived_output, &timing));
        assert!(!derived_output.exists());
        assert!(!timing.exists());
    }

    #[cfg(unix)]
    {
        use std::os::unix::fs::symlink;

        let output = temp.join("symlink-output.csv");
        let alias = temp.join("symlink-timing.json");
        symlink(&output, &alias).unwrap();
        assert_timing_collision(&run_with_timing_paths(&output, &alias));
        assert!(!output.exists());
    }

    let output = temp.join("hard-link-output.csv");
    let alias = temp.join("hard-link-timing.json");
    std::fs::write(&output, b"sentinel").unwrap();
    std::fs::hard_link(&output, &alias).unwrap();
    assert_timing_collision(&run_with_timing_paths(&output, &alias));
    assert_eq!(std::fs::read(&output).unwrap(), b"sentinel");
    assert_eq!(std::fs::read(&alias).unwrap(), b"sentinel");

    std::fs::remove_dir_all(temp).unwrap();
}

#[test]
fn timing_enabled_without_output_preserves_plain_stdout() {
    let temp = temp_dir("execution-timing-stdout");
    let timing = temp.join("timing.json");
    let base_args = [
        "run",
        "examples/two_state.json",
        "--seed",
        "42",
        "--ticks",
        "3",
        "--population",
        "64",
    ];
    let off = Command::new(env!("CARGO_BIN_EXE_sembla"))
        .current_dir(repository_path("."))
        .args(base_args)
        .output()
        .unwrap();
    let on = Command::new(env!("CARGO_BIN_EXE_sembla"))
        .current_dir(repository_path("."))
        .args(base_args)
        .arg("--timing-json")
        .arg(&timing)
        .output()
        .unwrap();
    assert_success(&off);
    assert_success(&on);
    assert_eq!(off.stdout, on.stdout);
    assert_eq!(off.stderr, on.stderr);
    assert!(timing.is_file());
    std::fs::remove_dir_all(temp).unwrap();
}
