//! GPU-less acceptance coverage for device-side control-report counts.

use sembla_cuda::generate;

fn sir_model() -> sembla_ir::ValidatedModel {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples/sir.json");
    let source = std::fs::read_to_string(path).unwrap();
    sembla_ir::validate(sembla_ir::parse_json(&source).unwrap()).unwrap()
}

fn section<'a>(source: &'a str, start: &str, end: &str) -> &'a str {
    source
        .split_once(start)
        .unwrap_or_else(|| panic!("missing start marker {start:?}"))
        .1
        .split_once(end)
        .unwrap_or_else(|| panic!("missing end marker {end:?}"))
        .0
}

fn kernel_body<'a>(source: &'a str, name: &str) -> &'a str {
    let marker = format!("extern \"C\" __global__ void {name}(");
    let start = source
        .find(&marker)
        .unwrap_or_else(|| panic!("kernel {name} missing from emitted source"));
    let rest = &source[start..];
    let end = rest.find("\n}\n").map_or(rest.len(), |index| index + 2);
    &rest[..end]
}

#[test]
fn generated_control_counts_use_resident_segmented_and_strided_reductions() {
    let generated = generate(&sir_model()).unwrap();

    let init = kernel_body(&generated.source, "sembla_init_control_counts");
    assert!(init.contains("fired_counts[rule] = 0ULL"));
    assert!(init.contains("deferred_counts[table] = 0ULL"));

    let fired = kernel_body(&generated.source, "sembla_count_fired");
    assert!(fired.contains("begin = candidate_offsets[rule]"));
    assert!(fired.contains("candidate_offsets[rule + 1ULL] : candidate_count"));
    assert!(fired.contains("local += wins[candidate] != 0U"));
    assert!(fired.contains("atomicAdd(fired_counts + rule, partials[0])"));

    let deferred = kernel_body(&generated.source, "sembla_count_deferred");
    assert!(deferred.contains("deferred[candidate * table_count + table] != 0U"));
    assert!(deferred.contains("atomicAdd(deferred_counts + table, partials[0])"));

    for kernel in [fired, deferred] {
        assert!(kernel.contains("gridDim.x * blockDim.x"));
        assert!(kernel.contains("extern __shared__ unsigned long long partials[]"));
        assert!(!kernel.contains("atomicCAS"));
        assert!(!kernel.contains("while ("));
    }
}

#[test]
fn tick_path_downloads_only_compact_counts_and_reuses_resident_offsets() {
    let backend = include_str!("../src/backend.rs");
    assert!(!backend.contains("memcpy_dtov(&self.wins)"));
    assert!(!backend.contains("memcpy_dtov(&self.deferred)"));

    let readback = section(
        backend,
        "    fn readback_control(&self)",
        "\n    /// Evaluates checked coordinate Philox vectors",
    );
    assert!(readback.contains("memcpy_dtov(&self.fired_counts)"));
    assert!(readback.contains("memcpy_dtov(&self.deferred_counts)"));
    assert!(!readback.contains("self.wins"));
    assert!(!readback.contains("self.deferred)"));

    let tick = section(
        backend,
        "    fn execute_tick(&mut self)",
        "\n    fn download_state_store(&mut self)",
    );
    assert!(tick.contains("launch_builder(&self.count_fired)"));
    assert!(tick.contains("launch_builder(&self.count_deferred)"));
    assert!(tick.contains(".arg(&self.candidate_offsets)"));
    assert!(!tick.contains("memcpy_htod"));
    assert!(
        tick.find("launch_builder(&self.count_deferred)").unwrap()
            < tick.find("memcpy_dtov(&self.status)").unwrap()
    );
}

#[test]
fn timed_and_untimed_paths_share_compact_readback_and_report_conversion() {
    let backend = include_str!("../src/backend.rs");
    let untimed = section(
        backend,
        "    pub fn run_tick_observed_reused(&mut self)",
        "\n    /// Executes one observed CUDA tick and returns durations",
    );
    let timed = section(
        backend,
        "    pub fn run_tick_observed_reused_timed(",
        "\n    /// Returns the backend-owned host snapshot",
    );
    for path in [untimed, timed] {
        assert_eq!(path.matches("self.readback_control()?").count(), 1);
        assert_eq!(path.matches("control_reports_from_counts(").count(), 1);
    }
    assert!(timed.contains("let readback_control = started.elapsed()"));
    assert!(timed.contains("let report = started.elapsed()"));
    assert!(timed.contains("[\n                kernels,\n                readback_control,"));
}
