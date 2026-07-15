use std::path::PathBuf;

use sembla_precision_spike::{benchmark, results};

fn main() {
    if let Err(error) = pollster::block_on(run()) {
        eprintln!("precision benchmark failed: {error}");
        std::process::exit(1);
    }
}

async fn run() -> Result<(), String> {
    println!("Running one-tick accuracy regression guard for every available strategy...");
    benchmark::run_regression_guard().await?;
    println!("Accuracy guard passed; running 10 warmup + 100 measured ticks per strategy...");
    let run = benchmark::run_benchmark().await?;
    let path = std::env::var_os("SEMBLA_RESULTS_PATH")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("RESULTS.md"));
    results::update_results(&path, run)?;
    println!("Wrote {}", path.display());
    Ok(())
}
