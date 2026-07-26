//! Spike: are we paying 8 bytes per row for values that need one?
//!
//! The bitset spike showed mask width does not matter — a `Vec<bool>` mask is
//! 4.8 MiB at 5M rows and already sits in cache. The *columns* are the traffic:
//! `AttrType::Int` is stored as `i64`, so `age` costs 40 MiB at 5M rows and
//! 400 MiB at 50M.
//!
//! But an age is 0..120. A status enum is a handful of variants. A tenure
//! counter is small. Every one of them fits in a byte. If physical storage
//! were narrowed to the declared or observed range, the dominant memory
//! traffic in the evaluator would drop by up to 8x — while the *semantics*
//! stay `i64`, because widening on read is exact and free.
//!
//! This measures the ceiling: the identical predicate over the identical
//! logical values, stored at four widths.
//!
//! Note this is a storage question, not a semantic one. `i64` arithmetic is
//! preserved: values widen on load. The open question a PRD would have to
//! answer is where the range comes from — declared in the IR, inferred by the
//! validator, or recorded in the state artifact — and what happens when a
//! value escapes it.
//!
//!   cargo run --release -p sembla-runtime --example narrow_spike -- [rows] [reps]

use std::time::Instant;

fn median(mut xs: Vec<f64>) -> f64 {
    xs.sort_by(f64::total_cmp);
    xs[xs.len() / 2]
}

macro_rules! arm {
    ($name:literal, $ty:ty, $age:expr, $ten:expr, $reps:expr, $out:expr) => {{
        let age: Vec<$ty> = $age.iter().map(|v| *v as $ty).collect();
        let ten: Vec<$ty> = $ten.iter().map(|v| *v as $ty).collect();
        let mut times = Vec::new();
        let mut selected = 0_u64;
        for _ in 0..$reps {
            let t = Instant::now();
            // Widen to i64 on load: the semantics stay exactly i64.
            let n = age
                .iter()
                .zip(&ten)
                .filter(|(a, t)| {
                    let (a, t) = (**a as i64, **t as i64);
                    a >= 20 && a < 65 && t > 0
                })
                .count() as u64;
            times.push(t.elapsed().as_secs_f64() * 1000.0);
            selected = n;
        }
        let bytes = std::mem::size_of::<$ty>() * age.len() * 2;
        $out.push((
            $name,
            median(times),
            selected,
            bytes as f64 / (1024.0 * 1024.0),
        ));
    }};
}

fn main() {
    let mut args = std::env::args().skip(1);
    let rows: usize = args
        .next()
        .and_then(|a| a.parse().ok())
        .unwrap_or(5_000_000);
    let reps: usize = args.next().and_then(|a| a.parse().ok()).unwrap_or(9);

    // Logical values within the range every real attribute in the model occupies.
    let age: Vec<i64> = (0..rows).map(|i| (i % 90) as i64).collect();
    let tenure: Vec<i64> = (0..rows).map(|i| (i % 7) as i64).collect();

    let mut out: Vec<(&str, f64, u64, f64)> = Vec::new();
    arm!("i64 (current)", i64, age, tenure, reps, out);
    arm!("i32", i32, age, tenure, reps, out);
    arm!("i16", i16, age, tenure, reps, out);
    arm!("u8", u8, age, tenure, reps, out);

    let base = out[0].1;
    let expect = out[0].2;
    println!("rows={rows} reps={reps} selected={expect}\n");
    println!(
        "{:<16} {:>9} {:>9} {:>12}",
        "storage", "ms", "speedup", "columns_MiB"
    );
    for (name, ms, sel, mib) in &out {
        assert_eq!(*sel, expect, "{name} selected a different set of rows");
        println!("{name:<16} {ms:>9.2} {:>8.1}x {mib:>12.1}", base / ms);
    }
    println!(
        "\nAll arms asserted to select the same rows. Values widen to i64 on load,\n\
         so i64 arithmetic semantics are unchanged — only physical width differs."
    );
}
