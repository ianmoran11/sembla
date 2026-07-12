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
