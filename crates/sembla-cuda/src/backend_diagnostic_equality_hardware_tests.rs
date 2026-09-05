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
    for case in cases::CASES {
        assert!(!case.expected_cpu_error.is_empty(), "{}", case.name);
        let model = cases::load_model(&case);
        let params = ParamEnv::defaults(&model);
        let mut first_status = None;

        for (grid, block) in cases::GEOMETRIES {
            let mut backend = CudaBackend::new(
                &model,
                cases::initial_state(&case),
                &params,
                7,
                HashMode::FinalOnly,
            )
            .expect("CUDA device, driver, and NVRTC are required");
            backend.validation_launch_override = Some(ValidationLaunchGeometry { grid, block });

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
