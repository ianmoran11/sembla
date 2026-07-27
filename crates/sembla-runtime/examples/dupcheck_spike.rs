//! Spike: PRD 0005 replaced a sort with a HashMap. Did that help?
//!
//! `detect_double_writes` must prove no two writes target one cell. It used to
//! sort ~800,000 write indices per tick, which the profile measured at 261
//! samples. PRD 0005 replaced that with a `HashMap`, and the profile now
//! measures 862 samples across `sip::Hasher`, `BuildHasher::hash_one` and
//! `HashMap::insert` — a net **+601 on the serial main thread**.
//!
//! The cause is the default hasher. Rust's `HashMap` uses SipHash-1-3, which is
//! DoS-resistant and slow, for a key of four small integers hashed 800,000
//! times per tick.
//!
//! Three ways to do the same job, measured here:
//!
//!   sort     — what PRD 0005 removed
//!   hashmap  — what PRD 0005 added (default SipHash)
//!   bitmap   — one bit per cell; detection needs no hashing and no ordering
//!
//! The bitmap is what PRD 0005's own §2 suggested before the implementation
//! chose the map. It cannot report *which* writes collided on its own, but the
//! writer pair is only needed on the error path, which terminates the tick — so
//! a linear scan there costs nothing in the common case.
//!
//! All three arms must agree on whether a duplicate exists.
//!
//!   cargo run --release -p sembla-runtime --example dupcheck_spike -- [writes] [rows]

use std::collections::HashMap;
use std::time::Instant;

#[derive(Clone, Copy)]
struct Write {
    table: u16,
    attr: u16,
    row: u32,
}

/// Pre-0005: sort indices by cell, then inspect adjacent pairs.
fn via_sort(writes: &[Write]) -> bool {
    let mut order: Vec<u32> = (0..writes.len() as u32).collect();
    order.sort_by_key(|i| {
        let w = &writes[*i as usize];
        (w.table, w.attr, w.row)
    });
    order.windows(2).any(|p| {
        let (a, b) = (&writes[p[0] as usize], &writes[p[1] as usize]);
        (a.table, a.attr, a.row) == (b.table, b.attr, b.row)
    })
}

/// Post-0005: insert each cell into a HashMap with the default hasher.
fn via_hashmap(writes: &[Write]) -> bool {
    let mut seen: HashMap<(u16, u16, u32), u32> = HashMap::with_capacity(writes.len());
    for (i, w) in writes.iter().enumerate() {
        if seen.insert((w.table, w.attr, w.row), i as u32).is_some() {
            return true;
        }
    }
    false
}

/// Proposed: one bit per cell. No hashing, no ordering, no allocation per write.
/// `scratch` is reused across ticks, which is why it is passed in.
fn via_bitmap(writes: &[Write], rows: usize, columns: usize, scratch: &mut Vec<u64>) -> bool {
    let words = (rows * columns).div_ceil(64);
    scratch.clear();
    scratch.resize(words, 0);
    for w in writes {
        let cell = (w.table as usize * columns + w.attr as usize) * rows + w.row as usize;
        let (word, bit) = (cell / 64, cell % 64);
        let mask = 1_u64 << bit;
        if scratch[word] & mask != 0 {
            return true;
        }
        scratch[word] |= mask;
    }
    false
}

fn median(mut xs: Vec<f64>) -> f64 {
    xs.sort_by(f64::total_cmp);
    xs[xs.len() / 2]
}

fn main() {
    let mut args = std::env::args().skip(1);
    let count: usize = args.next().and_then(|a| a.parse().ok()).unwrap_or(800_000);
    let rows: usize = args
        .next()
        .and_then(|a| a.parse().ok())
        .unwrap_or(1_000_000);
    // One destination column per transition, so a clean tick has no duplicate.
    // This matters: every arm short-circuits on the first duplicate, so seeded
    // collisions would time partial passes and make the comparison meaningless.
    let transitions = 10_usize;
    let columns = transitions;
    let tables = 1_u16;

    // Shape matters more than volume here. The executor pushes writes per
    // transition, and within a transition in ascending row order, so the array
    // arrives as a small number of ascending runs. Rust's merge sort exploits
    // existing runs, so scrambling the rows would make the sort arm look far
    // worse than it is in the product.
    let per = count / transitions;
    let mut writes: Vec<Write> = Vec::with_capacity(count);
    for t in 0..transitions {
        let stride = (rows / per.max(1)).max(1);
        for k in 0..per {
            writes.push(Write {
                table: (t as u16) % tables,
                attr: t as u16,
                row: ((k * stride) % rows) as u32,
            });
        }
    }

    let reps = 7;
    let mut scratch: Vec<u64> = Vec::new();
    let (mut ts, mut th, mut tb) = (Vec::new(), Vec::new(), Vec::new());
    let (mut rs, mut rh, mut rb) = (false, false, false);

    for _ in 0..reps {
        let t = Instant::now();
        rs = via_sort(&writes);
        ts.push(t.elapsed().as_secs_f64() * 1000.0);

        let t = Instant::now();
        rh = via_hashmap(&writes);
        th.push(t.elapsed().as_secs_f64() * 1000.0);

        let t = Instant::now();
        rb = via_bitmap(&writes, rows, columns, &mut scratch);
        tb.push(t.elapsed().as_secs_f64() * 1000.0);
    }
    assert_eq!(rs, rh, "sort and hashmap disagree");
    assert_eq!(rs, rb, "sort and bitmap disagree");

    let (s, h, b) = (median(ts), median(th), median(tb));
    println!("writes={count} rows={rows} columns={columns} duplicate_found={rs}\n");
    println!("{:<24} {:>9} {:>12}", "method", "ms", "vs hashmap");
    println!("{:<24} {s:>9.2} {:>11.2}x", "sort (pre-0005)", h / s);
    println!("{:<24} {h:>9.2} {:>11.2}x", "hashmap (post-0005)", 1.0);
    println!("{:<24} {b:>9.2} {:>11.2}x", "bitmap (proposed)", h / b);
    println!(
        "\nbitmap scratch: {:.1} KiB, reused across ticks",
        (rows * columns).div_ceil(64) as f64 * 8.0 / 1024.0
    );
    println!("All arms asserted to agree on whether a duplicate exists.");
}
