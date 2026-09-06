//! Optional command-level timings, separate from scientific output artifacts.

use super::*;

pub(super) fn check_path(
    path: &str,
    options: &RunOptions,
    model: &sembla_ir::ValidatedModel,
) -> Result<(), String> {
    let Some(report) = options.lifecycle_timing_json.as_deref() else {
        return Ok(());
    };
    let mut protected = vec![PathBuf::from(path), PathBuf::from(&options.population)];
    protected.extend(options.params.iter().map(PathBuf::from));
    protected.extend(options.export_state.iter().map(PathBuf::from));
    protected.extend(options.timing_json.iter().map(PathBuf::from));
    if let Some(out) = options.out.as_deref() {
        protected.extend([
            PathBuf::from(out),
            summaries_path(out),
            manifest::sidecar_path(out),
        ]);
        protected.extend(
            model
                .model()
                .boxes
                .iter()
                .flat_map(|b| b.grouped_views.iter())
                .map(|v| grouped_output_path(Path::new(out), &v.name)),
        );
    }
    for protected in protected {
        if paths_resolve_to_same_file(Path::new(report), &protected) {
            return Err(format!(
                "--lifecycle-timing-json path '{report}' conflicts with '{}'",
                protected.display()
            ));
        }
    }
    Ok(())
}

pub(super) fn write(
    path: &str,
    backend: BackendSelection,
    phases: [Duration; 3],
    elapsed: Duration,
    #[cfg(feature = "cuda")] construction_timing: Option<CudaConstructionTiming>,
) -> Result<(), String> {
    let mut document = serde_json::json!({
        "schema": "sembla-run-lifecycle-timing-v1",
        "scope": "command after option parsing, excluding lifecycle report write",
        "backend": match backend { BackendSelection::Cpu => "cpu", BackendSelection::Cuda => "cuda" },
        "total_ms": duration_ms(phases.iter().copied().sum()),
        "phases_ms": {
            "input_and_model_preparation": duration_ms(phases[0]),
            "backend_construction_execution_and_materialization": duration_ms(phases[1]),
            "hashing_export_and_publication": duration_ms(phases[2]),
        },
        "backend_execution_ms": duration_ms(elapsed),
        "cuda_construction_ms": null,
    });
    #[cfg(feature = "cuda")]
    if let Some(c) = construction_timing {
        document["cuda_construction_ms"] = serde_json::json!({
            "total": duration_ms(c.total), "availability": duration_ms(c.availability),
            "host_state_validation": duration_ms(c.host_state_validation),
            "code_generation": duration_ms(c.code_generation),
            "context_and_identity": duration_ms(c.context_and_identity),
            "nvrtc_compile_or_cache_lookup": duration_ms(c.nvrtc_compile),
            "module_and_stream": duration_ms(c.module_and_stream),
            "function_lookup": duration_ms(c.function_lookup),
            "layout_and_pack": duration_ms(c.layout_and_pack),
            "device_allocation_and_upload": duration_ms(c.device_allocation_and_upload),
            "other": duration_ms(c.other),
        });
        document["nvrtc_cache_hit"] = c.nvrtc_cache_hit.into();
    }
    // Keep the same shape on CPU-only builds without inventing CUDA timings.
    let _ = &mut document;
    write_atomic(
        path,
        (serde_json::to_string_pretty(&document).map_err(|e| e.to_string())? + "\n").as_bytes(),
    )
}
