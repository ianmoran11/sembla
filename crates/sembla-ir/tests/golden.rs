use sembla_ir::{parse_json, to_canonical_json, validate};

const VALID: &str = include_str!("../../../examples/two_state.json");

#[test]
fn valid_fixture_is_canonical_golden_json() {
    let model = parse_json(VALID).expect("golden fixture must parse");
    let validated = validate(model).expect("golden fixture must validate");
    let canonical = to_canonical_json(validated.model()).expect("model must serialize");
    assert_eq!(canonical.as_bytes(), VALID.as_bytes());
}

#[test]
fn canonical_serialization_is_idempotent() {
    let first = to_canonical_json(&parse_json(VALID).unwrap()).unwrap();
    let second = to_canonical_json(&parse_json(&first).unwrap()).unwrap();
    assert_eq!(first.as_bytes(), second.as_bytes());
}

#[test]
fn invalid_fixtures_report_the_offending_path() {
    let cases = [
        (
            "../../../examples/invalid/unresolved_param.json",
            include_str!("../../../examples/invalid/unresolved_param.json"),
            "$.boxes[0].transitions[0].hazard.name",
            "missing_rate",
        ),
        (
            "../../../examples/invalid/duplicate_param.json",
            include_str!("../../../examples/invalid/duplicate_param.json"),
            "$.params[1].name",
            "duplicate parameter",
        ),
        (
            "../../../examples/invalid/bad_prior_arity.json",
            include_str!("../../../examples/invalid/bad_prior_arity.json"),
            "$.params[0].prior.args",
            "exactly 2",
        ),
        (
            "../../../examples/invalid/wrong_guard_type.json",
            include_str!("../../../examples/invalid/wrong_guard_type.json"),
            "$.boxes[0].transitions[0].guard",
            "expected Bool",
        ),
        (
            "../../../examples/invalid/unknown_enum_variant.json",
            include_str!("../../../examples/invalid/unknown_enum_variant.json"),
            "$.boxes[0].transitions[0].guard.variant",
            "Excited",
        ),
        (
            "../../../examples/invalid/unknown_effect_attr.json",
            include_str!("../../../examples/invalid/unknown_effect_attr.json"),
            "$.boxes[0].transitions[0].effects[0].attr",
            "missing",
        ),
    ];

    for (fixture, source, expected_path, expected_message) in cases {
        let model = parse_json(source).unwrap_or_else(|error| panic!("{fixture}: {error}"));
        let error = validate(model).expect_err(fixture);
        assert_eq!(error.path, expected_path, "{fixture}");
        assert!(
            error.message.contains(expected_message),
            "{fixture}: {error}"
        );
    }
}
