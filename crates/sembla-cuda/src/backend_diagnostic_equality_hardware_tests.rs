use super::{CudaBackend, HashMode, ValidationLaunchGeometry};
use sembla_runtime::core::ParamEnv;

mod cases {
    include!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/support/diagnostic_cases.rs"
    ));
}

const CHILD_ENV: &str = "SEMBLA_CUDA_DIAGNOSTIC_CHILD";
const DEADLINE: std::time::Duration = std::time::Duration::from_secs(120);

fn run_negative_corpus() {
    assert_eq!(cases::FAILING_ROWS, [2, 5, 7]);
    check_recovery_beyond_grid();
    for case in cases::CASES {
        assert!(!case.expected_cpu_error.is_empty(), "{}", case.name);
        let model = cases::load_model(&case);
        let params = ParamEnv::defaults(&model);
        let mut first_status = None;

        for (grid, block) in [(0, 0)].into_iter().chain(cases::GEOMETRIES) {
            let mut backend = CudaBackend::new(
                &model,
                cases::initial_state(&case),
                &params,
                7,
                HashMode::FinalOnly,
            )
            .expect("CUDA device, driver, and NVRTC are required");
            backend.validation_launch_override =
                (grid != 0).then_some(ValidationLaunchGeometry { grid, block });

            let error = backend.run(1).unwrap_err();
            assert!(
                error.to_string().contains(case.expected_cuda_error),
                "{} ({grid}x{block}): {error}",
                case.name
            );
            let words = backend
                .stream
                .memcpy_dtov(&backend.status)
                .expect("download validation status");
            let committed = [words[0], words[1], words[2], words[3]];
            assert_eq!(
                (committed[0], committed[1]),
                case.expected_status,
                "{} ({grid}x{block})",
                case.name
            );
            assert_eq!(
                committed,
                *first_status.get_or_insert(committed),
                "{} changed diagnostic under geometry {grid}x{block}",
                case.name
            );
            eprintln!(
                "diagnostic_case={} geometry={}x{} status={:?}",
                case.name, grid, block, committed
            );
        }
    }
}

#[test]
#[ignore = "requires a CUDA GPU; run crates/sembla-cuda/scripts/run-differential-corpus.sh"]
fn negative_corpus_matches_cpu_status_under_four_geometries() {
    if std::env::var_os(CHILD_ENV).is_some() {
        run_negative_corpus();
        return;
    }

    // Run the GPU body in a child process so a non-terminating CUDA kernel
    // becomes a bounded test failure. A thread-level timeout cannot recover
    // a process whose CUDA context is blocked in stream synchronization.
    let executable = std::env::current_exe().expect("locate current lib test binary");
    let test_name =
        "diagnostic_equality_hardware::negative_corpus_matches_cpu_status_under_four_geometries";
    let mut child = std::process::Command::new(executable)
        .arg("--exact")
        .arg(test_name)
        .arg("--ignored")
        .arg("--nocapture")
        .env(CHILD_ENV, "1")
        .stdout(std::process::Stdio::inherit())
        .stderr(std::process::Stdio::inherit())
        .spawn()
        .expect("spawn bounded CUDA diagnostic child");
    let deadline = std::time::Instant::now() + DEADLINE;
    loop {
        if let Some(status) = child.try_wait().expect("poll CUDA diagnostic child") {
            assert!(
                status.success(),
                "CUDA diagnostic child failed with {status}"
            );
            return;
        }
        if std::time::Instant::now() >= deadline {
            child.kill().expect("kill timed-out CUDA diagnostic child");
            let _ = child.wait();
            panic!(
                "CUDA diagnostic corpus exceeded its {}s internal deadline; probable kernel deadlock",
                DEADLINE.as_secs()
            );
        }
        std::thread::sleep(std::time::Duration::from_millis(100));
    }
}

const RECOVERY_ROWS: usize = 65_539;
const RECOVERY_FIRST_ERROR: usize = 65_530;

fn large_recovery_case(
    case: &cases::DiagnosticCase,
) -> (
    sembla_ir::ValidatedModel,
    Vec<sembla_runtime::core::TableInit>,
) {
    use sembla_runtime::core::ColumnData;
    let mut initial = cases::initial_state(case);
    let mut raw_model = cases::load_model(case).model().clone();
    for table in &mut initial {
        table.row_count = RECOVERY_ROWS;
        for column in &mut table.columns {
            match &mut column.data {
                ColumnData::Int(values) => {
                    let fails = values.contains(&i64::MAX);
                    values.fill(1);
                    values.resize(RECOVERY_ROWS, 1);
                    if fails {
                        values[RECOVERY_FIRST_ERROR] = i64::MAX;
                        values[RECOVERY_ROWS - 1] = i64::MAX;
                    }
                }
                ColumnData::Real(values) => values.resize(RECOVERY_ROWS, values[0]),
                ColumnData::Enum(values) => values.resize(RECOVERY_ROWS, values[0]),
                ColumnData::Ref(values) => values.resize(RECOVERY_ROWS, values[0]),
            }
        }
        let model_box = raw_model
            .boxes
            .iter_mut()
            .find(|b| b.name == table.box_name)
            .unwrap();
        model_box
            .tables
            .iter_mut()
            .find(|t| t.name == table.table_name)
            .unwrap()
            .size_hint = RECOVERY_ROWS as u64;
    }
    let model = sembla_ir::validate(raw_model).unwrap();
    (model, initial)
}

#[test]
fn large_recovery_inputs_match_cpu_error_and_rollback() {
    use sembla_runtime::core::StateStore;
    for case in cases::CASES {
        let (model, initial) = large_recovery_case(&case);
        let params = ParamEnv::defaults(&model);
        let mut cpu = StateStore::new(&model, initial).unwrap();
        let before = cpu.state_hash();
        let error = sembla_cpu::run_tick(&model, &mut cpu, &params, 7, 0).unwrap_err();
        assert!(
            error
                .to_string()
                .contains(&format!("row {RECOVERY_FIRST_ERROR}")),
            "{}: {error}",
            case.name
        );
        assert_eq!(before, cpu.state_hash());
    }
}

// Errors beyond 32 * 1024 threads require another grid-stride iteration in
// recovery passes. Compare every payload word with the full-grid arm.
fn check_recovery_beyond_grid() {
    use sembla_runtime::core::StateStore;
    for case in cases::CASES {
        let (model, initial) = large_recovery_case(&case);
        let params = ParamEnv::defaults(&model);
        let before = StateStore::new(&model, initial.clone())
            .unwrap()
            .state_hash();
        let mut expected_status = None;
        for full_grid in [true, false] {
            let mut backend =
                CudaBackend::new(&model, initial.clone(), &params, 7, HashMode::FinalOnly).unwrap();
            if full_grid {
                backend.validation_launch_override = Some(ValidationLaunchGeometry {
                    grid: (RECOVERY_ROWS as u32).div_ceil(1024),
                    block: 1024,
                });
            }
            backend.run(1).unwrap_err();
            let words = backend.stream.memcpy_dtov(&backend.status).unwrap();
            let committed = [words[0], words[1], words[2], words[3]];
            assert_eq!(
                committed,
                *expected_status.get_or_insert(committed),
                "{}",
                case.name
            );
            assert_eq!(
                backend.ensure_observed_state().unwrap().state_hash(),
                before
            );
        }
        eprintln!(
            "recovery_beyond_grid={} first_error={RECOVERY_FIRST_ERROR} rows={RECOVERY_ROWS}",
            case.name
        );
    }
}
