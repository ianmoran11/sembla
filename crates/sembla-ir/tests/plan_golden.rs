use std::path::{Path, PathBuf};

use sembla_ir::{
    mailbox_identity, occurrence_of_leaf, parse_input, plan_envelope_hash, plan_semantic_hash,
    rule_word, to_canonical_string, transition_identity, validate_plan, ExecutablePlanV1,
    HashRecordV1, IdentityMapV1, LeafIdentityV1, MailboxIdentityV1, ParsedInput, PlanOrigin,
    SchedulerDomainV1, TransitionIdentityV1, EXECUTABLE_PLAN_SCHEMA, STABLE_IDENTITY_SCHEME,
};

fn repository_path(relative: impl AsRef<Path>) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join(relative)
}

fn direct_wire_occurrence(target_box: &str, target_port: &str) -> String {
    format!("occ:#wire:to_{target_box}_{target_port}")
}

fn canonicalize_model(model: &mut sembla_ir::Model) {
    model
        .params
        .sort_by(|left, right| left.name.cmp(&right.name));
    model
        .boxes
        .sort_by(|left, right| left.name.cmp(&right.name));
    for model_box in &mut model.boxes {
        model_box
            .tables
            .sort_by(|left, right| left.name.cmp(&right.name));
        model_box
            .transitions
            .sort_by(|left, right| left.name.cmp(&right.name));
        model_box
            .inputs
            .sort_by(|left, right| left.name.cmp(&right.name));
        model_box
            .outputs
            .sort_by(|left, right| left.name.cmp(&right.name));
        model_box
            .views
            .sort_by(|left, right| left.name.cmp(&right.name));
    }
    model
        .summaries
        .sort_by(|left, right| left.name.cmp(&right.name));
    model.wires.sort_by_cached_key(|wire| {
        mailbox_identity(
            &direct_wire_occurrence(&wire.to.r#box, &wire.to.port),
            &wire.from.r#box,
            &wire.from.port,
            &wire.to.r#box,
            &wire.to.port,
        )
    });
}

fn build_direct_stable_plan(model: sembla_ir::Model) -> ExecutablePlanV1 {
    let mut leaves: Vec<LeafIdentityV1> = model
        .boxes
        .iter()
        .map(|model_box| LeafIdentityV1 {
            r#box: model_box.name.clone(),
            occurrence: occurrence_of_leaf(&model_box.name),
        })
        .collect();
    leaves.sort_by(|left, right| left.r#box.cmp(&right.r#box));

    let mut transitions = Vec::new();
    for model_box in &model.boxes {
        for transition in &model_box.transitions {
            let identity =
                transition_identity(&occurrence_of_leaf(&model_box.name), &transition.name);
            transitions.push(TransitionIdentityV1 {
                r#box: model_box.name.clone(),
                name: transition.name.clone(),
                rule_word: rule_word(&identity),
                identity,
            });
        }
    }
    transitions.sort_by(|left, right| left.identity.cmp(&right.identity));

    let mut mailboxes: Vec<MailboxIdentityV1> = model
        .wires
        .iter()
        .map(|wire| MailboxIdentityV1 {
            identity: mailbox_identity(
                &direct_wire_occurrence(&wire.to.r#box, &wire.to.port),
                &wire.from.r#box,
                &wire.from.port,
                &wire.to.r#box,
                &wire.to.port,
            ),
            source_box: wire.from.r#box.clone(),
            source_port: wire.from.port.clone(),
            target_box: wire.to.r#box.clone(),
            target_port: wire.to.port.clone(),
        })
        .collect();
    mailboxes.sort_by(|left, right| left.identity.cmp(&right.identity));

    let scheduler_leaves = leaves.iter().map(|leaf| leaf.r#box.clone()).collect();
    ExecutablePlanV1 {
        schema_version: EXECUTABLE_PLAN_SCHEMA.to_owned(),
        identity_scheme: STABLE_IDENTITY_SCHEME.to_owned(),
        origin: PlanOrigin::DirectStable,
        identity: IdentityMapV1 {
            model_id: format!("model:{}", model.name),
            enabled_features: Vec::new(),
            scheduler_domains: vec![SchedulerDomainV1 {
                id: "domain:global".to_owned(),
                algorithm: "tau_leap".to_owned(),
                leaves: scheduler_leaves,
            }],
            leaves,
            transitions,
            mailboxes,
        },
        model,
        linked_provenance: None,
    }
}

#[test]
#[ignore = "explicit contract regeneration only"]
fn regenerate() {
    let source = std::fs::read_to_string(repository_path("examples/two_box.json")).unwrap();
    let ParsedInput::LegacyModel(mut model) = parse_input(&source).unwrap() else {
        panic!("legacy example dispatched as a plan");
    };
    canonicalize_model(&mut model);
    let plan = build_direct_stable_plan(model);
    validate_plan(&plan).unwrap();
    let canonical = to_canonical_string(&plan).unwrap();
    let fixture = repository_path("fixtures/plans/two_box.plan.json");
    std::fs::create_dir_all(fixture.parent().unwrap()).unwrap();
    std::fs::write(fixture, canonical.as_bytes()).unwrap();
}

#[test]
#[ignore = "explicit PRD 0004 sibling-fixture regeneration only"]
fn regenerate_plus_sibling() {
    let source = std::fs::read_to_string(repository_path("examples/two_box.json")).unwrap();
    let ParsedInput::LegacyModel(mut model) = parse_input(&source).unwrap() else {
        panic!("legacy example dispatched as a plan");
    };
    let mut bystander = model
        .boxes
        .iter()
        .find(|model_box| model_box.name == "controller")
        .unwrap()
        .clone();
    "bystander".clone_into(&mut bystander.name);
    bystander.inputs.clear();
    bystander.outputs.clear();
    bystander.transitions[0].guard = sembla_ir::Expr::Lt {
        lhs: Box::new(sembla_ir::Expr::SelfAttr {
            name: "modifier".to_owned(),
        }),
        rhs: Box::new(sembla_ir::Expr::Real { value: 0.5 }),
    };
    model.boxes.push(bystander);
    canonicalize_model(&mut model);
    let plan = build_direct_stable_plan(model);
    validate_plan(&plan).unwrap();
    let canonical = to_canonical_string(&plan).unwrap();
    let fixture = repository_path("fixtures/plans/two_box_plus_sibling.plan.json");
    std::fs::write(fixture, canonical.as_bytes()).unwrap();
}

#[test]
fn two_box_plan_is_canonical_valid_and_hash_pinned() {
    let source =
        std::fs::read_to_string(repository_path("fixtures/plans/two_box.plan.json")).unwrap();
    let value: serde_json::Value = serde_json::from_str(&source).unwrap();
    assert_eq!(to_canonical_string(&value).unwrap(), source);

    let ParsedInput::Plan(plan) = parse_input(&source).unwrap() else {
        panic!("versioned fixture dispatched as legacy");
    };
    let validated = validate_plan(&plan).unwrap();
    assert_eq!(validated.words_by_dense_rule_id().len(), 2);
    let executable = validated.model_with_rule_words();
    assert_eq!(
        executable
            .transitions()
            .iter()
            .map(|transition| (transition.rule_id, transition.rule_word))
            .collect::<Vec<_>>(),
        vec![(0, 1_866_690_995), (1, 2_501_600_445)]
    );

    assert_eq!(
        plan_semantic_hash(&plan).unwrap(),
        HashRecordV1 {
            algorithm: "sha256".to_owned(),
            domain: "sembla.plan-core/v1".to_owned(),
            digest: "0524e9403ce2e945a6a98bd5cc7db646779d565c963e83e9a881e86b3459cc9c".to_owned(),
        }
    );
    let envelope_hash = plan_envelope_hash(&plan).unwrap();
    assert_eq!(
        envelope_hash,
        HashRecordV1 {
            algorithm: "sha256".to_owned(),
            domain: "sembla.plan-envelope/v1".to_owned(),
            digest: "135c303af99e524e9260751891e38f8724e65cf9e6906ff0d441d77fe63a0028".to_owned(),
        }
    );

    let mut changed_origin = plan.clone();
    changed_origin.origin = PlanOrigin::Linked;
    assert_eq!(
        plan_semantic_hash(&changed_origin).unwrap(),
        plan_semantic_hash(&plan).unwrap()
    );
    assert_ne!(plan_envelope_hash(&changed_origin).unwrap(), envelope_hash);
}

#[test]
fn sibling_plan_is_canonical_and_preserves_shared_rule_words() {
    let source = std::fs::read_to_string(repository_path(
        "fixtures/plans/two_box_plus_sibling.plan.json",
    ))
    .unwrap();
    let value: serde_json::Value = serde_json::from_str(&source).unwrap();
    assert_eq!(to_canonical_string(&value).unwrap(), source);
    let ParsedInput::Plan(plan) = parse_input(&source).unwrap() else {
        panic!("versioned fixture dispatched as legacy");
    };
    let validated = validate_plan(&plan).unwrap();
    let executable = validated.model_with_rule_words();
    let shared = executable
        .transitions()
        .iter()
        .filter_map(|transition| {
            let model_box = &executable.model().boxes[transition.box_index];
            (model_box.name == "controller" || model_box.name == "population")
                .then_some((model_box.name.as_str(), transition.rule_word))
        })
        .collect::<Vec<_>>();
    assert_eq!(
        shared,
        vec![("controller", 1_866_690_995), ("population", 2_501_600_445)]
    );
    assert_eq!(
        executable.model().boxes[0].name,
        "bystander",
        "the unrelated sibling must sort before both shared boxes"
    );
}
