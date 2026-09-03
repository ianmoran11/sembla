use super::*;
use crate::eval::with_test_tick_tiles;
use sembla_ir::{
    parse_json, validate, Attr, Box as ModelBox, Model, ParamDecl, ParamType, ParamValue,
    ResourceClaim, Table, Transition, ViewDecl,
};
use sembla_runtime::core::{ColumnInit, StateStore, TableInit};

fn tiled_fixture(row_count: usize) -> (ValidatedModel, StateStore) {
    let model = validate(Model {
        name: "tile-determinism".to_owned(),
        dt: 5.0,
        params: Vec::new(),
        boxes: vec![ModelBox {
            name: "world".to_owned(),
            tables: vec![
                Table {
                    name: "Resource".to_owned(),
                    size_hint: 1_024,
                    attrs: Vec::new(),
                },
                Table {
                    name: "KeyResource".to_owned(),
                    size_hint: 1_024,
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
                            name: "resource".to_owned(),
                            ty: AttrType::Ref {
                                table: "Resource".to_owned(),
                            },
                        },
                        Attr {
                            name: "key_resource".to_owned(),
                            ty: AttrType::Ref {
                                table: "KeyResource".to_owned(),
                            },
                        },
                    ],
                },
            ],
            transitions: vec![
                Transition {
                    name: "race".to_owned(),
                    table: "Person".to_owned(),
                    guard: Expr::Gt {
                        lhs: Box::new(Expr::SelfAttr {
                            name: "x".to_owned(),
                        }),
                        rhs: Box::new(Expr::Real { value: -0.5 }),
                    },
                    hazard: Expr::Div {
                        lhs: Box::new(Expr::Add {
                            lhs: Box::new(Expr::Mul {
                                lhs: Box::new(Expr::SelfAttr {
                                    name: "x".to_owned(),
                                }),
                                rhs: Box::new(Expr::Real { value: 0.000_001 }),
                            }),
                            rhs: Box::new(Expr::Real { value: 0.001 }),
                        }),
                        rhs: Box::new(Expr::Real { value: 3.0 }),
                    },
                    effects: Vec::new(),
                    contests: vec![ResourceClaim {
                        resource: Expr::SelfAttr {
                            name: "resource".to_owned(),
                        },
                        ordering: ClaimOrdering::RaceTime,
                    }],
                },
                Transition {
                    name: "key".to_owned(),
                    table: "Person".to_owned(),
                    guard: Expr::Bool { value: true },
                    hazard: Expr::Real { value: 0.001 },
                    effects: vec![Effect::SetAttr {
                        attr: "x".to_owned(),
                        value: Expr::Add {
                            lhs: Box::new(Expr::SelfAttr {
                                name: "x".to_owned(),
                            }),
                            rhs: Box::new(Expr::Real { value: 0.125 }),
                        },
                    }],
                    contests: vec![ResourceClaim {
                        resource: Expr::SelfAttr {
                            name: "key_resource".to_owned(),
                        },
                        ordering: ClaimOrdering::Key {
                            expr: Expr::SelfAttr {
                                name: "x".to_owned(),
                            },
                        },
                    }],
                },
            ],
            inputs: Vec::new(),
            outputs: Vec::new(),
            views: vec![
                ViewDecl {
                    name: "positive_x".to_owned(),
                    table: "Person".to_owned(),
                    filter: Some(Expr::Gt {
                        lhs: Box::new(Expr::Div {
                            lhs: Box::new(Expr::Add {
                                lhs: Box::new(Expr::SelfAttr {
                                    name: "x".to_owned(),
                                }),
                                rhs: Box::new(Expr::Real { value: 0.25 }),
                            }),
                            rhs: Box::new(Expr::Real { value: 3.0 }),
                        }),
                        rhs: Box::new(Expr::Real { value: 0.1 }),
                    }),
                    value: None,
                    reduce: ViewReduce::Count,
                },
                ViewDecl {
                    name: "weighted_x".to_owned(),
                    table: "Person".to_owned(),
                    filter: Some(Expr::Gt {
                        lhs: Box::new(Expr::SelfAttr {
                            name: "x".to_owned(),
                        }),
                        rhs: Box::new(Expr::Real { value: 0.2 }),
                    }),
                    value: Some(Expr::Div {
                        lhs: Box::new(Expr::Add {
                            lhs: Box::new(Expr::SelfAttr {
                                name: "x".to_owned(),
                            }),
                            rhs: Box::new(Expr::Real { value: 0.375 }),
                        }),
                        rhs: Box::new(Expr::Real { value: 1.25 }),
                    }),
                    reduce: ViewReduce::Sum,
                },
            ],
            grouped_views: Vec::new(),
        }],
        wires: Vec::new(),
        summaries: Vec::new(),
    })
    .unwrap();
    let x = (0..row_count)
        .map(|row| (row % 997) as f64 / 997.0)
        .collect();
    let resources = (0..row_count).map(|row| (row % 1_024) as u32).collect();
    let key_resources = (0..row_count)
        .map(|row| ((row * 17) % 1_024) as u32)
        .collect();
    let state = StateStore::new(
        &model,
        vec![
            TableInit::new("world", "Resource", 1_024, Vec::new()),
            TableInit::new("world", "KeyResource", 1_024, Vec::new()),
            TableInit::new(
                "world",
                "Person",
                row_count,
                vec![
                    ColumnInit::new("x", ColumnData::Real(x)),
                    ColumnInit::new("resource", ColumnData::Ref(resources)),
                    ColumnInit::new("key_resource", ColumnData::Ref(key_resources)),
                ],
            ),
        ],
    )
    .unwrap();
    (model, state)
}

fn static_view_profiles(
    model: &ValidatedModel,
    box_index: usize,
    table_name: &str,
) -> Vec<TiledPlanProfile> {
    let model_box = &model.model().boxes[box_index];
    let table = EvalTable::new(model, &model_box.name, table_name).unwrap();
    model_box
        .views
        .iter()
        .filter(|view| view.table == table_name)
        .map(|view| {
            let mut profile = TiledPlanProfile::default();
            if let Some(filter) = &view.filter {
                profile.include(tiled_expr_footprint(filter, table).unwrap().unwrap());
            }
            if let Some(value) = &view.value {
                profile.include(tiled_expr_footprint(value, table).unwrap().unwrap());
            }
            profile
        })
        .collect()
}

#[test]
fn benchmark_shapes_derive_hand_checked_tiles_and_work_decisions() {
    let (mixed_transition_model, _) = tiled_fixture(1);
    let mixed_profiles = mixed_transition_model.model().boxes[0]
        .transitions
        .iter()
        .enumerate()
        .map(|(index, _)| {
            transition_tiling_profile(&mixed_transition_model, 0, index)
                .unwrap()
                .unwrap()
        })
        .collect::<Vec<_>>();
    assert_eq!(
        mixed_profiles
            .iter()
            .map(|profile| profile.node_count)
            .sum::<usize>(),
        15
    );
    let mixed_live_set = mixed_profiles
        .iter()
        .map(|profile| profile.live_set_bytes_per_row())
        .max()
        .unwrap();
    assert_eq!(mixed_live_set, 37);
    assert_eq!(tick_tile_rows_for_live_set(mixed_live_set), 832);

    let demographic = validate(
        parse_json(include_str!(
            "../../../fixtures/demographic/benchmark/demographic_slots.no-grouped.json"
        ))
        .unwrap(),
    )
    .unwrap();
    let transition_profiles = demographic.model().boxes[0]
        .transitions
        .iter()
        .enumerate()
        .map(|(index, _)| {
            transition_tiling_profile(&demographic, 0, index)
                .unwrap()
                .unwrap()
        })
        .collect::<Vec<_>>();
    let transition_nodes = transition_profiles
        .iter()
        .map(|profile| profile.node_count)
        .sum::<usize>();
    let transition_live_set = transition_profiles
        .iter()
        .map(|profile| profile.live_set_bytes_per_row())
        .max()
        .unwrap();
    assert_eq!(transition_nodes, 65);
    assert_eq!(transition_live_set, 33);
    assert_eq!(tick_tile_rows_for_live_set(transition_live_set), 960);
    assert!(tick_tiling_enabled(1_000_000, transition_nodes));

    let demographic_views = static_view_profiles(&demographic, 0, "person_slot");
    let demographic_view_nodes = demographic_views
        .iter()
        .map(|profile| profile.node_count)
        .sum::<usize>();
    let demographic_view_live_set = demographic_views
        .iter()
        .map(|profile| profile.live_set_bytes_per_row())
        .max()
        .unwrap();
    assert_eq!(demographic_view_nodes, 67);
    assert_eq!(demographic_view_live_set, 20);
    assert_eq!(
        tick_tile_rows_for_live_set(demographic_view_live_set),
        1_600
    );
    assert!(tick_tiling_enabled(1_000_000, demographic_view_nodes));

    let canary = validate(
        parse_json(include_str!(
            "../../../fixtures/performance/many_views_tiling_canary.json"
        ))
        .unwrap(),
    )
    .unwrap();
    let canary_views = static_view_profiles(&canary, 0, "row");
    let canary_view_nodes = canary_views
        .iter()
        .map(|profile| profile.node_count)
        .sum::<usize>();
    let canary_view_live_set = canary_views
        .iter()
        .map(|profile| profile.live_set_bytes_per_row())
        .max()
        .unwrap();
    assert_eq!(canary_view_nodes, 680);
    assert_eq!(canary_view_live_set, 41);
    assert_eq!(tick_tile_rows_for_live_set(canary_view_live_set), 768);
    assert!(tick_tiling_enabled(262_144, canary_view_nodes));
}

fn parameter_type_fixture(
    row_count: usize,
    parameter_type: ParamType,
    default: ParamValue,
) -> ValidatedModel {
    validate(Model {
        name: "tile-parameter-type".to_owned(),
        dt: 1.0,
        params: vec![ParamDecl {
            name: "rate".to_owned(),
            ty: parameter_type,
            default,
            prior: None,
        }],
        boxes: vec![ModelBox {
            name: "world".to_owned(),
            tables: vec![Table {
                name: "Person".to_owned(),
                size_hint: row_count as u64,
                attrs: Vec::new(),
            }],
            transitions: vec![Transition {
                name: "parameter-hazard".to_owned(),
                table: "Person".to_owned(),
                guard: Expr::Bool { value: true },
                hazard: Expr::Add {
                    lhs: Box::new(Expr::Param {
                        name: "rate".to_owned(),
                    }),
                    rhs: Box::new(Expr::Real { value: 0.0 }),
                },
                effects: Vec::new(),
                contests: Vec::new(),
            }],
            inputs: Vec::new(),
            outputs: Vec::new(),
            views: Vec::new(),
            grouped_views: Vec::new(),
        }],
        wires: Vec::new(),
        summaries: Vec::new(),
    })
    .unwrap()
}

fn parameter_type_state(model: &ValidatedModel, row_count: usize) -> StateStore {
    StateStore::new(
        model,
        vec![TableInit::new("world", "Person", row_count, Vec::new())],
    )
    .unwrap()
}

fn tiled_race_fingerprint(
    model: &ValidatedModel,
    state: &StateStore,
    workers: usize,
    tile_rows: usize,
) -> Vec<(u32, u32, usize, u32, usize, u32, u64)> {
    with_test_tick_tiles(workers, tile_rows, 0, || {
        let params = ParamEnv::defaults(model);
        let snapshot = state.snapshot();
        let mut results = prepare_tiled_candidates(model, &snapshot, &params, 0xC0FFEE, 7);
        results[0][0]
            .take()
            .expect("transition should clear the tiling threshold")
            .unwrap()
            .into_iter()
            .map(|candidate| {
                let claim = &candidate.claims[0];
                let OrderingValue::RaceTime(time) = claim.ordering else {
                    panic!("test claim must retain its race time");
                };
                (
                    candidate.rule_id,
                    candidate.rule_word,
                    candidate.row,
                    candidate.entity_id,
                    claim.table_index,
                    claim.resource_row,
                    time.to_bits(),
                )
            })
            .collect()
    })
}

fn tiled_key_fingerprint(
    model: &ValidatedModel,
    state: &StateStore,
    workers: usize,
    tile_rows: usize,
) -> Vec<(usize, u32, u64)> {
    with_test_tick_tiles(workers, tile_rows, 0, || {
        let params = ParamEnv::defaults(model);
        let snapshot = state.snapshot();
        let mut results = prepare_tiled_candidates(model, &snapshot, &params, 0xC0FFEE, 7);
        results[0][1]
            .take()
            .expect("key transition should clear the tiling threshold")
            .unwrap()
            .into_iter()
            .map(|candidate| {
                let claim = &candidate.claims[0];
                let OrderingValue::Real(key) = claim.ordering else {
                    panic!("test claim must retain its Real key");
                };
                (candidate.row, claim.resource_row, key.to_bits())
            })
            .collect()
    })
}

#[test]
fn real_chain_racing_clock_and_key_are_bit_identical_across_workers_and_tiles() {
    let (model, state) = tiled_fixture(65_537);
    let serial_races = tiled_race_fingerprint(&model, &state, 1, 257);
    let serial_keys = tiled_key_fingerprint(&model, &state, 1, 257);
    assert!(!serial_races.is_empty());
    assert!(!serial_keys.is_empty());
    for workers in [1, 2, 4] {
        for tile_rows in [257, 1_024, 4_093] {
            assert_eq!(
                tiled_race_fingerprint(&model, &state, workers, tile_rows),
                serial_races,
                "racing clock changed at {workers} workers and {tile_rows} rows/tile"
            );
            assert_eq!(
                tiled_key_fingerprint(&model, &state, workers, tile_rows),
                serial_keys,
                "claim key changed at {workers} workers and {tile_rows} rows/tile"
            );
        }
    }
}

#[test]
fn tick_report_is_identical_across_workers_and_tiles() {
    let evaluate = |workers, tile_rows| {
        let (model, mut state) = tiled_fixture(65_537);
        with_test_tick_tiles(workers, tile_rows, 0, || {
            let params = ParamEnv::defaults(&model);
            run_tick(&model, &mut state, &params, 0xC0FFEE, 7).unwrap()
        })
    };
    let serial = evaluate(1, 257);
    for workers in [1, 2, 4] {
        for tile_rows in [257, 1_024, 4_093] {
            assert_eq!(
                evaluate(workers, tile_rows),
                serial,
                "tick output changed at {workers} workers and {tile_rows} rows/tile"
            );
        }
    }
}

#[test]
fn numeric_view_sum_and_effect_state_are_bit_identical_across_workers_and_tiles() {
    let row_count = 65_537;
    let evaluate = |workers, tile_rows, threshold| {
        let (model, mut state) = tiled_fixture(row_count);
        let report = with_test_tick_tiles(workers, tile_rows, threshold, || {
            let params = ParamEnv::defaults(&model);
            run_tick(&model, &mut state, &params, 0xC0FFEE, 7).unwrap()
        });
        let sum_bits = report
            .views
            .iter()
            .find(|view| view.name == "weighted_x")
            .and_then(|view| match view.value {
                ObservationValue::Real(value) => Some(value.to_bits()),
                ObservationValue::Int(_) => None,
            })
            .expect("fixture must report the Real numeric view");
        let snapshot = state.snapshot();
        let state_bits = (0..row_count)
            .map(|row| {
                snapshot
                    .real("world", "Person", "x", row)
                    .unwrap()
                    .to_bits()
            })
            .collect::<Vec<_>>();
        (sum_bits, state_bits)
    };

    let fallback = evaluate(1, 1_024, row_count + 1);
    assert!(
        fallback
            .1
            .iter()
            .enumerate()
            .any(|(row, bits)| { *bits != ((row % 997) as f64 / 997.0).to_bits() }),
        "the fixture must execute at least one effect write"
    );
    for workers in [1, 2, 4] {
        for tile_rows in [257, 1_024, 4_093] {
            assert_eq!(
                evaluate(workers, tile_rows, 0),
                fallback,
                "numeric view or effect state changed at {workers} workers and {tile_rows} rows/tile"
            );
        }
    }
}

#[test]
fn tiled_real_view_reduction_keeps_canonical_row_order() {
    let source = include_str!("executor/tiling.rs");
    let reduction = source
        .split_once("// This is the canonical Level A reduction order")
        .expect("tiled Real view reduction must document its canonical order")
        .1
        .split_once("Ok(ObservationValue::Real(result))")
        .expect("tiled Real reduction must return its ordered result")
        .0;
    assert!(reduction.contains("for (column, filter) in columns.into_iter().zip(filters)"));
    assert!(reduction.contains("for (offset, value) in values.iter().copied().enumerate()"));
    assert!(reduction.contains("ViewReduce::Sum => result + value"));
    assert!(!reduction.contains("sum::<f64>"));
    assert!(!reduction.contains("par_iter"));
}

#[test]
fn wrong_typed_parameter_environment_matches_fallback_across_workers_and_tiles() {
    let row_count = 65_537;
    let model = parameter_type_fixture(
        row_count,
        ParamType::Real,
        ParamValue::Real { value: 0.001 },
    );
    let environment_model =
        parameter_type_fixture(row_count, ParamType::Int, ParamValue::Int { value: 1 });
    let params = ParamEnv::defaults(&environment_model);
    let evaluate = |workers, tile_rows, threshold| {
        let mut state = parameter_type_state(&model, row_count);
        with_test_tick_tiles(workers, tile_rows, threshold, || {
            run_tick(&model, &mut state, &params, 0xC0FFEE, 7)
                .expect_err("the mismatched parameter environment must be rejected")
                .to_string()
        })
    };

    let fallback_error = evaluate(1, 1_024, row_count + 1);
    assert!(fallback_error.contains("parameter environment value for 'rate' has the wrong type"));
    for workers in [1, 2, 4] {
        for tile_rows in [257, 1_024, 4_093] {
            assert_eq!(
                evaluate(workers, tile_rows, 0),
                fallback_error,
                "parameter error changed at {workers} workers and {tile_rows} rows/tile"
            );
        }
    }
}

#[test]
fn racing_clock_filter_rejects_below_lo_and_admits_boundary_envelope() {
    let filter = RacingClockFilter::for_hazard(0.025, 1.0).unwrap();
    let below = f64::from_bits(filter.lo.to_bits() - 1);
    let above = f64::from_bits(filter.lo.to_bits() + 1);

    assert!(!filter.admits(below));
    assert!(filter.admits(filter.lo));
    assert!(filter.admits(above));
}

#[test]
fn guarded_racing_clock_preserves_firing_set_bits_and_contested_winner() -> Result<(), TickError> {
    let seed = 0xC0FFEE;
    let tick = 7;
    let rule_id = 19;
    let rule_word = 29;
    let dt = 1.0;
    let mut rejected = 0;
    let mut admitted = 0;

    for lambda in [
        0.001, 0.002, 0.0025, 0.003, 0.012, 0.018, 0.020, 0.025, 1e300,
    ] {
        let filter = RacingClockFilter::for_hazard(lambda, dt).unwrap();
        for row in 0..100_000 {
            let entity_id = row as u32;
            let uniform = uniform_f64(seed, tick, rule_word, entity_id, 0);
            if filter.admits(uniform) {
                admitted += 1;
            } else {
                rejected += 1;
            }
            let oracle = exp_f64(seed, tick, rule_word, entity_id, 0, lambda);
            let expected = (oracle.partial_cmp(&dt) == Some(Ordering::Less))
                .then_some((entity_id, oracle.to_bits()));
            let actual = candidate_race_time(
                RacingClockCoordinates {
                    seed,
                    tick,
                    rule_id,
                    rule_word,
                    row,
                },
                lambda,
                dt,
                RacingClockStrategy::Guarded(filter),
            )?
            .map(|firing| {
                (
                    firing.entity_id,
                    firing
                        .race_time
                        .expect("the guarded path samples a race time")
                        .to_bits(),
                )
            });
            assert_eq!(actual, expected, "firing set changed for hazard {lambda}");
        }
    }
    assert!(rejected > 0, "the sweep must exercise the fast reject path");
    assert!(
        admitted > 0,
        "the sweep must exercise the canonical ln path"
    );

    // A contested transition consumes exact race-time bits for its argmin.
    // Group rows onto 128 resources and prove every winner is unchanged.
    let lambda = 0.025;
    let filter = RacingClockFilter::for_hazard(lambda, dt).unwrap();
    let mut oracle_winners: Vec<Option<(f64, u32)>> = vec![None; 128];
    let mut guarded_winners: Vec<Option<(f64, u32)>> = vec![None; 128];
    for row in 0..100_000 {
        let entity_id = row as u32;
        let resource = row % 128;
        let oracle = exp_f64(seed, tick, rule_word, entity_id, 0, lambda);
        if oracle.partial_cmp(&dt) == Some(Ordering::Less) {
            let candidate = (oracle, entity_id);
            if oracle_winners[resource].map_or(true, |winner| {
                candidate
                    .0
                    .total_cmp(&winner.0)
                    .then(candidate.1.cmp(&winner.1))
                    == Ordering::Less
            }) {
                oracle_winners[resource] = Some(candidate);
            }
        }
        if let Some(firing) = candidate_race_time(
            RacingClockCoordinates {
                seed,
                tick,
                rule_id,
                rule_word,
                row,
            },
            lambda,
            dt,
            RacingClockStrategy::Guarded(filter),
        )? {
            let candidate = (
                firing
                    .race_time
                    .expect("the guarded path samples a race time"),
                firing.entity_id,
            );
            if guarded_winners[resource].map_or(true, |winner| {
                candidate
                    .0
                    .total_cmp(&winner.0)
                    .then(candidate.1.cmp(&winner.1))
                    == Ordering::Less
            }) {
                guarded_winners[resource] = Some(candidate);
            }
        }
    }
    assert_eq!(
        guarded_winners
            .iter()
            .map(|winner| winner.map(|(time, entity)| (time.to_bits(), entity)))
            .collect::<Vec<_>>(),
        oracle_winners
            .iter()
            .map(|winner| winner.map(|(time, entity)| (time.to_bits(), entity)))
            .collect::<Vec<_>>()
    );

    Ok::<(), TickError>(())
}

#[test]
fn degenerate_uncontested_transition_skips_clock_but_contested_transition_does_not() {
    let make_transition = |contests| Transition {
        name: "degenerate".to_owned(),
        table: "Person".to_owned(),
        guard: Expr::Bool { value: true },
        hazard: Expr::Real { value: 1e300 },
        effects: Vec::new(),
        contests,
    };
    let filter = RacingClockFilter::for_hazard(1e300, 1.0).unwrap();
    assert_eq!(filter.threshold, 0.0);

    let uncontested = make_transition(Vec::new());
    let uncontested_strategy = RacingClockStrategy::for_transition(Some(filter), &uncontested);
    assert_eq!(uncontested_strategy, RacingClockStrategy::AlwaysFires);
    let firing = candidate_race_time(
        RacingClockCoordinates {
            seed: 1,
            tick: 0,
            rule_id: 7,
            rule_word: 11,
            row: 0,
        },
        1e300,
        1.0,
        uncontested_strategy,
    )
    .unwrap()
    .expect("the exact-zero threshold must always fire");
    assert_eq!(firing.entity_id, 0);
    assert_eq!(firing.race_time, None, "the fast path must not draw");

    let contested = make_transition(vec![ResourceClaim {
        resource: Expr::SelfAttr {
            name: "resource".to_owned(),
        },
        ordering: ClaimOrdering::RaceTime,
    }]);
    let contested_strategy = RacingClockStrategy::for_transition(Some(filter), &contested);
    assert_eq!(
        contested_strategy,
        RacingClockStrategy::Guarded(filter),
        "a contested transition consumes the exact race time"
    );
    let contested_firing = candidate_race_time(
        RacingClockCoordinates {
            seed: 1,
            tick: 0,
            rule_id: 7,
            rule_word: 11,
            row: 0,
        },
        1e300,
        1.0,
        contested_strategy,
    )
    .unwrap()
    .expect("the degenerate contested transition must still fire");
    assert_eq!(
        contested_firing.race_time.unwrap().to_bits(),
        exp_f64(1, 0, 11, 0, 0, 1e300).to_bits(),
        "the contested path must retain the canonical sampled time"
    );

    let ordinary = make_transition(Vec::new());
    let ordinary_filter = RacingClockFilter::for_hazard(0.025, 1.0).unwrap();
    assert_eq!(
        RacingClockStrategy::for_transition(Some(ordinary_filter), &ordinary),
        RacingClockStrategy::Guarded(ordinary_filter),
        "an uncontested transition with a nonzero threshold is not provably certain"
    );
}

#[cfg(target_pointer_width = "64")]
#[test]
fn racing_clock_fast_paths_cannot_hide_entity_id_overflow() {
    let row = u32::MAX as usize + 1;
    for strategy in [
        RacingClockStrategy::Guarded(RacingClockFilter {
            threshold: 1.0,
            lo: 1.0,
        }),
        RacingClockStrategy::AlwaysFires,
    ] {
        let error = candidate_race_time(
            RacingClockCoordinates {
                seed: 1,
                tick: 0,
                rule_id: 7,
                rule_word: 11,
                row,
            },
            0.001,
            1.0,
            strategy,
        )
        .expect_err("row conversion must precede every fast path");
        match error {
            TickError::EntityIdOverflow {
                rule_id,
                row: error_row,
            } => {
                assert_eq!(rule_id, 7);
                assert_eq!(error_row, row);
            }
            other => panic!("unexpected error: {other}"),
        }
    }
}

#[test]
fn threshold_falls_back_and_only_tick_orchestration_can_spawn() {
    let row_count = 4_097;
    let (model, state) = tiled_fixture(row_count);
    with_test_tick_tiles(4, 257, row_count + 1, || {
        let params = ParamEnv::defaults(&model);
        let snapshot = state.snapshot();
        let results = prepare_tiled_candidates(&model, &snapshot, &params, 1, 0);
        assert!(results[0][0].is_none());
    });

    let production_executor = concat!(
        include_str!("executor.rs"),
        include_str!("executor/tiling.rs"),
        include_str!("executor/staging.rs"),
    );
    let production_eval = include_str!("eval.rs")
        .split_once("#[cfg(test)]")
        .unwrap()
        .0;
    assert_eq!(
        production_executor
            .matches("std::thread::scope(|scope|")
            .count(),
        1,
        "transition and observation tiling must share one fixed-task worker implementation"
    );
    assert!(!production_eval.contains("std::thread::scope(|scope|"));
}
