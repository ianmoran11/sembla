use sembla_ir::{Prior, PriorFamily, ValidatedModel};
use sembla_runtime::prior::{sample_parameters_for_draw, sample_prior, PRIOR_DRAW_RULE_ID};

fn moments(values: impl Iterator<Item = f64>) -> (f64, f64) {
    let values = values.collect::<Vec<_>>();
    let mean = values.iter().sum::<f64>() / values.len() as f64;
    let variance = values
        .iter()
        .map(|value| (value - mean) * (value - mean))
        .sum::<f64>()
        / values.len() as f64;
    (mean, variance)
}

fn draws(prior: Prior) -> Vec<f64> {
    (0..100_000_u32)
        .map(|draw| sample_prior(&prior, 0x1357_9bdf, draw, 7).unwrap())
        .collect()
}

#[test]
fn sampler_mapping_is_bitwise_frozen() {
    let coordinates = [
        (0x0123_4567_89ab_cdef_u64, 3_u32, 1_u32),
        (0xfedc_ba98_7654_3210_u64, 1_234_u32, 17_u32),
    ];
    let cases = [
        (
            Prior {
                family: PriorFamily::Uniform,
                args: vec![-2.5, 4.25],
            },
            [0xbff5_d9db_f940_3609, 0xc003_1179_71a0_d1fb],
        ),
        (
            Prior {
                family: PriorFamily::Normal,
                args: vec![1.25, 0.75],
            },
            [0x3fe7_257d_7c65_3406, 0x3ffb_40b1_52df_e8ba],
        ),
        (
            Prior {
                family: PriorFamily::LogNormal,
                args: vec![-0.4, 0.3],
            },
            [0x3fe1_6026_d087_5c2c, 0x3fe9_b6e6_f694_c9c1],
        ),
    ];

    // These literals freeze the direct-Uniform mapping, the cosine branch of
    // Box--Muller with draw counters 0 then 1, and LogNormal's exp ordering.
    for (prior, expected) in cases {
        for ((seed, draw, parameter), expected_bits) in coordinates.into_iter().zip(expected) {
            assert_eq!(
                sample_prior(&prior, seed, draw, parameter)
                    .unwrap()
                    .to_bits(),
                expected_bits,
                "{:?} changed at ({seed:#018x}, {draw}, {parameter})",
                prior.family
            );
        }
    }
}

#[test]
fn hundred_thousand_draws_match_family_moments() {
    let uniform = draws(Prior {
        family: PriorFamily::Uniform,
        args: vec![2.0, 6.0],
    });
    let (mean, variance) = moments(uniform.into_iter());
    assert!((mean - 4.0).abs() < 0.02, "uniform mean {mean}");
    assert!(
        (variance - 4.0 / 3.0).abs() < 0.04,
        "uniform variance {variance}"
    );

    let normal = draws(Prior {
        family: PriorFamily::Normal,
        args: vec![1.5, 2.0],
    });
    let (mean, variance) = moments(normal.into_iter());
    assert!((mean - 1.5).abs() < 0.03, "normal mean {mean}");
    assert!((variance - 4.0).abs() < 0.10, "normal variance {variance}");

    let log_normal = draws(Prior {
        family: PriorFamily::LogNormal,
        args: vec![-0.4, 0.7],
    });
    let (log_mean, log_variance) = moments(log_normal.into_iter().map(f64::ln));
    assert!((log_mean - -0.4).abs() < 0.02, "log mean {log_mean}");
    assert!(
        (log_variance - 0.49).abs() < 0.03,
        "log variance {log_variance}"
    );
}

fn sir_model() -> ValidatedModel {
    let source = include_str!("../../../examples/sir.json");
    sembla_ir::validate(sembla_ir::parse_json(source).unwrap()).unwrap()
}

fn values(model: &ValidatedModel, seed: u64, draw: u32) -> Vec<(String, f64)> {
    sample_parameters_for_draw(model, seed, draw, &[])
        .unwrap()
        .values()
        .map(|(name, value)| {
            let value = match value {
                sembla_ir::ParamValue::Real { value } => *value,
                sembla_ir::ParamValue::Int { .. } => panic!("SIR parameters are real"),
            };
            (name.to_owned(), value)
        })
        .collect()
}

#[test]
fn reserved_namespace_and_draw_index_are_sweep_length_independent() {
    let model = sir_model();
    let five = (0..5)
        .map(|draw| values(&model, 99, draw))
        .collect::<Vec<_>>();
    let fifty = (0..50)
        .map(|draw| values(&model, 99, draw))
        .collect::<Vec<_>>();
    assert_eq!(five[3], fifty[3]);
    assert_eq!(PRIOR_DRAW_RULE_ID, u32::MAX);
    assert!(model
        .transitions()
        .iter()
        .all(|transition| transition.rule_id != PRIOR_DRAW_RULE_ID));

    let mut raw = sembla_ir::parse_json(include_str!("../../../examples/sir.json")).unwrap();
    raw.params[1].prior = None;
    let priorless = sembla_ir::validate(raw).unwrap();
    let first = values(&priorless, 1, 0);
    let later = values(&priorless, 999, 42);
    assert_eq!(first[1], ("gamma".to_owned(), 0.1));
    assert_eq!(later[1], first[1], "prior-less values stay at defaults");
}
