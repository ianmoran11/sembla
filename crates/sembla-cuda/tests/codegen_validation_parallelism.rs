#![cfg(feature = "cuda")]
//! GPU-less acceptance tests for PRD 0002 (parallel validation kernels).
//!
//! The four per-row validation kernels must execute across the device and
//! report the minimum failing candidate index for every launch geometry.
//! These tests assert on the emitted CUDA source and exercise a host-side
//! mirror of the device reduction protocol; no GPU is required.

use sembla_cuda::generate;

/// One model exercising every parallelised validation path: a checked guard
/// (transition), a contested resource with a checked key (claims), checked
/// and enum/ref-typed effects (effects), and a wired Int output with a
/// checked value (outputs plus the ordered fold).
fn parallel_validation_model() -> sembla_ir::ValidatedModel {
    let source = r#"{"name":"parallel_validation","dt":1.0,"params":[],"boxes":[{"name":"world","tables":[{"name":"Person","size_hint":2,"attrs":[{"name":"state","ty":{"kind":"enum","variants":["Off","On"]}},{"name":"x","ty":{"kind":"int"}},{"name":"priority","ty":{"kind":"int"}},{"name":"mate","ty":{"kind":"ref","table":"Person"}}]}],"transitions":[{"name":"act","table":"Person","guard":{"kind":"gt","lhs":{"kind":"mul","lhs":{"kind":"self_attr","name":"x"},"rhs":{"kind":"int","value":2}},"rhs":{"kind":"int","value":0}},"hazard":{"kind":"real","value":1.0},"effects":[{"kind":"set_attr","attr":"x","value":{"kind":"mul","lhs":{"kind":"self_attr","name":"x"},"rhs":{"kind":"int","value":2}}},{"kind":"set_attr","attr":"state","value":{"kind":"enum","variant":"On"}},{"kind":"set_attr","attr":"mate","value":{"kind":"self_attr","name":"mate"}}],"contests":[{"resource":{"kind":"self_attr","name":"mate"},"ordering":{"kind":"key","expr":{"kind":"mul","lhs":{"kind":"self_attr","name":"priority"},"rhs":{"kind":"int","value":2}}}}]}],"inputs":[],"outputs":[{"name":"totals","schema":[{"name":"total","ty":{"kind":"int"}}],"builder":{"kind":"per_table","table":"Person","fields":[{"name":"total","op":{"kind":"sum","value":{"kind":"mul","lhs":{"kind":"self_attr","name":"x"},"rhs":{"kind":"int","value":3}}},"filter":{"kind":"gt","lhs":{"kind":"self_attr","name":"x"},"rhs":{"kind":"int","value":0}}}]}}],"views":[]},{"name":"sink","tables":[],"transitions":[],"inputs":[{"name":"totals","schema":[{"name":"total","ty":{"kind":"int"}}]}],"outputs":[],"views":[]}],"wires":[{"from":{"box":"world","port":"totals"},"to":{"box":"sink","port":"totals"}}],"summaries":[]}"#;
    sembla_ir::validate(sembla_ir::parse_json(source).unwrap()).unwrap()
}

/// Extracts one kernel's body (signature included) from the emitted source.
/// Generated kernels close with a brace at column 0; every nested brace is
/// indented, so the first `\n}\n` after the signature ends the body.
fn kernel_body<'a>(source: &'a str, name: &str) -> &'a str {
    let marker = format!("extern \"C\" __global__ void {name}(");
    let start = source
        .find(&marker)
        .unwrap_or_else(|| panic!("kernel {name} missing from emitted source"));
    let rest = &source[start..];
    let end = rest
        .find("\n}\n")
        .map(|index| index + 2)
        .unwrap_or(rest.len());
    &rest[..end]
}

const PARALLEL_KERNELS: [&str; 4] = [
    "sembla_validate_claims",
    "sembla_validate_transition",
    "sembla_validate_effects",
    "sembla_validate_outputs",
];

/// The genuinely single-threaded kernels named by the PRD, plus the two new
/// O(1) status-protocol helpers, all launched with the `one` config.
const SERIAL_KERNELS: [&str; 10] = [
    "sembla_reset_status",
    "sembla_check_candidate_errors",
    "sembla_record_aggregate_errors",
    "sembla_check_output_errors",
    "sembla_mark_effect_aggregates",
    "sembla_prepare_effects",
    "sembla_prepare_outputs",
    "sembla_validate_claim_compatibility",
    "sembla_init_validation_scratch",
    "sembla_commit_validation_status",
];

#[test]
fn parallel_validation_kernels_drop_the_single_thread_guard_and_grid_stride() {
    let generated = generate(&parallel_validation_model()).unwrap();
    for kernel in PARALLEL_KERNELS {
        let body = kernel_body(&generated.source, kernel);
        assert!(
            !body.contains("blockIdx.x != 0"),
            "{kernel} must not retain the single-thread guard"
        );
        assert!(
            body.contains("row += (unsigned long long)gridDim.x * blockDim.x"),
            "{kernel} must grid-stride its row loop with gridDim.x * blockDim.x"
        );
        assert!(
            body.contains("if (status[0] != 0ULL) return;"),
            "{kernel} keeps only the cross-launch short-circuit guard"
        );
    }
}

#[test]
fn parallel_validation_kernels_report_only_through_the_reduction() {
    let generated = generate(&parallel_validation_model()).unwrap();
    for kernel in PARALLEL_KERNELS {
        let body = kernel_body(&generated.source, kernel);
        assert!(
            !body.contains("status[0] ="),
            "{kernel} workers must never write the committed code"
        );
        assert!(
            !body.contains("status[1] ="),
            "{kernel} must contain no bare candidate assignment"
        );
        assert!(
            body.contains("sembla_record_validation_failure(status,"),
            "{kernel} must route failures through the reduction"
        );
    }
    let prelude = &generated.source[..generated
        .source
        .find("extern \"C\" __global__ void")
        .unwrap()];
    assert!(
        prelude.contains("atomicMin(status + 6, candidate)"),
        "the reduction accumulates the minimum candidate with atomicMin"
    );
}

#[test]
fn serial_status_kernels_keep_the_single_thread_guard() {
    let generated = generate(&parallel_validation_model()).unwrap();
    for kernel in SERIAL_KERNELS {
        let body = kernel_body(&generated.source, kernel);
        assert!(
            body.contains("blockIdx.x != 0"),
            "{kernel} must retain the single-thread guard"
        );
    }
}

#[test]
fn commit_publishes_candidate_before_code_and_scratch_is_initialised() {
    let generated = generate(&parallel_validation_model()).unwrap();
    let commit = kernel_body(&generated.source, "sembla_commit_validation_status");
    let candidate = commit.find("status[1] = status[6];").unwrap();
    let code = commit.find("status[0] = status[7];").unwrap();
    assert!(
        candidate < code,
        "the candidate must be published before the code"
    );
    assert!(
        commit.contains("if (status[0] != 0ULL || status[5] == 0xffffffffffffffffULL) return;"),
        "commit defers to a prior launch's failure and to empty scratch"
    );
    let init = kernel_body(&generated.source, "sembla_init_validation_scratch");
    assert!(init.contains("status[5] = 0xffffffffffffffffULL;"));
    assert!(init.contains("status[6] = 0xffffffffffffffffULL;"));
    assert!(init.contains("effect_active[i] = 0U;"));
}

#[test]
fn effect_liveness_uses_the_parallel_activity_prepass() {
    let generated = generate(&parallel_validation_model()).unwrap();
    let effects = kernel_body(&generated.source, "sembla_validate_effects");
    assert!(
        !effects.contains("any_winner"),
        "per-thread winner rescans are incorrect; the flag must be precomputed"
    );
    assert!(effects.contains("effect_active["));
    let marker = kernel_body(&generated.source, "sembla_mark_effect_active");
    assert!(marker.contains("atomicOr(effect_active + rule_id, 1U);"));
    assert!(marker.contains("(unsigned long long)blockIdx.x * blockDim.x + threadIdx.x"));
}

#[test]
fn ordered_output_fold_stays_on_one_worker() {
    let generated = generate(&parallel_validation_model()).unwrap();
    let outputs = kernel_body(&generated.source, "sembla_validate_outputs");
    assert!(
        outputs.contains("if (validation_worker == 0ULL) for (unsigned long long row = 0;"),
        "the ordered checked fold is the documented single-worker exception"
    );
    assert!(
        !outputs.contains("status[0] ="),
        "fold failures also route through the reduction"
    );
}

// ---------------------------------------------------------------------------
// Host-side mirror of the device reduction protocol (acceptance criterion 8
// in its local form; the GPU run later confirms it on hardware).
// ---------------------------------------------------------------------------

const EMPTY: u64 = u64::MAX;

/// Mirrors the scratch slots status[4..=8]: (scan, candidate, code, branch).
/// The lock is elided because the host drives records sequentially, exactly
/// as serialised critical sections would interleave on device.
#[derive(Default)]
struct Scratch {
    scan: u64,
    candidate: u64,
    code: u64,
    branch: u64,
}

impl Scratch {
    fn new() -> Self {
        Scratch {
            scan: EMPTY,
            candidate: EMPTY,
            code: 0,
            branch: EMPTY,
        }
    }

    /// Faithful port of `sembla_record_validation_failure`.
    fn record(&mut self, code: u64, candidate: u64, scan: u64, branch: u64) {
        if scan < self.scan {
            self.scan = scan;
            self.code = code;
            self.branch = branch;
            self.candidate = candidate; // atomicExch reset for the new scan
        } else if scan == self.scan {
            let previous = self.candidate.min(candidate); // atomicMin
            if candidate < self.candidate || (candidate == self.candidate && branch < self.branch) {
                self.code = code;
                self.branch = branch;
            }
            self.candidate = previous;
        }
    }

    /// Faithful port of `sembla_commit_validation_status` for a clean status.
    fn commit(&self) -> Option<(u64, u64)> {
        (self.scan != EMPTY).then_some((self.code, self.candidate))
    }
}

/// One failure observed by one worker: (code, candidate, scan, branch).
type Failure = (u64, u64, u64, u64);

/// Reduces `failures` in the given interleaving, as device workers would.
fn reduce_in_order(failures: impl IntoIterator<Item = Failure>) -> Option<(u64, u64)> {
    let mut scratch = Scratch::new();
    for (code, candidate, scan, branch) in failures {
        scratch.record(code, candidate, scan, branch);
    }
    scratch.commit()
}

/// The serial validator's answer: the first failure in (scan, candidate,
/// branch) order, keeping the matching code.
fn serial_expectation(failures: &[Failure]) -> Option<(u64, u64)> {
    failures
        .iter()
        .min_by_key(|(_, candidate, scan, branch)| (*scan, *candidate, *branch))
        .map(|(code, candidate, _, _)| (*code, *candidate))
}

/// All permutations of up to 8 items (plenty for adversarial interleavings).
fn permutations(items: &[Failure]) -> Vec<Vec<Failure>> {
    fn go(prefix: &mut Vec<Failure>, rest: &mut Vec<Failure>, out: &mut Vec<Vec<Failure>>) {
        if rest.is_empty() {
            out.push(prefix.clone());
            return;
        }
        for index in 0..rest.len() {
            let item = rest.remove(index);
            prefix.push(item);
            go(prefix, rest, out);
            prefix.pop();
            rest.insert(index, item);
        }
    }
    assert!(items.len() <= 8, "permutation test keeps sets small");
    let mut out = Vec::new();
    go(&mut Vec::new(), &mut items.to_vec(), &mut out);
    out
}

fn assert_geometry_invariant(failures: &[Failure]) {
    let expected = serial_expectation(failures);
    for ordering in permutations(failures) {
        assert_eq!(
            reduce_in_order(ordering.iter().copied()),
            expected,
            "interleaving {ordering:?} must report the serial first failure"
        );
    }
}

#[test]
fn reduction_reports_minimum_candidate_within_the_first_failing_scan() {
    // One scan, failures at candidates {5, 42, 100}: min wins for every
    // interleaving (the PRD's {a, b, c} -> min(a, b, c) requirement).
    assert_geometry_invariant(&[(3, 100, 0, 0), (3, 5, 0, 0), (3, 42, 0, 0)]);
}

#[test]
fn reduction_preserves_scan_precedence_over_candidate_magnitude() {
    // A later scan's smaller candidate must not beat an earlier scan's
    // larger one: the serial validator returns from the first failing scan.
    assert_geometry_invariant(&[(3, 100, 0, 0), (10, 5, 1, 0)]);
    assert_eq!(
        reduce_in_order([(3, 100, 0, 0), (10, 5, 1, 0)]),
        Some((3, 100))
    );
}

#[test]
fn reduction_keeps_code_paired_with_candidate_and_branch() {
    // Same scan, same candidate: the earlier per-row branch wins, with its
    // code (an evaluation error, code 5, beats a range error, code 6).
    assert_geometry_invariant(&[(6, 7, 2, 1), (5, 7, 2, 0)]);
    // Same scan, different candidates: the smaller candidate wins with *its*
    // branch's code, even when a larger candidate carries an earlier branch.
    assert_geometry_invariant(&[(5, 9, 2, 0), (6, 3, 2, 1)]);
    assert_eq!(reduce_in_order([(5, 9, 2, 0), (6, 3, 2, 1)]), Some((6, 3)));
}

#[test]
fn reduction_matches_serial_under_simulated_launch_geometries() {
    // Distribute 237 rows over several (grid, block) geometries, fail a
    // fixed set of rows in one scan, and check the committed result each
    // time. Workers process their strided rows in order; block scheduling
    // interleaves workers arbitrarily, which the permutations above cover.
    let rows: Vec<u64> = (0..237).collect();
    let failing: [u64; 3] = [11, 190, 77];
    let expected = Some((3, 11));
    for (grid, block) in [(1, 1), (1, 256), (2, 128), (7, 35), (1024, 1)] {
        let workers = grid * block;
        let mut per_worker: Vec<Vec<Failure>> = vec![Vec::new(); workers];
        for (worker, chunk) in per_worker.iter_mut().enumerate() {
            for row in rows.iter().skip(worker).step_by(workers) {
                if failing.contains(row) {
                    chunk.push((3, *row, 0, 0));
                }
            }
        }
        // Any worker interleaving yields the same commit; fold workers in
        // both orders as representative interleavings.
        let forward = per_worker.iter().flatten().copied();
        assert_eq!(reduce_in_order(forward), expected);
        let reverse = per_worker.iter().rev().flatten().copied();
        assert_eq!(reduce_in_order(reverse), expected);
    }
}

#[test]
fn empty_launch_commits_nothing() {
    assert_eq!(reduce_in_order([]), None);
}
