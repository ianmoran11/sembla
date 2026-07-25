use std::path::{Path, PathBuf};
use std::process::Command;

use sembla_ir::AttrType;

const DEMOGRAPHIC_MODEL: &str = "fixtures/demographic/benchmark/demographic_slots.no-grouped.json";
const DEMOGRAPHIC_LISTING: &str = "corpus_model=fixtures/demographic/benchmark/demographic_slots.no-grouped.json configuration=no-grouped population=1000 seed=7 ticks=20";

fn repository_path(relative: impl AsRef<Path>) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join(relative)
}

#[test]
fn demographic_corpus_member_is_no_grouped_and_exercises_the_coverage_gap() {
    let source = std::fs::read_to_string(repository_path(DEMOGRAPHIC_MODEL)).unwrap();
    let model = sembla_ir::parse_json(&source).unwrap();

    assert_eq!(model.name, "demographic_slots");
    assert!(model
        .boxes
        .iter()
        .all(|model_box| model_box.grouped_views.is_empty()));
    assert!(model.boxes.iter().any(|model_box| {
        model_box
            .transitions
            .iter()
            .any(|transition| !transition.contests.is_empty())
    }));
    assert!(model.boxes.iter().any(|model_box| {
        model_box.tables.iter().any(|table| {
            table
                .attrs
                .iter()
                .any(|attr| matches!(&attr.ty, AttrType::Ref { .. }))
        })
    }));
}

#[test]
fn differential_runner_lists_the_demographic_corpus_contract() {
    let output = Command::new("bash")
        .arg(repository_path(
            "crates/sembla-cuda/scripts/run-differential-corpus.sh",
        ))
        .arg("--list")
        .current_dir(repository_path("."))
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "stdout={}\nstderr={}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.lines().any(|line| line == DEMOGRAPHIC_LISTING));
}
