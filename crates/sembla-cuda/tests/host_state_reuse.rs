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
fn cuda_cli_uses_the_reused_state_path_and_moves_the_state_only_at_run_end() {
    let cli = include_str!("../../sembla-cli/src/main.rs");
    let cuda_run = section(
        cli,
        "#[cfg(feature = \"cuda\")]\nfn run_results_output_cuda(",
        "\n#[cfg(feature = \"cuda\")]\nfn run_results_output_cuda_timed(",
    );
    assert!(cuda_run.contains("run_tick_observed_reused()"));
    assert!(cuda_run.contains("backend.observed_state()"));
    assert_eq!(cuda_run.matches("backend.into_observed_state()").count(), 1);
    assert!(!cuda_run.contains("observation.state"));

    let timed = section(
        cli,
        "#[cfg(feature = \"cuda\")]\nfn run_results_output_cuda_timed(",
        "\nfn summaries_csv(",
    );
    assert!(timed.contains("run_tick_observed_reused_timed()"));
    assert!(timed.contains("backend.observed_state()"));
    assert_eq!(timed.matches("backend.into_observed_state()").count(), 1);
    assert!(!timed.contains("observation.state"));
}
