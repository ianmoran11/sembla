use std::path::{Path, PathBuf};
use std::process::Command;

fn repository_path(relative: impl AsRef<Path>) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join(relative)
}

#[test]
fn validate_subcommand_accepts_the_golden_fixture() {
    let output = Command::new(env!("CARGO_BIN_EXE_sembla"))
        .arg("validate")
        .arg(repository_path("examples/two_state.json"))
        .output()
        .unwrap();

    assert!(output.status.success());
    assert!(output.stderr.is_empty());
}

#[test]
fn validate_subcommand_rejects_every_invalid_fixture() {
    let fixtures = [
        ("unresolved_param.json", "hazard.name"),
        ("duplicate_param.json", "params[1].name"),
        ("bad_prior_arity.json", "prior.args"),
        ("wrong_guard_type.json", "transitions[0].guard"),
        ("unknown_enum_variant.json", "guard.variant"),
        ("unknown_effect_attr.json", "effects[0].attr"),
    ];

    for (fixture, offending_path) in fixtures {
        let output = Command::new(env!("CARGO_BIN_EXE_sembla"))
            .arg("validate")
            .arg(repository_path(format!("examples/invalid/{fixture}")))
            .output()
            .unwrap();
        let stderr = String::from_utf8(output.stderr).unwrap();

        assert_eq!(output.status.code(), Some(1), "{fixture}: {stderr}");
        assert!(stderr.contains(offending_path), "{fixture}: {stderr}");
    }
}

#[test]
fn diff_ir_compares_validated_canonical_models() {
    let fixture = repository_path("examples/sir.json");
    let normalized_copy = std::env::temp_dir().join(format!(
        "sembla-diff-ir-{}-{}.json",
        std::process::id(),
        std::thread::current().name().unwrap_or("test")
    ));
    let source = std::fs::read_to_string(&fixture).unwrap();
    std::fs::write(&normalized_copy, format!("\n  {source}\n")).unwrap();

    let identical = Command::new(env!("CARGO_BIN_EXE_sembla"))
        .args(["diff-ir"])
        .arg(&fixture)
        .arg(&normalized_copy)
        .output()
        .unwrap();
    assert!(identical.status.success());
    assert!(String::from_utf8(identical.stdout)
        .unwrap()
        .contains("semantically identical"));

    let different = Command::new(env!("CARGO_BIN_EXE_sembla"))
        .args(["diff-ir"])
        .arg(&fixture)
        .arg(repository_path("examples/sir_policy.json"))
        .output()
        .unwrap();
    assert_eq!(different.status.code(), Some(1));
    assert!(String::from_utf8(different.stderr)
        .unwrap()
        .contains("canonical normalization"));

    let invalid = Command::new(env!("CARGO_BIN_EXE_sembla"))
        .args(["diff-ir"])
        .arg(repository_path("examples/invalid/wrong_guard_type.json"))
        .arg(&fixture)
        .output()
        .unwrap();
    assert_eq!(invalid.status.code(), Some(1));
    assert!(String::from_utf8(invalid.stderr)
        .unwrap()
        .contains("transitions[0].guard"));

    std::fs::remove_file(normalized_copy).unwrap();
}
