use std::path::{Path, PathBuf};

use sembla_ir::{
    is_slug, mailbox_identity, parse_input, rule_word, to_canonical_string, validate_plan,
    validate_rule_words, ExecutablePlanV1, HashRecordV1, LinkedProvenanceV1, LinkerDescriptorV1,
    ParsedInput, PlanOrigin,
};
use serde::Deserialize;

fn repository_path(relative: impl AsRef<Path>) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join(relative)
}

fn input_error(relative: &str) -> String {
    let source = std::fs::read_to_string(repository_path(relative)).unwrap();
    match parse_input(&source) {
        Err(error) => error,
        Ok(ParsedInput::LegacyModel(_)) => "unexpected legacy dispatch".to_owned(),
        Ok(ParsedInput::Plan(plan)) => match validate_plan(&plan) {
            Err(error) => error.to_string(),
            Ok(_) => {
                let value: serde_json::Value = serde_json::from_str(&source).unwrap();
                if to_canonical_string(&value).unwrap() != source {
                    "plan file is not canonical".to_owned()
                } else {
                    "unexpected validation success".to_owned()
                }
            }
        },
    }
}

#[test]
fn dispatch_uses_schema_version_presence() {
    let legacy = std::fs::read_to_string(repository_path("examples/two_box.json")).unwrap();
    assert!(matches!(
        parse_input(&legacy).unwrap(),
        ParsedInput::LegacyModel(_)
    ));

    let plan =
        std::fs::read_to_string(repository_path("fixtures/plans/two_box.plan.json")).unwrap();
    assert!(matches!(parse_input(&plan).unwrap(), ParsedInput::Plan(_)));

    let unknown = input_error("fixtures/plans/invalid/unknown_schema_version.plan.json");
    assert!(unknown.contains("sembla.executable-plan/v2"), "{unknown}");
    assert!(unknown.contains("sembla.executable-plan/v1"), "{unknown}");
}

#[test]
fn every_negative_plan_has_its_specific_error() {
    let fixtures = [
        (
            "unknown_schema_version.plan.json",
            "unsupported schema_version 'sembla.executable-plan/v2'",
        ),
        (
            "unknown_identity_scheme.plan.json",
            "unsupported identity_scheme 'sembla.identity/future-v2'",
        ),
        ("enabled_feature.plan.json", "future_feature"),
        ("missing_leaf.plan.json", "leaf map must be a bijection"),
        (
            "extra_transition.plan.json",
            "transition map must be a bijection",
        ),
        (
            "wrong_rule_word.plan.json",
            "does not match recomputed value",
        ),
        (
            "wrong_occurrence.plan.json",
            "does not match 'occ:controller'",
        ),
        (
            "unsorted_model_boxes.plan.json",
            "model boxes must be sorted by name",
        ),
        (
            "provenance_on_direct.plan.json",
            "direct_stable origin forbids linked_provenance",
        ),
        (
            "linked_without_provenance.plan.json",
            "linked origin requires linked_provenance",
        ),
        (
            "unknown_top_level.plan.json",
            "unknown field `unknown_field`",
        ),
        ("noncanonical.plan.json", "plan file is not canonical"),
    ];

    for (fixture, expected) in fixtures {
        let error = input_error(&format!("fixtures/plans/invalid/{fixture}"));
        assert!(error.contains(expected), "{fixture}: {error}");
    }
}

#[test]
fn reserved_and_duplicate_rule_words_are_rejected_directly() {
    let reserved = validate_rule_words(&[u32::MAX - 1]).unwrap_err();
    assert!(reserved.message.contains("reserved"));

    let duplicate = validate_rule_words(&[7, 11, 7]).unwrap_err();
    assert!(duplicate.message.contains("duplicate rule word 7"));
    assert!(duplicate.message.contains("transition index 0"));
}

#[test]
fn slug_validation_is_ascii_and_strict() {
    for valid in ["a", "alpha", "alpha_2", "z0"] {
        assert!(is_slug(valid), "{valid}");
    }
    for invalid in ["", "_alpha", "2alpha", "Alpha", "alpha-beta", "é"] {
        assert!(!is_slug(invalid), "{invalid}");
    }
}

#[test]
fn mailbox_identity_uses_the_frozen_format() {
    assert_eq!(
        mailbox_identity(
            "occ:#wire:to_controller_infection",
            "population",
            "infection",
            "controller",
            "infection",
        ),
        "mbox:occ:#wire:to_controller_infection|occ:population.port:infection|occ:controller.port:infection"
    );
}

#[test]
fn linked_provenance_fields_are_frozen() {
    let source =
        std::fs::read_to_string(repository_path("fixtures/plans/two_box.plan.json")).unwrap();
    let ParsedInput::Plan(mut plan): ParsedInput = parse_input(&source).unwrap() else {
        panic!("versioned fixture dispatched as legacy");
    };
    plan.origin = PlanOrigin::Linked;
    plan.linked_provenance = Some(LinkedProvenanceV1 {
        source_hash: HashRecordV1 {
            algorithm: "sha256".to_owned(),
            domain: "sembla.source-artifact/v1".to_owned(),
            digest: "0".repeat(64),
        },
        linker: LinkerDescriptorV1 {
            semantics: "sembla.linker/v1".to_owned(),
            source_schema: "sembla.composition-source/v1".to_owned(),
            plan_schema: "sembla.executable-plan/v1".to_owned(),
            identity_scheme: "sembla.identity/stable-v1".to_owned(),
            canonical_encoding: "sembla.canonical-json/v1".to_owned(),
            source_map_schema: "sembla.source-map/v1".to_owned(),
        },
        source_map: serde_json::json!({}),
    });
    validate_plan(&plan).unwrap();

    let mut wrong_descriptor: ExecutablePlanV1 = plan.clone();
    let semantics = &mut wrong_descriptor
        .linked_provenance
        .as_mut()
        .unwrap()
        .linker
        .semantics;
    semantics.clear();
    semantics.push_str("sembla.linker/v2");
    assert_eq!(
        validate_plan(&wrong_descriptor).unwrap_err().path,
        "$.linked_provenance.linker.semantics"
    );

    plan.linked_provenance.as_mut().unwrap().source_hash.digest = "A".repeat(64);
    assert_eq!(
        validate_plan(&plan).unwrap_err().path,
        "$.linked_provenance.source_hash.digest"
    );
}

#[derive(Deserialize)]
struct HashVectors {
    rule_words: Vec<RuleWordVector>,
}

#[derive(Deserialize)]
struct RuleWordVector {
    identity: String,
    word: u32,
}

#[test]
fn legacy_model_rule_words_equal_dense_ordinals() {
    let source = std::fs::read_to_string(repository_path("examples/two_box.json")).unwrap();
    let ParsedInput::LegacyModel(model) = parse_input(&source).unwrap() else {
        panic!("legacy fixture dispatched as a plan")
    };
    let validated = sembla_ir::validate(model).unwrap();
    assert!(validated
        .transitions()
        .iter()
        .all(|transition| transition.rule_word == transition.rule_id));
}

#[test]
fn rust_rule_words_match_the_cross_language_fixture() {
    let source = std::fs::read_to_string(repository_path("fixtures/hash/vectors.json")).unwrap();
    let vectors: HashVectors = serde_json::from_str(&source).unwrap();
    assert_eq!(vectors.rule_words.len(), 5);
    for vector in vectors.rule_words {
        assert_eq!(
            rule_word(&vector.identity),
            vector.word,
            "{}",
            vector.identity
        );
    }
}
