use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};

static TEMP_COUNTER: AtomicU64 = AtomicU64::new(0);

fn repository_path(relative: impl AsRef<Path>) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join(relative)
}

fn temp_dir(label: &str) -> PathBuf {
    let sequence = TEMP_COUNTER.fetch_add(1, Ordering::Relaxed);
    let path = std::env::temp_dir().join(format!(
        "sembla-arithmetic-int-{label}-{}-{sequence}",
        std::process::id()
    ));
    if path.exists() {
        std::fs::remove_dir_all(&path).unwrap();
    }
    std::fs::create_dir_all(&path).unwrap();
    path
}

#[test]
fn lean_exported_increment_model_runs_and_records_int_theta_override() {
    let temp = temp_dir("round-trip");
    let params = temp.join("params.json");
    std::fs::write(&params, "{\"retirement_months\":6}\n").unwrap();
    let results = temp.join("results.csv");

    // This fixture is emitted from `ArithmeticIntTests.incrementModel` through
    // `Sembla.IR.toJson`, the same canonical model renderer used by the exporter.
    let output = Command::new(env!("CARGO_BIN_EXE_sembla"))
        .arg("run")
        .arg(repository_path(
            "crates/sembla-cli/tests/fixtures/arithmetic_int_increment.json",
        ))
        .args(["--population", "1", "--seed", "9", "--ticks", "5"])
        .arg("--params")
        .arg(&params)
        .arg("--out")
        .arg(&results)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );

    let csv = std::fs::read_to_string(&results).unwrap();
    let trace = csv
        .lines()
        .filter(|line| !line.starts_with('#'))
        .skip(1)
        .map(|line| line.split(',').nth(1).unwrap().parse::<i64>().unwrap())
        .collect::<Vec<_>>();
    assert_eq!(trace, [1, 2, 3, 4, 5]);

    let manifest_path = PathBuf::from(format!("{}.manifest.json", results.display()));
    let manifest: serde_json::Value =
        serde_json::from_slice(&std::fs::read(manifest_path).unwrap()).unwrap();
    assert_eq!(manifest["resolved_theta"]["retirement_months"], 6);
    assert!(manifest["resolved_theta"]["retirement_months"].is_i64());

    std::fs::remove_dir_all(temp).unwrap();
}

#[test]
#[ignore = "fixture regeneration requires the pinned Lean toolchain; run explicitly"]
fn increment_fixture_matches_the_canonical_lean_export() {
    let temp = temp_dir("regenerate");
    let exporter = temp.join("ExportArithmeticInt.lean");
    let generated = temp.join("arithmetic_int_increment.json");
    std::fs::write(
        &exporter,
        r#"import Sembla.ArithmeticIntTests

open Sembla.ArithmeticIntTests

def main (args : List String) : IO UInt32 := do
  match args with
  | [path] => IO.FS.writeFile path incrementModelJson; pure 0
  | _ => pure 2
"#,
    )
    .unwrap();
    let frontend = repository_path("frontend");
    let build = Command::new("lake")
        .args(["build", "Sembla.ArithmeticIntTests"])
        .current_dir(&frontend)
        .output()
        .unwrap();
    assert!(
        build.status.success(),
        "{}",
        String::from_utf8_lossy(&build.stderr)
    );
    let export = Command::new("lake")
        .args(["env", "lean", "--run"])
        .arg(&exporter)
        .arg(&generated)
        .current_dir(frontend)
        .output()
        .unwrap();
    assert!(
        export.status.success(),
        "{}",
        String::from_utf8_lossy(&export.stderr)
    );
    assert_eq!(
        std::fs::read(generated).unwrap(),
        std::fs::read(repository_path(
            "crates/sembla-cli/tests/fixtures/arithmetic_int_increment.json",
        ))
        .unwrap()
    );
    std::fs::remove_dir_all(temp).unwrap();
}
