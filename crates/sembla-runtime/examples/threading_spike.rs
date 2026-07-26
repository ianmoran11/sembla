//! Spike: the CPU oracle is single-threaded on a ten-core machine.
//!
//! `DESIGN.md` §4.2 defines the closed kernel fragment as "operations whose
//! parallel execution is order-free or has a canonical order". The design was
//! built for parallelism and the GPU backend exploits it. The CPU oracle does
//! not: every guard, comparison, hazard and racing-clock draw runs on one core.
//!
//! For element-wise work this is bit-identical when parallelised, and not
//! approximately so — each row's result depends only on that row's inputs, and
//! the RNG is counter-based, so a draw is a pure function of
//! (seed, tick, rule, entity, index) with no stream state. Row `i` gets the same
//! answer whichever thread computes it and whatever order threads run in.
//!
//! What is NOT safely parallel: Real sums (`eval.rs` documents ascending row
//! order as the canonical Level A reduction order, and a tree reduction would
//! change the result), and conflict resolution. Those stay sequential. This
//! measures only the element-wise portion.
//!
//! Two workloads, because they stress different limits:
//!   guard — memory-bound comparison chain
//!   clock — the racing-clock draw, compute-bound on Philox and `ln`
//!
//! No new dependency: `std::thread::scope` with fixed chunking. Chunk
//! boundaries are a function of row index, never of scheduling.
//!
//!   cargo run --release -p sembla-runtime --example threading_spike -- [rows] [reps]

use std::time::Instant;

use sembla_runtime::rng::exp_f64;

fn guard_serial(age: &[i64], tenure: &[i64], out: &mut [bool]) {
    for (i, slot) in out.iter_mut().enumerate() {
        *slot = age[i] >= 20 && age[i] < 65 && tenure[i] > 0;
    }
}

fn guard_parallel(age: &[i64], tenure: &[i64], out: &mut [bool], threads: usize) {
    let rows = out.len();
    let chunk = rows.div_ceil(threads);
    std::thread::scope(|scope| {
        for (t, slice) in out.chunks_mut(chunk).enumerate() {
            let base = t * chunk;
            let (age, tenure) = (&age, &tenure);
            scope.spawn(move || {
                for (i, slot) in slice.iter_mut().enumerate() {
                    let r = base + i;
                    *slot = age[r] >= 20 && age[r] < 65 && tenure[r] > 0;
                }
            });
        }
    });
}

fn clock_serial(lambda: f64, out: &mut [f64]) {
    for (i, slot) in out.iter_mut().enumerate() {
        *slot = exp_f64(0xC0FFEE, 7, 3, i as u32, 0, lambda);
    }
}

fn clock_parallel(lambda: f64, out: &mut [f64], threads: usize) {
    let chunk = out.len().div_ceil(threads);
    std::thread::scope(|scope| {
        for (t, slice) in out.chunks_mut(chunk).enumerate() {
            let base = t * chunk;
            scope.spawn(move || {
                for (i, slot) in slice.iter_mut().enumerate() {
                    *slot = exp_f64(0xC0FFEE, 7, 3, (base + i) as u32, 0, lambda);
                }
            });
        }
    });
}

fn median(mut xs: Vec<f64>) -> f64 {
    xs.sort_by(f64::total_cmp);
    xs[xs.len() / 2]
}

fn main() {
    let mut args = std::env::args().skip(1);
    let rows: usize = args
        .next()
        .and_then(|a| a.parse().ok())
        .unwrap_or(5_000_000);
    let reps: usize = args.next().and_then(|a| a.parse().ok()).unwrap_or(7);
    let cores = std::thread::available_parallelism().map_or(1, |n| n.get());

    let age: Vec<i64> = (0..rows).map(|i| (i % 90) as i64).collect();
    let tenure: Vec<i64> = (0..rows).map(|i| (i % 7) as i64).collect();

    println!("rows={rows} reps={reps} available_parallelism={cores}\n");

    // --- guard: memory-bound -------------------------------------------------
    let mut base_out = vec![false; rows];
    let mut times = Vec::new();
    for _ in 0..reps {
        let t = Instant::now();
        guard_serial(&age, &tenure, &mut base_out);
        times.push(t.elapsed().as_secs_f64() * 1000.0);
    }
    let guard_serial_ms = median(times);

    println!("{:<10} {:>9} {:>9} {:>10}", "guard", "threads", "ms", "speedup");
    println!("{:<10} {:>9} {guard_serial_ms:>9.2} {:>9.1}x", "", 1, 1.0);
    for threads in [2, 4, 8, cores] {
        if threads <= 1 || threads > cores {
            continue;
        }
        let mut out = vec![false; rows];
        let mut times = Vec::new();
        for _ in 0..reps {
            let t = Instant::now();
            guard_parallel(&age, &tenure, &mut out, threads);
            times.push(t.elapsed().as_secs_f64() * 1000.0);
        }
        assert!(out == base_out, "guard: {threads} threads changed the result");
        let ms = median(times);
        println!(
            "{:<10} {threads:>9} {ms:>9.2} {:>9.1}x",
            "",
            guard_serial_ms / ms
        );
    }

    // --- racing clock: compute-bound ----------------------------------------
    let lambda = 0.003_f64;
    let mut base_clock = vec![0.0_f64; rows];
    let mut times = Vec::new();
    for _ in 0..reps {
        let t = Instant::now();
        clock_serial(lambda, &mut base_clock);
        times.push(t.elapsed().as_secs_f64() * 1000.0);
    }
    let clock_serial_ms = median(times);

    println!("\n{:<10} {:>9} {:>9} {:>10}", "clock", "threads", "ms", "speedup");
    println!("{:<10} {:>9} {clock_serial_ms:>9.2} {:>9.1}x", "", 1, 1.0);
    for threads in [2, 4, 8, cores] {
        if threads <= 1 || threads > cores {
            continue;
        }
        let mut out = vec![0.0_f64; rows];
        let mut times = Vec::new();
        for _ in 0..reps {
            let t = Instant::now();
            clock_parallel(lambda, &mut out, threads);
            times.push(t.elapsed().as_secs_f64() * 1000.0);
        }
        // Bit-identical, not approximately equal: compare the raw bits.
        assert!(
            out.iter().zip(&base_clock).all(|(a, b)| a.to_bits() == b.to_bits()),
            "clock: {threads} threads changed a draw"
        );
        let ms = median(times);
        println!(
            "{:<10} {threads:>9} {ms:>9.2} {:>9.1}x",
            "",
            clock_serial_ms / ms
        );
    }

    println!(
        "\nEvery parallel arm asserted equal to the serial one — bitwise for the\n\
         racing clock. Reductions and conflict resolution are excluded and would\n\
         stay sequential."
    );
}
