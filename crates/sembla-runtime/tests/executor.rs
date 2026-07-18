use sembla_ir::{
    validate, Attr, AttrType, Box as ModelBox, ClaimOrdering, Effect, Expr, Model, ResourceClaim,
    Table, Transition,
};
use sembla_runtime::eval::ParamEnv;
use sembla_runtime::executor::{run, run_tick, TickError};
use sembla_runtime::rng::exp_f64;
use sembla_runtime::state::{ColumnData, ColumnInit, StateStore, TableInit};

const ALWAYS: f64 = 1.0e300;

fn attr(name: &str, ty: AttrType) -> Attr {
    Attr {
        name: name.to_owned(),
        ty,
    }
}

fn self_attr(name: &str) -> Expr {
    Expr::SelfAttr {
        name: name.to_owned(),
    }
}

fn set(attr: &str, value: Expr) -> Effect {
    Effect::SetAttr {
        attr: attr.to_owned(),
        value,
    }
}

fn transition(
    name: &str,
    table: &str,
    guard: Expr,
    hazard: Expr,
    effects: Vec<Effect>,
    contests: Vec<ResourceClaim>,
) -> Transition {
    Transition {
        name: name.to_owned(),
        table: table.to_owned(),
        guard,
        hazard,
        effects,
        contests,
    }
}

fn model(tables: Vec<Table>, transitions: Vec<Transition>, dt: f64) -> sembla_ir::ValidatedModel {
    validate(Model {
        name: "executor_test".to_owned(),
        dt,
        params: Vec::new(),
        boxes: vec![ModelBox {
            name: "world".to_owned(),
            tables,
            transitions,
            inputs: Vec::new(),
            outputs: Vec::new(),
            views: Vec::new(),
        }],
        wires: Vec::new(),
        summaries: Vec::new(),
    })
    .unwrap()
}

fn table(name: &str, attrs: Vec<Attr>) -> Table {
    Table {
        name: name.to_owned(),
        size_hint: 0,
        attrs,
    }
}

fn claim(resource: &str, ordering: ClaimOrdering) -> ResourceClaim {
    ResourceClaim {
        resource: self_attr(resource),
        ordering,
    }
}

fn enum_lit(variant: &str) -> Expr {
    Expr::Enum {
        variant: variant.to_owned(),
    }
}

#[test]
fn analytic_hazard_and_survival_curve_match_binomial_bounds() {
    let population = 100_000;
    let lambda = -(0.9_f64.ln());
    let validated = model(
        vec![table(
            "Person",
            vec![attr(
                "state",
                AttrType::Enum {
                    variants: vec!["Calm".to_owned(), "Agitated".to_owned()],
                },
            )],
        )],
        vec![transition(
            "agitate",
            "Person",
            Expr::EnumIs {
                attr: "state".to_owned(),
                variant: "Calm".to_owned(),
            },
            Expr::Real { value: lambda },
            vec![set("state", enum_lit("Agitated"))],
            Vec::new(),
        )],
        1.0,
    );
    let mut state = StateStore::new(
        &validated,
        vec![TableInit::new(
            "world",
            "Person",
            population,
            vec![ColumnInit::new(
                "state",
                ColumnData::Enum(vec![0; population]),
            )],
        )],
    )
    .unwrap();
    let params = ParamEnv::defaults(&validated);

    for tick in 0..10 {
        let report = run_tick(&validated, &mut state, &params, 7, tick).unwrap();
        if tick == 0 {
            assert_binomial(
                report.fired[0].1,
                population as f64 * 0.1,
                population as f64 * 0.1 * 0.9,
            );
        }
        let snapshot = state.snapshot();
        let survivors = (0..population)
            .filter(|row| {
                snapshot
                    .enum_index("world", "Person", "state", *row)
                    .unwrap()
                    == 0
            })
            .count();
        let probability = (-lambda * f64::from(tick + 1)).exp();
        assert_binomial(
            survivors,
            population as f64 * probability,
            population as f64 * probability * (1.0 - probability),
        );
    }
}

fn assert_binomial(actual: usize, expected: f64, variance: f64) {
    let delta = (actual as f64 - expected).abs();
    assert!(
        delta <= 3.0 * variance.sqrt(),
        "actual {actual}, expected {expected}, delta {delta}, sigma {}",
        variance.sqrt()
    );
}

fn two_state_store(model: &sembla_ir::ValidatedModel, rows: usize) -> StateStore {
    StateStore::new(
        model,
        vec![TableInit::new(
            "population",
            "Person",
            rows,
            vec![ColumnInit::new("mood", ColumnData::Enum(vec![0; rows]))],
        )],
    )
    .unwrap()
}

#[test]
fn fifty_ticks_are_report_and_state_hash_deterministic() {
    let source = include_str!("../../../examples/two_state.json");
    let validated = validate(sembla_ir::parse_json(source).unwrap()).unwrap();
    let params = ParamEnv::defaults(&validated);
    let mut lhs = two_state_store(&validated, 1_000);
    let mut rhs = two_state_store(&validated, 1_000);
    for tick in 0..50 {
        let lhs_report = run_tick(&validated, &mut lhs, &params, 42, tick).unwrap();
        let rhs_report = run_tick(&validated, &mut rhs, &params, 42, tick).unwrap();
        assert_eq!(lhs_report, rhs_report);
        assert_eq!(lhs.state_hash(), rhs.state_hash());
    }
}

fn race_fixture(ordering: ClaimOrdering) -> (sembla_ir::ValidatedModel, StateStore) {
    let validated = model(
        vec![
            table("Worker", Vec::new()),
            table(
                "Applicant",
                vec![
                    attr(
                        "worker",
                        AttrType::Ref {
                            table: "Worker".to_owned(),
                        },
                    ),
                    attr("fifo", AttrType::Int),
                    attr(
                        "hired_by",
                        AttrType::Enum {
                            variants: vec!["None".to_owned(), "A".to_owned(), "B".to_owned()],
                        },
                    ),
                ],
            ),
        ],
        vec![
            transition(
                "hire_a",
                "Applicant",
                Expr::Bool { value: true },
                Expr::Real { value: ALWAYS },
                vec![set("hired_by", enum_lit("A"))],
                vec![claim("worker", ordering.clone())],
            ),
            transition(
                "hire_b",
                "Applicant",
                Expr::Bool { value: true },
                Expr::Real { value: ALWAYS },
                vec![set("hired_by", enum_lit("B"))],
                vec![claim("worker", ordering)],
            ),
        ],
        1.0,
    );
    let state = StateStore::new(
        &validated,
        vec![
            TableInit::new("world", "Worker", 3, Vec::new()),
            TableInit::new(
                "world",
                "Applicant",
                3,
                vec![
                    ColumnInit::new("worker", ColumnData::Ref(vec![0, 1, 2])),
                    ColumnInit::new("fifo", ColumnData::Int(vec![5, 5, 5])),
                    ColumnInit::new("hired_by", ColumnData::Enum(vec![0; 3])),
                ],
            ),
        ],
    )
    .unwrap();
    (validated, state)
}

#[test]
fn race_time_microcase_matches_coordinate_rng() {
    let (validated, mut state) = race_fixture(ClaimOrdering::RaceTime);
    let report = run_tick(
        &validated,
        &mut state,
        &ParamEnv::defaults(&validated),
        91,
        0,
    )
    .unwrap();
    let snapshot = state.snapshot();
    let mut expected_counts = [0_usize; 2];
    for row in 0..3_u32 {
        let a = exp_f64(91, 0, 0, row, 0, ALWAYS);
        let b = exp_f64(91, 0, 1, row, 0, ALWAYS);
        let winner = if a.total_cmp(&b).then((0, row).cmp(&(1, row))).is_le() {
            0
        } else {
            1
        };
        expected_counts[winner] += 1;
        assert_eq!(
            snapshot
                .enum_index("world", "Applicant", "hired_by", row as usize)
                .unwrap(),
            (winner + 1) as u16
        );
    }
    assert_eq!(
        report.fired,
        vec![(0, expected_counts[0]), (1, expected_counts[1])]
    );
    assert_eq!(
        report.deferred_per_resource_table,
        vec![("Worker".to_owned(), 3)]
    );
}

#[test]
fn equal_key_tie_uses_rule_then_entity_id() {
    let ordering = ClaimOrdering::Key {
        expr: self_attr("fifo"),
    };
    let (validated, mut state) = race_fixture(ordering);
    let report = run_tick(
        &validated,
        &mut state,
        &ParamEnv::defaults(&validated),
        5,
        0,
    )
    .unwrap();
    assert_eq!(report.fired, vec![(0, 3), (1, 0)]);

    let (validated, mut state) = fifo_fixture(vec![7, 7, 7]);
    let report = run_tick(
        &validated,
        &mut state,
        &ParamEnv::defaults(&validated),
        5,
        0,
    )
    .unwrap();
    assert_eq!(report.fired, vec![(0, 1)]);
    let snapshot = state.snapshot();
    assert_eq!(
        snapshot
            .enum_index("world", "Applicant", "hired", 0)
            .unwrap(),
        1
    );
    assert_eq!(
        snapshot
            .enum_index("world", "Applicant", "hired", 1)
            .unwrap(),
        0
    );
}

#[test]
fn equivalent_enum_key_domains_compare_across_tables_and_attributes() {
    let key_type = AttrType::Enum {
        variants: vec!["Low".to_owned(), "High".to_owned()],
    };
    let applicant_attrs = |key_name: &str| {
        vec![
            attr(
                "worker",
                AttrType::Ref {
                    table: "Worker".to_owned(),
                },
            ),
            attr(key_name, key_type.clone()),
            attr(
                "hired",
                AttrType::Enum {
                    variants: vec!["No".to_owned(), "Yes".to_owned()],
                },
            ),
        ]
    };
    let validated = model(
        vec![
            table("Worker", Vec::new()),
            table("ApplicantA", applicant_attrs("priority")),
            table("ApplicantB", applicant_attrs("queue")),
        ],
        vec![
            transition(
                "hire_a",
                "ApplicantA",
                Expr::Bool { value: true },
                Expr::Real { value: ALWAYS },
                vec![set("hired", enum_lit("Yes"))],
                vec![claim(
                    "worker",
                    ClaimOrdering::Key {
                        expr: self_attr("priority"),
                    },
                )],
            ),
            transition(
                "hire_b",
                "ApplicantB",
                Expr::Bool { value: true },
                Expr::Real { value: ALWAYS },
                vec![set("hired", enum_lit("Yes"))],
                vec![claim(
                    "worker",
                    ClaimOrdering::Key {
                        expr: self_attr("queue"),
                    },
                )],
            ),
        ],
        1.0,
    );
    let applicant_init = |table_name: &str, key_name: &str, key: u16| {
        TableInit::new(
            "world",
            table_name,
            1,
            vec![
                ColumnInit::new("worker", ColumnData::Ref(vec![0])),
                ColumnInit::new(key_name, ColumnData::Enum(vec![key])),
                ColumnInit::new("hired", ColumnData::Enum(vec![0])),
            ],
        )
    };
    let mut state = StateStore::new(
        &validated,
        vec![
            TableInit::new("world", "Worker", 1, Vec::new()),
            applicant_init("ApplicantA", "priority", 1),
            applicant_init("ApplicantB", "queue", 0),
        ],
    )
    .unwrap();

    let report = run_tick(
        &validated,
        &mut state,
        &ParamEnv::defaults(&validated),
        17,
        0,
    )
    .unwrap();
    assert_eq!(report.fired, vec![(0, 0), (1, 1)]);
    assert_eq!(
        state
            .snapshot()
            .enum_index("world", "ApplicantB", "hired", 0),
        Ok(1)
    );
}

fn fifo_fixture(keys: Vec<i64>) -> (sembla_ir::ValidatedModel, StateStore) {
    let rows = keys.len();
    let validated = model(
        vec![
            table("Worker", Vec::new()),
            table(
                "Applicant",
                vec![
                    attr(
                        "worker",
                        AttrType::Ref {
                            table: "Worker".to_owned(),
                        },
                    ),
                    attr("fifo", AttrType::Int),
                    attr(
                        "hired",
                        AttrType::Enum {
                            variants: vec!["No".to_owned(), "Yes".to_owned()],
                        },
                    ),
                ],
            ),
        ],
        vec![transition(
            "hire",
            "Applicant",
            Expr::Bool { value: true },
            Expr::Real { value: ALWAYS },
            vec![set("hired", enum_lit("Yes"))],
            vec![claim(
                "worker",
                ClaimOrdering::Key {
                    expr: self_attr("fifo"),
                },
            )],
        )],
        1.0,
    );
    let state = StateStore::new(
        &validated,
        vec![
            TableInit::new("world", "Worker", 1, Vec::new()),
            TableInit::new(
                "world",
                "Applicant",
                rows,
                vec![
                    ColumnInit::new("worker", ColumnData::Ref(vec![0; rows])),
                    ColumnInit::new("fifo", ColumnData::Int(keys)),
                    ColumnInit::new("hired", ColumnData::Enum(vec![0; rows])),
                ],
            ),
        ],
    )
    .unwrap();
    (validated, state)
}

#[test]
fn integer_key_orders_fifo_regardless_of_race_time() {
    let keys = vec![10, 30, 20];
    let seed = 124;
    let fastest = (0..3_u32)
        .min_by(|lhs, rhs| {
            exp_f64(seed, 0, 0, *lhs, 0, ALWAYS).total_cmp(&exp_f64(seed, 0, 0, *rhs, 0, ALWAYS))
        })
        .unwrap();
    assert_ne!(fastest, 0, "fixture must distinguish key and race ordering");
    let (validated, mut state) = fifo_fixture(keys);
    let report = run_tick(
        &validated,
        &mut state,
        &ParamEnv::defaults(&validated),
        seed,
        0,
    )
    .unwrap();
    assert_eq!(report.fired, vec![(0, 1)]);
    assert_eq!(
        report.deferred_per_resource_table,
        vec![("Worker".to_owned(), 2)]
    );
    let snapshot = state.snapshot();
    for row in 0..3 {
        assert_eq!(
            snapshot
                .enum_index("world", "Applicant", "hired", row)
                .unwrap(),
            u16::from(row == 0)
        );
    }
}

#[test]
fn multi_claim_candidate_must_win_every_resource() {
    let validated = model(
        vec![
            table("Resource", Vec::new()),
            table(
                "Job",
                vec![
                    attr(
                        "left",
                        AttrType::Ref {
                            table: "Resource".to_owned(),
                        },
                    ),
                    attr(
                        "right",
                        AttrType::Ref {
                            table: "Resource".to_owned(),
                        },
                    ),
                    attr("left_key", AttrType::Int),
                    attr("right_key", AttrType::Int),
                    attr(
                        "done",
                        AttrType::Enum {
                            variants: vec!["No".to_owned(), "Yes".to_owned()],
                        },
                    ),
                ],
            ),
        ],
        vec![transition(
            "work",
            "Job",
            Expr::Bool { value: true },
            Expr::Real { value: ALWAYS },
            vec![set("done", enum_lit("Yes"))],
            vec![
                claim(
                    "left",
                    ClaimOrdering::Key {
                        expr: self_attr("left_key"),
                    },
                ),
                claim(
                    "right",
                    ClaimOrdering::Key {
                        expr: self_attr("right_key"),
                    },
                ),
            ],
        )],
        1.0,
    );
    let mut state = StateStore::new(
        &validated,
        vec![
            TableInit::new("world", "Resource", 2, Vec::new()),
            TableInit::new(
                "world",
                "Job",
                2,
                vec![
                    ColumnInit::new("left", ColumnData::Ref(vec![0, 0])),
                    ColumnInit::new("right", ColumnData::Ref(vec![1, 1])),
                    ColumnInit::new("left_key", ColumnData::Int(vec![0, 1])),
                    ColumnInit::new("right_key", ColumnData::Int(vec![1, 0])),
                    ColumnInit::new("done", ColumnData::Enum(vec![0, 0])),
                ],
            ),
        ],
    )
    .unwrap();
    let report = run_tick(
        &validated,
        &mut state,
        &ParamEnv::defaults(&validated),
        1,
        0,
    )
    .unwrap();
    assert_eq!(report.fired, vec![(0, 0)]);
    assert_eq!(
        report.deferred_per_resource_table,
        vec![("Resource".to_owned(), 2)]
    );
}

#[test]
fn double_write_names_both_transitions_and_leaves_state_reusable() {
    let bad = model(
        vec![table("Item", vec![attr("value", AttrType::Int)])],
        vec![
            transition(
                "first",
                "Item",
                Expr::Bool { value: true },
                Expr::Real { value: ALWAYS },
                vec![set("value", Expr::Int { value: 1 })],
                Vec::new(),
            ),
            transition(
                "second",
                "Item",
                Expr::Bool { value: true },
                Expr::Real { value: ALWAYS },
                vec![set("value", Expr::Int { value: 2 })],
                Vec::new(),
            ),
        ],
        1.0,
    );
    let mut state = StateStore::new(
        &bad,
        vec![TableInit::new(
            "world",
            "Item",
            1,
            vec![ColumnInit::new("value", ColumnData::Int(vec![0]))],
        )],
    )
    .unwrap();
    let old_hash = state.state_hash();
    let error = run_tick(&bad, &mut state, &ParamEnv::defaults(&bad), 1, 0).unwrap_err();
    assert!(matches!(error, TickError::DoubleWrite { .. }));
    let message = error.to_string();
    assert!(message.contains("'first' (rule 0)"));
    assert!(message.contains("'second' (rule 1)"));
    assert_eq!(state.state_hash(), old_hash);
    state
        .write_buffer()
        .unwrap()
        .set_int("world", "Item", "value", 0, 9)
        .unwrap();
    state.commit().unwrap();
    assert_eq!(
        state.snapshot().int("world", "Item", "value", 0).unwrap(),
        9
    );
}

#[test]
fn saturation_is_structured_and_uses_resource_firings() {
    let (validated, mut state) = fifo_fixture(vec![0, 1, 2, 3, 4]);
    let report = run(
        &validated,
        &mut state,
        &ParamEnv::defaults(&validated),
        8,
        1,
    )
    .unwrap();
    assert_eq!(
        report.ticks[0].deferred_per_resource_table,
        vec![("Worker".to_owned(), 4)]
    );
    assert_eq!(report.warnings.len(), 1);
    let warning = &report.warnings[0];
    assert_eq!((warning.tick, warning.table.as_str()), (0, "Worker"));
    assert_eq!((warning.deferred_count, warning.fired_count), (4, 1));

    // Ten resource winners and one deferred candidate is exactly 10%, not greater.
    let rows = 11;
    let validated = model(
        vec![
            table("Worker", Vec::new()),
            table(
                "Applicant",
                vec![
                    attr(
                        "worker",
                        AttrType::Ref {
                            table: "Worker".to_owned(),
                        },
                    ),
                    attr("fifo", AttrType::Int),
                ],
            ),
        ],
        vec![transition(
            "hire",
            "Applicant",
            Expr::Bool { value: true },
            Expr::Real { value: ALWAYS },
            Vec::new(),
            vec![claim(
                "worker",
                ClaimOrdering::Key {
                    expr: self_attr("fifo"),
                },
            )],
        )],
        1.0,
    );
    let mut state = StateStore::new(
        &validated,
        vec![
            TableInit::new("world", "Worker", 10, Vec::new()),
            TableInit::new(
                "world",
                "Applicant",
                rows,
                vec![
                    ColumnInit::new(
                        "worker",
                        ColumnData::Ref(vec![0, 0, 1, 2, 3, 4, 5, 6, 7, 8, 9]),
                    ),
                    ColumnInit::new("fifo", ColumnData::Int((0..rows as i64).collect())),
                ],
            ),
        ],
    )
    .unwrap();
    let report = run(
        &validated,
        &mut state,
        &ParamEnv::defaults(&validated),
        8,
        1,
    )
    .unwrap();
    assert_eq!(report.ticks[0].fired, vec![(0, 10)]);
    assert_eq!(
        report.ticks[0].deferred_per_resource_table,
        vec![("Worker".to_owned(), 1)]
    );
    assert!(report.warnings.is_empty());
}
