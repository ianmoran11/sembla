//! Spike: what if a tick read the state once instead of ~40 times?
//!
//! Every optimisation measured so far keeps the same execution shape: one pass
//! per operation, column at a time. Fusion collapses the passes *within* one
//! expression. Threading spreads each pass across cores. Neither changes the
//! fact that a tick walks the `age` column roughly forty times — twice for each
//! of eighteen view bands, plus once per transition guard.
//!
//! The physical floor is set by reading the state once and writing it once:
//! ~2.3 ms/tick at 5M rows on 200 GB/s. Current `execute_tick` is ~2,075 ms/tick.
//!
//! Row-wise execution inverts the loop nest. Instead of
//!
//!     for each operation:  for each row:  ...
//!
//! do
//!
//!     for each row:  for each operation:  ...
//!
//! The row's attributes are loaded once into registers and every guard, every
//! band counter and every clock draw is computed against them before moving on.
//! State traffic collapses from ~40 passes to 1.
//!
//! This is the change the other spikes cannot substitute for, because they all
//! operate inside the existing shape.
//!
//! Determinism is preserved by construction: guards and clocks are per-row and
//! the RNG is counter-based, and the view counters are integers, so per-thread
//! partials combine associatively. Real sums would still need the canonical
//! ascending order and are excluded here, as in the threading spike.
//!
//!   cargo run --release -p sembla-runtime --example rowwise_spike -- [rows] [reps]

use std::time::Instant;

use sembla_runtime::rng::{exp_f64, uniform_f64};

const BANDS: usize = 18;
const WIDTH: i64 = 5;
const LAMBDA: f64 = 0.003;
const DT: f64 = 1.0;

#[derive(Debug, PartialEq, Eq, Clone)]
struct TickResult {
    band_counts: Vec<u64>,
    guard_counts: [u64; 5],
    fired: u64,
}

/// Column-at-a-time with materialised intermediates: the current shape.
fn column_wise(age: &[i64], tenure: &[i64], status: &[i64]) -> TickResult {
    let rows = age.len();

    // Eighteen view bands, each an independent expression with intermediates.
    let mut band_counts = Vec::with_capacity(BANDS);
    for band in 0..BANDS {
        let lo = band as i64 * WIDTH;
        let hi = lo + WIDTH;
        let ge: Vec<bool> = age.iter().map(|a| *a >= lo).collect();
        let lt: Vec<bool> = age.iter().map(|a| *a < hi).collect();
        let sel: Vec<bool> = ge.iter().zip(&lt).map(|(x, y)| *x && *y).collect();
        band_counts.push(sel.iter().filter(|b| **b).count() as u64);
    }

    // Five transition guards, each its own pass with intermediates.
    let mut guard_counts = [0_u64; 5];
    let mut guards: Vec<Vec<bool>> = Vec::with_capacity(5);
    for (i, spec) in [(0_i64, 20_i64), (20, 65), (65, 200), (0, 200), (18, 90)]
        .into_iter()
        .enumerate()
    {
        let (lo, hi) = spec;
        let ge: Vec<bool> = age.iter().map(|a| *a >= lo).collect();
        let lt: Vec<bool> = age.iter().map(|a| *a < hi).collect();
        let tn: Vec<bool> = tenure.iter().map(|t| *t > 0).collect();
        let ab: Vec<bool> = ge.iter().zip(&lt).map(|(x, y)| *x && *y).collect();
        let g: Vec<bool> = ab.iter().zip(&tn).map(|(x, y)| *x && *y).collect();
        guard_counts[i] = g.iter().filter(|b| **b).count() as u64;
        guards.push(g);
    }

    // Hazard: a constant, materialised full-length as `eval_expr` does today.
    let hazard: Vec<f64> = vec![LAMBDA; rows];

    // Racing clock over the enabled candidates of the first transition.
    let mut fired = 0_u64;
    for (row, (g, lambda)) in guards[1].iter().zip(&hazard).enumerate() {
        if !*g || *lambda <= 0.0 {
            continue;
        }
        if exp_f64(0xC0FFEE, 7, 3, row as u32, 0, *lambda) < DT {
            fired += 1;
        }
    }
    let _ = status;

    TickResult {
        band_counts,
        guard_counts,
        fired,
    }
}

/// One pass. Attributes load once per row; everything is computed against them.
fn row_wise(age: &[i64], tenure: &[i64], status: &[i64]) -> TickResult {
    let mut band_counts = vec![0_u64; BANDS];
    let mut guard_counts = [0_u64; 5];
    let mut fired = 0_u64;

    for row in 0..age.len() {
        let a = age[row];
        let t = tenure[row];
        let _s = status[row];

        let band = a / WIDTH;
        if a >= 0 && (band as usize) < BANDS {
            band_counts[band as usize] += 1;
        }

        let tn = t > 0;
        let g0 = a >= 0 && a < 20 && tn;
        let g1 = a >= 20 && a < 65 && tn;
        let g2 = a >= 65 && a < 200 && tn;
        let g3 = a >= 0 && a < 200 && tn;
        let g4 = a >= 18 && a < 90 && tn;
        guard_counts[0] += g0 as u64;
        guard_counts[1] += g1 as u64;
        guard_counts[2] += g2 as u64;
        guard_counts[3] += g3 as u64;
        guard_counts[4] += g4 as u64;

        if g1 && exp_f64(0xC0FFEE, 7, 3, row as u32, 0, LAMBDA) < DT {
            fired += 1;
        }
    }

    TickResult {
        band_counts,
        guard_counts,
        fired,
    }
}

/// Row-wise plus threading. Chunk boundaries derive from row index; per-thread
/// counters are integers and combine associatively, so the result is identical.
fn row_wise_threaded(age: &[i64], tenure: &[i64], status: &[i64], threads: usize) -> TickResult {
    let rows = age.len();
    let chunk = rows.div_ceil(threads);
    let partials: Vec<TickResult> = std::thread::scope(|scope| {
        let handles: Vec<_> = (0..threads)
            .map(|t| {
                let base = t * chunk;
                let end = (base + chunk).min(rows);
                scope.spawn(move || {
                    let mut band_counts = vec![0_u64; BANDS];
                    let mut guard_counts = [0_u64; 5];
                    let mut fired = 0_u64;
                    for row in base..end {
                        let a = age[row];
                        let t = tenure[row];
                        let _s = status[row];
                        let band = a / WIDTH;
                        if a >= 0 && (band as usize) < BANDS {
                            band_counts[band as usize] += 1;
                        }
                        let tn = t > 0;
                        let g1 = a >= 20 && a < 65 && tn;
                        guard_counts[0] += (a >= 0 && a < 20 && tn) as u64;
                        guard_counts[1] += g1 as u64;
                        guard_counts[2] += (a >= 65 && a < 200 && tn) as u64;
                        guard_counts[3] += (a >= 0 && a < 200 && tn) as u64;
                        guard_counts[4] += (a >= 18 && a < 90 && tn) as u64;
                        if g1 && exp_f64(0xC0FFEE, 7, 3, row as u32, 0, LAMBDA) < DT {
                            fired += 1;
                        }
                    }
                    TickResult {
                        band_counts,
                        guard_counts,
                        fired,
                    }
                })
            })
            .collect();
        handles.into_iter().map(|h| h.join().unwrap()).collect()
    });

    let mut out = TickResult {
        band_counts: vec![0_u64; BANDS],
        guard_counts: [0_u64; 5],
        fired: 0,
    };
    for p in partials {
        for (o, v) in out.band_counts.iter_mut().zip(&p.band_counts) {
            *o += v;
        }
        for (o, v) in out.guard_counts.iter_mut().zip(&p.guard_counts) {
            *o += v;
        }
        out.fired += p.fired;
    }
    out
}

/// Row-wise, threaded, and with the guarded-`ln` filter from the ln spike.
/// Below `lo` a fire is impossible by ~4500 ULPs, so `ln` is skipped; every
/// decision near the boundary is still made by the same comparison the oracle
/// makes today, so the result is bit-identical.
fn row_wise_guarded(age: &[i64], tenure: &[i64], status: &[i64], threads: usize) -> TickResult {
    let rows = age.len();
    let chunk = rows.div_ceil(threads);
    let lo = (-LAMBDA * DT).exp() * (1.0 - 1e-12);
    let partials: Vec<TickResult> = std::thread::scope(|scope| {
        let handles: Vec<_> = (0..threads)
            .map(|t| {
                let base = t * chunk;
                let end = (base + chunk).min(rows);
                scope.spawn(move || {
                    let mut band_counts = vec![0_u64; BANDS];
                    let mut guard_counts = [0_u64; 5];
                    let mut fired = 0_u64;
                    for row in base..end {
                        let a = age[row];
                        let t = tenure[row];
                        let _s = status[row];
                        let band = a / WIDTH;
                        if a >= 0 && (band as usize) < BANDS {
                            band_counts[band as usize] += 1;
                        }
                        let tn = t > 0;
                        let g1 = a >= 20 && a < 65 && tn;
                        guard_counts[0] += (a >= 0 && a < 20 && tn) as u64;
                        guard_counts[1] += g1 as u64;
                        guard_counts[2] += (a >= 65 && a < 200 && tn) as u64;
                        guard_counts[3] += (a >= 0 && a < 200 && tn) as u64;
                        guard_counts[4] += (a >= 18 && a < 90 && tn) as u64;
                        if g1 {
                            let u = uniform_f64(0xC0FFEE, 7, 3, row as u32, 0);
                            if u >= lo && (-u.ln() / LAMBDA) < DT {
                                fired += 1;
                            }
                        }
                    }
                    TickResult { band_counts, guard_counts, fired }
                })
            })
            .collect();
        handles.into_iter().map(|h| h.join().unwrap()).collect()
    });
    let mut out = TickResult {
        band_counts: vec![0_u64; BANDS],
        guard_counts: [0_u64; 5],
        fired: 0,
    };
    for p in partials {
        for (o, v) in out.band_counts.iter_mut().zip(&p.band_counts) { *o += v; }
        for (o, v) in out.guard_counts.iter_mut().zip(&p.guard_counts) { *o += v; }
        out.fired += p.fired;
    }
    out
}

/// Attribute the residual: same threaded row-wise shape, progressively less work.
fn partial(age: &[i64], tenure: &[i64], threads: usize, stage: u8) -> u64 {
    let rows = age.len();
    let chunk = rows.div_ceil(threads);
    let lo = (-LAMBDA * DT).exp() * (1.0 - 1e-12);
    let partials: Vec<u64> = std::thread::scope(|scope| {
        let handles: Vec<_> = (0..threads)
            .map(|t| {
                let base = t * chunk;
                let end = (base + chunk).min(rows);
                scope.spawn(move || {
                    let mut bands = vec![0_u64; BANDS];
                    let mut acc = 0_u64;
                    for row in base..end {
                        let a = age[row];
                        // stage 0: read only
                        if stage == 0 {
                            acc += a as u64 & 1;
                            continue;
                        }
                        // stage 1: + band histogram
                        let b = a / WIDTH;
                        if a >= 0 && (b as usize) < BANDS {
                            bands[b as usize] += 1;
                        }
                        if stage == 1 {
                            continue;
                        }
                        // stage 2: + five guards
                        let tn = tenure[row] > 0;
                        let g1 = a >= 20 && a < 65 && tn;
                        acc += (a >= 0 && a < 20 && tn) as u64;
                        acc += g1 as u64;
                        acc += (a >= 65 && a < 200 && tn) as u64;
                        acc += (a >= 0 && a < 200 && tn) as u64;
                        acc += (a >= 18 && a < 90 && tn) as u64;
                        if stage == 2 {
                            continue;
                        }
                        // stage 3: + Philox draw, guarded ln
                        if g1 {
                            let u = uniform_f64(0xC0FFEE, 7, 3, row as u32, 0);
                            if u >= lo && (-u.ln() / LAMBDA) < DT {
                                acc += 1;
                            }
                        }
                    }
                    acc + bands.iter().sum::<u64>()
                })
            })
            .collect();
        handles.into_iter().map(|h| h.join().unwrap()).collect()
    });
    partials.iter().sum()
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
    let reps: usize = args.next().and_then(|a| a.parse().ok()).unwrap_or(5);
    let cores = std::thread::available_parallelism().map_or(1, |n| n.get());

    let age: Vec<i64> = (0..rows).map(|i| (i % 90) as i64).collect();
    let tenure: Vec<i64> = (0..rows).map(|i| (i % 7) as i64).collect();
    let status: Vec<i64> = (0..rows).map(|i| (i % 5) as i64).collect();

    let bytes = rows * 3 * 8;
    let floor_ms = (bytes as f64 * 2.0) / (200.0e9) * 1000.0;

    let mut t_col = Vec::new();
    let mut t_row = Vec::new();
    let mut t_par = Vec::new();
    let mut t_grd = Vec::new();
    let (mut r_col, mut r_row, mut r_par, mut r_grd) = (None, None, None, None);

    for _ in 0..reps {
        let t = Instant::now();
        r_col = Some(column_wise(&age, &tenure, &status));
        t_col.push(t.elapsed().as_secs_f64() * 1000.0);

        let t = Instant::now();
        r_row = Some(row_wise(&age, &tenure, &status));
        t_row.push(t.elapsed().as_secs_f64() * 1000.0);

        let t = Instant::now();
        r_par = Some(row_wise_threaded(&age, &tenure, &status, cores));
        t_par.push(t.elapsed().as_secs_f64() * 1000.0);

        let t = Instant::now();
        r_grd = Some(row_wise_guarded(&age, &tenure, &status, cores));
        t_grd.push(t.elapsed().as_secs_f64() * 1000.0);
    }

    let (c, r, p, g) = (r_col.unwrap(), r_row.unwrap(), r_par.unwrap(), r_grd.unwrap());
    assert_eq!(c, r, "row-wise disagreed with column-wise");
    assert_eq!(c, p, "threaded row-wise disagreed with column-wise");
    assert_eq!(c, g, "guarded row-wise disagreed with column-wise");

    let (mc, mr, mp, mg) = (median(t_col), median(t_row), median(t_par), median(t_grd));
    println!("rows={rows} reps={reps} cores={cores}");
    println!(
        "workload: {BANDS} view bands + 5 transition guards + 1 racing clock\n\
         state touched: {:.0} MiB\n",
        bytes as f64 / (1024.0 * 1024.0)
    );
    println!("{:<26} {:>10} {:>10} {:>14}", "shape", "ms", "speedup", "x_above_floor");
    for (name, ms) in [
        ("column-wise (current)", mc),
        ("row-wise, 1 thread", mr),
        ("row-wise, threaded", mp),
        ("+ guarded ln", mg),
    ] {
        println!(
            "{name:<26} {ms:>10.2} {:>9.1}x {:>14.0}",
            mc / ms,
            ms / floor_ms
        );
    }
    println!(
        "\nfloor = {floor_ms:.2} ms: reading and writing the touched state once at\n\
         200 GB/s. All arms asserted to produce identical counts."
    );

    println!("\n--- what the residual is made of (threaded row-wise) ---");
    println!("{:<34} {:>9} {:>11}", "cumulative work", "ms", "delta_ms");
    let labels = [
        "read age only",
        "+ 18-band histogram",
        "+ 5 guards",
        "+ Philox draw & guarded ln",
    ];
    let mut prev = 0.0;
    for (stage, label) in labels.iter().enumerate() {
        let mut times = Vec::new();
        for _ in 0..reps {
            let t = Instant::now();
            std::hint::black_box(partial(&age, &tenure, cores, stage as u8));
            times.push(t.elapsed().as_secs_f64() * 1000.0);
        }
        let ms = median(times);
        println!("{label:<34} {ms:>9.2} {:>11.2}", ms - prev);
        prev = ms;
    }
}
