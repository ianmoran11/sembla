use sembla_ir::{
    validate, AggJoin, AggOp, Aggregate, Attr, AttrType, Box as ModelBox, ClaimOrdering, Effect,
    Expr, Model, OutputBuilder, OutputDecl, OutputField, ParamDecl, ParamType, ParamValue,
    PortDecl, Prior, PriorFamily, ResourceClaim, SummaryDecl, SummaryReduce, Table, Transition,
    ViewDecl, ViewReduce, Wire, WireEndpoint,
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
        views: vec![],
    }
}

fn resource_claim(resource: Expr) -> ResourceClaim {
    ResourceClaim {
        resource,
        ordering: ClaimOrdering::RaceTime,
    }
}

fn observation_model(views: Vec<ViewDecl>, summaries: Vec<SummaryDecl>) -> Model {
    Model {
        name: "observations".into(),
        dt: 1.0,
        params: vec![],
        boxes: vec![ModelBox {
            name: "population".into(),
            tables: vec![Table {
                name: "Person".into(),
                size_hint: 10,
                attrs: vec![
                    Attr {
                        name: "score".into(),
                        ty: AttrType::Real,
                    },
                    Attr {
                        name: "age".into(),
                        ty: AttrType::Int,
                    },
                ],
            }],
            transitions: vec![],
            inputs: vec![],
            outputs: vec![],
            views,
        }],
        wires: vec![],
        summaries,
    }
}

fn count_view(name: &str) -> ViewDecl {
    ViewDecl {
        name: name.into(),
        table: "Person".into(),
        filter: None,
        value: None,
        reduce: ViewReduce::Count,
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
            views: vec![],
        }],
        wires: vec![],
        summaries: vec![],
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
        summaries: vec![],
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
            views: vec![],
        }],
        wires: vec![],
        summaries: vec![],
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
        summaries: vec![],
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
        views: vec![],
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
        views: vec![],
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
        summaries: vec![],
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
        summaries: vec![],
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
            views: vec![],
        }],
        wires: vec![],
        summaries: vec![],
    };

    let error = validate(model).unwrap_err();
    assert_eq!(error.path, "$.boxes[0].transitions[0].contests[1].resource");
    assert!(error.message.contains("duplicate resource claim"));
}

#[test]
fn duplicate_view_name_is_rejected() {
    let error = validate(observation_model(
        vec![count_view("total"), count_view("total")],
        vec![],
    ))
    .unwrap_err();
    assert_eq!(error.path, "$.boxes[0].views[1].name");
    assert_eq!(error.message, "duplicate view name 'total'");
}

#[test]
fn duplicate_summary_name_is_rejected() {
    let summary = SummaryDecl {
        name: "total_over_time".into(),
        r#box: "population".into(),
        view: "total".into(),
        reduce: SummaryReduce::Sum,
    };
    let error = validate(observation_model(
        vec![count_view("total")],
        vec![summary.clone(), summary],
    ))
    .unwrap_err();
    assert_eq!(error.path, "$.summaries[1].name");
    assert_eq!(error.message, "duplicate summary name 'total_over_time'");
}

#[test]
fn view_unknown_table_is_rejected() {
    let mut view = count_view("total");
    view.table = "Missing".into();
    let error = validate(observation_model(vec![view], vec![])).unwrap_err();
    assert_eq!(error.path, "$.boxes[0].views[0].table");
    assert_eq!(
        error.message,
        "view 'total' refers to unknown table 'Missing'"
    );
}

#[test]
fn view_unknown_filter_attribute_is_rejected() {
    let mut view = count_view("filtered");
    view.filter = Some(Expr::SelfAttr {
        name: "missing_filter".into(),
    });
    let error = validate(observation_model(vec![view], vec![])).unwrap_err();
    assert_eq!(error.path, "$.boxes[0].views[0].filter.name");
    assert_eq!(error.message, "unresolved self attribute 'missing_filter'");
}

#[test]
fn view_unknown_value_attribute_is_rejected() {
    let view = ViewDecl {
        name: "sum".into(),
        table: "Person".into(),
        filter: None,
        value: Some(Expr::SelfAttr {
            name: "missing_value".into(),
        }),
        reduce: ViewReduce::Sum,
    };
    let error = validate(observation_model(vec![view], vec![])).unwrap_err();
    assert_eq!(error.path, "$.boxes[0].views[0].value.name");
    assert_eq!(error.message, "unresolved self attribute 'missing_value'");
}

#[test]
fn non_bool_view_filter_is_rejected() {
    let mut view = count_view("filtered");
    view.filter = Some(Expr::SelfAttr { name: "age".into() });
    let error = validate(observation_model(vec![view], vec![])).unwrap_err();
    assert_eq!(error.path, "$.boxes[0].views[0].filter");
    assert_eq!(error.message, "expected Bool, found Int");
}

#[test]
fn count_view_with_value_is_rejected() {
    let mut view = count_view("counted");
    view.value = Some(Expr::SelfAttr { name: "age".into() });
    let error = validate(observation_model(vec![view], vec![])).unwrap_err();
    assert_eq!(error.path, "$.boxes[0].views[0].value");
    assert_eq!(
        error.message,
        "count view 'counted' must not declare a value"
    );
}

#[test]
fn non_numeric_view_value_is_rejected() {
    let view = ViewDecl {
        name: "invalid_sum".into(),
        table: "Person".into(),
        filter: None,
        value: Some(Expr::Bool { value: true }),
        reduce: ViewReduce::Sum,
    };
    let error = validate(observation_model(vec![view], vec![])).unwrap_err();
    assert_eq!(error.path, "$.boxes[0].views[0].value");
    assert_eq!(error.message, "view value must be numeric, found Bool");
}

#[test]
fn summary_missing_view_is_rejected() {
    let summary = SummaryDecl {
        name: "missing_summary".into(),
        r#box: "population".into(),
        view: "missing_view".into(),
        reduce: SummaryReduce::Last,
    };
    let error = validate(observation_model(vec![count_view("total")], vec![summary])).unwrap_err();
    assert_eq!(error.path, "$.summaries[0].view");
    assert_eq!(
        error.message,
        "summary 'missing_summary' refers to unknown view 'population.missing_view'"
    );
}

#[test]
fn summary_missing_box_is_rejected() {
    let summary = SummaryDecl {
        name: "missing_summary".into(),
        r#box: "missing_box".into(),
        view: "total".into(),
        reduce: SummaryReduce::Last,
    };
    let error = validate(observation_model(vec![count_view("total")], vec![summary])).unwrap_err();
    assert_eq!(error.path, "$.summaries[0].box");
    assert_eq!(
        error.message,
        "summary 'missing_summary' refers to unknown box 'missing_box'"
    );
}

#[test]
fn observation_names_are_not_transition_attributes() {
    let mut model = observation_model(
        vec![count_view("reported_total")],
        vec![SummaryDecl {
            name: "reported_total".into(),
            r#box: "population".into(),
            view: "reported_total".into(),
            reduce: SummaryReduce::Last,
        }],
    );
    model.boxes[0].transitions.push(Transition {
        hazard: Expr::SelfAttr {
            name: "reported_total".into(),
        },
        ..transition("cannot-read-observation")
    });

    let error = validate(model).unwrap_err();
    assert_eq!(error.path, "$.boxes[0].transitions[0].hazard.name");
    assert_eq!(error.message, "unresolved self attribute 'reported_total'");
}

#[test]
fn non_count_view_without_value_is_rejected() {
    let view = ViewDecl {
        name: "maximum".into(),
        table: "Person".into(),
        filter: None,
        value: None,
        reduce: ViewReduce::Max,
    };
    let error = validate(observation_model(vec![view], vec![])).unwrap_err();
    assert_eq!(error.path, "$.boxes[0].views[0].value");
    assert_eq!(error.message, "Max view 'maximum' must declare a value");
}
