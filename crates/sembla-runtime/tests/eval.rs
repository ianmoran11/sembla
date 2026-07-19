use sembla_ir::{
    AggJoin, AggOp, Aggregate, Attr, AttrType, Box as IrBox, Expr, Model, ParamDecl, ParamType,
    ParamValue, PortDecl, Table,
};
use sembla_runtime::eval::{
    eval_column, eval_ref_column, eval_typed_ref_column, AggCache, EvalTable, ParamEnv,
    ParamOverride, ValueColumn,
};
use sembla_runtime::state::{ColumnData, ColumnInit, StateStore, TableInit};

fn attr(name: &str, ty: AttrType) -> Attr {
    Attr {
        name: name.into(),
        ty,
    }
}

fn validated_model(person_rows: u64, employer_rows: u64) -> sembla_ir::ValidatedModel {
    sembla_ir::validate(Model {
        name: "eval_fixture".into(),
        dt: 1.0,
        params: vec![
            ParamDecl {
                name: "rate".into(),
                ty: ParamType::Real,
                default: ParamValue::Real { value: 2.0 },
                prior: None,
            },
            ParamDecl {
                name: "offset".into(),
                ty: ParamType::Int,
                default: ParamValue::Int { value: 3 },
                prior: None,
            },
        ],
        boxes: vec![IrBox {
            name: "world".into(),
            tables: vec![
                Table {
                    name: "Employer".into(),
                    size_hint: employer_rows,
                    attrs: vec![],
                },
                Table {
                    name: "Person".into(),
                    size_hint: person_rows,
                    attrs: vec![
                        attr("x", AttrType::Real),
                        attr("age", AttrType::Int),
                        attr(
                            "health",
                            AttrType::Enum {
                                variants: vec!["S".into(), "I".into()],
                            },
                        ),
                        attr(
                            "phase",
                            AttrType::Enum {
                                variants: vec!["I".into(), "R".into()],
                            },
                        ),
                        attr(
                            "employer",
                            AttrType::Ref {
                                table: "Employer".into(),
                            },
                        ),
                    ],
                },
            ],
            transitions: vec![],
            inputs: vec![PortDecl {
                name: "events".into(),
                schema: vec![attr("amount", AttrType::Real), attr("code", AttrType::Int)],
            }],
            outputs: vec![],
            views: vec![],
        }],
        wires: vec![],
        summaries: vec![],
    })
    .expect("evaluation fixture must validate")
}

fn state(
    model: &sembla_ir::ValidatedModel,
    x: Vec<f64>,
    age: Vec<i64>,
    health: Vec<u16>,
    employer: Vec<u32>,
    employer_count: usize,
) -> StateStore {
    let row_count = x.len();
    StateStore::new(
        model,
        vec![
            TableInit::new("world", "Employer", employer_count, vec![]),
            TableInit::new(
                "world",
                "Person",
                row_count,
                vec![
                    ColumnInit::new("x", ColumnData::Real(x)),
                    ColumnInit::new("age", ColumnData::Int(age)),
                    ColumnInit::new("health", ColumnData::Enum(health)),
                    ColumnInit::new("phase", ColumnData::Enum(vec![0; row_count])),
                    ColumnInit::new("employer", ColumnData::Ref(employer)),
                ],
            ),
        ],
    )
    .expect("fixture state must initialize")
}

fn boxed(expr: Expr) -> std::boxed::Box<Expr> {
    std::boxed::Box::new(expr)
}

fn self_attr(name: &str) -> Expr {
    Expr::SelfAttr { name: name.into() }
}

fn evaluate<'tick>(
    expr: &Expr,
    model: &'tick sembla_ir::ValidatedModel,
    _store: &StateStore,
    params: &'tick ParamEnv,
    cache: &mut AggCache<'tick, '_>,
) -> ValueColumn {
    let snapshot = cache.snapshot();
    eval_column(
        expr,
        EvalTable::new(model, "world", "Person").unwrap(),
        snapshot,
        params,
        cache,
    )
    .unwrap()
}

fn count_infected_by_employer() -> Expr {
    Expr::Agg {
        op: AggOp::Count,
        table: "Person".into(),
        on: AggJoin {
            fk_attr: "employer".into(),
            self_fk_attr: "employer".into(),
        },
        filter: boxed(Expr::EnumIs {
            attr: "health".into(),
            variant: "I".into(),
        }),
    }
}

#[test]
fn every_expression_form_and_parameter_resolution_are_evaluated() {
    let model = validated_model(3, 2);
    let store = state(
        &model,
        vec![1.0, 2.0, 4.0],
        vec![1, 2, 3],
        vec![0, 1, 1],
        vec![0, 1, 0],
        2,
    );
    let defaults = ParamEnv::defaults(&model);
    let snapshot = store.snapshot();
    let mut cache = AggCache::new(&model, &snapshot, &defaults);

    assert_eq!(
        evaluate(
            &Expr::Real { value: 1.25 },
            &model,
            &store,
            &defaults,
            &mut cache
        ),
        ValueColumn::Real(vec![1.25; 3])
    );
    assert_eq!(
        evaluate(
            &Expr::Int { value: 7 },
            &model,
            &store,
            &defaults,
            &mut cache
        ),
        ValueColumn::Int(vec![7; 3])
    );
    assert_eq!(
        evaluate(
            &Expr::Bool { value: true },
            &model,
            &store,
            &defaults,
            &mut cache
        ),
        ValueColumn::Bool(vec![true; 3])
    );
    let enum_literal = Expr::Enum {
        variant: "I".into(),
    };
    assert_eq!(
        eval_column(
            &enum_literal,
            EvalTable::new(&model, "world", "Person")
                .unwrap()
                .with_expected_attr("health")
                .unwrap(),
            &snapshot,
            &defaults,
            &mut cache,
        )
        .unwrap(),
        ValueColumn::Enum(vec![1; 3])
    );
    assert_eq!(
        eval_column(
            &enum_literal,
            EvalTable::new(&model, "world", "Person")
                .unwrap()
                .with_expected_attr("phase")
                .unwrap(),
            &snapshot,
            &defaults,
            &mut cache,
        )
        .unwrap(),
        ValueColumn::Enum(vec![0; 3])
    );
    assert!(eval_column(
        &enum_literal,
        EvalTable::new(&model, "world", "Person").unwrap(),
        &snapshot,
        &defaults,
        &mut cache,
    )
    .unwrap_err()
    .to_string()
    .contains("requires an Enum context"));
    assert_eq!(
        eval_ref_column(
            &self_attr("employer"),
            EvalTable::new(&model, "world", "Person").unwrap(),
            &snapshot,
            &defaults,
            &mut cache,
        )
        .unwrap(),
        vec![0, 1, 0]
    );
    let typed_ref = eval_typed_ref_column(
        &self_attr("employer"),
        EvalTable::new(&model, "world", "Person").unwrap(),
        &snapshot,
        &defaults,
        &mut cache,
    )
    .unwrap();
    assert_eq!(typed_ref.target_table, "Employer");
    assert_eq!(typed_ref.values, vec![0, 1, 0]);
    let equal_but_distinct_params = defaults.clone();
    assert!(eval_column(
        &Expr::Real { value: 1.0 },
        EvalTable::new(&model, "world", "Person").unwrap(),
        &snapshot,
        &equal_but_distinct_params,
        &mut cache,
    )
    .unwrap_err()
    .to_string()
    .contains("different parameter environment"));
    assert_eq!(
        evaluate(&self_attr("x"), &model, &store, &defaults, &mut cache),
        ValueColumn::Real(vec![1.0, 2.0, 4.0])
    );
    assert_eq!(
        evaluate(&self_attr("age"), &model, &store, &defaults, &mut cache),
        ValueColumn::Int(vec![1, 2, 3])
    );
    assert_eq!(
        evaluate(&self_attr("health"), &model, &store, &defaults, &mut cache),
        ValueColumn::Enum(vec![0, 1, 1])
    );
    let nested = Expr::Add {
        lhs: boxed(Expr::Mul {
            lhs: boxed(self_attr("x")),
            rhs: boxed(Expr::Real { value: 2.0 }),
        }),
        rhs: boxed(Expr::Param {
            name: "rate".into(),
        }),
    };
    assert_eq!(
        evaluate(&nested, &model, &store, &defaults, &mut cache),
        ValueColumn::Real(vec![4.0, 6.0, 10.0])
    );
    assert_eq!(
        evaluate(
            &Expr::Sub {
                lhs: boxed(self_attr("age")),
                rhs: boxed(Expr::Param {
                    name: "offset".into(),
                }),
            },
            &model,
            &store,
            &defaults,
            &mut cache,
        ),
        ValueColumn::Int(vec![-2, -1, 0])
    );
    assert_eq!(
        evaluate(
            &Expr::Div {
                lhs: boxed(self_attr("x")),
                rhs: boxed(Expr::Real { value: 0.0 }),
            },
            &model,
            &store,
            &defaults,
            &mut cache,
        ),
        ValueColumn::Real(vec![f64::INFINITY; 3])
    );

    let equality = Expr::Eq {
        lhs: boxed(Expr::Enum {
            variant: "I".into(),
        }),
        rhs: boxed(self_attr("health")),
    };
    assert_eq!(
        evaluate(&equality, &model, &store, &defaults, &mut cache),
        ValueColumn::Bool(vec![false, true, true])
    );
    assert_eq!(
        evaluate(
            &Expr::Ne {
                lhs: boxed(self_attr("employer")),
                rhs: boxed(self_attr("employer")),
            },
            &model,
            &store,
            &defaults,
            &mut cache,
        ),
        ValueColumn::Bool(vec![false; 3])
    );

    for (expr, expected) in [
        (
            Expr::Lt {
                lhs: boxed(self_attr("age")),
                rhs: boxed(Expr::Int { value: 2 }),
            },
            vec![true, false, false],
        ),
        (
            Expr::Le {
                lhs: boxed(self_attr("age")),
                rhs: boxed(Expr::Int { value: 2 }),
            },
            vec![true, true, false],
        ),
        (
            Expr::Gt {
                lhs: boxed(self_attr("age")),
                rhs: boxed(Expr::Int { value: 2 }),
            },
            vec![false, false, true],
        ),
        (
            Expr::Ge {
                lhs: boxed(self_attr("age")),
                rhs: boxed(Expr::Int { value: 2 }),
            },
            vec![false, true, true],
        ),
    ] {
        assert_eq!(
            evaluate(&expr, &model, &store, &defaults, &mut cache),
            ValueColumn::Bool(expected)
        );
    }

    let infected = Expr::EnumIs {
        attr: "health".into(),
        variant: "I".into(),
    };
    assert_eq!(
        evaluate(&infected, &model, &store, &defaults, &mut cache),
        ValueColumn::Bool(vec![false, true, true])
    );
    assert_eq!(
        evaluate(
            &Expr::And {
                lhs: boxed(infected.clone()),
                rhs: boxed(Expr::Bool { value: true }),
            },
            &model,
            &store,
            &defaults,
            &mut cache,
        ),
        ValueColumn::Bool(vec![false, true, true])
    );
    assert_eq!(
        evaluate(
            &Expr::Or {
                lhs: boxed(infected.clone()),
                rhs: boxed(Expr::Bool { value: false }),
            },
            &model,
            &store,
            &defaults,
            &mut cache,
        ),
        ValueColumn::Bool(vec![false, true, true])
    );
    assert_eq!(
        evaluate(
            &Expr::Not {
                expr: boxed(infected),
            },
            &model,
            &store,
            &defaults,
            &mut cache,
        ),
        ValueColumn::Bool(vec![true, false, false])
    );

    let input_count = Expr::Input {
        port: "events".into(),
        agg: Aggregate {
            op: AggOp::Count,
            filter: Some(boxed(Expr::Gt {
                lhs: boxed(self_attr("amount")),
                rhs: boxed(Expr::Real { value: 0.0 }),
            })),
        },
    };
    assert_eq!(
        evaluate(&input_count, &model, &store, &defaults, &mut cache),
        ValueColumn::Int(vec![0; 3])
    );
    let input_sum = Expr::Input {
        port: "events".into(),
        agg: Aggregate {
            op: AggOp::Sum {
                value: boxed(self_attr("amount")),
            },
            filter: None,
        },
    };
    assert_eq!(
        evaluate(&input_sum, &model, &store, &defaults, &mut cache),
        ValueColumn::Real(vec![0.0; 3])
    );

    assert_eq!(
        evaluate(
            &count_infected_by_employer(),
            &model,
            &store,
            &defaults,
            &mut cache,
        ),
        ValueColumn::Int(vec![1, 1, 1])
    );
    let sum = Expr::Agg {
        op: AggOp::Sum {
            value: boxed(self_attr("x")),
        },
        table: "Person".into(),
        on: AggJoin {
            fk_attr: "employer".into(),
            self_fk_attr: "employer".into(),
        },
        filter: boxed(Expr::Bool { value: true }),
    };
    assert_eq!(
        evaluate(&sum, &model, &store, &defaults, &mut cache),
        ValueColumn::Real(vec![5.0, 2.0, 5.0])
    );

    let overridden = ParamEnv::resolve(
        &model,
        &[
            ParamOverride::new("rate", ParamValue::Real { value: 10.0 }),
            ParamOverride::new("offset", ParamValue::Int { value: -4 }),
        ],
    )
    .unwrap();
    let mut overridden_cache = AggCache::new(&model, &snapshot, &overridden);
    assert_eq!(
        evaluate(&nested, &model, &store, &overridden, &mut overridden_cache,),
        ValueColumn::Real(vec![12.0, 14.0, 18.0])
    );
    assert_eq!(
        evaluate(
            &Expr::Param {
                name: "offset".into(),
            },
            &model,
            &store,
            &overridden,
            &mut overridden_cache,
        ),
        ValueColumn::Int(vec![-4; 3])
    );
}

#[test]
fn group_by_count_matches_naive_quadratic_lumping_reference() {
    const PERSON_COUNT: usize = 1_003;
    const EMPLOYER_COUNT: usize = 37;
    let model = validated_model(PERSON_COUNT as u64, EMPLOYER_COUNT as u64);
    let employers: Vec<u32> = (0..PERSON_COUNT)
        .map(|row| ((row * 17 + row / 7 + 11) % EMPLOYER_COUNT) as u32)
        .collect();
    let health: Vec<u16> = (0..PERSON_COUNT)
        .map(|row| u16::from((row * 29 + row / 5 + 3) % 11 < 4))
        .collect();
    let store = state(
        &model,
        (0..PERSON_COUNT).map(|row| row as f64 * 0.25).collect(),
        (0..PERSON_COUNT).map(|row| row as i64 - 500).collect(),
        health.clone(),
        employers.clone(),
        EMPLOYER_COUNT,
    );
    let params = ParamEnv::defaults(&model);
    let snapshot = store.snapshot();
    let mut cache = AggCache::new(&model, &snapshot, &params);
    let actual = evaluate(
        &count_infected_by_employer(),
        &model,
        &store,
        &params,
        &mut cache,
    );

    let expected: Vec<i64> = employers
        .iter()
        .map(|query_employer| {
            employers
                .iter()
                .zip(&health)
                .filter(|(target_employer, target_health)| {
                    *target_employer == query_employer && **target_health == 1
                })
                .count() as i64
        })
        .collect();
    assert_eq!(actual, ValueColumn::Int(expected));
}

#[test]
fn identical_aggregates_build_one_accumulator() {
    let model = validated_model(3, 2);
    let store = state(
        &model,
        vec![1.0, 2.0, 4.0],
        vec![1, 2, 3],
        vec![0, 1, 1],
        vec![0, 1, 0],
        2,
    );
    let expr = count_infected_by_employer();
    let params = ParamEnv::defaults(&model);
    let snapshot = store.snapshot();
    let mut cache = AggCache::new(&model, &snapshot, &params);

    let first = evaluate(&expr, &model, &store, &params, &mut cache);
    let second = evaluate(&expr, &model, &store, &params, &mut cache);
    assert_eq!(first, second);
    assert_eq!(cache.len(), 1);
    assert_eq!(cache.build_count(), 1);
}

#[test]
fn cache_distinguishes_ieee_structure_and_uses_explicit_tick_scopes() {
    let model = validated_model(3, 2);
    let mut store = state(
        &model,
        vec![1.0, 2.0, 4.0],
        vec![1, 2, 3],
        vec![0, 1, 1],
        vec![0, 1, 0],
        2,
    );
    let defaults = ParamEnv::defaults(&model);
    let signed_sum = |zero| Expr::Agg {
        op: AggOp::Sum {
            value: boxed(Expr::Div {
                lhs: boxed(Expr::Real { value: 1.0 }),
                rhs: boxed(Expr::Real { value: zero }),
            }),
        },
        table: "Person".into(),
        on: AggJoin {
            fk_attr: "employer".into(),
            self_fk_attr: "employer".into(),
        },
        filter: boxed(Expr::Bool { value: true }),
    };
    let parameter_sum = Expr::Agg {
        op: AggOp::Sum {
            value: boxed(Expr::Param {
                name: "rate".into(),
            }),
        },
        table: "Person".into(),
        on: AggJoin {
            fk_attr: "employer".into(),
            self_fk_attr: "employer".into(),
        },
        filter: boxed(Expr::Bool { value: true }),
    };
    let count = count_infected_by_employer();

    {
        let snapshot = store.snapshot();
        let mut cache = AggCache::new(&model, &snapshot, &defaults);
        assert_eq!(
            evaluate(&signed_sum(0.0), &model, &store, &defaults, &mut cache),
            ValueColumn::Real(vec![f64::INFINITY; 3])
        );
        assert_eq!(
            evaluate(&signed_sum(-0.0), &model, &store, &defaults, &mut cache),
            ValueColumn::Real(vec![f64::NEG_INFINITY; 3])
        );
        assert_eq!(cache.len(), 2, "signed-zero expressions must not alias");
        assert_eq!(
            evaluate(&parameter_sum, &model, &store, &defaults, &mut cache),
            ValueColumn::Real(vec![4.0, 2.0, 4.0])
        );
        assert_eq!(
            evaluate(&count, &model, &store, &defaults, &mut cache),
            ValueColumn::Int(vec![1, 1, 1])
        );

        let overridden = ParamEnv::resolve(
            &model,
            &[ParamOverride::new("rate", ParamValue::Real { value: 10.0 })],
        )
        .unwrap();
        let mut overridden_cache = AggCache::new(&model, &snapshot, &overridden);
        assert_eq!(
            evaluate(
                &parameter_sum,
                &model,
                &store,
                &overridden,
                &mut overridden_cache,
            ),
            ValueColumn::Real(vec![20.0, 10.0, 20.0])
        );
    }

    store
        .write_buffer()
        .unwrap()
        .set_enum("world", "Person", "health", 0, 1)
        .unwrap();
    store.commit().unwrap();
    let snapshot = store.snapshot();
    let mut cache = AggCache::new(&model, &snapshot, &defaults);
    assert_eq!(
        evaluate(&count, &model, &store, &defaults, &mut cache),
        ValueColumn::Int(vec![2, 1, 2])
    );
}

#[test]
fn sum_uses_sequential_row_order_and_is_repeatable() {
    let model = validated_model(4, 1);
    let store = state(
        &model,
        vec![1.0e16, 1.0, -1.0e16, 1.0],
        vec![0, 0, 0, 0],
        vec![0, 0, 0, 0],
        vec![0, 0, 0, 0],
        1,
    );
    let expr = Expr::Agg {
        op: AggOp::Sum {
            value: boxed(self_attr("x")),
        },
        table: "Person".into(),
        on: AggJoin {
            fk_attr: "employer".into(),
            self_fk_attr: "employer".into(),
        },
        filter: boxed(Expr::Bool { value: true }),
    };
    let params = ParamEnv::defaults(&model);
    let snapshot = store.snapshot();
    let mut first_cache = AggCache::new(&model, &snapshot, &params);
    let first = evaluate(&expr, &model, &store, &params, &mut first_cache);
    let mut second_cache = AggCache::new(&model, &snapshot, &params);
    let second = evaluate(&expr, &model, &store, &params, &mut second_cache);

    // Canonical CPU schedule is one ascending-row left fold.
    assert_eq!(first, ValueColumn::Real(vec![1.0; 4]));
    assert_eq!(second, first);
}
