//! Spike: the racing-clock draw is 62% of what remains after row-wise+threading.
//!
//! The `rowwise_spike` breakdown at 5M rows, threaded:
//!
//!     read age only                 0.41 ms
//!     + 18-band histogram          +1.66 ms
//!     + 5 guards                   +0.15 ms
//!     + Philox draw & guarded ln   +3.60 ms   <-- 62% of the residual
//!
//! Philox-4x32-10 is ten rounds of two 32x32->64 multiplies. That should run at
//! a few nanoseconds, so 3.60 ms for ~2.1M draws looks slow.
//!
//! Hypothesis: it is not Philox that is slow, it is *where* Philox sits. The
//! draw is behind `if guard`, which is data-dependent and unpredictable, so the
//! core cannot overlap consecutive draws. Every entity shares the same key
//! schedule and differs only in one counter lane, so the work is ideal for
//! instruction-level parallelism — and a mispredicted branch throws that away.
//!
//! Three shapes, same draws, same results:
//!   branchy   — draw inside the guard test, as the executor does today
//!   gathered  — collect enabled row indices first, then draw in a tight loop
//!   unrolled  — gathered, four independent draws per iteration
//!
//! Everything here is integer arithmetic plus one comparison, so all arms are
//! bit-identical by construction; the spike asserts it anyway.
//!
//!   cargo run --release -p sembla-runtime --example rng_batch_spike -- [rows] [reps]

use std::time::Instant;

use sembla_runtime::rng::uniform_f64;

const LAMBDA: f64 = 0.003;
const DT: f64 = 1.0;

fn lo_threshold() -> f64 {
    (-LAMBDA * DT).exp() * (1.0 - 1e-12)
}

fn decide(u: f64, lo: f64) -> bool {
    u >= lo && (-u.ln() / LAMBDA) < DT
}

/// Today's shape: the draw hides behind an unpredictable branch.
fn branchy(guards: &[bool]) -> u64 {
    let lo = lo_threshold();
    let mut fired = 0_u64;
    for (row, g) in guards.iter().enumerate() {
        if *g {
            let u = uniform_f64(0xC0FFEE, 7, 3, row as u32, 0);
            if decide(u, lo) {
                fired += 1;
            }
        }
    }
    fired
}

/// Separate the selection from the draw: one predictable pass to gather the
/// enabled rows, then a tight branch-free loop over them.
fn gathered(guards: &[bool], scratch: &mut Vec<u32>) -> u64 {
    let lo = lo_threshold();
    scratch.clear();
    scratch.extend(
        guards
            .iter()
            .enumerate()
            .filter_map(|(row, g)| g.then_some(row as u32)),
    );
    let mut fired = 0_u64;
    for row in scratch.iter() {
        let u = uniform_f64(0xC0FFEE, 7, 3, *row, 0);
        if decide(u, lo) {
            fired += 1;
        }
    }
    fired
}

/// Gathered, with four independent draws in flight per iteration so the core
/// can overlap them. Philox has no carried dependency between entities.
fn unrolled(guards: &[bool], scratch: &mut Vec<u32>) -> u64 {
    let lo = lo_threshold();
    scratch.clear();
    scratch.extend(
        guards
            .iter()
            .enumerate()
            .filter_map(|(row, g)| g.then_some(row as u32)),
    );
    let mut fired = 0_u64;
    let chunks = scratch.chunks_exact(4);
    let tail = chunks.remainder();
    for c in chunks {
        let u0 = uniform_f64(0xC0FFEE, 7, 3, c[0], 0);
        let u1 = uniform_f64(0xC0FFEE, 7, 3, c[1], 0);
        let u2 = uniform_f64(0xC0FFEE, 7, 3, c[2], 0);
        let u3 = uniform_f64(0xC0FFEE, 7, 3, c[3], 0);
        fired += decide(u0, lo) as u64;
        fired += decide(u1, lo) as u64;
        fired += decide(u2, lo) as u64;
        fired += decide(u3, lo) as u64;
    }
    for row in tail {
        let u = uniform_f64(0xC0FFEE, 7, 3, *row, 0);
        fired += decide(u, lo) as u64;
    }
    fired
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

    // The same enabled fraction the row-wise spike produced: age in [20,65)
    // and tenure > 0, which is unpredictable row to row.
    let guards: Vec<bool> = (0..rows)
        .map(|i| {
            let a = (i % 90) as i64;
            let t = (i % 7) as i64;
            a >= 20 && a < 65 && t > 0
        })
        .collect();
    let enabled = guards.iter().filter(|g| **g).count();

    let mut scratch: Vec<u32> = Vec::with_capacity(enabled);
    let mut results = Vec::new();

    for (name, f) in [
        ("branchy (current)", 0_u8),
        ("gathered", 1),
        ("unrolled x4", 2),
    ] {
        let mut times = Vec::new();
        let mut fired = 0;
        for _ in 0..reps {
            let t = Instant::now();
            fired = match f {
                0 => branchy(&guards),
                1 => gathered(&guards, &mut scratch),
                _ => unrolled(&guards, &mut scratch),
            };
            times.push(t.elapsed().as_secs_f64() * 1000.0);
        }
        results.push((name, median(times), fired));
    }

    let base = results[0].1;
    let expect = results[0].2;
    println!("rows={rows} reps={reps} enabled={enabled} ({:.0}%)\n", enabled as f64 / rows as f64 * 100.0);
    println!("{:<22} {:>9} {:>9} {:>12}", "shape", "ms", "speedup", "ns_per_draw");
    for (name, ms, fired) in &results {
        assert_eq!(*fired, expect, "{name} changed the fired count");
        println!(
            "{name:<22} {ms:>9.2} {:>8.1}x {:>12.2}",
            base / ms,
            ms * 1.0e6 / enabled as f64
        );
    }
    println!("\nAll arms asserted to fire the same count. Single-threaded here;\n\
              this composes with the ~5.6x from threading.");
}
