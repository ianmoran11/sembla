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
    assert!(cli.contains(".run_draw_lockstep("));
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
fn free_stream_spike_uses_nonblocking_streams_without_tick_barriers() {
    let cli = include_str!("../../sembla-cli/src/main.rs");
    assert!(cli.contains("SEMBLA_SWEEP_SPIKE_CUDA_FREE_STREAMS"));
    assert!(cli.contains("SweepConcurrencyMode::CudaFreeNonblocking"));

    // Only the lockstep mode enters the tick-barrier branch; free-running
    // non-blocking lanes share the dynamic claim-based scheduler with the
    // ordinary independent mode.
    assert!(cli.contains("if mode == SweepConcurrencyMode::CudaLockstepNonblocking {"));
    let scheduler = section(
        cli,
        "if mode == SweepConcurrencyMode::CudaLockstepNonblocking {",
        "Ok((identity, completed))",
    );
    assert!(scheduler.contains("} else {"));
    assert!(scheduler.contains("next_draw.fetch_add("));
    assert!(scheduler.contains(".run_draw("));

    // Both CUDA stream modes share the non-blocking constructor; independent
    // mode keeps the default stream.
    let lane_ctor = section(cli, "    fn new_concurrency_lane(", "\n    fn identity(");
    assert!(lane_ctor.contains("if mode == SweepConcurrencyMode::IndependentDefaultStreams {"));
    assert!(lane_ctor.contains("CudaBackend::new_nonblocking_stream("));
    assert!(cli.contains("\"cuda-free-nonblocking-streams\""));
    assert!(cli.contains("\"cuda-lockstep-nonblocking-streams\""));
    assert!(cli.contains("\"independent-backends\""));
}

#[test]
fn final_state_diagnostic_reuses_one_hash_and_retains_synchronized_pinned_buffers() {
    let backend = include_str!("../src/backend.rs");
    let method = section(
        backend,
        "    pub fn final_state_readback(",
        "\n    /// Returns the once-per-run IR eligibility decision",
    );
    assert!(method.contains("CudaFinalStateReadbackMode::Materialized"));
    assert!(method.contains("CudaFinalStateReadbackMode::PackedPageable"));
    assert!(method.contains("CudaFinalStateReadbackMode::PackedPinned"));
    assert!(method.contains("self.download_state_parts()?"));
    assert!(method.matches("hash_state(").count() >= 2);
    assert!(method.contains(".memcpy_dtoh("));
    assert!(method.contains("wait_until_readable()?"));
    assert!(method.contains(".stage()?"));
    assert!(!method.contains("new_stream("));
    assert!(!method.contains("fork("));
    assert!(!method.contains("final_state_hash_device"));
    assert!(!backend.contains("struct DeviceHashPlan"));
    assert!(!backend.contains("hash_digest: CudaSlice"));

    let cli = include_str!("../../sembla-cli/src/main.rs");
    assert!(cli.contains("SEMBLA_SWEEP_CUDA_FINAL_STATE_MODE"));
    assert_eq!(cli.matches(".final_state_readback(").count(), 2);
    assert!(cli.contains("sembla-cuda-final-state-readback-v2"));

    let fields = section(
        backend,
        "pub struct CudaBackend {",
        "\n}\n\nimpl CudaBackend",
    );
    assert!(fields.contains("pinned_final_state: Option<PinnedFinalStateBuffers>"));
    let owner = section(
        backend,
        "struct PinnedFinalStateBuffers {",
        "\nfn checked_final_state_component_bytes(",
    );
    assert!(owner.contains("stream: std::sync::Arc<CudaStream>"));
    assert!(owner.contains("self.stream.synchronize()"));
    assert!(owner.contains("impl Drop for PinnedFinalStateBuffers"));
    let constructor = section(
        backend,
        "impl<T> PinnedFinalStateComponent<T>",
        "\n#[derive(Debug)]\nstruct PinnedFinalStateBuffers",
    );
    assert_eq!(constructor.matches("alloc_pinned::<T>").count(), 1);
    assert!(constructor.contains("if len == 0"));
    assert!(constructor.contains("return Ok(None)"));
    assert!(constructor.contains("SAFETY: cudarc returns uninitialized page-locked memory"));
    assert!(constructor.contains("never exposes or reads it until"));
    assert_eq!(method.matches("if let Some(destination)").count(), 3);
    assert!(owner.contains("map_or(&[], |component| component.cacheable.as_slice())"));
    assert!(!backend.contains("cuMemHostAlloc"));
    assert!(!backend.contains("device snapshot"));
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
