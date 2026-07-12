//! Coordinate-keyed Philox4x32-10 random draws.
//!
//! Every draw is a pure function of `(seed, tick, rule_id, entity_id,
//! draw_idx)`, so evaluation order cannot change randomness. The exact packing
//! is:
//!
//! - Philox key word 0 is `seed as u32` (`seed_lo`).
//! - Philox key word 1 is `(seed >> 32) as u32` (`seed_hi`).
//! - Counter words 0 through 3 are `tick`, `rule_id`, `entity_id`, and
//!   `draw_idx`, respectively.
//!
//! This coordinate contract provides common random numbers (CRN): identical
//! coordinates produce identical draws across scenario variants, as described
//! in `DESIGN.md` §5.3. There is deliberately no mutable RNG state or stream.

const PHILOX_M0: u32 = 0xD251_1F53;
const PHILOX_M1: u32 = 0xCD9E_8D57;
const PHILOX_W0: u32 = 0x9E37_79B9;
const PHILOX_W1: u32 = 0xBB67_AE85;
const PHILOX_ROUNDS: usize = 10;

#[inline]
fn round(counter: [u32; 4], key: [u32; 2]) -> [u32; 4] {
    let product_0 = u64::from(PHILOX_M0) * u64::from(counter[0]);
    let product_1 = u64::from(PHILOX_M1) * u64::from(counter[2]);

    let low_0 = product_0 as u32;
    let high_0 = (product_0 >> 32) as u32;
    let low_1 = product_1 as u32;
    let high_1 = (product_1 >> 32) as u32;

    [
        high_1 ^ counter[1] ^ key[0],
        low_1,
        high_0 ^ counter[3] ^ key[1],
        low_0,
    ]
}

/// Returns one Philox4x32-10 block for the supplied coordinates.
///
/// The 64-bit `seed` is packed into the two key words in low-then-high order.
/// The four counter words are, in order, `tick`, `rule_id`, `entity_id`, and
/// `draw_idx`. Philox is counter-based, so this function is pure and does not
/// consume or update a stream.
#[must_use]
pub fn draw_u32x4(seed: u64, tick: u32, rule_id: u32, entity_id: u32, draw_idx: u32) -> [u32; 4] {
    let mut counter = [tick, rule_id, entity_id, draw_idx];
    let mut key = [seed as u32, (seed >> 32) as u32];

    for round_index in 0..PHILOX_ROUNDS {
        counter = round(counter, key);
        if round_index + 1 != PHILOX_ROUNDS {
            key[0] = key[0].wrapping_add(PHILOX_W0);
            key[1] = key[1].wrapping_add(PHILOX_W1);
        }
    }

    counter
}

/// Returns a uniform sample strictly inside `(0, 1)` for the coordinates.
///
/// Lanes 0 and 1 of [`draw_u32x4`] supply a 53-bit integer: all 32 bits of
/// lane 0 followed by the high 21 bits of lane 1. Adding one half and scaling
/// by `2^-53` places samples at bin midpoints. The sole floating-point rounding
/// case that could become `1.0` is clamped to the greatest representable value
/// below one, preserving the documented open interval.
#[must_use]
pub fn uniform_f64(seed: u64, tick: u32, rule_id: u32, entity_id: u32, draw_idx: u32) -> f64 {
    let lanes = draw_u32x4(seed, tick, rule_id, entity_id, draw_idx);
    let mantissa = (u64::from(lanes[0]) << 21) | (u64::from(lanes[1]) >> 11);
    mantissa_to_open_f64(mantissa)
}

#[inline]
fn mantissa_to_open_f64(mantissa: u64) -> f64 {
    debug_assert!(mantissa < (1_u64 << 53));
    let sample = (mantissa as f64 + 0.5) * (1.0 / ((1_u64 << 53) as f64));

    if sample == 1.0 {
        f64::from_bits(1.0_f64.to_bits() - 1)
    } else {
        sample
    }
}

/// Samples an exponential racing-clock delay at rate `lambda`.
///
/// For positive rates this computes `-ln(U) / lambda`, where `U` is the open
/// uniform sample for the same coordinates. Per `DESIGN.md` §4.3, non-positive
/// rates return [`f64::INFINITY`], meaning that the transition never fires.
#[must_use]
pub fn exp_f64(
    seed: u64,
    tick: u32,
    rule_id: u32,
    entity_id: u32,
    draw_idx: u32,
    lambda: f64,
) -> f64 {
    if lambda <= 0.0 {
        f64::INFINITY
    } else {
        -uniform_f64(seed, tick, rule_id, entity_id, draw_idx).ln() / lambda
    }
}

#[cfg(test)]
mod tests {
    use super::mantissa_to_open_f64;

    #[test]
    fn mantissa_conversion_excludes_both_endpoints() {
        let smallest = mantissa_to_open_f64(0);
        let largest = mantissa_to_open_f64((1_u64 << 53) - 1);

        assert!(smallest > 0.0);
        assert!(largest < 1.0);
        assert_eq!(largest, f64::from_bits(1.0_f64.to_bits() - 1));
    }
}
