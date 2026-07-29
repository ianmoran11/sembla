//! GPU-less structural coverage for CUDA host-state reuse.

fn section<'a>(source: &'a str, start: &str, end: &str) -> &'a str {
    source
        .split_once(start)
        .unwrap_or_else(|| panic!("missing start marker {start:?}"))
        .1
        .split_once(end)
        .unwrap_or_else(|| panic!("missing end marker {end:?}"))
        .0
}

#[test]
fn cuda_backend_retains_and_refreshes_one_host_state_store() {
    let backend = include_str!("../src/backend.rs");
    let fields = section(
        backend,
        "pub struct CudaBackend {",
        "\n}\n\nimpl CudaBackend",
    );
    assert!(fields.contains("host_state: StateStore"));
    assert!(fields.contains("host_tables: Vec<TableInit>"));

    let constructor = section(backend, "    pub fn new(", "\n    pub fn generated(&self)");
    assert!(constructor.contains("let host_state = StateStore::new"));
    assert!(constructor.contains("host_tables: initial_tables"));

    let reconstruction = section(
        backend,
        "    fn reconstruct_state_store(",
        "\n    fn download_hash(&self)",
    );
    assert!(reconstruction.contains("unpack_state_into("));
    assert!(reconstruction.contains("refresh_backend_snapshot"));
    assert!(!reconstruction.contains("StateStore::new"));

    let unpack = section(backend, "fn unpack_state_into(", "\nfn unpack_inputs(");
    assert!(unpack.contains("read_column_into("));
    let read_into = section(backend, "fn read_column_into(", "\nfn read_column(");
    assert!(read_into.contains("values.clear();"));
    assert!(read_into.contains("values.extend("));
}

#[test]
fn lockstep_spike_uses_nonblocking_streams_without_changing_the_default() {
    let backend = include_str!("../src/backend.rs");
    let constructor = section(backend, "    pub fn new(", "\n    pub fn generated(&self)");
    assert!(constructor.contains("Self::new_with_stream_mode("));
    assert!(constructor.contains("hash_mode, false"));
    assert!(constructor.contains("pub fn new_nonblocking_stream("));
    assert!(constructor.contains("hash_mode, true"));
    assert!(constructor.contains(".new_stream()"));
    assert!(constructor.contains("context.default_stream()"));

    let cli = include_str!("../../sembla-cli/src/main.rs");
    assert!(cli.contains("SEMBLA_SWEEP_SPIKE_CUDA_LOCKSTEP_STREAMS"));
    assert!(cli.contains("CudaBackend::new_nonblocking_stream("));
    assert!(cli.contains("backend.run_draw_lockstep("));
    assert!(cli.contains("let lockstep_tick = std::sync::Barrier::new(workers);"));
}

#[test]
fn fused_spike_uses_one_module_stream_and_grid_y_launch_path() {
    let backend = include_str!("../src/backend.rs");
    assert!(backend.contains("pub fn new_fused_batch("));
    assert!(backend.contains("generate_fused_batch(model)?"));
    assert!(backend.contains("context.default_stream()"));
    assert!(backend.contains("config.grid_dim.1 = self.grid_y;"));
    assert!(backend.contains(".arg(&batch.strides)"));
    assert!(backend.contains("pub fn reset_fused_batch("));
    assert!(backend.contains("pub fn run_tick_observed_reused_fused("));
    assert!(!backend.contains("Vec<CudaBackend>"));

    let batch_tick = section(
        backend,
        "    fn execute_tick_batch_statuses(",
        "\n    fn download_fused_state_stores(",
    );
    let ordinary_error = batch_tick
        .find("if self.fused_batch.is_none()")
        .expect("ordinary error guard exists");
    let state_swap = batch_tick
        .find("mem::swap(&mut self.state")
        .expect("state swap exists");
    assert!(ordinary_error < state_swap);

    let fused_tick = section(
        backend,
        "    pub fn run_tick_observed_reused_fused(",
        "\n    #[doc(hidden)]\n    pub fn fused_observed_state(",
    );
    assert!(fused_tick.contains("if active_width == 0"));
    assert!(fused_tick.contains("must be reset with at least one active draw"));

    let cli = include_str!("../../sembla-cli/src/main.rs");
    assert!(cli.contains("SEMBLA_SWEEP_SPIKE_CUDA_FUSED_DRAWS"));
    assert!(cli.contains("prepared.chunks(capacity)"));
    assert!(cli.contains("CudaBackend::new_fused_batch("));
    assert!(cli.contains("run_tick_observed_reused_fused()"));
    let fused_cli = section(
        cli,
        "#[cfg(feature = \"cuda\")]\nfn run_fused_sweep_spike(",
        "\n#[cfg(not(feature = \"cuda\"))]\nfn run_fused_sweep_spike(",
    );
    assert!(fused_cli.contains("failures[slot] = Some(format!"));
    assert!(fused_cli.contains(".finish(model, None)\n                    .and_then("));
    assert!(!fused_cli.contains(".finish(model, None)?"));
}

#[test]
fn host_ineligible_view_forces_state_download_while_device_views_skip_it() {
    let backend = include_str!("../src/backend.rs");
    let reused = section(
        backend,
        "    pub fn run_tick_observed_reused(&mut self)",
        "\n    /// Executes one observed CUDA tick and returns durations",
    );
    assert!(reused.contains("let views = self.observe_device_views(tick)?"));
    assert!(reused.contains("host_observation_fallback(views.is_none()"));
    assert!(reused.contains("self.download_state_store()"));

    let timed = section(
        backend,
        "    pub fn run_tick_observed_reused_timed(",
        "\n    /// Returns the backend-owned host snapshot",
    );
    assert!(timed.contains("let views = self.observe_device_views(tick)?"));
    assert!(timed.contains("host_observation_fallback("));
    assert!(timed.contains("views.is_none()"));
    assert!(timed.contains("self.download_state_parts()?"));
    assert!(timed.contains("(std::time::Duration::ZERO, std::time::Duration::ZERO)"));
}

#[test]
fn cuda_cli_uses_the_reused_state_path_and_moves_the_state_only_at_run_end() {
    let cli = include_str!("../../sembla-cli/src/main.rs");
    let cuda_run = section(
        cli,
        "#[cfg(feature = \"cuda\")]\nfn run_results_output_cuda(",
        "\n#[cfg(feature = \"cuda\")]\nfn run_results_output_cuda_timed(",
    );
    assert!(cuda_run.contains("run_tick_observed_reused()"));
    assert!(cuda_run.contains("backend.observed_state()"));
    assert_eq!(cuda_run.matches(".into_observed_state()").count(), 1);
    assert!(!cuda_run.contains("observation.state"));

    let timed = section(
        cli,
        "#[cfg(feature = \"cuda\")]\nfn run_results_output_cuda_timed(",
        "\nfn summaries_csv(",
    );
    assert!(timed.contains("run_tick_observed_reused_timed()"));
    assert!(timed.contains("backend.observed_state()"));
    assert_eq!(timed.matches(".into_observed_state()").count(), 1);
    assert!(!timed.contains("observation.state"));
}
