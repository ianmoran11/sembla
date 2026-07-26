//! Spike: what would buffer reuse actually buy?
//!
//! PRD 0004 removed a provable 8 MB-per-operand copy, watched the allocator
//! symbols drop ~20% in the profile, and measured no time change at all. That
//! killed the argument for buffer pooling *as it was then framed* — reducing
//! allocation counts.
//!
//! But the post-0004 profile still shows `madvise` at 343 samples, `_nanov2_free`
//! at 241 and malloc paths at ~167, which is a different mechanism: buffers of
//! this size go to the large allocator, get returned to the OS on free, and are
//! re-faulted on next use. Page churn is not the same thing as allocation count,
//! and PRD 0004 did not test it.
//!
//! This measures the two directly:
//!
//!   fresh — allocate, fill, consume, drop. What the evaluator does per node.
//!   reuse — one buffer, cleared and refilled. What pooling would achieve.
//!
//! Identical work in both arms; only the allocation lifetime differs.
//!
//!   cargo run --release -p sembla-runtime --example alloc_spike -- [rows] [iters]

use std::time::Instant;

/// Consume the buffer so neither arm can be optimised away.
fn consume(values: &[f64]) -> f64 {
    values.iter().step_by(4096).sum()
}

fn fresh(rows: usize, iters: usize) -> (f64, f64) {
    let start = Instant::now();
    let mut sink = 0.0;
    for i in 0..iters {
        let buffer: Vec<f64> = (0..rows).map(|r| (r ^ i) as f64).collect();
        sink += consume(&buffer);
        // dropped here: the allocator may return the pages to the OS
    }
    (start.elapsed().as_secs_f64() * 1000.0, sink)
}

fn reuse(rows: usize, iters: usize) -> (f64, f64) {
    let mut buffer: Vec<f64> = Vec::with_capacity(rows);
    let start = Instant::now();
    let mut sink = 0.0;
    for i in 0..iters {
        buffer.clear();
        buffer.extend((0..rows).map(|r| (r ^ i) as f64));
        sink += consume(&buffer);
    }
    (start.elapsed().as_secs_f64() * 1000.0, sink)
}

fn main() {
    let mut args = std::env::args().skip(1);
    let rows: usize = args
        .next()
        .and_then(|a| a.parse().ok())
        .unwrap_or(5_000_000);
    let iters: usize = args.next().and_then(|a| a.parse().ok()).unwrap_or(60);

    let mb = (rows * 8) as f64 / (1024.0 * 1024.0);
    println!("rows={rows} ({mb:.0} MiB per buffer) iters={iters}\n");

    // Warm both paths so neither pays first-touch costs the other avoids.
    let _ = fresh(rows, 2);
    let _ = reuse(rows, 2);

    let mut fresh_runs = Vec::new();
    let mut reuse_runs = Vec::new();
    let mut checksum = (0.0_f64, 0.0_f64);
    for _ in 0..5 {
        let (t, s) = fresh(rows, iters);
        fresh_runs.push(t);
        checksum.0 = s;
        let (t, s) = reuse(rows, iters);
        reuse_runs.push(t);
        checksum.1 = s;
    }
    assert_eq!(
        checksum.0, checksum.1,
        "arms computed different values; the comparison would be meaningless"
    );

    let med = |mut v: Vec<f64>| {
        v.sort_by(f64::total_cmp);
        v[v.len() / 2]
    };
    let f = med(fresh_runs);
    let r = med(reuse_runs);

    println!("{:<10} {:>12} {:>14}", "arm", "median_ms", "ms_per_iter");
    println!("{:<10} {:>12.1} {:>14.3}", "fresh", f, f / iters as f64);
    println!("{:<10} {:>12.1} {:>14.3}", "reuse", r, r / iters as f64);
    println!("\nreuse is {:.2}x the speed of fresh allocation", f / r);
    println!(
        "Identical work in both arms; only allocation lifetime differs.\n\
         This is the ceiling for buffer pooling on buffers of this size."
    );
}
