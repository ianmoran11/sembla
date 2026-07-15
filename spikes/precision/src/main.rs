use sembla_precision_spike::{
    cuda::cuda_status, native_f64::probe_native_f64, probe_default_adapter, DEFAULT_GROUPS,
    DEFAULT_ROWS,
};

fn main() {
    if let Err(error) = run() {
        eprintln!("precision spike adapter probe failed: {error}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), String> {
    let probe = pollster::block_on(probe_default_adapter(DEFAULT_ROWS, DEFAULT_GROUPS))
        .map_err(|error| error.to_string())?;
    println!(
        "adapter: {} (backend: {}, device type: {})",
        probe.name, probe.backend, probe.device_type
    );
    println!(
        "SHADER_F64: {}",
        if probe.shader_f64 {
            "supported"
        } else {
            "unsupported"
        }
    );
    println!(
        "safe workload: N={} person rows, G={} employer groups (estimated resident columns: {} bytes)",
        probe.sizing.rows, probe.sizing.groups, probe.sizing.estimated_resident_bytes
    );
    println!(
        "downscale reason: {}",
        probe
            .sizing
            .downscale_reason
            .as_deref()
            .unwrap_or("none; the full requested workload fits the sizing limits")
    );
    println!("{}", pollster::block_on(probe_native_f64()));
    println!("{}", cuda_status());
    Ok(())
}
