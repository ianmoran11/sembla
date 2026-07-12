use std::path::{Path, PathBuf};
use std::process::Command;

fn repository_path(relative: impl AsRef<Path>) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join(relative)
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
