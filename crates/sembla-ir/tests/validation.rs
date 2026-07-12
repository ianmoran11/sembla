use sembla_ir::{
    validate, AggJoin, AggOp, Attr, AttrType, Box as ModelBox, ClaimOrdering, Effect, Expr, Model,
    ResourceClaim, Table, Transition,
};

fn transition(name: &str) -> Transition {
    Transition {
        name: name.into(),
        table: "Person".into(),
        guard: Expr::Bool { value: true },
        hazard: Expr::Real { value: 0.1 },
        effects: vec![],
        contests: vec![],
    }
}

fn simple_box(name: &str, transitions: Vec<Transition>) -> ModelBox {
    ModelBox {
        name: name.into(),
        tables: vec![Table {
            name: "Person".into(),
            size_hint: 10,
            attrs: vec![],
        }],
        transitions,
        inputs: vec![],
        outputs: vec![],
    }
}

fn resource_claim(resource: Expr) -> ResourceClaim {
    ResourceClaim {
        resource,
        ordering: ClaimOrdering::RaceTime,
    }
}

fn ref_write_model(value: Expr, contests: Vec<ResourceClaim>) -> Model {
    Model {
        name: "ref-write".into(),
        dt: 1.0,
        params: vec![],
        boxes: vec![ModelBox {
            name: "box".into(),
            tables: vec![
                Table {
                    name: "Person".into(),
                    size_hint: 10,
                    attrs: vec![
                        Attr {
                            name: "employer".into(),
                            ty: AttrType::Ref {
                                table: "Employer".into(),
                            },
                        },
                        Attr {
                            name: "alternate_employer".into(),
                            ty: AttrType::Ref {
                                table: "Employer".into(),
                            },
                        },
                    ],
                },
                Table {
                    name: "Employer".into(),
                    size_hint: 1,
                    attrs: vec![],
                },
            ],
            transitions: vec![Transition {
                effects: vec![Effect::SetAttr {
                    attr: "employer".into(),
                    value,
                }],
                contests,
                ..transition("assign")
            }],
            inputs: vec![],
            outputs: vec![],
        }],
        wires: vec![],
    }
}

#[test]
fn aggregate_supports_the_infect_group_by_pattern() {
    let person = Table {
        name: "Person".into(),
        size_hint: 100,
        attrs: vec![
            Attr {
                name: "employer".into(),
                ty: AttrType::Ref {
                    table: "Employer".into(),
                },
            },
            Attr {
                name: "health".into(),
                ty: AttrType::Enum {
                    variants: vec!["S".into(), "I".into(), "R".into()],
                },
            },
        ],
    };
    let employer = Table {
        name: "Employer".into(),
        size_hint: 10,
        attrs: vec![],
    };
    let infect_count = Expr::Agg {
        op: AggOp::Count,
        table: "Person".into(),
        on: AggJoin {
            fk_attr: "employer".into(),
            self_fk_attr: "employer".into(),
        },
        filter: std::boxed::Box::new(Expr::EnumIs {
            attr: "health".into(),
            variant: "I".into(),
        }),
    };
    let model = Model {
        name: "infect-pattern".into(),
        dt: 1.0,
        params: vec![],
        boxes: vec![ModelBox {
            name: "workplace".into(),
            tables: vec![person, employer],
            transitions: vec![Transition {
                name: "infect".into(),
                table: "Person".into(),
                guard: Expr::Gt {
                    lhs: std::boxed::Box::new(infect_count),
                    rhs: std::boxed::Box::new(Expr::Int { value: 0 }),
                },
                hazard: Expr::Real { value: 0.1 },
                effects: vec![],
                contests: vec![],
            }],
            inputs: vec![],
            outputs: vec![],
        }],
        wires: vec![],
    };

    validate(model).expect("infect aggregate must type-check");
}

#[test]
fn rule_ids_follow_global_declaration_order() {
    let model = Model {
        name: "rules".into(),
        dt: 1.0,
        params: vec![],
        boxes: vec![
            simple_box("first", vec![transition("a"), transition("b")]),
            simple_box("second", vec![transition("c")]),
        ],
        wires: vec![],
    };
    let validated = validate(model).unwrap();

    assert_eq!(validated.rule_id(0, 0), Some(0));
    assert_eq!(validated.rule_id(0, 1), Some(1));
    assert_eq!(validated.rule_id(1, 0), Some(2));
    assert_eq!(validated.rule_id(1, 1), None);
}

#[test]
fn ref_write_without_a_matching_claim_is_rejected() {
    let value = Expr::SelfAttr {
        name: "alternate_employer".into(),
    };
    let error = validate(ref_write_model(value, vec![])).unwrap_err();

    assert_eq!(error.path, "$.boxes[0].transitions[0].effects[0].value");
    assert!(error.message.contains("Ref attribute 'employer'"));
    assert!(error.message.contains("matching resource claim"));
}

#[test]
fn ref_write_with_a_matching_claim_is_valid() {
    let value = Expr::SelfAttr {
        name: "alternate_employer".into(),
    };
    let model = ref_write_model(value.clone(), vec![resource_claim(value)]);

    validate(model).expect("matching claim must cover the Ref write");
}

#[test]
fn claim_for_a_different_ref_does_not_cover_the_write() {
    let value = Expr::SelfAttr {
        name: "alternate_employer".into(),
    };
    let different = Expr::SelfAttr {
        name: "employer".into(),
    };
    let error = validate(ref_write_model(value, vec![resource_claim(different)])).unwrap_err();

    assert_eq!(error.path, "$.boxes[0].transitions[0].effects[0].value");
    assert!(error.message.contains("matching resource claim"));
}

#[test]
fn duplicate_resource_claim_is_rejected_with_its_path() {
    let resource = Expr::SelfAttr {
        name: "employer".into(),
    };
    let claims = vec![
        ResourceClaim {
            resource: resource.clone(),
            ordering: ClaimOrdering::RaceTime,
        },
        ResourceClaim {
            resource,
            ordering: ClaimOrdering::RaceTime,
        },
    ];
    let model = Model {
        name: "claims".into(),
        dt: 1.0,
        params: vec![],
        boxes: vec![ModelBox {
            name: "box".into(),
            tables: vec![
                Table {
                    name: "Person".into(),
                    size_hint: 10,
                    attrs: vec![Attr {
                        name: "employer".into(),
                        ty: AttrType::Ref {
                            table: "Employer".into(),
                        },
                    }],
                },
                Table {
                    name: "Employer".into(),
                    size_hint: 1,
                    attrs: vec![],
                },
            ],
            transitions: vec![Transition {
                contests: claims,
                ..transition("compete")
            }],
            inputs: vec![],
            outputs: vec![],
        }],
        wires: vec![],
    };

    let error = validate(model).unwrap_err();
    assert_eq!(error.path, "$.boxes[0].transitions[0].contests[1].resource");
    assert!(error.message.contains("duplicate resource claim"));
}
