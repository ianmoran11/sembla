//! Spike: the model's view filters are a histogram written out longhand.
//!
//! `demographic_slots.no-grouped.json` declares eighteen views whose filters are
//! age bands — `age >= 0 && age < 5`, `age >= 5 && age < 10`, and so on. Today
//! `observe_views` evaluates each one independently, so the age column is walked
//! eighteen times per tick and each walk does two comparisons and allocates two
//! `Vec<bool>` intermediates plus a result.
//!
//! But the bands are *mutually exclusive and over the same column*. That is a
//! histogram: one pass, one bucket index per row, eighteen counters. The work
//! collapses from 18 passes to 1.
//!
//! This is not a micro-optimisation of the evaluator — it is recognising an
//! algebraic property of a set of expressions the evaluator currently treats as
//! unrelated. The recognition is mechanical: same column, disjoint constant
//! ranges, count reduction.
//!
//! Arms are asserted to produce identical counts per band.
//!
//!   cargo run --release -p sembla-runtime --example histogram_spike -- [rows] [reps]

use std::time::Instant;

const BANDS: usize = 18;
const WIDTH: i64 = 5;

/// What `observe_views` does today: one independent pass per band, each
/// materialising intermediates the way `eval_expr` does.
fn separate_passes(age: &[i64]) -> Vec<u64> {
    let mut counts = Vec::with_capacity(BANDS);
    for band in 0..BANDS {
        let lo = band as i64 * WIDTH;
        let hi = lo + WIDTH;
        let ge: Vec<bool> = age.iter().map(|a| *a >= lo).collect();
        let lt: Vec<bool> = age.iter().map(|a| *a < hi).collect();
        let sel: Vec<bool> = ge.iter().zip(&lt).map(|(x, y)| *x && *y).collect();
        counts.push(sel.iter().filter(|b| **b).count() as u64);
    }
    counts
}

/// Same eighteen passes, but fused per band — no intermediates. Isolates how
/// much is fusion and how much is the pass count.
fn separate_fused(age: &[i64]) -> Vec<u64> {
    let mut counts = Vec::with_capacity(BANDS);
    for band in 0..BANDS {
        let lo = band as i64 * WIDTH;
        let hi = lo + WIDTH;
        counts.push(age.iter().filter(|a| **a >= lo && **a < hi).count() as u64);
    }
    counts
}

/// One pass. The bands are disjoint ranges over one column, so the band index
/// is arithmetic and the whole set is a single histogram.
fn histogram(age: &[i64]) -> Vec<u64> {
    let mut counts = vec![0_u64; BANDS];
    for a in age {
        let band = *a / WIDTH;
        if *a >= 0 && (band as usize) < BANDS {
            counts[band as usize] += 1;
        }
    }
    counts
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

    let age: Vec<i64> = (0..rows).map(|i| (i % 90) as i64).collect();

    let (mut t_sep, mut t_sepf, mut t_hist) = (Vec::new(), Vec::new(), Vec::new());
    let (mut c_sep, mut c_sepf, mut c_hist) = (vec![], vec![], vec![]);

    for _ in 0..reps {
        let t = Instant::now();
        c_sep = separate_passes(&age);
        t_sep.push(t.elapsed().as_secs_f64() * 1000.0);

        let t = Instant::now();
        c_sepf = separate_fused(&age);
        t_sepf.push(t.elapsed().as_secs_f64() * 1000.0);

        let t = Instant::now();
        c_hist = histogram(&age);
        t_hist.push(t.elapsed().as_secs_f64() * 1000.0);
    }

    assert_eq!(c_sep, c_sepf, "fused per-band disagreed with the current shape");
    assert_eq!(c_sep, c_hist, "histogram disagreed with the current shape");

    let (a, b, c) = (median(t_sep), median(t_sepf), median(t_hist));
    println!("rows={rows} reps={reps} bands={BANDS}\n");
    println!("{:<28} {:>10} {:>9} {:>8}", "arm", "ms", "speedup", "passes");
    println!("{:<28} {a:>10.2} {:>8.1}x {BANDS:>8}", "separate + intermediates", 1.0);
    println!("{:<28} {b:>10.2} {:>8.1}x {BANDS:>8}", "separate, fused", a / b);
    println!("{:<28} {c:>10.2} {:>8.1}x {:>8}", "single histogram", a / c, 1);
    println!("\nAll arms asserted to produce identical per-band counts.");
}
