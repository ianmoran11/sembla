use std::collections::HashSet;

use sembla_runtime::rng::{draw_u32x4, exp_f64, uniform_f64};

#[test]
fn philox4x32_10_matches_random123_known_answers() {
    // Random123 tests/kat_vectors: philox4x32, 10 rounds.
    assert_eq!(
        draw_u32x4(0, 0, 0, 0, 0),
        [0x6627_e8d5, 0xe169_c58d, 0xbc57_ac4c, 0x9b00_dbd8]
    );
    assert_eq!(
        draw_u32x4(u64::MAX, u32::MAX, u32::MAX, u32::MAX, u32::MAX),
        [0x408f_276d, 0x41c8_3b0e, 0xa20b_c7c6, 0x6d54_51fd]
    );
    // The asymmetric Random123 vector also freezes Sembla's word packing.
    assert_eq!(
        draw_u32x4(
            0x299f_31d0_a409_3822,
            0x243f_6a88,
            0x85a3_08d3,
            0x1319_8a2e,
            0x0370_7344,
        ),
        [0xd16c_fe09, 0x94fd_cceb, 0x5001_e420, 0x2412_6ea1]
    );
}

#[test]
fn public_draws_are_pure_and_coordinate_changes_do_not_collide() {
    let coordinates = (0x0123_4567_89ab_cdef, 17, 23, 42, 5);
    let block = draw_u32x4(
        coordinates.0,
        coordinates.1,
        coordinates.2,
        coordinates.3,
        coordinates.4,
    );
    assert_eq!(
        block,
        draw_u32x4(
            coordinates.0,
            coordinates.1,
            coordinates.2,
            coordinates.3,
            coordinates.4,
        )
    );

    let uniform = uniform_f64(
        coordinates.0,
        coordinates.1,
        coordinates.2,
        coordinates.3,
        coordinates.4,
    );
    assert_eq!(
        uniform,
        uniform_f64(
            coordinates.0,
            coordinates.1,
            coordinates.2,
            coordinates.3,
            coordinates.4,
        )
    );

    let exponential = exp_f64(
        coordinates.0,
        coordinates.1,
        coordinates.2,
        coordinates.3,
        coordinates.4,
        2.0,
    );
    assert_eq!(
        exponential,
        exp_f64(
            coordinates.0,
            coordinates.1,
            coordinates.2,
            coordinates.3,
            coordinates.4,
            2.0,
        )
    );

    let mut outputs = HashSet::with_capacity(6_000);
    for index in 0_u32..1_000 {
        let seed = 0x243f_6a88_85a3_08d3 ^ u64::from(index);
        let tick = index;
        let rule_id = index.rotate_left(7);
        let entity_id = index.wrapping_mul(0x9e37_79b9);
        let draw_idx = index.rotate_right(11);
        let variants = [
            draw_u32x4(seed, tick, rule_id, entity_id, draw_idx),
            draw_u32x4(seed ^ (1_u64 << 63), tick, rule_id, entity_id, draw_idx),
            draw_u32x4(seed, tick ^ (1_u32 << 31), rule_id, entity_id, draw_idx),
            draw_u32x4(seed, tick, rule_id ^ (1_u32 << 31), entity_id, draw_idx),
            draw_u32x4(seed, tick, rule_id, entity_id ^ (1_u32 << 31), draw_idx),
            draw_u32x4(seed, tick, rule_id, entity_id, draw_idx ^ (1_u32 << 31)),
        ];

        for output in variants {
            assert!(outputs.insert(output), "full 128-bit output collision");
        }
    }
}

#[test]
fn uniform_samples_are_open_and_well_distributed() {
    const SAMPLE_COUNT: u32 = 1_000_000;
    const BUCKET_COUNT: usize = 100;

    let mut sum = 0.0;
    let mut histogram = [0_u32; BUCKET_COUNT];
    for draw_idx in 0..SAMPLE_COUNT {
        let sample = uniform_f64(0xa409_3822_299f_31d0, 11, 7, 19, draw_idx);
        assert!(sample > 0.0 && sample < 1.0);
        sum += sample;
        histogram[(sample * BUCKET_COUNT as f64) as usize] += 1;
    }

    let mean = sum / f64::from(SAMPLE_COUNT);
    assert!((mean - 0.5).abs() <= 0.002, "uniform mean was {mean}");

    let expected = f64::from(SAMPLE_COUNT) / BUCKET_COUNT as f64;
    for (index, count) in histogram.into_iter().enumerate() {
        let relative_deviation = (f64::from(count) - expected).abs() / expected;
        assert!(
            relative_deviation <= 0.05,
            "uniform bucket {index} had {count} samples ({relative_deviation:.3} deviation)"
        );
    }
}

#[test]
fn exponential_samples_have_the_expected_mean_and_zero_rate_never_fires() {
    const SAMPLE_COUNT: u32 = 1_000_000;

    let mut sum = 0.0;
    for draw_idx in 0..SAMPLE_COUNT {
        sum += exp_f64(0x1319_8a2e_0370_7344, 29, 31, 37, draw_idx, 2.0);
    }

    let mean = sum / f64::from(SAMPLE_COUNT);
    assert!((mean - 0.5).abs() <= 0.005, "exponential mean was {mean}");
    assert!(exp_f64(1, 2, 3, 4, 5, 0.0).is_infinite());
    assert!(exp_f64(1, 2, 3, 4, 5, -1.0).is_infinite());
}
