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
        "sembla-plan-run-{label}-{}-{nonce}",
        std::process::id()
    ));
    std::fs::create_dir_all(&path).unwrap();
    path
}

fn sidecar(output: &Path) -> PathBuf {
    PathBuf::from(format!("{}.manifest.json", output.display()))
}

fn summaries(output: &Path) -> PathBuf {
    PathBuf::from(format!("{}.summaries.csv", output.display()))
}

fn run(model: &Path, output: &Path) -> Output {
    Command::new(env!("CARGO_BIN_EXE_sembla"))
        .arg("run")
        .arg(model)
        .args([
            "--population",
            "16",
            "--seed",
            "55",
            "--ticks",
            "40",
            "--out",
        ])
        .arg(output)
        .output()
        .unwrap()
}

fn assert_success(output: &Output) {
    assert!(
        output.status.success(),
        "stdout={}\nstderr={}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

fn normalized_manifest(path: &Path) -> Vec<u8> {
    let mut value: serde_json::Value =
        serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap();
    value.as_object_mut().unwrap().remove("component_versions");
    let mut bytes = serde_json::to_vec(&value).unwrap();
    bytes.push(b'\n');
    bytes
}

fn csv_table(bytes: &[u8]) -> (Vec<String>, Vec<Vec<String>>) {
    let source = std::str::from_utf8(bytes).unwrap();
    let mut lines = source.lines().filter(|line| !line.starts_with('#'));
    let headers = lines
        .next()
        .unwrap()
        .split(',')
        .map(str::to_owned)
        .collect::<Vec<_>>();
    let rows = lines
        .map(|line| line.split(',').map(str::to_owned).collect())
        .collect();
    (headers, rows)
}

#[test]
fn plan_run_matches_frozen_golden_and_is_byte_deterministic() {
    let temp = temp_dir("golden");
    let plan = repository_path("fixtures/plans/two_box.plan.json");
    let outputs = [temp.join("first.csv"), temp.join("second.csv")];
    let first = run(&plan, &outputs[0]);
    let second = run(&plan, &outputs[1]);
    assert_success(&first);
    assert_success(&second);

    let hashes: serde_json::Value = serde_json::from_slice(
        &std::fs::read(repository_path(
            "fixtures/plans/goldens/two_box.seed55.ticks40.hashes.json",
        ))
        .unwrap(),
    )
    .unwrap();
    let expected_stdout = format!(
        "results_sha256={} final_state_sha256={} observation_sha256={}\n",
        hashes["results_sha256"].as_str().unwrap(),
        hashes["final_state_sha256"].as_str().unwrap(),
        hashes["observation_sha256"].as_str().unwrap(),
    );
    assert_eq!(first.stdout, expected_stdout.as_bytes());
    assert_eq!(second.stdout, first.stdout);

    let golden_csv = std::fs::read(repository_path(
        "fixtures/plans/goldens/two_box.seed55.ticks40.csv",
    ))
    .unwrap();
    assert_eq!(std::fs::read(&outputs[0]).unwrap(), golden_csv);
    assert_eq!(std::fs::read(&outputs[1]).unwrap(), golden_csv);
    assert_eq!(
        std::fs::read(summaries(&outputs[0])).unwrap(),
        std::fs::read(summaries(&outputs[1])).unwrap()
    );
    assert_eq!(
        std::fs::read(sidecar(&outputs[0])).unwrap(),
        std::fs::read(sidecar(&outputs[1])).unwrap()
    );
    assert_eq!(
        normalized_manifest(&sidecar(&outputs[0])),
        std::fs::read(repository_path(
            "fixtures/plans/goldens/two_box.seed55.ticks40.manifest.json",
        ))
        .unwrap()
    );

    let manifest: serde_json::Value =
        serde_json::from_slice(&std::fs::read(sidecar(&outputs[0])).unwrap()).unwrap();
    assert_eq!(
        manifest["plan"],
        serde_json::json!({
            "plan_schema": "sembla.executable-plan/v1",
            "identity_scheme": "sembla.identity/stable-v1",
            "origin": "direct_stable",
            "plan_semantic_hash": {
                "algorithm": "sha256",
                "domain": "sembla.plan-core/v1",
                "digest": "0524e9403ce2e945a6a98bd5cc7db646779d565c963e83e9a881e86b3459cc9c"
            },
            "enabled_features": []
        })
    );
    assert!(manifest.get("linked_source").is_none());

    // The checked-in values intentionally differ from a legacy two-box run:
    // stable occurrence words, rather than dense legacy ordinals, drive RNG.
    std::fs::remove_dir_all(temp).unwrap();
}

#[test]
fn sibling_insertion_preserves_every_shared_output_trace() {
    let temp = temp_dir("sibling");
    let base_output = temp.join("base.csv");
    let sibling_output = temp.join("sibling.csv");
    let base = run(
        &repository_path("fixtures/plans/two_box.plan.json"),
        &base_output,
    );
    let sibling = run(
        &repository_path("fixtures/plans/two_box_plus_sibling.plan.json"),
        &sibling_output,
    );
    assert_success(&base);
    assert_success(&sibling);

    let (base_headers, base_rows) = csv_table(&std::fs::read(base_output).unwrap());
    let (sibling_headers, sibling_rows) = csv_table(&std::fs::read(sibling_output).unwrap());
    assert_eq!(base_rows.len(), 40);
    assert_eq!(sibling_rows.len(), 40);

    let shared_indices = |headers: &[String]| {
        headers
            .iter()
            .enumerate()
            .filter_map(|(index, header)| {
                (header == "tick"
                    || header == "deferred_total"
                    || header.contains("controller")
                    || header.contains("population"))
                .then_some(index)
            })
            .collect::<Vec<_>>()
    };
    let base_indices = shared_indices(&base_headers);
    let sibling_indices = shared_indices(&sibling_headers);
    let base_shared_headers = base_indices
        .iter()
        .map(|&index| base_headers[index].clone())
        .collect::<Vec<_>>();
    let sibling_shared_headers = sibling_indices
        .iter()
        .map(|&index| sibling_headers[index].clone())
        .collect::<Vec<_>>();
    assert_eq!(base_shared_headers, sibling_shared_headers);
    assert!(base_shared_headers
        .iter()
        .any(|header| header.starts_with("fired:controller.")));
    assert!(base_shared_headers
        .iter()
        .any(|header| header.starts_with("fired:population.")));
    assert!(base_shared_headers
        .iter()
        .any(|header| header.starts_with("count:population.")));
    assert!(base_shared_headers
        .iter()
        .any(|header| header == "deferred_total"));

    for (tick, (base_row, sibling_row)) in base_rows.iter().zip(&sibling_rows).enumerate() {
        let base_shared = base_indices
            .iter()
            .map(|&index| &base_row[index])
            .collect::<Vec<_>>();
        let sibling_shared = sibling_indices
            .iter()
            .map(|&index| &sibling_row[index])
            .collect::<Vec<_>>();
        assert_eq!(
            base_shared, sibling_shared,
            "shared trace changed at tick {tick}"
        );
    }
    std::fs::remove_dir_all(temp).unwrap();
}

#[test]
fn legacy_run_manifest_matches_the_pre_prd_byte_baseline() {
    let temp = temp_dir("legacy-manifest");
    let output = temp.join("legacy.csv");
    let result = Command::new(env!("CARGO_BIN_EXE_sembla"))
        .arg("run")
        .arg(repository_path("examples/reversible_ctmc.json"))
        .args([
            "--population",
            "25",
            "--seed",
            "55",
            "--ticks",
            "40",
            "--out",
        ])
        .arg(&output)
        .output()
        .unwrap();
    assert_success(&result);
    assert_eq!(
        std::fs::read(sidecar(&output)).unwrap(),
        std::fs::read(repository_path(
            "crates/sembla-cli/tests/fixtures/run_manifest_prd0004_legacy.json",
        ))
        .unwrap()
    );
    std::fs::remove_dir_all(temp).unwrap();
}
