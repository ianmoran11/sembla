#![cfg(feature = "cuda")]

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::time::{SystemTime, UNIX_EPOCH};

fn repository_path(relative: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join(relative)
}

fn temp_dir(label: &str) -> PathBuf {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let path = std::env::temp_dir().join(format!("sembla-gpu-{label}-{nonce}"));
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

fn output_files(root: &Path) -> BTreeMap<PathBuf, Vec<u8>> {
    fn visit(root: &Path, directory: &Path, files: &mut BTreeMap<PathBuf, Vec<u8>>) {
        for entry in std::fs::read_dir(directory).unwrap() {
            let path = entry.unwrap().path();
            if path.is_dir() {
                visit(root, &path, files);
            } else {
                files.insert(
                    path.strip_prefix(root).unwrap().to_owned(),
                    std::fs::read(path).unwrap(),
                );
            }
        }
    }
    let mut files = BTreeMap::new();
    visit(root, root, &mut files);
    files
}

fn plan_fixture_paths() -> Vec<PathBuf> {
    let mut paths = Vec::new();
    for directory in ["fixtures/plans", "fixtures/plans/linked"] {
        paths.extend(
            std::fs::read_dir(repository_path(directory))
                .unwrap()
                .map(|entry| entry.unwrap().path())
                .filter(|path| {
                    path.is_file()
                        && path
                            .file_name()
                            .and_then(|name| name.to_str())
                            .is_some_and(|name| name.ends_with(".plan.json"))
                }),
        );
    }
    paths.sort();
    paths
}

fn plan_uses_grouped_views(path: &Path) -> bool {
    let source = std::fs::read_to_string(path).unwrap();
    let sembla_ir::ParsedInput::Plan(plan) = sembla_ir::parse_input(&source).unwrap() else {
        panic!("{} is not a plan", path.display());
    };
    plan.model
        .boxes
        .iter()
        .any(|model_box| !model_box.grouped_views.is_empty())
}

#[test]
#[ignore = "requires a CUDA GPU; run explicitly as lockstep-stream hardware evidence"]
fn lockstep_nonblocking_stream_sweep_matches_sequential_cuda() {
    let temp = temp_dir("lockstep-stream-sweep");
    let model = repository_path("fixtures/demographic/demographic_slots.json");
    let population = repository_path("fixtures/state/demographic_slots.state");
    for noise in ["crn", "independent"] {
        let sequential = temp.join(format!("{noise}-sequential"));
        let lockstep = temp.join(format!("{noise}-lockstep"));
        let timing = temp.join(format!("{noise}-lockstep-timing.json"));
        let common = [
            "--seed",
            "7",
            "--draws",
            "4",
            "--ticks",
            "2",
            "--noise",
            noise,
            "--backend",
            "cuda",
            "--enable",
            sembla_ir::GROUPED_OBSERVATIONS_FEATURE,
        ];
        let sequential_output = Command::new(env!("CARGO_BIN_EXE_sembla"))
            .arg("sweep")
            .arg(&model)
            .arg("--population")
            .arg(&population)
            .args(common)
            .arg("--out")
            .arg(&sequential)
            .output()
            .unwrap();
        assert_success(&sequential_output);

        let lockstep_output = Command::new(env!("CARGO_BIN_EXE_sembla"))
            .arg("sweep")
            .arg(&model)
            .arg("--population")
            .arg(&population)
            .args(common)
            .arg("--timing-json")
            .arg(&timing)
            .arg("--out")
            .arg(&lockstep)
            .env("SEMBLA_SWEEP_SPIKE_DRAW_WORKERS", "2")
            .env("SEMBLA_SWEEP_SPIKE_CUDA_LOCKSTEP_STREAMS", "1")
            .output()
            .unwrap();
        assert_success(&lockstep_output);
        assert_eq!(output_files(&lockstep), output_files(&sequential));
        let document: serde_json::Value =
            serde_json::from_slice(&std::fs::read(&timing).unwrap()).unwrap();
        assert_eq!(
            document["execution_mode"],
            "cuda-lockstep-nonblocking-streams"
        );
    }
    std::fs::remove_dir_all(temp).unwrap();
}

#[test]
#[ignore = "requires a CUDA GPU; run explicitly as fused-grid-y hardware evidence"]
fn fused_grid_y_sweep_matches_sequential_cuda_with_partial_tail() {
    let temp = temp_dir("fused-grid-y-sweep");
    let model = repository_path("fixtures/demographic/demographic_slots.json");
    let population = repository_path("fixtures/state/demographic_slots.state");
    for noise in ["crn", "independent"] {
        let sequential = temp.join(format!("{noise}-sequential"));
        let common = [
            "--seed",
            "7",
            "--draws",
            "5",
            "--ticks",
            "2",
            "--noise",
            noise,
            "--backend",
            "cuda",
            "--enable",
            sembla_ir::GROUPED_OBSERVATIONS_FEATURE,
        ];
        let sequential_output = Command::new(env!("CARGO_BIN_EXE_sembla"))
            .arg("sweep")
            .arg(&model)
            .arg("--population")
            .arg(&population)
            .args(common)
            .arg("--out")
            .arg(&sequential)
            .output()
            .unwrap();
        assert_success(&sequential_output);
        for width in [1, 2, 4] {
            let fused = temp.join(format!("{noise}-fused-{width}"));
            let timing = temp.join(format!("{noise}-fused-{width}-timing.json"));
            let fused_output = Command::new(env!("CARGO_BIN_EXE_sembla"))
                .arg("sweep")
                .arg(&model)
                .arg("--population")
                .arg(&population)
                .args(common)
                .arg("--timing-json")
                .arg(&timing)
                .arg("--out")
                .arg(&fused)
                .env("SEMBLA_SWEEP_SPIKE_CUDA_FUSED_DRAWS", width.to_string())
                .output()
                .unwrap();
            assert_success(&fused_output);
            assert_eq!(output_files(&fused), output_files(&sequential));
            let document: serde_json::Value =
                serde_json::from_slice(&std::fs::read(&timing).unwrap()).unwrap();
            assert_eq!(document["schema"], "sembla-cuda-fused-draw-spike-timing-v1");
            assert_eq!(document["requested_capacity"], width);
            assert_eq!(document["maximum_active_slots"], width);
            let active_slots = document["chunks"]
                .as_array()
                .unwrap()
                .iter()
                .map(|chunk| chunk["active_slots"].as_u64().unwrap())
                .collect::<Vec<_>>();
            let expected = match width {
                1 => vec![1, 1, 1, 1, 1],
                2 => vec![2, 2, 1],
                4 => vec![4, 1],
                _ => unreachable!(),
            };
            assert_eq!(active_slots, expected);
        }
    }

    let single_timing = temp.join("single-draw-width-four-timing.json");
    let single_output = Command::new(env!("CARGO_BIN_EXE_sembla"))
        .arg("sweep")
        .arg(&model)
        .arg("--population")
        .arg(&population)
        .args([
            "--seed",
            "7",
            "--draws",
            "1",
            "--ticks",
            "1",
            "--backend",
            "cuda",
            "--enable",
            sembla_ir::GROUPED_OBSERVATIONS_FEATURE,
            "--timing-json",
        ])
        .arg(&single_timing)
        .arg("--out")
        .arg(temp.join("single-draw-width-four"))
        .env("SEMBLA_SWEEP_SPIKE_CUDA_FUSED_DRAWS", "4")
        .output()
        .unwrap();
    assert_success(&single_output);
    let document: serde_json::Value =
        serde_json::from_slice(&std::fs::read(&single_timing).unwrap()).unwrap();
    assert_eq!(document["requested_capacity"], 4);
    assert_eq!(document["maximum_active_slots"], 1);
    assert_eq!(document["chunks"][0]["active_slots"], 1);
    std::fs::remove_dir_all(temp).unwrap();
}

#[test]
#[ignore = "requires a CUDA GPU; run crates/sembla-cuda/scripts/run-differential-corpus.sh"]
fn eligible_views_take_device_observation_differential_path() {
    let output = Command::new(env!("CARGO_BIN_EXE_sembla"))
        .current_dir(repository_path("."))
        .args([
            "diff-backends",
            "examples/sir.json",
            "--population",
            "100",
            "--seed",
            "7",
            "--ticks",
            "2",
        ])
        .output()
        .unwrap();
    assert_success(&output);
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("cuda_device_observation eligible=true")
    );
}

#[test]
#[ignore = "requires a CUDA GPU; run crates/sembla-cuda/scripts/run-differential-corpus.sh"]
fn ineligible_views_take_state_download_differential_fallback() {
    let output = Command::new(env!("CARGO_BIN_EXE_sembla"))
        .current_dir(repository_path("."))
        .args([
            "diff-backends",
            "examples/observations.json",
            "--population",
            "100",
            "--seed",
            "7",
            "--ticks",
            "2",
        ])
        .output()
        .unwrap();
    assert_success(&output);
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("cuda_device_observation eligible=false")
    );
}

#[test]
#[ignore = "requires a CUDA GPU; run crates/sembla-cuda/scripts/run-differential-corpus.sh"]
fn grouped_demographic_configuration_uses_device_histograms_differentially() {
    let output = Command::new(env!("CARGO_BIN_EXE_sembla"))
        .current_dir(repository_path("."))
        .args([
            "diff-backends",
            "fixtures/demographic/demographic_slots.json",
            "--population",
            "fixtures/state/demographic_slots.state",
            "--seed",
            "7",
            "--ticks",
            "2",
            "--enable",
            "grouped-observations",
        ])
        .output()
        .unwrap();
    assert_success(&output);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("cuda_device_observation eligible=true"));
    for view in ["population_cells", "deaths_cells", "vacancy_cells"] {
        assert!(
            stderr.contains(&format!("view=\"{view}\" key_space_size=")),
            "missing grouped statistics for {view}: {stderr}"
        );
    }
}

#[test]
#[ignore = "requires a CUDA GPU; run crates/sembla-cuda/scripts/run-differential-corpus.sh"]
fn differential_corpus_passes() {
    let output = Command::new(env!("CARGO_BIN_EXE_sembla"))
        .current_dir(repository_path("."))
        .args([
            "diff-backends",
            "--all-examples",
            "--population",
            "100",
            "--seed",
            "7",
            "--ticks",
            "20",
        ])
        .output()
        .unwrap();
    assert_success(&output);
}

#[test]
fn composition_plan_differential_corpus_requires_grouped_feature_flag() {
    let output = Command::new(env!("CARGO_BIN_EXE_sembla"))
        .current_dir(repository_path("."))
        .args([
            "diff-backends",
            "--all-plan-fixtures",
            "--population",
            "1",
            "--seed",
            "1",
            "--ticks",
            "1",
        ])
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(1));
    assert!(output.stdout.is_empty());
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("requires --enable grouped-observations")
    );
}

#[test]
#[ignore = "requires a CUDA GPU; run crates/sembla-cuda/scripts/run-differential-corpus.sh"]
fn supported_composition_plan_differential_corpus_passes() {
    let mut grouped = 0;
    let mut supported = 0;
    for plan in plan_fixture_paths() {
        let uses_grouped = plan_uses_grouped_views(&plan);
        if uses_grouped {
            grouped += 1;
        }
        let mut command = Command::new(env!("CARGO_BIN_EXE_sembla"));
        command
            .current_dir(repository_path("."))
            .arg("diff-backends")
            .arg(&plan)
            .args(["--population", "1000", "--seed", "7", "--ticks", "20"]);
        if uses_grouped {
            command.args(["--enable", "grouped-observations"]);
        }
        let output = command.output().unwrap();
        assert!(
            output.status.success(),
            "plan={}\nstdout={}\nstderr={}",
            plan.display(),
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        supported += 1;
    }
    assert!(
        grouped > 0,
        "plan corpus must exercise grouped CUDA support"
    );
    assert!(
        supported > 0,
        "plan corpus must retain CUDA-supported members"
    );
}

#[test]
#[ignore = "requires a CUDA GPU; run crates/sembla-cuda/scripts/run-differential-corpus.sh"]
fn cuda_manifest_verify_and_level_a_bytes_round_trip() {
    let temp = temp_dir("manifest");
    let model = repository_path("examples/two_state.json");
    let mut outputs = Vec::new();
    for name in ["first.csv", "second.csv"] {
        let out = temp.join(name);
        let output = Command::new(env!("CARGO_BIN_EXE_sembla"))
            .arg("run")
            .arg(&model)
            .args([
                "--population",
                "100",
                "--seed",
                "11",
                "--ticks",
                "20",
                "--backend",
                "cuda",
                "--out",
            ])
            .arg(&out)
            .output()
            .unwrap();
        assert_success(&output);
        outputs.push(out);
    }
    assert_eq!(
        std::fs::read(&outputs[0]).unwrap(),
        std::fs::read(&outputs[1]).unwrap()
    );
    assert_eq!(
        std::fs::read(format!("{}.summaries.csv", outputs[0].display())).unwrap(),
        std::fs::read(format!("{}.summaries.csv", outputs[1].display())).unwrap()
    );
    assert_eq!(
        std::fs::read(format!("{}.manifest.json", outputs[0].display())).unwrap(),
        std::fs::read(format!("{}.manifest.json", outputs[1].display())).unwrap()
    );
    let manifest_path = format!("{}.manifest.json", outputs[0].display());
    let manifest: serde_json::Value =
        serde_json::from_slice(&std::fs::read(&manifest_path).unwrap()).unwrap();
    assert_eq!(manifest["backend_identity"]["backend"], "cuda-native-f64");
    assert_eq!(manifest["backend_identity"]["precision"], "f64");
    assert_eq!(manifest["backend_identity"]["fell_back"], false);
    assert!(manifest["backend_identity"]["gpu_model"]
        .as_str()
        .is_some_and(|value| !value.is_empty()));
    assert!(manifest["backend_identity"]["driver_version"]
        .as_str()
        .is_some_and(|value| !value.is_empty()));
    let verify = Command::new(env!("CARGO_BIN_EXE_sembla"))
        .arg("verify-run")
        .arg(&manifest_path)
        .arg(&model)
        .args(["--population", "100"])
        .output()
        .unwrap();
    assert_success(&verify);
    std::fs::remove_dir_all(temp).unwrap();
}
