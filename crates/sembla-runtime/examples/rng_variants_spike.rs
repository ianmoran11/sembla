//! Spike: cheaper randomness — and what it actually costs you.
//!
//! The framing worth correcting first. `DECISIONS.md` §E1 says the load-bearing
//! property is that a draw is "a pure function of
//! (seed, tick, rule_id, entity_id, draw_idx)", which makes randomness
//! order-independent so that "reproducibility reduces *entirely* to
//! floating-point reduction order". That property belongs to the *counter-based
//! construction*, not to Philox.
//!
//! So a cheaper counter-based mixer keeps **every** determinism guarantee at
//! **every** §E2 level. Same seed, same draws, independent of thread count,
//! scheduling, or backend. It is not a departure from replicability at all.
//!
//! What it does change is the *stream* — the same seed yields different numbers,
//! exactly as a different seed would — and the **statistical quality** of that
//! stream. That is the real axis, and it is the one measured here.
//!
//! (§E2's existing levels cannot express this: they explicitly keep the same
//! draws and vary only FP accumulation. A cheaper mixer would need a recorded
//! choice of its own, and every frozen vector regenerated.)
//!
//! Variants, all counter-based and all fully reproducible:
//!   philox-10  current; the Random123 safety margin
//!   philox-7   the authors' stated minimum for passing BigCrush
//!   philox-4   deliberately too few rounds, as a control
//!   mix64      one splitmix64-style finalizer over a packed counter
//!
//! Quality screen — crude, not a substitute for TestU01, but enough to catch
//! gross failure. The avalanche test is the one that matters: adjacent entities
//! have counters differing by one, and a weak mixer will correlate their draws.
//!
//!   cargo run --release -p sembla-runtime --example rng_variants_spike -- [draws]

use std::time::Instant;

const M0: u32 = 0xD251_1F53;
const M1: u32 = 0xCD9E_8D57;
const W0: u32 = 0x9E37_79B9;
const W1: u32 = 0xBB67_AE85;

fn philox_round(counter: [u32; 4], key: [u32; 2]) -> [u32; 4] {
    let p0 = u64::from(M0) * u64::from(counter[0]);
    let p1 = u64::from(M1) * u64::from(counter[2]);
    let (l0, h0) = (p0 as u32, (p0 >> 32) as u32);
    let (l1, h1) = (p1 as u32, (p1 >> 32) as u32);
    [h1 ^ counter[1] ^ key[0], l1, h0 ^ counter[3] ^ key[1], l0]
}

fn philox(seed: u64, tick: u32, rule: u32, entity: u32, idx: u32, rounds: usize) -> [u32; 4] {
    let mut counter = [tick, rule, entity, idx];
    let mut key = [seed as u32, (seed >> 32) as u32];
    for r in 0..rounds {
        counter = philox_round(counter, key);
        if r + 1 != rounds {
            key[0] = key[0].wrapping_add(W0);
            key[1] = key[1].wrapping_add(W1);
        }
    }
    counter
}

/// One splitmix64 finalizer over a packed coordinate. Still a pure function of
/// the coordinates, so still order-independent and still reproducible.
fn mix64(seed: u64, tick: u32, rule: u32, entity: u32, idx: u32) -> u64 {
    let mut z = seed
        ^ (u64::from(tick) << 48)
        ^ (u64::from(rule) << 32)
        ^ (u64::from(entity) << 8)
        ^ u64::from(idx);
    z = z.wrapping_add(0x9E37_79B9_7F4A_7C15);
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^ (z >> 31)
}

fn mantissa_to_open(mantissa: u64) -> f64 {
    let sample = (mantissa as f64 + 0.5) * (1.0 / ((1_u64 << 53) as f64));
    if sample == 1.0 {
        f64::from_bits(1.0_f64.to_bits() - 1)
    } else {
        sample
    }
}

fn draw(variant: u8, entity: u32) -> (f64, u64) {
    match variant {
        0..=2 => {
            let rounds = match variant {
                0 => 10,
                1 => 7,
                _ => 4,
            };
            let lanes = philox(0xC0FF_EE00_1234_5678, 7, 3, entity, 0, rounds);
            let m = (u64::from(lanes[0]) << 21) | (u64::from(lanes[1]) >> 11);
            (
                mantissa_to_open(m),
                u64::from(lanes[0]) << 32 | u64::from(lanes[1]),
            )
        }
        _ => {
            let bits = mix64(0xC0FF_EE00_1234_5678, 7, 3, entity, 0);
            (mantissa_to_open(bits >> 11), bits)
        }
    }
}

fn main() {
    let draws: u32 = std::env::args()
        .nth(1)
        .and_then(|a| a.parse().ok())
        .unwrap_or(4_000_000);

    println!("draws={draws}\n");
    println!(
        "{:<12} {:>9} {:>8} {:>12} {:>12} {:>12}",
        "variant", "ns/draw", "speedup", "chi2_p_ok", "avalanche", "serial_corr"
    );

    let mut base_ns = 0.0;
    for (variant, name) in [
        (0_u8, "philox-10"),
        (1, "philox-7"),
        (2, "philox-4"),
        (3, "mix64"),
    ] {
        // --- speed ---
        let mut times = Vec::new();
        for _ in 0..5 {
            let t = Instant::now();
            let mut sink = 0.0_f64;
            for e in 0..draws {
                sink += draw(variant, e).0;
            }
            std::hint::black_box(sink);
            times.push(t.elapsed().as_secs_f64() * 1.0e9 / draws as f64);
        }
        times.sort_by(f64::total_cmp);
        let ns = times[times.len() / 2];
        if variant == 0 {
            base_ns = ns;
        }

        // --- uniformity: chi-square over 256 buckets ---
        let mut buckets = [0_u64; 256];
        for e in 0..draws {
            let u = draw(variant, e).0;
            buckets[(u * 256.0) as usize & 255] += 1;
        }
        let expected = draws as f64 / 256.0;
        let chi2: f64 = buckets
            .iter()
            .map(|b| {
                let d = *b as f64 - expected;
                d * d / expected
            })
            .sum();
        // 255 df: the 0.1%..99.9% band is roughly 180..330.
        let chi_ok = (180.0..330.0).contains(&chi2);

        // --- avalanche: adjacent counters must decorrelate ---
        // Counter-based RNG feeds entity_id straight in, so entity e and e+1
        // differ by one bit-ish. A good mixer flips ~half the output bits.
        let sample = 200_000_u32.min(draws - 1);
        let mut hamming = 0_u64;
        for e in 0..sample {
            let a = draw(variant, e).1;
            let b = draw(variant, e + 1).1;
            hamming += (a ^ b).count_ones() as u64;
        }
        let avalanche = hamming as f64 / sample as f64;

        // --- serial correlation of consecutive draws ---
        let n = 500_000_u32.min(draws - 1);
        let (mut sx, mut sy, mut sxy, mut sxx, mut syy) = (0.0, 0.0, 0.0, 0.0, 0.0);
        for e in 0..n {
            let x = draw(variant, e).0;
            let y = draw(variant, e + 1).0;
            sx += x;
            sy += y;
            sxy += x * y;
            sxx += x * x;
            syy += y * y;
        }
        let nf = n as f64;
        let corr = (nf * sxy - sx * sy) / (((nf * sxx - sx * sx) * (nf * syy - sy * sy)).sqrt());

        println!(
            "{name:<12} {ns:>9.2} {:>7.2}x {:>12} {:>12.1} {:>12.5}",
            base_ns / ns,
            if chi_ok { "pass" } else { "FAIL" },
            avalanche,
            corr
        );
    }

    println!(
        "\navalanche: mean bits differing between adjacent entities, of 64.\n\
         ~32 is ideal; well below that means adjacent agents get correlated draws.\n\
         serial_corr: Pearson r between consecutive draws; want ~0.\n\
         \n\
         All variants are counter-based, so all are fully reproducible and\n\
         order-independent. The trade is stream quality, not replicability."
    );
}
