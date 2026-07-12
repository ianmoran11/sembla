use std::path::PathBuf;

use sembla_gpu_throughput_spike::{write_results, GpuTick, TARGET_GROUPS, TARGET_ROWS};

fn main() {
    if let Err(error) = run() {
        eprintln!("GPU throughput spike failed: {error}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), String> {
    let requested_rows = std::env::var("SEMBLA_GPU_ROWS")
        .ok()
        .map(|value| {
            value
                .parse::<u32>()
                .map_err(|e| format!("invalid SEMBLA_GPU_ROWS: {e}"))
        })
        .transpose()?
        .unwrap_or(TARGET_ROWS);
    let requested_groups = if requested_rows == TARGET_ROWS {
        TARGET_GROUPS
    } else {
        (requested_rows / 20).max(1)
    };
    let (gpu, downscale_reason) =
        pollster::block_on(GpuTick::new(requested_rows, requested_groups))?;
    let adapter = gpu.adapter_description();
    println!(
        "adapter: {} ({}, {})",
        adapter.name, adapter.backend, adapter.device_type
    );
    if adapter.software {
        println!(
            "SOFTWARE ADAPTER: benchmark will be reduced and throughput verdict will be unanswered"
        );
    }
    if let Some(reason) = &downscale_reason {
        println!("downscaled: {reason}");
    }
    println!("warming up 10 ticks, then measuring 100 ticks...");
    let result = gpu.benchmark(10, 100, downscale_reason);
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("RESULTS.md");
    write_results(&path, &result)
        .map_err(|e| format!("failed to write {}: {e}", path.display()))?;
    println!(
        "median total: {:.4} ms/tick ({:.3} million rows/sec)",
        result.total_ms,
        result.rows_per_second / 1_000_000.0
    );
    println!("wrote {}", path.display());
    Ok(())
}
