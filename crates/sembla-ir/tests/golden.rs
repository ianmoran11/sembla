use sembla_ir::{parse_json, to_canonical_json, validate, SummaryReduce, ViewReduce};

const VALID: &str = include_str!("../../../examples/two_state.json");
const OBSERVATIONS: &str = include_str!("../../../examples/observations.json");

const VALID_EXAMPLES: [(&str, &str); 11] = [
    (
        "noisy_voter.json",
        include_str!("../../../examples/noisy_voter.json"),
    ),
    ("observations.json", OBSERVATIONS),
    (
        "radioactive_decay_chain.json",
        include_str!("../../../examples/radioactive_decay_chain.json"),
    ),
    (
        "reversible_ctmc.json",
        include_str!("../../../examples/reversible_ctmc.json"),
    ),
    (
        "seirs_waning.json",
        include_str!("../../../examples/seirs_waning.json"),
    ),
    ("sir.json", include_str!("../../../examples/sir.json")),
    (
        "sir_policy.json",
        include_str!("../../../examples/sir_policy.json"),
    ),
    (
        "sis_importation.json",
        include_str!("../../../examples/sis_importation.json"),
    ),
    (
        "two_box.json",
        include_str!("../../../examples/two_box.json"),
    ),
    (
        "two_box_merged.json",
        include_str!("../../../examples/two_box_merged.json"),
    ),
    ("two_state.json", VALID),
];

#[test]
fn valid_fixture_is_canonical_golden_json() {
    let model = parse_json(VALID).expect("golden fixture must parse");
    let validated = validate(model).expect("golden fixture must validate");
    let canonical = to_canonical_json(validated.model()).expect("model must serialize");
    assert_eq!(canonical.as_bytes(), VALID.as_bytes());
}

#[test]
fn observation_fixture_is_canonical_golden_json() {
    let model = parse_json(OBSERVATIONS).expect("observation fixture must parse");
    let validated = validate(model).expect("observation fixture must validate");
    let canonical = to_canonical_json(validated.model()).expect("model must serialize");
    assert_eq!(canonical.as_bytes(), OBSERVATIONS.as_bytes());
}

#[test]
fn every_valid_example_validates_and_is_canonical() {
    for (name, source) in VALID_EXAMPLES {
        let model = parse_json(source).unwrap_or_else(|error| panic!("{name}: {error}"));
        let validated = validate(model).unwrap_or_else(|error| panic!("{name}: {error}"));
        let canonical = to_canonical_json(validated.model()).unwrap();
        assert_eq!(canonical.as_bytes(), source.as_bytes(), "{name}");
    }
}

#[test]
fn canonical_serialization_is_idempotent() {
    for source in [VALID, OBSERVATIONS] {
        let first = to_canonical_json(&parse_json(source).unwrap()).unwrap();
        let second = to_canonical_json(&parse_json(&first).unwrap()).unwrap();
        assert_eq!(first.as_bytes(), second.as_bytes());
    }
}

#[test]
fn observation_reduce_wire_names_are_frozen() {
    let view_reduces = [
        (ViewReduce::Sum, "\"sum\""),
        (ViewReduce::Count, "\"count\""),
        (ViewReduce::Min, "\"min\""),
        (ViewReduce::Max, "\"max\""),
    ];
    for (reduce, expected) in view_reduces {
        assert_eq!(serde_json::to_string(&reduce).unwrap(), expected);
    }

    let summary_reduces = [
        (SummaryReduce::Sum, "\"sum\""),
        (SummaryReduce::Min, "\"min\""),
        (SummaryReduce::Max, "\"max\""),
        (SummaryReduce::Last, "\"last\""),
        (SummaryReduce::ArgmaxTick, "\"argmax_tick\""),
    ];
    for (reduce, expected) in summary_reduces {
        assert_eq!(serde_json::to_string(&reduce).unwrap(), expected);
    }
}

#[test]
fn omitted_observation_fields_default_to_empty() {
    let model = parse_json(
        r#"{"name":"legacy","dt":1.0,"params":[],"boxes":[{"name":"box","tables":[],"transitions":[],"inputs":[],"outputs":[]}],"wires":[]}"#,
    )
    .expect("views-free legacy model must parse");
    assert!(model.summaries.is_empty());
    assert!(model.boxes[0].views.is_empty());
    validate(model).expect("views-free legacy model must validate");
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
