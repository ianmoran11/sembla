//! GPU-less CPU oracle coverage for the validation-diagnostic corpus.
//!
//! CPU execution does not emit CUDA's numeric status array. Each case freezes
//! the CUDA code/identity mapping separately, while the CPU oracle proves the
//! semantic failure class and earliest failing source row.

#[path = "support/diagnostic_cases.rs"]
mod diagnostic_cases;

use diagnostic_cases::{initial_state, load_model, CaseKind, CASES, FAILING_ROWS, GEOMETRIES};
use sembla_runtime::eval::ParamEnv;
use sembla_runtime::executor::run_tick;
use sembla_runtime::state::StateStore;

#[test]
fn negative_corpus_has_known_nonzero_multiple_failures() {
    assert_eq!(CASES.len(), 4);
    assert!(FAILING_ROWS.len() >= 3);
    assert_ne!(FAILING_ROWS[0], 0);
    assert!(FAILING_ROWS.windows(2).all(|rows| rows[0] < rows[1]));

    assert_eq!(GEOMETRIES, [(1, 1), (1, 32), (3, 4)]);
    for case in CASES {
        let _ = load_model(&case);
        assert_eq!(initial_state(&case)[0].row_count, 8, "{}", case.name);
        assert!(!case.expected_cuda_error.is_empty(), "{}", case.name);
    }
}

#[test]
fn cpu_oracle_rejects_every_case_at_the_expected_first_row() {
    for case in CASES {
        let model = load_model(&case);
        let params = ParamEnv::defaults(&model);
        for seed in [0, 7, u64::MAX] {
            let mut state = StateStore::new(&model, initial_state(&case)).unwrap();
            let error = run_tick(&model, &mut state, &params, seed, 0)
                .unwrap_err()
                .to_string();
            assert_eq!(
                error,
                format!("expression evaluation failed: {}", case.expected_cpu_error),
                "{} seed {seed}",
                case.name
            );
        }

        // This normalization is an explicit test contract, not a claim that
        // TickError itself contains CUDA status words.
        let expected = match case.kind {
            CaseKind::ClaimKey => (10, 2),
            CaseKind::TransitionGuard => (3, 2),
            CaseKind::Effect => (5, 2),
            CaseKind::Output => (9, 1),
        };
        assert_eq!(case.expected_status, expected, "{}", case.name);
    }
}
