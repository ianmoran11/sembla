//! Spike: can the racing-clock `ln` be skipped for candidates that do not fire?
//!
//! `executor.rs` computes `race_time = -ln(u)/λ` for every enabled candidate and
//! fires when `race_time < dt`. `log` is 554 of 6088 samples in the post-0004
//! profile — the largest non-allocation item.
//!
//! In exact arithmetic:
//!
//!     -ln(u)/λ < dt   ⟺   ln(u) > -λ·dt   ⟺   u > exp(-λ·dt)
//!
//! Every λ in the benchmark model is constant per transition, so `exp(-λ·dt)` is
//! one call per transition per tick rather than one `ln` per row. Non-firing
//! candidates would need no `ln` at all — and at these λ values that is 97.5% to
//! 99.9% of them.
//!
//! The question is whether the two tests can ever DISAGREE in floating point.
//! If they can, the substitution changes which candidates fire, which is a
//! semantic change and fails the byte-identical constraint outright.
//!
//! Two parts, because they answer different questions:
//!
//!   bulk     — realistic draws. Does it matter in practice?
//!   boundary — u walked ULP by ULP across the threshold. CAN it disagree at all?
//!
//! The boundary sweep is the decisive one. Random sampling essentially never
//! lands within a few ULPs of the threshold, so a clean bulk result proves
//! nothing on its own.
//!
//!   cargo run --release -p sembla-runtime --example ln_threshold_spike

use sembla_runtime::rng::uniform_f64;

/// The model's actual hazard rates. dt = 1.0.
const LAMBDAS: &[(&str, f64)] = &[
    ("mortality_young", 0.001),
    ("emigration_rate", 0.002),
    ("internal_departure", 0.0025),
    ("mortality_adult", 0.003),
    ("mortality_old", 0.012),
    ("internal_arrival", 0.018),
    ("overseas_arrival", 0.02),
    ("birth_rate", 0.025),
    ("age_monthly (1e300)", 1e300),
];

const DT: f64 = 1.0;

fn next_up(x: f64) -> f64 {
    f64::from_bits(x.to_bits() + 1)
}
fn next_down(x: f64) -> f64 {
    f64::from_bits(x.to_bits() - 1)
}

/// What the executor does today.
fn fires_via_ln(u: f64, lambda: f64) -> bool {
    (-u.ln() / lambda) < DT
}

/// The naive substitution. Fast, but its decision comes from `exp` rather than
/// from `ln`, so it is only safe if the two agree on every platform.
fn fires_via_threshold(u: f64, threshold: f64) -> bool {
    u > threshold
}

/// The form actually worth proposing: the threshold is a conservative *filter*,
/// never the decision. Below `lo` a fire is impossible by a margin far wider
/// than any plausible exp/ln disagreement, so `ln` can be skipped. Everything
/// else is decided by exactly the comparison the oracle makes today, so the
/// result is bit-identical by construction on any platform.
fn fires_guarded(u: f64, lambda: f64, lo: f64) -> bool {
    if u < lo {
        return false;
    }
    fires_via_ln(u, lambda)
}

fn main() {
    println!("dt = {DT}\n");
    println!(
        "{:<22} {:>12} {:>11} {:>12} {:>11}",
        "hazard", "threshold", "fire_prob", "bulk_disagr", "ulp_band"
    );

    let bulk_draws: u32 = 2_000_000;
    let mut any_boundary_disagreement = false;

    for (name, lambda) in LAMBDAS {
        let threshold = (-lambda * DT).exp();
        let fire_prob = 1.0 - threshold;

        // --- bulk: real draws from the real generator -----------------------
        let mut bulk_disagreements = 0_u64;
        for i in 0..bulk_draws {
            let u = uniform_f64(0xC0FFEE, 7, 3, i, 0);
            if fires_via_ln(u, *lambda) != fires_via_threshold(u, threshold) {
                bulk_disagreements += 1;
            }
        }

        // --- boundary: walk ULP by ULP across the threshold -----------------
        // Adversarial by construction. If the two tests can ever differ, this
        // is where it happens.
        let mut band = 0_u32;
        let mut u = threshold;
        for _ in 0..4096 {
            u = next_down(u);
            if u <= 0.0 {
                break;
            }
            if fires_via_ln(u, *lambda) != fires_via_threshold(u, threshold) {
                band += 1;
            }
        }
        let mut u = threshold;
        for _ in 0..4096 {
            u = next_up(u);
            if u >= 1.0 {
                break;
            }
            if fires_via_ln(u, *lambda) != fires_via_threshold(u, threshold) {
                band += 1;
            }
        }
        if band > 0 {
            any_boundary_disagreement = true;
        }

        println!(
            "{:<22} {:>12.6e} {:>10.4}% {:>12} {:>11}",
            name,
            threshold,
            fire_prob * 100.0,
            bulk_disagreements,
            band
        );
    }

    println!("\nbulk_disagr: disagreements over {bulk_draws} real draws per hazard");
    println!("ulp_band:    disagreements found while walking 4096 ULPs either side");

    // --- how big is the prize? -------------------------------------------
    let n = 5_000_000_u32;
    let lambda = 0.003_f64;
    let threshold = (-lambda * DT).exp();
    let draws: Vec<f64> = (0..n).map(|i| uniform_f64(0xC0FFEE, 7, 3, i, 0)).collect();

    let t0 = std::time::Instant::now();
    let mut fired = 0_u64;
    for u in &draws {
        if fires_via_ln(*u, lambda) {
            fired += 1;
        }
    }
    let ln_ms = t0.elapsed().as_secs_f64() * 1000.0;

    let t0 = std::time::Instant::now();
    let mut fired2 = 0_u64;
    for u in &draws {
        if fires_via_threshold(*u, threshold) {
            fired2 += 1;
        }
    }
    let thr_ms = t0.elapsed().as_secs_f64() * 1000.0;
    assert_eq!(fired, fired2, "arms disagreed on the fired count");

    // The guarded form: margin 1e-12 relative, ~4500 ULPs, versus the ~1 ULP
    // at issue. Wide enough that no plausible exp/ln disagreement reaches it.
    let lo = threshold * (1.0 - 1e-12);
    let t0 = std::time::Instant::now();
    let mut fired3 = 0_u64;
    let mut ln_calls = 0_u64;
    for u in &draws {
        if *u >= lo {
            ln_calls += 1;
        }
        if fires_guarded(*u, lambda, lo) {
            fired3 += 1;
        }
    }
    let guarded_ms = t0.elapsed().as_secs_f64() * 1000.0;
    assert_eq!(
        fired, fired3,
        "guarded form changed the firing decision; it must not"
    );

    println!(
        "\nprize, {n} rows at lambda={lambda}: ln {ln_ms:.1} ms vs threshold {thr_ms:.1} ms \
         ({:.1}x), {fired} fired ({:.2}%)",
        ln_ms / thr_ms,
        fired as f64 / n as f64 * 100.0
    );

    println!(
        "guarded form: {guarded_ms:.1} ms ({:.1}x), ln computed for {ln_calls} of {n} rows ({:.2}%)",
        ln_ms / guarded_ms,
        ln_calls as f64 / n as f64 * 100.0
    );

    println!(
        "\nCAVEAT that decides this. `rng::exp_f64`'s `f64::ln` is DECISIONS.md\n\
         §E7's documented exemption — it is the platform's, not the pinned libm.\n\
         §E7 exists because CI found a one-ULP cross-platform difference between\n\
         macOS/aarch64 and Linux/x86_64 in exactly this family of functions.\n\
         \n\
         So the clean result above is from ONE platform. The naive substitution\n\
         would make firing depend on this platform's `exp` agreeing with this\n\
         platform's `ln`, and the differential harness runs on Linux/x86_64.\n\
         \n\
         The guarded form needs no such agreement: every decision near the\n\
         boundary is still made by `ln`, so it is bit-identical by construction\n\
         and the cross-platform question never arises."
    );

    if any_boundary_disagreement {
        println!(
            "\nVERDICT: the substitution is NOT bit-identical. The two tests differ\n\
             within a few ULPs of the threshold, so swapping them outright would\n\
             change which candidates fire.\n\
             \n\
             Viable form: use the threshold as a conservative filter with a margin,\n\
             and compute the exact `ln` only inside the uncertain band. The band is\n\
             a handful of ULPs wide, so essentially no draw lands in it — the `ln`\n\
             is still avoided for effectively every non-firing candidate, and the\n\
             firing decision stays exactly the one the oracle makes today."
        );
    } else {
        println!(
            "\nVERDICT: no disagreement found, including adversarially at the\n\
             boundary. That is evidence, not proof — an interval argument would\n\
             still be needed before relying on it."
        );
    }
}
