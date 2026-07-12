use sembla_ir::{
    validate, AggJoin, AggOp, Aggregate, Attr, AttrType, Box as ModelBox, ClaimOrdering, Effect,
    Expr, Model, OutputBuilder, OutputDecl, OutputField, ParamDecl, ParamType, ParamValue,
    PortDecl, Prior, PriorFamily, ResourceClaim, Table, Transition, Wire, WireEndpoint,
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
fn int_parameter_priors_are_rejected_at_the_declaration_path() {
    let model = Model {
        name: "int-prior".into(),
        dt: 1.0,
        params: vec![ParamDecl {
            name: "count".into(),
            ty: ParamType::Int,
            default: ParamValue::Int { value: 1 },
            prior: Some(Prior {
                family: PriorFamily::Uniform,
                args: vec![0.0, 2.0],
            }),
        }],
        boxes: vec![simple_box("box", vec![])],
        wires: vec![],
    };
    let error = validate(model).unwrap_err().to_string();
    assert!(error.contains("$.params[0].prior"), "{error}");
    assert!(error.contains("integer parameter 'count'"), "{error}");
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
fn duplicate_wire_destinations_are_rejected_at_the_second_wire() {
    let schema = vec![Attr {
        name: "count".into(),
        ty: AttrType::Int,
    }];
    let source = |name: &str| ModelBox {
        name: name.into(),
        tables: vec![Table {
            name: "Person".into(),
            size_hint: 1,
            attrs: vec![],
        }],
        transitions: vec![],
        inputs: vec![],
        outputs: vec![OutputDecl {
            name: "out".into(),
            schema: schema.clone(),
            builder: OutputBuilder::PerTable {
                table: "Person".into(),
                fields: vec![OutputField {
                    name: "count".into(),
                    op: AggOp::Count,
                    filter: None,
                }],
            },
        }],
    };
    let destination = ModelBox {
        name: "destination".into(),
        tables: vec![Table {
            name: "Person".into(),
            size_hint: 1,
            attrs: vec![],
        }],
        transitions: vec![],
        inputs: vec![PortDecl {
            name: "in".into(),
            schema: schema.clone(),
        }],
        outputs: vec![],
    };
    let endpoint = WireEndpoint {
        r#box: "destination".into(),
        port: "in".into(),
    };
    let model = Model {
        name: "duplicate-wire".into(),
        dt: 1.0,
        params: vec![],
        boxes: vec![source("first"), source("second"), destination],
        wires: vec![
            Wire {
                from: WireEndpoint {
                    r#box: "first".into(),
                    port: "out".into(),
                },
                to: endpoint.clone(),
            },
            Wire {
                from: WireEndpoint {
                    r#box: "second".into(),
                    port: "out".into(),
                },
                to: endpoint,
            },
        ],
    };

    let error = validate(model).unwrap_err();
    assert_eq!(error.path, "$.wires[1].to");
    assert!(error.message.contains("multiple wires target"));
}

#[test]
fn nested_aggregate_in_input_row_expression_is_rejected() {
    let mut model_box = simple_box(
        "box",
        vec![Transition {
            guard: Expr::Input {
                port: "events".into(),
                agg: Aggregate {
                    op: AggOp::Count,
                    filter: Some(std::boxed::Box::new(Expr::Input {
                        port: "events".into(),
                        agg: Aggregate {
                            op: AggOp::Count,
                            filter: None,
                        },
                    })),
                },
            },
            ..transition("nested")
        }],
    );
    model_box.inputs.push(PortDecl {
        name: "events".into(),
        schema: vec![],
    });
    let error = validate(Model {
        name: "nested-input".into(),
        dt: 1.0,
        params: vec![],
        boxes: vec![model_box],
        wires: vec![],
    })
    .unwrap_err();
    assert_eq!(error.path, "$.boxes[0].transitions[0].guard.agg.filter");
    assert!(error.message.contains("nested aggregates"));
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
