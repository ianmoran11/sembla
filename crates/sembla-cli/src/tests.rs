use std::time::Duration;

use super::{
    collect_diff_corpus_paths, compare_per_tick_hashes, csv_field, duration_ms,
    fused_publishable_prefix, initialize_population, parse_backend, parse_diff_options,
    parse_sweep_options, run, run_bounded_concurrent_sweep, run_file_result, run_results_output,
    run_results_output_with_features, sweep_cuda_final_state_selection, sweep_file_result,
    sweep_file_result_with_runtime, BackendSelection, HashMode, ProductionSweepRuntime, RunOptions,
    SweepBackend, SweepConcurrencyMode, SweepConcurrentInputs, SweepCudaFinalStateMode,
    SweepOptions, SweepPreparedDraw, SweepRuntime, RETIRED_SWEEP_CUDA_DEVICE_FINAL_SHA256_ENV,
    RETIRED_SWEEP_CUDA_DEVICE_FINAL_SHA256_VERIFY_ENV, SWEEP_BACKEND_CONSTRUCTIONS,
    SWEEP_BACKEND_CONSTRUCTION_TEST_LOCK, SWEEP_CONCURRENCY_SPIKE_CUDA_FREE_STREAMS_ENV,
    SWEEP_CONCURRENCY_SPIKE_CUDA_LOCKSTEP_ENV, SWEEP_CONCURRENCY_SPIKE_DELAY_DRAW_ZERO_ENV,
    SWEEP_CONCURRENCY_SPIKE_WORKERS_ENV, SWEEP_CUDA_FINAL_STATE_MODE_ENV,
    SWEEP_CUDA_FUSED_DRAWS_ENV, VERSION,
};
use sembla_runtime::core::{ColumnData, ColumnInit, ParamEnv, StateStore, TableInit};

#[cfg(feature = "cuda")]
use super::{preflight_cuda_sweep_capacity_with_memory, SWEEP_TEST_CUDA_FREE_MEMORY_ENV};

fn load(source: &str) -> sembla_ir::ValidatedModel {
    sembla_ir::validate(sembla_ir::parse_json(source).unwrap()).unwrap()
}

fn initialized(model: &sembla_ir::ValidatedModel, rows: usize) -> StateStore {
    StateStore::new(model, initialize_population(model, rows)).unwrap()
}

/// A local policy-conformance runtime, never available to production.
/// It replaces only device-dependent construction and admission while the
/// supported CUDA option, free-stream mode, bounded scheduler, ordered
/// publication, manifest, and timing paths remain unchanged.
#[derive(Default)]
struct LocalConformanceSweepRuntime {
    fail_pinned_allocation: bool,
}

impl SweepRuntime for LocalConformanceSweepRuntime {
    fn preflight_cuda_capacity(
        &self,
        _model: &sembla_ir::ValidatedModel,
        _initial_tables: &[TableInit],
        workers: usize,
        final_state_mode: SweepCudaFinalStateMode,
    ) -> Result<super::SweepFinalStateAdmission, String> {
        if final_state_mode == SweepCudaFinalStateMode::PackedPinned {
            Ok(super::SweepFinalStateAdmission {
                requested_lane_count: workers,
                requested_pinned_bytes_per_lane: 1,
                requested_cacheable_staging_bytes_per_lane: 1,
                requested_pinned_bytes: workers,
                requested_cacheable_staging_bytes: workers,
                requested_buffer_set_count: workers,
                requested_underlying_pinned_allocation_count: workers,
            })
        } else {
            Ok(super::SweepFinalStateAdmission::without_treatment(workers))
        }
    }

    fn new_backend(
        &self,
        model: &sembla_ir::ValidatedModel,
        initial: Vec<TableInit>,
        initial_params: &ParamEnv,
        seed: u64,
        _backend: BackendSelection,
    ) -> Result<SweepBackend, String> {
        let _ = (initial_params, seed);
        if self.fail_pinned_allocation {
            SweepBackend::new_local_conformance_with_pinned_failure(model, initial, true)
        } else {
            SweepBackend::new_local_conformance(model, initial)
        }
    }

    fn new_concurrency_lane(
        &self,
        model: &sembla_ir::ValidatedModel,
        initial: Vec<TableInit>,
        initial_params: &ParamEnv,
        seed: u64,
        _backend: BackendSelection,
        _mode: SweepConcurrencyMode,
    ) -> Result<SweepBackend, String> {
        let _ = (initial_params, seed);
        if self.fail_pinned_allocation {
            SweepBackend::new_local_conformance_with_pinned_failure(model, initial, true)
        } else {
            SweepBackend::new_local_conformance(model, initial)
        }
    }
}

struct ScopedEnv {
    key: &'static str,
    previous: Option<std::ffi::OsString>,
}

impl ScopedEnv {
    fn remove(key: &'static str) -> Self {
        let previous = std::env::var_os(key);
        std::env::remove_var(key);
        Self { key, previous }
    }

    fn set(key: &'static str, value: &str) -> Self {
        let previous = std::env::var_os(key);
        std::env::set_var(key, value);
        Self { key, previous }
    }
}

impl Drop for ScopedEnv {
    fn drop(&mut self) {
        match &self.previous {
            Some(value) => std::env::set_var(self.key, value),
            None => std::env::remove_var(self.key),
        }
    }
}

#[test]
fn cuda_sweep_final_state_default_is_packed_pageable() {
    let _guard = SWEEP_BACKEND_CONSTRUCTION_TEST_LOCK.lock().unwrap();
    let _selector = ScopedEnv::remove(SWEEP_CUDA_FINAL_STATE_MODE_ENV);
    let default = sweep_cuda_final_state_selection(BackendSelection::Cuda).unwrap();
    assert_eq!(
        SweepCudaFinalStateMode::default(),
        SweepCudaFinalStateMode::PackedPageable
    );
    assert_eq!(default.mode, SweepCudaFinalStateMode::PackedPageable);
    assert!(!default.explicitly_set);

    let explicit = {
        let _materialized = ScopedEnv::set(SWEEP_CUDA_FINAL_STATE_MODE_ENV, "materialized");
        sweep_cuda_final_state_selection(BackendSelection::Cuda).unwrap()
    };
    assert_eq!(explicit.mode, SweepCudaFinalStateMode::Materialized);
    assert!(explicit.explicitly_set);
}

#[test]
fn sweep_constructs_one_backend_for_multiple_draws() {
    use std::sync::atomic::Ordering;

    let _guard = SWEEP_BACKEND_CONSTRUCTION_TEST_LOCK.lock().unwrap();
    let out = std::env::temp_dir().join(format!(
        "sembla-sweep-construction-count-{}-{}",
        std::process::id(),
        std::thread::current().name().unwrap_or("unnamed")
    ));
    let _ = std::fs::remove_dir_all(&out);
    let model = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../examples/reversible_ctmc.json");
    let options = SweepOptions {
        seed: 71,
        draws: Some(4),
        theta_file: None,
        noise_mode: super::manifest::NoiseMode::Independent,
        ticks: 3,
        population: "32".to_owned(),
        out: out.display().to_string(),
        params: None,
        export_pairs: None,
        timing_json: None,
        backend: BackendSelection::Cpu,
        draw_workers: None,
        enabled_features: sembla_ir::FeatureSet::new(),
    };

    SWEEP_BACKEND_CONSTRUCTIONS.store(0, Ordering::SeqCst);
    sweep_file_result(model.to_str().unwrap(), options).unwrap();
    assert_eq!(SWEEP_BACKEND_CONSTRUCTIONS.load(Ordering::SeqCst), 1);
    assert_eq!(
        std::fs::read_dir(&out)
            .unwrap()
            .filter_map(Result::ok)
            .filter(|entry| entry.file_name().to_string_lossy().starts_with("draw_"))
            .count(),
        4
    );
    std::fs::remove_dir_all(out).unwrap();
}

#[test]
fn supported_draw_workers_local_conformance_is_byte_stable_and_ordered() {
    use std::collections::BTreeMap;
    use std::path::{Path, PathBuf};
    use std::time::{SystemTime, UNIX_EPOCH};

    fn output_tree(root: &Path) -> BTreeMap<PathBuf, Vec<u8>> {
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

    fn parsed_options(
        state: &Path,
        out: &Path,
        timing: Option<&Path>,
        noise: &str,
        workers: usize,
        grouped: bool,
    ) -> SweepOptions {
        let mut flags = vec![
            "--seed".to_owned(),
            "9182".to_owned(),
            "--draws".to_owned(),
            "4".to_owned(),
            "--ticks".to_owned(),
            "5".to_owned(),
            "--noise".to_owned(),
            noise.to_owned(),
            "--population".to_owned(),
            state.display().to_string(),
            "--out".to_owned(),
            out.display().to_string(),
            "--backend".to_owned(),
            "cuda".to_owned(),
            "--draw-workers".to_owned(),
            workers.to_string(),
        ];
        if let Some(timing) = timing {
            flags.extend(["--timing-json".to_owned(), timing.display().to_string()]);
        }
        if grouped {
            flags.extend([
                "--enable".to_owned(),
                sembla_ir::GROUPED_OBSERVATIONS_FEATURE.to_owned(),
            ]);
        }
        parse_sweep_options(&flags).unwrap()
    }

    let _guard = SWEEP_BACKEND_CONSTRUCTION_TEST_LOCK.lock().unwrap();
    // The supported option must be the only concurrency selector in this
    // local conformance corpus. Scoped restoration keeps parallel tests and
    // caller environments intact even if an assertion panics.
    let _hidden_workers = ScopedEnv::remove(SWEEP_CONCURRENCY_SPIKE_WORKERS_ENV);
    let _hidden_lockstep = ScopedEnv::remove(SWEEP_CONCURRENCY_SPIKE_CUDA_LOCKSTEP_ENV);
    let _hidden_free = ScopedEnv::remove(SWEEP_CONCURRENCY_SPIKE_CUDA_FREE_STREAMS_ENV);
    let _hidden_fused = ScopedEnv::remove(SWEEP_CUDA_FUSED_DRAWS_ENV);
    let _final_state_mode = ScopedEnv::remove(SWEEP_CUDA_FINAL_STATE_MODE_ENV);
    let _retired_device_sha = ScopedEnv::remove(RETIRED_SWEEP_CUDA_DEVICE_FINAL_SHA256_ENV);
    let _retired_device_sha_verify =
        ScopedEnv::remove(RETIRED_SWEEP_CUDA_DEVICE_FINAL_SHA256_VERIFY_ENV);
    let _delay = ScopedEnv::set(SWEEP_CONCURRENCY_SPIKE_DELAY_DRAW_ZERO_ENV, "100");

    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let temp = std::env::temp_dir().join(format!(
        "sembla-supported-draw-workers-local-conformance-{}-{nonce}",
        std::process::id()
    ));
    std::fs::create_dir_all(&temp).unwrap();
    let repository = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let runtime = LocalConformanceSweepRuntime::default();

    for model_name in ["contest_competing_exits", "grouped_observation"] {
        let grouped = model_name == "grouped_observation";
        let model_path = repository.join(format!(
            "crates/sembla-cli/tests/fixtures/{model_name}.json"
        ));
        let source = std::fs::read_to_string(&model_path).unwrap();
        let features = if grouped {
            sembla_ir::FeatureSet::from([sembla_ir::GROUPED_OBSERVATIONS_FEATURE.to_owned()])
        } else {
            sembla_ir::FeatureSet::new()
        };
        let model =
            sembla_ir::validate_with_features(sembla_ir::parse_json(&source).unwrap(), &features)
                .unwrap();
        let tables = if model_name == "contest_competing_exits" {
            vec![
                TableInit::new("World", "slot_resource", 100, Vec::new()),
                TableInit::new(
                    "World",
                    "slot",
                    100,
                    vec![
                        ColumnInit::new("occupancy", ColumnData::Enum(vec![0; 100])),
                        ColumnInit::new("cause", ColumnData::Enum(vec![0; 100])),
                        ColumnInit::new("slot_resource", ColumnData::Ref((0_u32..100).collect())),
                    ],
                ),
            ]
        } else {
            vec![
                TableInit::new("world", "area", 12, Vec::new()),
                TableInit::new(
                    "world",
                    "person_slot",
                    5,
                    vec![
                        ColumnInit::new("sex", ColumnData::Enum(vec![0, 0, 1, 1, 0])),
                        ColumnInit::new("area", ColumnData::Ref(vec![10, 2, 10, 2, 2])),
                        ColumnInit::new("age_months", ColumnData::Int(vec![-1, 0, 59, 60, 120])),
                        ColumnInit::new("occupancy", ColumnData::Enum(vec![0; 5])),
                    ],
                ),
            ]
        };
        let state = temp.join(format!("{model_name}.state"));
        sembla_runtime::state_artifact::write(&state, &model, &tables).unwrap();

        for noise in ["crn", "independent"] {
            let sequential = temp.join(format!("{model_name}-{noise}-sequential"));
            let concurrent = temp.join(format!("{model_name}-{noise}-concurrent"));
            let timing = temp.join(format!("{model_name}-{noise}-timing.json"));

            let sequential_options = parsed_options(&state, &sequential, None, noise, 1, grouped);
            assert_eq!(sequential_options.backend, BackendSelection::Cuda);
            assert_eq!(sequential_options.draw_workers, Some(1));
            sweep_file_result_with_runtime(
                model_path.to_str().unwrap(),
                sequential_options,
                &runtime,
            )
            .unwrap();

            let concurrent_options =
                parsed_options(&state, &concurrent, Some(&timing), noise, 2, grouped);
            assert_eq!(concurrent_options.backend, BackendSelection::Cuda);
            assert_eq!(concurrent_options.draw_workers, Some(2));
            sweep_file_result_with_runtime(
                model_path.to_str().unwrap(),
                concurrent_options,
                &runtime,
            )
            .unwrap();

            let sequential_files = output_tree(&sequential);
            let concurrent_files = output_tree(&concurrent);
            assert_eq!(
                sequential_files.keys().collect::<Vec<_>>(),
                concurrent_files.keys().collect::<Vec<_>>(),
                "supported file set changed for {model_name}/{noise}"
            );
            for (path, sequential_bytes) in &sequential_files {
                assert_eq!(
                    concurrent_files.get(path),
                    Some(sequential_bytes),
                    "supported output '{}' changed for {model_name}/{noise}",
                    path.display()
                );
            }

            let timing_document: serde_json::Value =
                serde_json::from_slice(&std::fs::read(&timing).unwrap()).unwrap();
            assert_eq!(
                timing_document["schema"],
                "sembla-sweep-concurrency-spike-timing-v3"
            );
            assert_eq!(
                timing_document["execution_mode"],
                "cuda-free-nonblocking-streams"
            );
            assert_eq!(timing_document["requested_draw_workers"], 2);
            assert_eq!(timing_document["effective_draw_workers"], 2);
            assert!(timing_document["setup_wall_time_ms"].as_f64().unwrap() >= 0.0);
            assert!(
                timing_document["execution_window_wall_time_ms"]
                    .as_f64()
                    .unwrap()
                    >= 0.0
            );
            assert!(
                timing_document["publication_wall_time_ms"]
                    .as_f64()
                    .unwrap()
                    >= 0.0
            );
            let draw_timings = timing_document["draw_timings"].as_array().unwrap();
            assert_eq!(
                draw_timings
                    .iter()
                    .map(|draw| draw["k"].as_u64().unwrap())
                    .collect::<Vec<_>>(),
                vec![0, 1, 2, 3]
            );
            assert!(
                draw_timings[1]["finish_offset_ms"].as_f64().unwrap()
                    < draw_timings[0]["finish_offset_ms"].as_f64().unwrap(),
                "forced completion inversion did not occur for {model_name}/{noise}"
            );
            assert!(
                draw_timings[2]["start_offset_ms"].as_f64().unwrap()
                    < draw_timings[0]["finish_offset_ms"].as_f64().unwrap(),
                "delayed low-k draw held up later work for {model_name}/{noise}"
            );
            for draw in draw_timings {
                let final_state = &draw["final_state"];
                assert_eq!(final_state["schema"], "sembla-cuda-final-state-readback-v2");
                assert_eq!(final_state["mode"], "packed-pageable");
                assert!(final_state["pageable_dtoh_host_api_ms"].is_number());
                assert!(final_state["pinned_dtoh_enqueue_api_ms"].is_null());
                assert!(final_state["wait_to_pinned_host_readable_ms"].is_null());
                assert!(final_state["pinned_to_cacheable_staging_copy_ms"].is_null());
                assert!(final_state["host_state_reconstruction_ms"].is_null());
                assert_eq!(final_state["downloaded_bytes"]["total"], 1);
                assert_eq!(final_state["buffer_accounting"]["buffer_set_count"], 0);
            }
            let accounting = &timing_document["final_state_buffer_accounting"];
            assert_eq!(accounting["buffer_set_count"], 0);
            assert_eq!(accounting["underlying_pinned_allocation_count"], 0);
            assert_eq!(accounting["effective_pinned_bytes"], 0);
            assert_eq!(accounting["effective_cacheable_staging_bytes"], 0);

            let concurrent_manifest: serde_json::Value = serde_json::from_slice(
                &std::fs::read(concurrent.join("run-manifest.json")).unwrap(),
            )
            .unwrap();
            let execution = &concurrent_manifest["executions"][3];
            let fresh = temp.join(format!("{model_name}-{noise}-fresh.csv"));
            run_file_result(
                model_path.to_str().unwrap(),
                RunOptions {
                    seed: execution["seed"].as_u64().unwrap(),
                    ticks: 5,
                    population: state.display().to_string(),
                    out: Some(fresh.display().to_string()),
                    export_state: None,
                    dt: None,
                    params: None,
                    timing_json: None,
                    backend: BackendSelection::Cpu,
                    enabled_features: features.clone(),
                },
            )
            .unwrap();
            assert_eq!(
                std::fs::read(concurrent.join("draw_3.csv")).unwrap(),
                std::fs::read(&fresh).unwrap(),
                "draw 3 changed when run alongside peers for {model_name}/{noise}"
            );
            if grouped {
                assert_eq!(
                    std::fs::read(concurrent.join("draw_3.grouped.population_cells.csv")).unwrap(),
                    std::fs::read(temp.join(format!(
                        "{model_name}-{noise}-fresh.grouped.population_cells.csv"
                    )))
                    .unwrap(),
                    "grouped draw 3 changed when run alongside peers for {noise}"
                );
            }
            let fresh_manifest: serde_json::Value = serde_json::from_slice(
                &std::fs::read(format!("{}.manifest.json", fresh.display())).unwrap(),
            )
            .unwrap();
            assert_eq!(execution["resolved_theta"], serde_json::json!({}));
            for field in [
                "results_sha256",
                "final_state_sha256",
                "observation_sha256",
                "grouped_outputs",
            ] {
                assert_eq!(
                    execution[field], fresh_manifest[field],
                    "draw-alone manifest field {field} changed for {model_name}/{noise}"
                );
            }
        }
    }

    std::fs::remove_dir_all(temp).unwrap();
}

#[test]
fn packed_readback_modes_are_ordered_reused_and_scientifically_identical() {
    use std::collections::BTreeMap;
    use std::path::{Path, PathBuf};

    fn output_tree(root: &Path) -> BTreeMap<PathBuf, Vec<u8>> {
        let mut files = BTreeMap::new();
        for entry in std::fs::read_dir(root).unwrap() {
            let entry = entry.unwrap();
            if entry.path().is_file() {
                files.insert(
                    entry.path().strip_prefix(root).unwrap().to_owned(),
                    std::fs::read(entry.path()).unwrap(),
                );
            }
        }
        files
    }

    fn options(out: &Path, timing: Option<&Path>, workers: usize) -> SweepOptions {
        SweepOptions {
            seed: 91,
            draws: Some(4),
            theta_file: None,
            noise_mode: super::manifest::NoiseMode::Independent,
            ticks: 3,
            population: "32".to_owned(),
            out: out.display().to_string(),
            params: None,
            export_pairs: None,
            timing_json: timing.map(|path| path.display().to_string()),
            backend: BackendSelection::Cuda,
            draw_workers: Some(workers),
            enabled_features: sembla_ir::FeatureSet::new(),
        }
    }

    let _guard = SWEEP_BACKEND_CONSTRUCTION_TEST_LOCK.lock().unwrap();
    let _workers = ScopedEnv::remove(SWEEP_CONCURRENCY_SPIKE_WORKERS_ENV);
    let _lockstep = ScopedEnv::remove(SWEEP_CONCURRENCY_SPIKE_CUDA_LOCKSTEP_ENV);
    let _free = ScopedEnv::remove(SWEEP_CONCURRENCY_SPIKE_CUDA_FREE_STREAMS_ENV);
    let _fused = ScopedEnv::remove(SWEEP_CUDA_FUSED_DRAWS_ENV);
    let _retired = ScopedEnv::remove(RETIRED_SWEEP_CUDA_DEVICE_FINAL_SHA256_ENV);
    let _retired_verify = ScopedEnv::remove(RETIRED_SWEEP_CUDA_DEVICE_FINAL_SHA256_VERIFY_ENV);
    let _selector = ScopedEnv::remove(SWEEP_CUDA_FINAL_STATE_MODE_ENV);
    let _delay = ScopedEnv::set(SWEEP_CONCURRENCY_SPIKE_DELAY_DRAW_ZERO_ENV, "100");

    let root = std::env::temp_dir().join(format!(
        "sembla-packed-pageable-local-conformance-{}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).unwrap();
    let model = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples/reversible_ctmc.json");
    let runtime = LocalConformanceSweepRuntime::default();
    let control = root.join("control");
    {
        let _materialized = ScopedEnv::set(SWEEP_CUDA_FINAL_STATE_MODE_ENV, "materialized");
        sweep_file_result_with_runtime(
            model.to_str().unwrap(),
            options(&control, None, 1),
            &runtime,
        )
        .unwrap();
    }

    let default = root.join("default");
    let default_timing = root.join("default-timing.json");
    sweep_file_result_with_runtime(
        model.to_str().unwrap(),
        options(&default, Some(&default_timing), 1),
        &runtime,
    )
    .unwrap();
    assert_eq!(output_tree(&control), output_tree(&default));
    let default_document: serde_json::Value =
        serde_json::from_slice(&std::fs::read(&default_timing).unwrap()).unwrap();
    assert!(default_document["draw_timings"]
        .as_array()
        .unwrap()
        .iter()
        .all(|draw| draw["final_state"]["mode"] == "packed-pageable"));

    for mode in ["packed-pageable", "packed-pinned"] {
        let sequential = root.join(format!("{mode}-sequential"));
        let sequential_timing = root.join(format!("{mode}-sequential-timing.json"));
        let concurrent = root.join(format!("{mode}-concurrent"));
        let concurrent_timing = root.join(format!("{mode}-concurrent-timing.json"));
        {
            let _packed = ScopedEnv::set(SWEEP_CUDA_FINAL_STATE_MODE_ENV, mode);
            sweep_file_result_with_runtime(
                model.to_str().unwrap(),
                options(&sequential, Some(&sequential_timing), 1),
                &runtime,
            )
            .unwrap();
            sweep_file_result_with_runtime(
                model.to_str().unwrap(),
                options(&concurrent, Some(&concurrent_timing), 2),
                &runtime,
            )
            .unwrap();
        }

        assert_eq!(output_tree(&control), output_tree(&sequential));
        assert_eq!(output_tree(&control), output_tree(&concurrent));
        for (path, workers) in [(&sequential_timing, 1_usize), (&concurrent_timing, 2)] {
            let document: serde_json::Value =
                serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap();
            let draws = document["draw_timings"].as_array().unwrap();
            assert_eq!(
                draws
                    .iter()
                    .map(|draw| draw["k"].as_u64().unwrap())
                    .collect::<Vec<_>>(),
                vec![0, 1, 2, 3]
            );
            let mut allocation_draws_per_lane = std::collections::BTreeMap::new();
            for draw in draws {
                let final_state = &draw["final_state"];
                assert_eq!(final_state["schema"], "sembla-cuda-final-state-readback-v2");
                assert_eq!(final_state["mode"], mode);
                assert!(final_state["host_state_reconstruction_ms"].is_null());
                assert_eq!(final_state["downloaded_bytes"]["state"], 1);
                assert_eq!(final_state["downloaded_bytes"]["total"], 1);
                assert!(final_state["cpu_sha256_ms"].as_f64().unwrap() >= 0.0);
                assert!(final_state["final_state_seam_total_ms"].as_f64().unwrap() >= 0.0);
                assert_eq!(final_state["phases_reconcile"], true);
                assert_eq!(
                    final_state["final_state_seam_total_excludes_one_time_allocation"],
                    true
                );
                assert_eq!(final_state["timer_tolerance_ms"], 0.001);
                assert_eq!(
                    final_state["allocation_plus_seam_reconciles_with_draw_wall"],
                    true
                );
                if mode == "packed-pinned" {
                    assert!(final_state["pageable_dtoh_host_api_ms"].is_null());
                    assert!(final_state["pinned_dtoh_enqueue_api_ms"].is_number());
                    assert!(final_state["wait_to_pinned_host_readable_ms"].is_number());
                    assert!(final_state["pinned_to_cacheable_staging_copy_ms"].is_number());
                    assert_eq!(final_state["buffer_accounting"]["buffer_set_count"], 1);
                    assert_eq!(
                        final_state["buffer_accounting"]["underlying_pinned_allocation_count"],
                        1
                    );
                    let lane = draw
                        .get("lane")
                        .and_then(serde_json::Value::as_u64)
                        .unwrap_or(0);
                    if final_state["one_time_allocation_ms"].as_f64().unwrap() > 0.0 {
                        *allocation_draws_per_lane.entry(lane).or_insert(0_usize) += 1;
                    }
                } else {
                    assert!(final_state["pageable_dtoh_host_api_ms"].is_number());
                    assert!(final_state["pinned_dtoh_enqueue_api_ms"].is_null());
                    assert!(final_state["wait_to_pinned_host_readable_ms"].is_null());
                    assert!(final_state["pinned_to_cacheable_staging_copy_ms"].is_null());
                }
            }
            let accounting = &document["final_state_buffer_accounting"];
            if mode == "packed-pinned" {
                assert_eq!(accounting["buffer_set_count"], workers);
                assert_eq!(accounting["underlying_pinned_allocation_count"], workers);
                assert_eq!(accounting["requested_pinned_bytes"], workers);
                assert_eq!(accounting["effective_pinned_bytes"], workers);
                assert_eq!(accounting["requested_cacheable_staging_bytes"], workers);
                assert_eq!(accounting["effective_cacheable_staging_bytes"], workers);
                assert_eq!(allocation_draws_per_lane.len(), workers);
                assert!(allocation_draws_per_lane.values().all(|count| *count == 1));
            } else {
                assert_eq!(accounting["buffer_set_count"], 0);
                assert_eq!(accounting["underlying_pinned_allocation_count"], 0);
                assert_eq!(accounting["effective_pinned_bytes"], 0);
                assert_eq!(accounting["effective_cacheable_staging_bytes"], 0);
            }
        }
        let scientific_manifest =
            std::fs::read_to_string(sequential.join("run-manifest.json")).unwrap();
        assert!(!scientific_manifest.contains(mode));
        assert!(!scientific_manifest.contains(SWEEP_CUDA_FINAL_STATE_MODE_ENV));
    }
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn packed_pinned_injected_allocation_failure_has_no_fallback_or_publication() {
    let _guard = SWEEP_BACKEND_CONSTRUCTION_TEST_LOCK.lock().unwrap();
    let _selector = ScopedEnv::set(SWEEP_CUDA_FINAL_STATE_MODE_ENV, "packed-pinned");
    let _workers = ScopedEnv::remove(SWEEP_CONCURRENCY_SPIKE_WORKERS_ENV);
    let _lockstep = ScopedEnv::remove(SWEEP_CONCURRENCY_SPIKE_CUDA_LOCKSTEP_ENV);
    let _free = ScopedEnv::remove(SWEEP_CONCURRENCY_SPIKE_CUDA_FREE_STREAMS_ENV);
    let _fused = ScopedEnv::remove(SWEEP_CUDA_FUSED_DRAWS_ENV);
    let root = std::env::temp_dir().join(format!(
        "sembla-packed-pinned-allocation-failure-{}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&root);
    let model = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../examples/reversible_ctmc.json");
    let runtime = LocalConformanceSweepRuntime {
        fail_pinned_allocation: true,
    };
    let result = sweep_file_result_with_runtime(
        model.to_str().unwrap(),
        SweepOptions {
            seed: 91,
            draws: Some(3),
            theta_file: None,
            noise_mode: super::manifest::NoiseMode::Independent,
            ticks: 1,
            population: "8".to_owned(),
            out: root.display().to_string(),
            params: None,
            export_pairs: None,
            timing_json: None,
            backend: BackendSelection::Cuda,
            draw_workers: Some(1),
            enabled_features: sembla_ir::FeatureSet::new(),
        },
        &runtime,
    );
    let error = result.unwrap_err();
    assert!(error.contains("lane 0 of 1"));
    assert!(error.contains("injected packed-pinned cacheable staging allocation failure"));
    assert!(error.contains("requested 1 byte"));
    assert!(error.contains("no pageable fallback"));
    if root.exists() {
        assert!(std::fs::read_dir(&root).unwrap().next().is_none());
    }
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn pinned_accounting_allows_a_retained_lane_with_no_assigned_draw() {
    let diagnostic = super::SweepFinalStateDiagnostic {
        mode: SweepCudaFinalStateMode::PackedPinned,
        allocation: Duration::from_nanos(1),
        pageable_dtoh_host_api: None,
        pinned_dtoh_enqueue_api: Some(Duration::ZERO),
        wait_to_pinned_host_readable: Some(Duration::ZERO),
        pinned_to_cacheable_staging_copy: Some(Duration::ZERO),
        host_state_reconstruction: None,
        cpu_sha256: Duration::ZERO,
        total: Duration::ZERO,
        downloaded_bytes: super::SweepFinalStateDownloadedBytes {
            state: 1,
            inputs: 0,
            input_counts: 0,
            total: 1,
        },
        buffer_accounting: super::SweepFinalStateBufferAccounting {
            buffer_set_count: 1,
            underlying_pinned_allocation_count: 1,
            pinned_bytes: 1,
            cacheable_staging_bytes: 1,
        },
    };
    let timing =
        Some(super::SweepFinalStateTiming::new(&diagnostic, Duration::from_millis(1)).unwrap());
    let accounting = super::aggregate_final_state_buffer_accounting(
        SweepCudaFinalStateMode::PackedPinned,
        super::SweepFinalStateAdmission {
            requested_lane_count: 2,
            requested_pinned_bytes_per_lane: 1,
            requested_cacheable_staging_bytes_per_lane: 1,
            requested_pinned_bytes: 2,
            requested_cacheable_staging_bytes: 2,
            requested_buffer_set_count: 2,
            requested_underlying_pinned_allocation_count: 2,
        },
        [(0, &timing)],
    )
    .unwrap();
    assert_eq!(accounting.requested_lane_count, 2);
    assert_eq!(accounting.retained_lane_count, 2);
    assert_eq!(accounting.requested_buffer_set_count, 2);
    assert_eq!(accounting.buffer_set_count, 1);
    assert_eq!(accounting.requested_pinned_bytes, 2);
    assert_eq!(accounting.effective_pinned_bytes, 1);
    assert_eq!(accounting.requested_cacheable_staging_bytes, 2);
    assert_eq!(accounting.effective_cacheable_staging_bytes, 1);
}

#[test]
fn concurrent_scheduler_constructs_exactly_one_retained_backend_per_lane() {
    use std::sync::atomic::Ordering;

    let _guard = SWEEP_BACKEND_CONSTRUCTION_TEST_LOCK.lock().unwrap();
    let model = load(include_str!("../../../examples/reversible_ctmc.json"));
    let initial = initialize_population(&model, 32);
    let construction_params = ParamEnv::defaults(&model);
    let options = SweepOptions {
        seed: 71,
        draws: Some(5),
        theta_file: None,
        noise_mode: super::manifest::NoiseMode::Independent,
        ticks: 2,
        population: "32".to_owned(),
        out: "unused".to_owned(),
        params: None,
        export_pairs: None,
        timing_json: None,
        backend: BackendSelection::Cpu,
        draw_workers: None,
        enabled_features: sembla_ir::FeatureSet::new(),
    };
    let prepared = (0_u32..5)
        .map(|k| SweepPreparedDraw {
            k,
            params: ParamEnv::defaults(&model),
            execution_seed: sembla_runtime::rng::derive_sweep_replica_seed(71, k),
        })
        .collect::<Vec<_>>();

    SWEEP_BACKEND_CONSTRUCTIONS.store(0, Ordering::SeqCst);
    std::env::set_var(SWEEP_CONCURRENCY_SPIKE_DELAY_DRAW_ZERO_ENV, "100");
    let mut published = Vec::new();
    let result = run_bounded_concurrent_sweep(
        SweepConcurrentInputs {
            model: &model,
            initial_tables: &initial,
            construction_params: &construction_params,
            options: &options,
            prepared: &prepared,
            final_state_mode: SweepCudaFinalStateMode::Materialized,
        },
        2,
        SweepConcurrencyMode::IndependentDefaultStreams,
        &ProductionSweepRuntime,
        |_, completed| {
            published.push(completed.index);
            Ok(())
        },
    );
    std::env::remove_var(SWEEP_CONCURRENCY_SPIKE_DELAY_DRAW_ZERO_ENV);
    let completed = result.unwrap();
    assert_eq!(published, [0, 1, 2, 3, 4]);
    assert!(completed.maximum_pending_results <= 4);
    assert!(
        completed.timings[2].start_offset_ms < completed.timings[0].finish_offset_ms,
        "delayed draw zero held up later dynamically claimed work"
    );
    assert!(
        (duration_ms(completed.execution_window_elapsed)
            - completed
                .timings
                .iter()
                .map(|draw| draw.finish_offset_ms)
                .fold(0.0_f64, f64::max))
        .abs()
            < 0.001
    );
    assert_eq!(SWEEP_BACKEND_CONSTRUCTIONS.load(Ordering::SeqCst), 2);
}

#[test]
fn bounded_scheduler_reports_lowest_k_and_stops_higher_publication() {
    let _guard = SWEEP_BACKEND_CONSTRUCTION_TEST_LOCK.lock().unwrap();
    let model = load(include_str!("../../../examples/reversible_ctmc.json"));
    let initial = initialize_population(&model, 32);
    let construction_params = ParamEnv::defaults(&model);
    let options = SweepOptions {
        seed: 71,
        draws: Some(5),
        theta_file: None,
        noise_mode: super::manifest::NoiseMode::Independent,
        ticks: 2,
        population: "32".to_owned(),
        out: "unused".to_owned(),
        params: None,
        export_pairs: None,
        timing_json: None,
        backend: BackendSelection::Cpu,
        draw_workers: None,
        enabled_features: sembla_ir::FeatureSet::new(),
    };
    let prepared = (0_u32..5)
        .map(|k| SweepPreparedDraw {
            k,
            params: ParamEnv::defaults(&model),
            execution_seed: sembla_runtime::rng::derive_sweep_replica_seed(71, k),
        })
        .collect::<Vec<_>>();
    let mut publication_attempts = Vec::new();
    let result = run_bounded_concurrent_sweep(
        SweepConcurrentInputs {
            model: &model,
            initial_tables: &initial,
            construction_params: &construction_params,
            options: &options,
            prepared: &prepared,
            final_state_mode: SweepCudaFinalStateMode::Materialized,
        },
        2,
        SweepConcurrencyMode::IndependentDefaultStreams,
        &ProductionSweepRuntime,
        |_, completed| {
            publication_attempts.push(completed.index);
            if completed.index == 2 {
                Err("draw 2: injected failure".to_owned())
            } else {
                Ok(())
            }
        },
    );
    let error = match result {
        Err(error) => error,
        Ok(_) => panic!("injected publication failure unexpectedly succeeded"),
    };
    assert_eq!(publication_attempts, [0, 1, 2]);
    assert!(error.contains("draw 2: injected failure"));
}

#[cfg(feature = "cuda")]
#[test]
fn fake_memory_limit_rejects_before_any_lane_construction() {
    use std::sync::atomic::Ordering;

    let _guard = SWEEP_BACKEND_CONSTRUCTION_TEST_LOCK.lock().unwrap();
    let model = load(include_str!("../../../examples/reversible_ctmc.json"));
    let initial = initialize_population(&model, 32);
    SWEEP_BACKEND_CONSTRUCTIONS.store(0, Ordering::SeqCst);
    std::env::set_var(SWEEP_TEST_CUDA_FREE_MEMORY_ENV, "1");
    let result = preflight_cuda_sweep_capacity_with_memory(
        &model,
        &initial,
        4,
        SweepCudaFinalStateMode::PackedPinned,
        80 * 1024 * 1024 * 1024,
        80 * 1024 * 1024 * 1024,
    );
    std::env::remove_var(SWEEP_TEST_CUDA_FREE_MEMORY_ENV);
    let error = result.unwrap_err();
    assert!(error.contains("insufficient CUDA device memory"));
    assert!(error.contains("no lanes were constructed"));
    assert!(error.contains("packed-pinned page-locked bytes"));
    assert!(error.contains("cacheable staging bytes"));
    assert_eq!(SWEEP_BACKEND_CONSTRUCTIONS.load(Ordering::SeqCst), 0);

    let admitted = preflight_cuda_sweep_capacity_with_memory(
        &model,
        &initial,
        4,
        SweepCudaFinalStateMode::PackedPinned,
        usize::MAX,
        usize::MAX,
    )
    .unwrap();
    assert_eq!(admitted.requested_lane_count, 4);
    assert_eq!(admitted.requested_buffer_set_count, 4);
    assert!(admitted.requested_underlying_pinned_allocation_count <= 12);
    assert!(admitted.requested_pinned_bytes > 0);
    assert_eq!(
        admitted.requested_pinned_bytes,
        admitted.requested_cacheable_staging_bytes
    );
}

#[test]
fn fused_publication_stops_at_the_lowest_failed_k() {
    assert_eq!(
        fused_publishable_prefix(
            &[(0, false), (1, false), (2, true), (3, false), (4, true)],
            5,
        )
        .unwrap(),
        2
    );
    assert_eq!(
        fused_publishable_prefix(&[(0, false), (1, false), (2, false)], 3).unwrap(),
        3
    );
    assert!(fused_publishable_prefix(&[(0, false), (2, true)], 3)
        .unwrap_err()
        .contains("draw index 2 at position 1"));
    assert!(fused_publishable_prefix(&[(0, false), (1, false)], 3)
        .unwrap_err()
        .contains("returned 2 successful draws"));
}

#[test]
fn version_matches_library_versions() {
    assert_eq!(VERSION, sembla_cpu::VERSION);
    assert_eq!(VERSION, sembla_ir::VERSION);
    assert_eq!(VERSION, sembla_runtime::VERSION);
}

#[test]
fn invalid_usage_is_nonzero() {
    assert_eq!(run(&[]), 2);
}

#[test]
fn backend_and_differential_run_options_are_strict() {
    assert_eq!(parse_backend("cpu").unwrap(), BackendSelection::Cpu);
    assert_eq!(parse_backend("cuda").unwrap(), BackendSelection::Cuda);
    assert!(parse_backend("auto").is_err());

    let options = parse_diff_options(&[
        "model.json".to_owned(),
        "--population".to_owned(),
        "10".to_owned(),
        "--seed".to_owned(),
        "2".to_owned(),
        "--ticks".to_owned(),
        "3".to_owned(),
        "--dt".to_owned(),
        "0.5".to_owned(),
        "--params".to_owned(),
        "params.json".to_owned(),
    ])
    .unwrap();
    assert_eq!(options.dt, Some(0.5));
    assert_eq!(options.params.as_deref(), Some("params.json"));
    let grouped = parse_diff_options(&[
        "model.json".to_owned(),
        "--enable".to_owned(),
        sembla_ir::GROUPED_OBSERVATIONS_FEATURE.to_owned(),
    ])
    .unwrap();
    assert!(grouped
        .enabled_features
        .contains(sembla_ir::GROUPED_OBSERVATIONS_FEATURE));
    assert!(parse_diff_options(&[
        "model.json".to_owned(),
        "--enable".to_owned(),
        "future-feature".to_owned(),
    ])
    .unwrap_err()
    .contains("unknown feature 'future-feature'"));
    assert!(parse_diff_options(&[
        "--all-examples".to_owned(),
        "--params".to_owned(),
        "params.json".to_owned(),
    ])
    .unwrap_err()
    .contains("single diff-backends input"));
    assert!(parse_diff_options(&[
        "--all-plan-fixtures".to_owned(),
        "--all-examples".to_owned(),
    ])
    .unwrap_err()
    .contains("cannot be combined"));
    assert!(
        parse_diff_options(&["model.json".to_owned(), "--all-plan-fixtures".to_owned(),])
            .unwrap_err()
            .contains("cannot be combined")
    );
    assert!(
        parse_diff_options(&["--all-plan-fixtures".to_owned(), "model.json".to_owned(),])
            .unwrap_err()
            .contains("positional path")
    );
    assert!(parse_diff_options(&[
        "--all-plan-fixtures".to_owned(),
        "--dt".to_owned(),
        "0.5".to_owned(),
    ])
    .unwrap_err()
    .contains("plan envelopes do not support --dt overrides"));
}

#[test]
fn plan_fixture_corpus_is_the_exact_sorted_top_level_and_linked_set() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let plans = root.join("fixtures/plans");
    let linked = plans.join("linked");
    let mut paths = collect_diff_corpus_paths(plans.to_str().unwrap(), ".plan.json").unwrap();
    paths.extend(collect_diff_corpus_paths(linked.to_str().unwrap(), ".plan.json").unwrap());
    paths.sort();
    let relative = paths
        .iter()
        .map(|path| {
            std::path::Path::new(path)
                .strip_prefix(&root)
                .unwrap()
                .to_string_lossy()
                .into_owned()
        })
        .collect::<Vec<_>>();
    assert_eq!(
        relative,
        [
            // DECISIONS §K6 sanctions this first feature-bearing plan fixture.
            "fixtures/plans/grouped_observation.plan.json",
            "fixtures/plans/linked/epidemic_policy.plan.json",
            "fixtures/plans/linked/independent_epidemic_policy.plan.json",
            "fixtures/plans/linked/ping_pong.plan.json",
            "fixtures/plans/linked/regional_response.plan.json",
            "fixtures/plans/linked/solo_population.plan.json",
            "fixtures/plans/linked/two_independent_regions.plan.json",
            "fixtures/plans/linked/two_regions.plan.json",
            "fixtures/plans/linked/wrapped_ping_pong.plan.json",
            "fixtures/plans/observations.plan.json",
            "fixtures/plans/sir.plan.json",
            "fixtures/plans/sir_policy.plan.json",
            "fixtures/plans/two_box.plan.json",
            "fixtures/plans/two_box_plus_sibling.plan.json",
        ]
    );
}

#[test]
fn generic_csv_is_ordered_deterministic_and_conservative() {
    let model = load(include_str!("../../../examples/reversible_ctmc.json"));
    let params = ParamEnv::defaults(&model);
    let mut first_state = initialized(&model, 1000);
    let mut second_state = initialized(&model, 1000);
    let first = run_results_output(&model, &mut first_state, &params, 55, 20).unwrap();
    let second = run_results_output(&model, &mut second_state, &params, 55, 20).unwrap();
    assert_eq!(first.csv, second.csv);
    assert!(first.per_tick_hashes.is_none());
    assert!(second.per_tick_hashes.is_none());
    assert_eq!(
        first.csv.lines().nth(2).unwrap(),
        "tick,count:chain.particle.phase=A,count:chain.particle.phase=B,fired:chain.move_ab,fired:chain.move_ba,deferred_total"
    );
    assert_eq!(first.series.rows.len(), 20);
    for row in &first.series.rows {
        assert_eq!(
            row[0].as_usize("A").unwrap() + row[1].as_usize("B").unwrap(),
            1000
        );
        assert_eq!(row.len(), 5);
    }
    assert_eq!(
        first.series.rows[0][3].as_usize("B to A").unwrap(),
        0,
        "B to A must still have a zero-valued column"
    );
    assert!(first.series.rows.last().unwrap()[1].as_usize("B").unwrap() > 0);
}

#[test]
fn device_generic_enum_counts_preserve_legacy_csv_bytes() {
    let model = load(include_str!("../../../examples/reversible_ctmc.json"));
    let params = ParamEnv::defaults(&model);
    let mut state = initialized(&model, 100);
    let report = sembla_cpu::run_tick(&model, &mut state, &params, 55, 0).unwrap();
    let snapshot = state.snapshot();
    let values = snapshot.enum_values("chain", "particle", "phase").unwrap();
    let counts = [
        values.iter().filter(|value| **value == 0).count(),
        values.iter().filter(|value| **value == 1).count(),
    ];

    let mut host = super::RunOutputAccumulator::new(&model, &params, 1).unwrap();
    host.push_tick(&state, 0, report.clone()).unwrap();
    let mut device = super::RunOutputAccumulator::new(&model, &params, 1).unwrap();
    device
        .push_tick_with_enum_counts(&state, 0, report, Some(&counts))
        .unwrap();
    assert_eq!(host.csv, device.csv);
}

#[test]
fn plan_run_is_bitwise_deterministic_twice_in_process() {
    use std::time::{SystemTime, UNIX_EPOCH};

    let _guard = SWEEP_BACKEND_CONSTRUCTION_TEST_LOCK.lock().unwrap();
    let _selector = ScopedEnv::remove(SWEEP_CUDA_FINAL_STATE_MODE_ENV);
    let _retired = ScopedEnv::remove(RETIRED_SWEEP_CUDA_DEVICE_FINAL_SHA256_ENV);
    let _retired_verify = ScopedEnv::remove(RETIRED_SWEEP_CUDA_DEVICE_FINAL_SHA256_VERIFY_ENV);
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let temp = std::env::temp_dir().join(format!(
        "sembla-plan-in-process-{}-{nonce}",
        std::process::id()
    ));
    std::fs::create_dir_all(&temp).unwrap();
    let plan = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join("fixtures/plans/two_box.plan.json");
    let outputs = [temp.join("first.csv"), temp.join("second.csv")];
    for output in &outputs {
        run_file_result(
            plan.to_str().unwrap(),
            RunOptions {
                seed: 55,
                ticks: 40,
                population: "16".to_owned(),
                out: Some(output.to_str().unwrap().to_owned()),
                export_state: None,
                dt: None,
                params: None,
                timing_json: None,
                backend: BackendSelection::Cpu,
                enabled_features: sembla_ir::FeatureSet::new(),
            },
        )
        .unwrap();
    }
    for suffix in ["", ".summaries.csv", ".manifest.json"] {
        assert_eq!(
            std::fs::read(format!("{}{suffix}", outputs[0].display())).unwrap(),
            std::fs::read(format!("{}{suffix}", outputs[1].display())).unwrap(),
            "plan run artifact '{suffix}' changed between in-process executions"
        );
    }
    std::fs::remove_dir_all(temp).unwrap();
}

#[test]
fn per_tick_hash_mode_type_enforces_absence() {
    let model = load(include_str!("../../../examples/reversible_ctmc.json"));
    let params = ParamEnv::defaults(&model);
    let mut state = initialized(&model, 10);
    let output = run_results_output_with_features(
        &model,
        &mut state,
        &params,
        55,
        3,
        HashMode::EveryTick,
        &sembla_ir::FeatureSet::new(),
    )
    .unwrap();
    assert_eq!(output.per_tick_hashes.as_ref().unwrap().len(), 3);
}

#[test]
fn per_tick_hash_comparison_detects_the_first_divergent_tick() {
    let cpu = Some(vec![[0; 32], [1; 32], [2; 32]]);
    let cuda = Some(vec![[0; 32], [9; 32], [8; 32]]);
    let error = compare_per_tick_hashes("model.json", &cpu, &cuda).unwrap_err();
    assert!(error.contains("first divergence at tick 1"));
    assert!(error.contains(&format!("cpu={}", super::hex(&[1; 32]))));
    assert!(error.contains(&format!("cuda={}", super::hex(&[9; 32]))));
}

#[test]
fn per_tick_hash_comparison_checks_lengths_before_elements() {
    let cpu = Some(vec![[1; 32], [2; 32]]);
    let cuda = Some(vec![[9; 32]]);
    let error = compare_per_tick_hashes("model.json", &cpu, &cuda).unwrap_err();
    assert_eq!(
        error,
        "model.json: per-tick hash sequence lengths differ: cpu=2 cuda=1"
    );
}

#[test]
fn per_tick_hash_comparison_rejects_absent_sequences() {
    let hashes = Some(vec![[0; 32]]);
    let cpu_error = compare_per_tick_hashes("model.json", &None, &hashes).unwrap_err();
    assert!(cpu_error.contains("internal invariant violation"));
    assert!(cpu_error.contains("cpu per-tick hashes are absent"));

    let cuda_error = compare_per_tick_hashes("model.json", &hashes, &None).unwrap_err();
    assert!(cuda_error.contains("internal invariant violation"));
    assert!(cuda_error.contains("cuda per-tick hashes are absent"));
}

#[test]
fn generated_csv_headers_are_escaped() {
    assert_eq!(csv_field("plain"), "plain");
    assert_eq!(csv_field("has,comma"), "\"has,comma\"");
    assert_eq!(csv_field("has\"quote"), "\"has\"\"quote\"");
    assert_eq!(csv_field("has\nnewline"), "\"has\nnewline\"");
}
