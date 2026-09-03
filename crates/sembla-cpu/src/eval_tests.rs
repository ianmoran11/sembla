use super::*;
use sembla_ir::{validate, Box as ModelBox, Model};
use sembla_runtime::core::{ColumnInit, StateStore, TableInit};

#[test]
fn input_enum_equality_accepts_literal_on_the_left() {
    let model = validate(Model {
        name: "input-enum".to_owned(),
        dt: 1.0,
        params: Vec::new(),
        boxes: vec![ModelBox {
            name: "box".to_owned(),
            tables: Vec::new(),
            transitions: Vec::new(),
            inputs: Vec::new(),
            outputs: Vec::new(),
            views: Vec::new(),
            grouped_views: Vec::new(),
        }],
        wires: Vec::new(),
        summaries: Vec::new(),
    })
    .unwrap();
    let params = ParamEnv::defaults(&model);
    let input = InputTable {
        box_name: "box".to_owned(),
        port_name: "events".to_owned(),
        schema: vec![Attr {
            name: "status".to_owned(),
            ty: AttrType::Enum {
                variants: vec!["Off".to_owned(), "On".to_owned()],
            },
        }],
        row_count: 1,
        columns: vec![ColumnData::Enum(vec![1])],
    };
    let expression = Expr::Eq {
        lhs: Box::new(Expr::Enum {
            variant: "On".to_owned(),
        }),
        rhs: Box::new(Expr::SelfAttr {
            name: "status".to_owned(),
        }),
    };

    assert!(matches!(
        eval_input_scalar(&expression, &input, 0, &params),
        Ok(InputScalar::Bool(true))
    ));
}

fn parallel_fixture(row_count: usize) -> (ValidatedModel, StateStore) {
    let model = validate(Model {
        name: "parallel-eval".to_owned(),
        dt: 1.0,
        params: Vec::new(),
        boxes: vec![ModelBox {
            name: "world".to_owned(),
            tables: vec![
                Table {
                    name: "Group".to_owned(),
                    size_hint: 4,
                    attrs: Vec::new(),
                },
                Table {
                    name: "Person".to_owned(),
                    size_hint: row_count as u64,
                    attrs: vec![
                        Attr {
                            name: "x".to_owned(),
                            ty: AttrType::Real,
                        },
                        Attr {
                            name: "group".to_owned(),
                            ty: AttrType::Ref {
                                table: "Group".to_owned(),
                            },
                        },
                    ],
                },
            ],
            transitions: Vec::new(),
            inputs: Vec::new(),
            outputs: Vec::new(),
            views: Vec::new(),
            grouped_views: Vec::new(),
        }],
        wires: Vec::new(),
        summaries: Vec::new(),
    })
    .unwrap();
    let values = (0..row_count)
        .map(|row| (row as f64 - 17_000.0) / 7.0)
        .collect();
    let groups = (0..row_count).map(|row| (row % 4) as u32).collect();
    let state = StateStore::new(
        &model,
        vec![
            TableInit::new("world", "Group", 4, Vec::new()),
            TableInit::new(
                "world",
                "Person",
                row_count,
                vec![
                    ColumnInit::new("x", ColumnData::Real(values)),
                    ColumnInit::new("group", ColumnData::Ref(groups)),
                ],
            ),
        ],
    )
    .unwrap();
    (model, state)
}

fn evaluate_real_bits(
    expr: &Expr,
    model: &ValidatedModel,
    state: &StateStore,
    workers: usize,
    tile_rows: usize,
) -> Vec<u64> {
    with_test_tick_tiles(workers, tile_rows, 0, || {
        let params = ParamEnv::defaults(model);
        let snapshot = state.snapshot();
        let table = EvalTable::new(model, "world", "Person").unwrap();
        if let Some(prepared) = prepare_row_expr(expr, table, &snapshot, &params).unwrap() {
            let row_count = snapshot.row_count("world", "Person").unwrap();
            let mut bits = Vec::with_capacity(row_count);
            for start in (0..row_count).step_by(tick_tile_rows()) {
                let end = (start + tick_tile_rows()).min(row_count);
                let PreparedColumn::Real(values) = prepared.tile(start, end).unwrap() else {
                    panic!("prepared test expression must be Real");
                };
                bits.extend(values.iter().copied().map(f64::to_bits));
            }
            bits
        } else {
            let mut cache = AggCache::new(model, &snapshot, &params);
            let ValueColumn::Real(values) =
                eval_column(expr, table, &snapshot, &params, &mut cache).unwrap()
            else {
                panic!("fallback test expression must be Real");
            };
            values.into_iter().map(f64::to_bits).collect()
        }
    })
}

#[test]
fn fixed_row_tiles_depend_only_on_row_index_and_tile_size() {
    let row_count = 10_003;
    for tile_rows in [257, 1_024, 4_093] {
        let expected = (0..row_count)
            .step_by(tile_rows)
            .map(|start| (start, (start + tile_rows).min(row_count)))
            .collect::<Vec<_>>();
        for workers in [1, 2, 4] {
            let actual = with_test_tick_tiles(workers, tile_rows, 0, || {
                (0..row_count)
                    .step_by(tick_tile_rows())
                    .map(|start| (start, (start + tick_tile_rows()).min(row_count)))
                    .collect::<Vec<_>>()
            });
            assert_eq!(actual, expected, "boundaries changed at {workers} workers");
        }
    }
}

#[test]
fn real_arithmetic_chain_is_bit_identical_across_workers_and_tile_sizes() {
    let (model, state) = parallel_fixture(262_144);
    let expr = Expr::Div {
        lhs: Box::new(Expr::Mul {
            lhs: Box::new(Expr::Add {
                lhs: Box::new(Expr::SelfAttr {
                    name: "x".to_owned(),
                }),
                rhs: Box::new(Expr::Real { value: 0.25 }),
            }),
            rhs: Box::new(Expr::Sub {
                lhs: Box::new(Expr::SelfAttr {
                    name: "x".to_owned(),
                }),
                rhs: Box::new(Expr::Real { value: -0.5 }),
            }),
        }),
        rhs: Box::new(Expr::Real { value: 3.0 }),
    };
    let serial = evaluate_real_bits(&expr, &model, &state, 1, 257);
    for workers in [1, 2, 4] {
        for tile_rows in [257, 1_024, 4_093] {
            assert_eq!(
                evaluate_real_bits(&expr, &model, &state, workers, tile_rows),
                serial,
                "Real arithmetic changed at {workers} workers and {tile_rows} rows/tile"
            );
        }
    }
}

#[test]
fn real_aggregate_reduction_stays_bit_identical_and_sequential() {
    let (model, state) = parallel_fixture(262_144);
    let expr = Expr::Agg {
        op: AggOp::Sum {
            value: Box::new(Expr::SelfAttr {
                name: "x".to_owned(),
            }),
        },
        table: "Person".to_owned(),
        on: AggJoin {
            fk_attr: "group".to_owned(),
            self_fk_attr: "group".to_owned(),
        },
        filter: Box::new(Expr::Bool { value: true }),
    };
    let params = ParamEnv::defaults(&model);
    let snapshot = state.snapshot();
    assert!(
        prepare_row_expr(
            &expr,
            EvalTable::new(&model, "world", "Person").unwrap(),
            &snapshot,
            &params,
        )
        .unwrap()
        .is_none(),
        "expressions containing f64 reductions must remain off the tiled path"
    );
    let serial = evaluate_real_bits(&expr, &model, &state, 1, 257);
    for workers in [1, 2, 4] {
        for tile_rows in [257, 1_024, 4_093] {
            assert_eq!(
                evaluate_real_bits(&expr, &model, &state, workers, tile_rows),
                serial,
                "Real aggregate changed at {workers} workers and {tile_rows} rows/tile"
            );
        }
    }

    let reduction = include_str!("eval.rs")
        .split_once("// This ascending target-row pass is the canonical CPU reduction order.")
        .unwrap()
        .1
        .split_once("Ok(Accumulator::Real(groups))")
        .unwrap()
        .0;
    assert!(reduction.contains("for (row, (include, value))"));
    assert!(
        !reduction.contains("element_wise_map"),
        "the canonical f64 reduction must remain sequential"
    );
}

#[test]
fn tiled_expression_footprints_follow_evaluator_liveness_not_node_width() {
    let (model, _) = parallel_fixture(1);
    let table = EvalTable::new(&model, "world", "Person").unwrap();
    let leaf = Expr::SelfAttr {
        name: "x".to_owned(),
    };
    assert_eq!(
        tiled_expr_footprint(&leaf, table).unwrap().unwrap(),
        TiledExprFootprint {
            root_bytes_per_row: 8,
            peak_bytes_per_row: 8,
            node_count: 1,
            is_int: false,
        }
    );

    let arithmetic = Expr::Add {
        lhs: Box::new(leaf.clone()),
        rhs: Box::new(Expr::Real { value: 1.0 }),
    };
    assert_eq!(
        tiled_expr_footprint(&arithmetic, table).unwrap().unwrap(),
        TiledExprFootprint {
            root_bytes_per_row: 8,
            peak_bytes_per_row: 24,
            node_count: 3,
            is_int: false,
        }
    );

    let compare = |value| Expr::Gt {
        lhs: Box::new(leaf.clone()),
        rhs: Box::new(Expr::Real { value }),
    };
    let deep_guard = Expr::And {
        lhs: Box::new(compare(0.0)),
        rhs: Box::new(Expr::And {
            lhs: Box::new(compare(1.0)),
            rhs: Box::new(compare(2.0)),
        }),
    };
    assert_eq!(
        tiled_expr_footprint(&deep_guard, table).unwrap().unwrap(),
        TiledExprFootprint {
            root_bytes_per_row: 1,
            peak_bytes_per_row: 19,
            node_count: 11,
            is_int: false,
        }
    );
}

#[test]
fn cache_budget_derives_hand_checked_tiles_for_three_model_shapes() {
    assert_eq!(tick_tile_rows_for_live_set(8), 4_096);
    assert_eq!(tick_tile_rows_for_live_set(33), 960);
    assert_eq!(tick_tile_rows_for_live_set(64), 512);
}

#[test]
fn work_threshold_tracks_the_threading_spike_crossover() {
    assert!(!tick_tiling_enabled(131_072, 7));
    assert!(tick_tiling_enabled(262_144, 7));
}
