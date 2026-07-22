use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::time::{SystemTime, UNIX_EPOCH};

const ARTIFACTS: [&str; 4] = [
    "bundle-manifest.json",
    "composition-source.json",
    "executable-plan.json",
    "link-report.json",
];

fn repository_path(path: impl AsRef<Path>) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join(path)
}

fn fixture() -> PathBuf {
    repository_path("fixtures/bundles/epidemic_policy")
}

fn temp_dir(label: &str) -> PathBuf {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let path = std::env::temp_dir().join(format!(
        "sembla-bundle-{label}-{}-{nonce}",
        std::process::id()
    ));
    std::fs::create_dir_all(&path).unwrap();
    path
}

fn copy_fixture(label: &str) -> PathBuf {
    let target = temp_dir(label);
    for artifact in ARTIFACTS {
        std::fs::copy(fixture().join(artifact), target.join(artifact)).unwrap();
    }
    target
}

fn verify(directory: &Path) -> Output {
    Command::new(env!("CARGO_BIN_EXE_sembla"))
        .arg("bundle-verify")
        .arg(directory)
        .output()
        .unwrap()
}

fn assert_success(output: &Output) {
    assert!(
        output.status.success(),
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

fn assert_named_failure(output: &Output, record: &str) {
    assert!(!output.status.success(), "corruption unexpectedly verified");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains(record),
        "failure did not name '{record}': {stderr}"
    );
}

#[test]
fn checked_and_moved_bundles_verify_in_deterministic_order() {
    let expected_stdout = concat!(
        "ok bundle-manifest schema, encoding, domains, and canonicality\n",
        "ok composition-source.json source.hash\n",
        "ok executable-plan.json plan.envelope_hash\n",
        "ok executable-plan.json plan.semantic_hash\n",
        "ok composition-source.json, executable-plan.json, and link-report.json bundle_integrity\n",
        "ok executable-plan.json validation and canonicality\n",
        "ok manifest/plan origin, schemas, identity, features, and source provenance agreement\n",
    );

    let output = verify(&fixture());
    assert_success(&output);
    assert_eq!(String::from_utf8(output.stdout).unwrap(), expected_stdout);

    let moved = copy_fixture("moved");
    let moved_output = verify(&moved);
    assert_success(&moved_output);
    assert_eq!(
        String::from_utf8(moved_output.stdout).unwrap(),
        expected_stdout
    );
    std::fs::remove_dir_all(moved).unwrap();
}

#[test]
fn every_bundle_corruption_names_the_failing_record() {
    let source = copy_fixture("corrupt-source");
    let mut bytes = std::fs::read(source.join("composition-source.json")).unwrap();
    bytes.push(b' ');
    std::fs::write(source.join("composition-source.json"), bytes).unwrap();
    assert_named_failure(&verify(&source), "source.hash");
    std::fs::remove_dir_all(source).unwrap();

    let plan = copy_fixture("corrupt-plan");
    let mut bytes = std::fs::read(plan.join("executable-plan.json")).unwrap();
    bytes.push(b' ');
    std::fs::write(plan.join("executable-plan.json"), bytes).unwrap();
    assert_named_failure(&verify(&plan), "plan.envelope_hash");
    std::fs::remove_dir_all(plan).unwrap();

    let report = copy_fixture("corrupt-report");
    let mut bytes = std::fs::read(report.join("link-report.json")).unwrap();
    bytes.push(b' ');
    std::fs::write(report.join("link-report.json"), bytes).unwrap();
    assert_named_failure(&verify(&report), "bundle_integrity");
    std::fs::remove_dir_all(report).unwrap();

    let digest = copy_fixture("corrupt-manifest-digest");
    let manifest_path = digest.join("bundle-manifest.json");
    let mut manifest: serde_json::Value =
        serde_json::from_slice(&std::fs::read(&manifest_path).unwrap()).unwrap();
    let recorded = manifest["source"]["hash"]["digest"].as_str().unwrap();
    let replacement = format!(
        "{}{}",
        if &recorded[..1] == "0" { "1" } else { "0" },
        &recorded[1..]
    );
    manifest["source"]["hash"]["digest"] = serde_json::Value::String(replacement);
    std::fs::write(&manifest_path, serde_json::to_vec(&manifest).unwrap()).unwrap();
    assert_named_failure(&verify(&digest), "source.hash");
    std::fs::remove_dir_all(digest).unwrap();

    let missing = copy_fixture("missing-integrity");
    let manifest_path = missing.join("bundle-manifest.json");
    let mut manifest: serde_json::Value =
        serde_json::from_slice(&std::fs::read(&manifest_path).unwrap()).unwrap();
    manifest.as_object_mut().unwrap().remove("bundle_integrity");
    std::fs::write(&manifest_path, serde_json::to_vec(&manifest).unwrap()).unwrap();
    assert_named_failure(&verify(&missing), "bundle_integrity");
    std::fs::remove_dir_all(missing).unwrap();
}

#[test]
fn plan_copied_out_of_bundle_runs_without_source_or_manifest() {
    let temp = temp_dir("standalone-plan");
    let copied_plan = temp.join("detached.plan.json");
    std::fs::copy(fixture().join("executable-plan.json"), &copied_plan).unwrap();
    let output_path = temp.join("results.csv");
    let output = Command::new(env!("CARGO_BIN_EXE_sembla"))
        .arg("run")
        .arg(&copied_plan)
        .args([
            "--population",
            "1000",
            "--seed",
            "55",
            "--ticks",
            "1",
            "--out",
        ])
        .arg(&output_path)
        .output()
        .unwrap();
    assert_success(&output);
    assert!(output_path.is_file());
    assert!(PathBuf::from(format!("{}.manifest.json", output_path.display())).is_file());
    std::fs::remove_dir_all(temp).unwrap();
}
