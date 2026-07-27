//! Spike: is `Vec<bool>` the wrong representation for mask columns?
//!
//! `InternalColumn::Bool(Vec<bool>)` stores one byte per row. Guards, filters,
//! and every `And`/`Or`/`Not` node produce and consume these. At 50M rows a
//! single mask is 50 MB; the same mask as a bitset is 6.25 MB.
//!
//! Two effects, and they compound:
//!   * 8x less memory traffic per mask
//!   * boolean ops become 64 rows per instruction instead of one
//!
//! The model's expression trees are dominated by exactly this shape — banded
//! filters and multi-clause guards are chains of comparisons combined with
//! `And`. So if masks are the wrong width, it is the wrong width in the hottest
//! place.
//!
//! Arms compute the same predicate over the same data; popcount is asserted
//! equal so a faster arm cannot be a wrong arm.
//!
//!   cargo run --release -p sembla-runtime --example bitset_spike -- [rows] [reps]

use std::time::Instant;

const WORD: usize = 64;

fn words_for(rows: usize) -> usize {
    rows.div_ceil(WORD)
}

/// Current shape: byte-per-row masks, combined pairwise.
fn bool_chain(age: &[i64], tenure: &[i64]) -> Vec<bool> {
    let a: Vec<bool> = age.iter().map(|v| *v >= 20).collect();
    let b: Vec<bool> = age.iter().map(|v| *v < 65).collect();
    let c: Vec<bool> = tenure.iter().map(|v| *v > 0).collect();
    let ab: Vec<bool> = a.iter().zip(&b).map(|(x, y)| *x && *y).collect();
    ab.iter().zip(&c).map(|(x, y)| *x && *y).collect()
}

/// Same structure, bitset masks. Still one intermediate per node — this
/// isolates representation, not fusion.
fn bitset_chain(age: &[i64], tenure: &[i64], rows: usize) -> Vec<u64> {
    let pack = |f: &dyn Fn(usize) -> bool| -> Vec<u64> {
        let mut out = vec![0_u64; words_for(rows)];
        for (w, slot) in out.iter_mut().enumerate() {
            let base = w * WORD;
            let end = (base + WORD).min(rows);
            let mut bits = 0_u64;
            for r in base..end {
                if f(r) {
                    bits |= 1 << (r - base);
                }
            }
            *slot = bits;
        }
        out
    };
    let a = pack(&|r| age[r] >= 20);
    let b = pack(&|r| age[r] < 65);
    let c = pack(&|r| tenure[r] > 0);
    let ab: Vec<u64> = a.iter().zip(&b).map(|(x, y)| x & y).collect();
    ab.iter().zip(&c).map(|(x, y)| x & y).collect()
}

/// Bitset plus fusion: one pass, one output.
fn bitset_fused(age: &[i64], tenure: &[i64], rows: usize) -> Vec<u64> {
    let mut out = vec![0_u64; words_for(rows)];
    for (w, slot) in out.iter_mut().enumerate() {
        let base = w * WORD;
        let end = (base + WORD).min(rows);
        let mut bits = 0_u64;
        for r in base..end {
            if age[r] >= 20 && age[r] < 65 && tenure[r] > 0 {
                bits |= 1 << (r - base);
            }
        }
        *slot = bits;
    }
    out
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
    let reps: usize = args.next().and_then(|a| a.parse().ok()).unwrap_or(9);

    let age: Vec<i64> = (0..rows).map(|i| (i % 90) as i64).collect();
    let tenure: Vec<i64> = (0..rows).map(|i| (i % 7) as i64).collect();

    let (mut t_bool, mut t_bits, mut t_fused) = (Vec::new(), Vec::new(), Vec::new());
    let (mut n_bool, mut n_bits, mut n_fused) = (0_u32, 0_u32, 0_u32);

    for _ in 0..reps {
        let t = Instant::now();
        let r = bool_chain(&age, &tenure);
        t_bool.push(t.elapsed().as_secs_f64() * 1000.0);
        n_bool = r.iter().filter(|b| **b).count() as u32;

        let t = Instant::now();
        let r = bitset_chain(&age, &tenure, rows);
        t_bits.push(t.elapsed().as_secs_f64() * 1000.0);
        n_bits = r.iter().map(|w| w.count_ones()).sum();

        let t = Instant::now();
        let r = bitset_fused(&age, &tenure, rows);
        t_fused.push(t.elapsed().as_secs_f64() * 1000.0);
        n_fused = r.iter().map(|w| w.count_ones()).sum();
    }

    assert_eq!(n_bool, n_bits, "bitset chain disagreed with Vec<bool>");
    assert_eq!(n_bool, n_fused, "fused bitset disagreed with Vec<bool>");

    let (b, s, f) = (median(t_bool), median(t_bits), median(t_fused));
    println!("rows={rows} reps={reps} selected={n_bool}\n");
    println!(
        "{:<26} {:>10} {:>9} {:>10}",
        "arm", "ms", "speedup", "mask_MiB"
    );
    let bool_mb = rows as f64 / (1024.0 * 1024.0);
    let bits_mb = words_for(rows) as f64 * 8.0 / (1024.0 * 1024.0);
    println!(
        "{:<26} {b:>10.2} {:>8.1}x {bool_mb:>10.1}",
        "Vec<bool> chain", 1.0
    );
    println!(
        "{:<26} {s:>10.2} {:>8.1}x {bits_mb:>10.1}",
        "bitset chain",
        b / s
    );
    println!(
        "{:<26} {f:>10.2} {:>8.1}x {bits_mb:>10.1}",
        "bitset + fused",
        b / f
    );
    println!("\nAll arms asserted to select the same rows.");
}
