use super::{exp_f64, exp_f64_from_uniform, mantissa_to_open_f64, uniform_f64};

#[test]
fn mantissa_conversion_excludes_both_endpoints() {
    let smallest = mantissa_to_open_f64(0);
    let largest = mantissa_to_open_f64((1_u64 << 53) - 1);

    assert!(smallest > 0.0);
    assert!(largest < 1.0);
    assert_eq!(largest, f64::from_bits(1.0_f64.to_bits() - 1));
}

#[test]
fn split_exponential_transform_is_bit_identical() {
    for lambda in [0.001, 0.025, 1.0, 1e300] {
        for (seed, tick, rule_word, entity_id) in [
            (0, 0, 0, 0),
            (0xC0FFEE, 7, 19, 65_536),
            (u64::MAX, u32::MAX, u32::MAX - 2, u32::MAX),
        ] {
            let uniform = uniform_f64(seed, tick, rule_word, entity_id, 0);
            assert_eq!(
                exp_f64_from_uniform(uniform, lambda).to_bits(),
                exp_f64(seed, tick, rule_word, entity_id, 0, lambda).to_bits()
            );
        }
    }
}
