use std::path::Path;

use sembla_ir::{Expr, ViewReduce};

use super::{
    cuda_f64_order_key, decode_grouped_histogram, generate, generate_fused_batch,
    grouped_observation_layout, host_observation_fallback, GeneratedGroupedObservation,
    GroupedObservationAxis, GroupedViewValue, DUMP_ENV, GROUPED_OBSERVATION_KEY_SPACE_LIMIT,
};

fn example_model(name: &str) -> sembla_ir::ValidatedModel {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join(format!("../../examples/{name}"));
    let source = std::fs::read_to_string(path).unwrap();
    sembla_ir::validate(sembla_ir::parse_json(&source).unwrap()).unwrap()
}

fn sir_model() -> sembla_ir::ValidatedModel {
    example_model("sir.json")
}

fn grouped_model() -> sembla_ir::ValidatedModel {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../sembla-cli/tests/fixtures/grouped_observation.json");
    let source = std::fs::read_to_string(path).unwrap();
    let features =
        sembla_ir::FeatureSet::from([sembla_ir::GROUPED_OBSERVATIONS_FEATURE.to_owned()]);
    sembla_ir::validate_with_features(sembla_ir::parse_json(&source).unwrap(), &features).unwrap()
}

fn grouped_only_model() -> sembla_ir::ValidatedModel {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../sembla-cli/tests/fixtures/grouped_observation.json");
    let mut model = sembla_ir::parse_json(&std::fs::read_to_string(path).unwrap()).unwrap();
    model.boxes[0].views.clear();
    let features =
        sembla_ir::FeatureSet::from([sembla_ir::GROUPED_OBSERVATIONS_FEATURE.to_owned()]);
    sembla_ir::validate_with_features(model, &features).unwrap()
}

fn nested_output_model(wired: bool) -> sembla_ir::ValidatedModel {
    use sembla_ir::{
        AggJoin, AggOp, Attr, AttrType, Box as ModelBox, Expr, Model, OutputBuilder, OutputDecl,
        OutputField, PortDecl, Table, Wire, WireEndpoint,
    };
    let group_attr = Attr {
        name: "group".to_owned(),
        ty: AttrType::Ref {
            table: "Group".to_owned(),
        },
    };
    let total_attr = Attr {
        name: "total".to_owned(),
        ty: AttrType::Real,
    };
    sembla_ir::validate(Model {
        name: "nested_output".to_owned(),
        dt: 1.0,
        params: Vec::new(),
        boxes: vec![
            ModelBox {
                name: "source".to_owned(),
                tables: vec![
                    Table {
                        name: "Group".to_owned(),
                        size_hint: 1,
                        attrs: Vec::new(),
                    },
                    Table {
                        name: "Person".to_owned(),
                        size_hint: 2,
                        attrs: vec![
                            group_attr.clone(),
                            Attr {
                                name: "x".to_owned(),
                                ty: AttrType::Real,
                            },
                        ],
                    },
                ],
                transitions: Vec::new(),
                inputs: Vec::new(),
                outputs: vec![OutputDecl {
                    name: "totals".to_owned(),
                    schema: vec![total_attr.clone()],
                    builder: OutputBuilder::PerTable {
                        table: "Person".to_owned(),
                        fields: vec![OutputField {
                            name: "total".to_owned(),
                            op: AggOp::Sum {
                                value: Box::new(Expr::Agg {
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
                                }),
                            },
                            filter: None,
                        }],
                    },
                }],
                views: Vec::new(),
                grouped_views: Vec::new(),
            },
            ModelBox {
                name: "sink".to_owned(),
                tables: Vec::new(),
                transitions: Vec::new(),
                inputs: vec![PortDecl {
                    name: "totals".to_owned(),
                    schema: vec![total_attr],
                }],
                outputs: Vec::new(),
                views: Vec::new(),
                grouped_views: Vec::new(),
            },
        ],
        wires: if wired {
            vec![Wire {
                from: WireEndpoint {
                    r#box: "source".to_owned(),
                    port: "totals".to_owned(),
                },
                to: WireEndpoint {
                    r#box: "sink".to_owned(),
                    port: "totals".to_owned(),
                },
            }]
        } else {
            Vec::new()
        },
        summaries: Vec::new(),
    })
    .unwrap()
}

fn contested_model() -> sembla_ir::ValidatedModel {
    let source = r#"{"name":"claims","dt":1.0,"params":[],"boxes":[{"name":"world","tables":[{"name":"Worker","size_hint":1,"attrs":[]},{"name":"Applicant","size_hint":2,"attrs":[{"name":"worker","ty":{"kind":"ref","table":"Worker"}},{"name":"priority","ty":{"kind":"int"}},{"name":"state","ty":{"kind":"enum","variants":["Waiting","Done"]}}]}],"transitions":[{"name":"finish","table":"Applicant","guard":{"kind":"enum_is","attr":"state","variant":"Waiting"},"hazard":{"kind":"real","value":1.0},"effects":[{"kind":"set_attr","attr":"state","value":{"kind":"enum","variant":"Done"}}],"contests":[{"resource":{"kind":"self_attr","name":"worker"},"ordering":{"kind":"key","expr":{"kind":"mul","lhs":{"kind":"self_attr","name":"priority"},"rhs":{"kind":"int","value":2}}}}]}],"inputs":[],"outputs":[],"views":[]}],"wires":[],"summaries":[]}"#;
    sembla_ir::validate(sembla_ir::parse_json(source).unwrap()).unwrap()
}

fn stable_contested_model() -> sembla_ir::ValidatedModel {
    use sembla_ir::{
        occurrence_of_leaf, rule_word, transition_identity, ExecutablePlanV1, IdentityMapV1,
        LeafIdentityV1, PlanOrigin, SchedulerDomainV1, TransitionIdentityV1,
        EXECUTABLE_PLAN_SCHEMA, STABLE_IDENTITY_SCHEME,
    };

    let mut model = contested_model().into_model();
    model.boxes[0]
        .tables
        .sort_by(|left, right| left.name.cmp(&right.name));
    let mut second = model.boxes[0].transitions[0].clone();
    second.name = String::from("finish_other");
    model.boxes[0].transitions.push(second);

    let occurrence = occurrence_of_leaf("world");
    let mut transitions = model.boxes[0]
        .transitions
        .iter()
        .map(|transition| {
            let identity = transition_identity(&occurrence, &transition.name);
            TransitionIdentityV1 {
                r#box: "world".to_owned(),
                name: transition.name.clone(),
                rule_word: rule_word(&identity),
                identity,
            }
        })
        .collect::<Vec<_>>();
    transitions.sort_by(|left, right| left.identity.cmp(&right.identity));
    let plan = ExecutablePlanV1 {
        schema_version: EXECUTABLE_PLAN_SCHEMA.to_owned(),
        identity_scheme: STABLE_IDENTITY_SCHEME.to_owned(),
        origin: PlanOrigin::DirectStable,
        identity: IdentityMapV1 {
            model_id: "model:claims".to_owned(),
            enabled_features: Vec::new(),
            scheduler_domains: vec![SchedulerDomainV1 {
                id: "domain:global".to_owned(),
                algorithm: "tau_leap".to_owned(),
                leaves: vec!["world".to_owned()],
            }],
            leaves: vec![LeafIdentityV1 {
                r#box: "world".to_owned(),
                occurrence,
            }],
            transitions,
            mailboxes: Vec::new(),
        },
        model,
        linked_provenance: None,
    };
    sembla_ir::validate_plan(&plan)
        .unwrap()
        .model_with_rule_words()
}

fn incompatible_claim_model() -> sembla_ir::ValidatedModel {
    let source = r#"{"name":"incompatible_claims","dt":1.0,"params":[],"boxes":[{"name":"world","tables":[{"name":"Worker","size_hint":1,"attrs":[]},{"name":"Applicant","size_hint":1,"attrs":[{"name":"worker","ty":{"kind":"ref","table":"Worker"}},{"name":"priority","ty":{"kind":"int"}}]}],"transitions":[{"name":"race","table":"Applicant","guard":{"kind":"bool","value":true},"hazard":{"kind":"real","value":1e300},"effects":[],"contests":[{"resource":{"kind":"self_attr","name":"worker"},"ordering":{"kind":"race_time"}}]},{"name":"priority","table":"Applicant","guard":{"kind":"bool","value":true},"hazard":{"kind":"real","value":1e300},"effects":[],"contests":[{"resource":{"kind":"self_attr","name":"worker"},"ordering":{"kind":"key","expr":{"kind":"self_attr","name":"priority"}}}]}],"inputs":[],"outputs":[],"views":[]}],"wires":[],"summaries":[]}"#;
    sembla_ir::validate(sembla_ir::parse_json(source).unwrap()).unwrap()
}

fn minimum_integer_model() -> sembla_ir::ValidatedModel {
    let source = r#"{"name":"minimum_integer","dt":1.0,"params":[],"boxes":[{"name":"world","tables":[{"name":"Person","size_hint":1,"attrs":[{"name":"x","ty":{"kind":"int"}}]}],"transitions":[{"name":"set_minimum","table":"Person","guard":{"kind":"lt","lhs":{"kind":"int","value":-9223372036854775808},"rhs":{"kind":"self_attr","name":"x"}},"hazard":{"kind":"real","value":1e300},"effects":[{"kind":"set_attr","attr":"x","value":{"kind":"int","value":-9223372036854775808}}],"contests":[]}],"inputs":[],"outputs":[],"views":[]}],"wires":[],"summaries":[]}"#;
    sembla_ir::validate(sembla_ir::parse_json(source).unwrap()).unwrap()
}

fn input_integer_ordering_model() -> sembla_ir::ValidatedModel {
    let source = r#"{"name":"input_integer_ordering","dt":1.0,"params":[],"boxes":[{"name":"source","tables":[{"name":"Event","size_hint":1,"attrs":[{"name":"amount","ty":{"kind":"int"}}]}],"transitions":[],"inputs":[],"outputs":[{"name":"events","schema":[{"name":"amount","ty":{"kind":"int"}}],"builder":{"kind":"per_table","table":"Event","fields":[{"name":"amount","op":{"kind":"sum","value":{"kind":"self_attr","name":"amount"}},"filter":null}]}}],"views":[]},{"name":"sink","tables":[{"name":"Agent","size_hint":1,"attrs":[{"name":"state","ty":{"kind":"enum","variants":["Off","On"]}}]}],"transitions":[{"name":"activate","table":"Agent","guard":{"kind":"gt","lhs":{"kind":"input","port":"events","agg":{"op":{"kind":"count"},"filter":{"kind":"gt","lhs":{"kind":"self_attr","name":"amount"},"rhs":{"kind":"int","value":9007199254740992}}}},"rhs":{"kind":"int","value":0}},"hazard":{"kind":"real","value":1e300},"effects":[{"kind":"set_attr","attr":"state","value":{"kind":"enum","variant":"On"}}],"contests":[]}],"inputs":[{"name":"events","schema":[{"name":"amount","ty":{"kind":"int"}}]}],"outputs":[],"views":[]}],"wires":[{"from":{"box":"source","port":"events"},"to":{"box":"sink","port":"events"}}],"summaries":[]}"#;
    sembla_ir::validate(sembla_ir::parse_json(source).unwrap()).unwrap()
}

fn shared_schedule_output_aggregate_model() -> sembla_ir::ValidatedModel {
    let source = r#"{"name":"shared_aggregate","dt":1.0,"params":[],"boxes":[{"name":"source","tables":[{"name":"Group","size_hint":1,"attrs":[]},{"name":"Person","size_hint":1,"attrs":[{"name":"group","ty":{"kind":"ref","table":"Group"}}]}],"transitions":[{"name":"observe","table":"Person","guard":{"kind":"gt","lhs":{"kind":"agg","op":{"kind":"count"},"table":"Person","on":{"fk_attr":"group","self_fk_attr":"group"},"filter":{"kind":"bool","value":true}},"rhs":{"kind":"int","value":0}},"hazard":{"kind":"real","value":1.0},"effects":[],"contests":[]}],"inputs":[],"outputs":[{"name":"totals","schema":[{"name":"total","ty":{"kind":"int"}}],"builder":{"kind":"per_table","table":"Person","fields":[{"name":"total","op":{"kind":"sum","value":{"kind":"agg","op":{"kind":"count"},"table":"Person","on":{"fk_attr":"group","self_fk_attr":"group"},"filter":{"kind":"bool","value":true}}},"filter":null}]}}],"views":[]},{"name":"sink","tables":[],"transitions":[],"inputs":[{"name":"totals","schema":[{"name":"total","ty":{"kind":"int"}}]}],"outputs":[],"views":[]}],"wires":[{"from":{"box":"source","port":"totals"},"to":{"box":"sink","port":"totals"}}],"summaries":[]}"#;
    sembla_ir::validate(sembla_ir::parse_json(source).unwrap()).unwrap()
}

#[test]
fn hostile_model_name_is_represented_by_ascii_digest_only() {
    let source = r#"{"name":"ok\n#error injected_model_name\r\\☃","dt":1.0,"params":[],"boxes":[],"wires":[],"summaries":[]}"#;
    let model = sembla_ir::validate(sembla_ir::parse_json(source).unwrap()).unwrap();
    let first = generate(&model).unwrap();
    let second = generate(&model).unwrap();
    assert_eq!(first, second);
    assert!(!first.source.contains("#error injected_model_name"));
    let label = first.source.lines().nth(1).unwrap();
    assert!(label.starts_with("// model-name-sha256: "));
    assert_eq!(label.len(), "// model-name-sha256: ".len() + 64);
    assert!(label.is_ascii());
}

/// Extracts one emitted kernel body for scoped source assertions.
/// Generated kernels close with a brace at column 0; every nested brace
/// is indented.
fn kernel_body<'a>(source: &'a str, name: &str) -> &'a str {
    let marker = format!("extern \"C\" __global__ void {name}(");
    let start = source
        .find(&marker)
        .unwrap_or_else(|| panic!("kernel {name} missing from emitted source"));
    let rest = &source[start..];
    let end = rest
        .find("\n}\n")
        .map(|index| index + 2)
        .unwrap_or(rest.len());
    &rest[..end]
}

#[test]
fn generation_is_deterministic_and_has_one_kernel_per_transition() {
    let model = sir_model();
    let first = generate(&model).unwrap();
    let second = generate(&model).unwrap();
    assert_eq!(first, second);
    assert_eq!(
        first.transition_kernels,
        ["sembla_transition_00000000", "sembla_transition_00000001"]
    );
    assert!(first.source.contains("sembla_build_aggregate_partials"));
    assert!(first.source.contains("sembla_finish_aggregates"));
    assert!(first.source.contains("sembla_validate_claims"));
    assert!(first.source.contains("sembla_resolve_conflicts"));
    assert!(first.source.contains("sembla_prepare_effects"));
    assert!(first.source.contains("sembla_apply_effects"));
    assert!(first.source.contains("sembla_build_output_partials"));
    assert!(first.source.contains("sembla_finish_outputs"));
    let observation = kernel_body(&first.source, "sembla_observe_view");
    assert!(observation.contains("extern __shared__ long long partials[]"));
    assert!(observation.contains("atomicAdd"));
    // The argmin uses only order-independent atomic minima in staged
    // prefix passes. Finalization itself is a bounded own-claim lookup.
    let resolver = kernel_body(&first.source, "sembla_resolve_conflicts");
    assert!(!resolver.contains("atomicMin"));
    assert!(!resolver.contains("atomicAdd"));
    assert!(!resolver.contains("other_row"));
}

#[test]
fn generated_sources_contain_no_scalar_final_state_sha_kernel() {
    for generated in [
        generate(&sir_model()).unwrap(),
        generate_fused_batch(&sir_model()).unwrap(),
    ] {
        assert!(!generated.source.contains("sembla_final_state_sha256"));
        assert!(!generated.source.contains("sembla_sha256_compress"));
        assert!(!generated.source.contains("sembla_sha256_context"));
    }
}

#[test]
fn host_ineligible_route_executes_download_and_device_route_does_not() {
    let mut downloads = 0;
    let fallback = host_observation_fallback(true, || {
        downloads += 1;
        Ok::<_, ()>(())
    })
    .unwrap();
    assert_eq!(fallback, Some(()));
    assert_eq!(downloads, 1);

    let fast = host_observation_fallback(false, || {
        downloads += 1;
        Ok::<_, ()>(())
    })
    .unwrap();
    assert_eq!(fast, None);
    assert_eq!(downloads, 1);
}

#[test]
fn device_observation_codegen_is_all_or_nothing() {
    let eligible = generate(&sir_model()).unwrap();
    assert!(eligible.observation_eligibility.eligible);
    assert_eq!(eligible.observation_view_tables.len(), 3);
    let kernel = kernel_body(&eligible.source, "sembla_observe_view");
    assert!(kernel.contains("extern __shared__ long long partials[]"));
    assert!(kernel.contains("if (view_index == 0U)"));

    let ineligible = generate(&example_model("observations.json")).unwrap();
    assert!(!ineligible.observation_eligibility.eligible);
    assert!(ineligible.observation_view_tables.is_empty());
    let kernel = kernel_body(&ineligible.source, "sembla_observe_view");
    assert!(!kernel.contains("if (view_index == 0U)"));

    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../sembla-cli/tests/fixtures/grouped_observation.json");
    let mut raw = sembla_ir::parse_json(&std::fs::read_to_string(path).unwrap()).unwrap();
    raw.boxes[0].views[0].reduce = ViewReduce::Sum;
    raw.boxes[0].views[0].value = Some(Expr::SelfAttr {
        name: "age_months".to_owned(),
    });
    let features =
        sembla_ir::FeatureSet::from([sembla_ir::GROUPED_OBSERVATIONS_FEATURE.to_owned()]);
    let model = sembla_ir::validate_with_features(raw, &features).unwrap();
    let ineligible_mixed = generate(&model).unwrap();
    assert!(!ineligible_mixed.observation_eligibility.eligible);
    assert!(ineligible_mixed.grouped_observation_views.is_empty());
    let grouped = kernel_body(&ineligible_mixed.source, "sembla_observe_grouped_view");
    assert!(!grouped.contains("if (view_index == 0U)"));
}

#[test]
fn generated_unsigned_band_formula_matches_host_euclidean_division() {
    let cases = [
        (-1_i64, 60_u64),
        (-60, 60),
        (-61, 60),
        (0, 60),
        (61, 60),
        (i64::MIN, 1),
        (i64::MIN, i64::MAX as u64 + 1),
        (i64::MAX, u64::MAX),
    ];
    for (value, width) in cases {
        let device_formula = if value >= 0 {
            (value as u64 / width) as i64
        } else {
            let magnitude_minus_one = (-(value + 1)) as u64;
            -1 - (magnitude_minus_one / width) as i64
        };
        let host = i128::from(value).div_euclid(i128::from(width));
        assert_eq!(i128::from(device_formula), host, "{value} / {width}");
    }
}

#[test]
fn grouped_layout_uses_exact_banded_extrema_and_fails_past_the_limit() {
    let banded = GeneratedGroupedObservation {
        box_name: "world".to_owned(),
        name: "by_band".to_owned(),
        table: 0,
        axes: vec![GroupedObservationAxis::BandedInt {
            column: 0,
            width: 60,
            extrema_index: 0,
        }],
    };
    let layout = grouped_observation_layout(&banded, &[7], &[-121, 121]).unwrap();
    assert_eq!(layout.axes[0].minimum, -3);
    assert_eq!(layout.axes[0].cardinality, 6);
    assert_eq!(layout.key_space_size, 6);

    let exact = GeneratedGroupedObservation {
        box_name: "world".to_owned(),
        name: "exact_limit".to_owned(),
        table: 0,
        axes: vec![GroupedObservationAxis::Enum {
            column: 0,
            cardinality: GROUPED_OBSERVATION_KEY_SPACE_LIMIT as u64,
        }],
    };
    assert_eq!(
        grouped_observation_layout(&exact, &[1], &[])
            .unwrap()
            .key_space_size,
        GROUPED_OBSERVATION_KEY_SPACE_LIMIT
    );

    let over = GeneratedGroupedObservation {
        name: "over_limit".to_owned(),
        axes: vec![GroupedObservationAxis::Enum {
            column: 0,
            cardinality: GROUPED_OBSERVATION_KEY_SPACE_LIMIT as u64 + 1,
        }],
        ..exact.clone()
    };
    let error = grouped_observation_layout(&over, &[1], &[])
        .unwrap_err()
        .to_string();
    assert!(error.contains("box='world' view='over_limit'"), "{error}");
    assert!(
        error.contains("computed_size=1048577 limit=1048576"),
        "{error}"
    );

    let full_i64_range = GeneratedGroupedObservation {
        box_name: "world".to_owned(),
        name: "full_i64_range".to_owned(),
        table: 0,
        axes: vec![GroupedObservationAxis::BandedInt {
            column: 0,
            width: 1,
            extrema_index: 0,
        }],
    };
    let error = grouped_observation_layout(&full_i64_range, &[1], &[i64::MIN, i64::MAX])
        .unwrap_err()
        .to_string();
    assert!(
        error.contains("computed_size=18446744073709551616 limit=1048576"),
        "{error}"
    );
}

#[test]
fn grouped_histogram_omits_empty_groups_and_matches_btree_order_exactly() {
    let view = GeneratedGroupedObservation {
        box_name: "world".to_owned(),
        name: "cells".to_owned(),
        table: 0,
        axes: vec![
            GroupedObservationAxis::Enum {
                column: 0,
                cardinality: 2,
            },
            GroupedObservationAxis::BandedInt {
                column: 1,
                width: 60,
                extrema_index: 0,
            },
        ],
    };
    let layout = grouped_observation_layout(&view, &[5], &[-61, 121]).unwrap();
    assert_eq!(layout.key_space_size, 10);
    let mut counters = vec![0_u64; layout.key_space_size];
    counters[0] = 3;
    counters[4] = 1;
    counters[6] = 2;
    assert_eq!(
        decode_grouped_histogram(&view, &layout, &counters).unwrap(),
        vec![
            GroupedViewValue {
                box_name: "world".to_owned(),
                name: "cells".to_owned(),
                keys: vec![0, -2],
                count: 3,
            },
            GroupedViewValue {
                box_name: "world".to_owned(),
                name: "cells".to_owned(),
                keys: vec![0, 2],
                count: 1,
            },
            GroupedViewValue {
                box_name: "world".to_owned(),
                name: "cells".to_owned(),
                keys: vec![1, -1],
                count: 2,
            },
        ]
    );
}

#[test]
fn grouped_codegen_collects_boundable_axes_and_emits_dense_histogram_kernels() {
    let generated = generate(&grouped_model()).unwrap();
    assert!(generated.observation_eligibility.eligible);
    assert_eq!(generated.grouped_observation_band_axes, 1);
    assert_eq!(generated.grouped_observation_views.len(), 1);
    let view = &generated.grouped_observation_views[0];
    assert_eq!(
        (view.box_name.as_str(), view.name.as_str()),
        ("world", "population_cells")
    );
    assert!(matches!(
        view.axes.as_slice(),
        [
            GroupedObservationAxis::Enum { cardinality: 2, .. },
            GroupedObservationAxis::Ref { .. },
            GroupedObservationAxis::BandedInt { width: 60, .. }
        ]
    ));
    for symbol in [
        "sembla_init_grouped_extrema",
        "sembla_bound_grouped_view",
        "sembla_init_grouped_histogram",
        "sembla_observe_grouped_view",
        "sembla_div_euclid_i64_u64",
    ] {
        assert!(generated.source.contains(symbol), "missing {symbol}");
    }
    let histogram = kernel_body(&generated.source, "sembla_observe_grouped_view");
    assert!(histogram.contains("atomicAdd(counts + group, 1ULL)"));
    assert!(histogram.contains("group = group * axis_cardinalities"));

    let layout = grouped_observation_layout(view, &[4, 5], &[0, 119]).unwrap();
    assert_eq!(layout.key_space_size, 16);
    let mut counters = vec![0_u64; layout.key_space_size];
    counters[0] = 2;
    counters[15] = 1;
    assert_eq!(
        decode_grouped_histogram(view, &layout, &counters).unwrap(),
        vec![
            GroupedViewValue {
                box_name: "world".to_owned(),
                name: "population_cells".to_owned(),
                keys: vec![0, 0, 0],
                count: 2,
            },
            GroupedViewValue {
                box_name: "world".to_owned(),
                name: "population_cells".to_owned(),
                keys: vec![1, 3, 1],
                count: 1,
            },
        ]
    );
}

#[test]
fn grouped_only_models_emit_legacy_enum_counts_without_state_download() {
    let generated = generate(&grouped_only_model()).unwrap();
    assert!(generated.observation_eligibility.eligible);
    assert!(generated.observation_view_tables.is_empty());
    assert_eq!(generated.generic_enum_observations.len(), 2);
    assert_eq!(generated.generic_enum_count, 4);
    let kernel = kernel_body(&generated.source, "sembla_observe_generic_enum");
    assert!(kernel.contains("atomicAdd(counts + 0ULL + value, 1ULL)"));
    assert!(kernel.contains("atomicAdd(counts + 2ULL + value, 1ULL)"));
}

#[test]
fn nested_output_aggregate_is_collected_before_ordered_output() {
    let generated = generate(&nested_output_model(true)).unwrap();
    assert_eq!(generated.aggregate_group_tables.len(), 1);
    assert!(generated.schedule_aggregate_indices.is_empty());
    assert!(generated.effect_aggregate_indices.is_empty());
    assert_eq!(generated.output_aggregate_indices, [0]);
    assert!(generated.source.contains("const unsigned char* aggs"));
    assert!(generated.source.contains("sembla_build_output_partials"));
}

#[test]
fn unwired_output_aggregates_are_not_collected() {
    let generated = generate(&nested_output_model(false)).unwrap();
    assert!(generated.aggregate_group_tables.is_empty());
    assert!(generated.schedule_aggregate_indices.is_empty());
    assert!(generated.effect_aggregate_indices.is_empty());
    assert!(generated.output_aggregate_indices.is_empty());
}

#[test]
fn contested_source_eagerly_checks_claims_and_uses_candidate_parallel_argmin() {
    let generated = generate(&contested_model()).unwrap();
    assert!(generated.source.contains("sembla_validate_claims"));
    assert!(generated
        .source
        .contains("sembla_record_validation_failure(status, 10ULL, candidate,"));
    assert!(generated
        .source
        .contains("self_candidate = candidate_begin + local_candidate"));
    assert!(generated.source.contains("sembla_prepare_effects"));
    assert!(generated.source.contains("owner_values[owner]"));
}

#[test]
fn stable_rule_words_key_philox_and_conflict_ordering_while_ordinals_index() {
    let model = stable_contested_model();
    assert!(model
        .transitions()
        .iter()
        .all(|transition| transition.rule_word != transition.rule_id));
    let generated = generate(&model).unwrap();

    for transition in model.transitions() {
        assert!(generated.source.contains(&format!(
            "sembla_exp(seed, tick, {}U,",
            transition.rule_word
        )));
        assert!(generated.source.contains(&format!(
            "instance_rules[instance] = {}U",
            transition.rule_word
        )));
        assert!(generated
            .source
            .contains(&format!("candidate_offsets[{}]", transition.rule_id)));
        assert!(generated
            .source
            .contains(&format!("sembla_transition_{:08x}", transition.rule_id)));
        assert!(!generated
            .source
            .contains(&format!("sembla_exp(seed, tick, {}U,", transition.rule_id)));
    }
}

#[test]
fn incompatible_claims_are_checked_serially_before_parallel_argmin() {
    let generated = generate(&incompatible_claim_model()).unwrap();
    let (before_resolve, resolver_and_after) = generated
        .source
        .split_once("extern \"C\" __global__ void sembla_resolve_conflicts")
        .unwrap();
    let (resolver, _) = resolver_and_after
        .split_once("extern \"C\" __global__ void sembla_prepare_effects")
        .unwrap();

    assert!(before_resolve.contains("const unsigned char* enabled"));
    assert!(before_resolve.contains("status[0] = 4ULL"));
    assert!(before_resolve.contains("if (!enabled[left_candidate]) continue"));
    assert!(before_resolve.contains("if (!enabled[right_candidate]) continue"));
    assert!(resolver.contains("const unsigned long long* status"));
    assert!(!resolver.contains("status[0] ="));
    assert!(!resolver.contains("status[1] ="));
    assert!(!resolver.contains("status[2] ="));
    assert!(!resolver.contains("atomicAdd"));
    // The validation diagnostic reduction and prefix argmin passes may
    // use atomicMin; candidate finalization itself must not.
    assert!(!resolver.contains("atomicMin"));
}

#[test]
fn segmented_argmin_has_no_cross_table_row_scan_and_prefixes_every_pass() {
    let generated = generate(&stable_contested_model()).unwrap();
    for kernel in [
        "sembla_build_claim_instances",
        "sembla_reduce_claim_keys",
        "sembla_reduce_claim_rules",
        "sembla_reduce_claim_entities",
        "sembla_reduce_claim_instances",
        "sembla_resolve_conflicts",
    ] {
        let body = kernel_body(&generated.source, kernel);
        assert!(
            !body.contains("other_row") && !body.contains("row_counts[other"),
            "{kernel} contains a cross-table all-row scan"
        );
    }

    let rules = kernel_body(&generated.source, "sembla_reduce_claim_rules");
    assert!(rules.contains("instance_keys[instance] == winner_keys[resource]"));
    let entities = kernel_body(&generated.source, "sembla_reduce_claim_entities");
    assert!(entities.contains("instance_keys[instance] == winner_keys[resource]"));
    assert!(entities.contains("instance_rules[instance] == winner_rules[resource]"));
    let instances = kernel_body(&generated.source, "sembla_reduce_claim_instances");
    assert!(instances.contains("instance_keys[instance] == winner_keys[resource]"));
    assert!(instances.contains("instance_rules[instance] == winner_rules[resource]"));
    assert!(instances.contains("instance_entities[instance] == winner_entities[resource]"));
}

#[test]
fn prefix_reduction_is_lexicographic_not_component_wise() {
    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    struct Instance {
        key: u64,
        rule: u32,
        entity: u32,
        stable: u64,
    }

    // Every component minimum comes from a different instance. An
    // incorrect component-wise reduction would synthesize (0, 0, 0, 0),
    // while compare_instances' lexicographic key selects the first row.
    let instances = [
        Instance {
            key: 0,
            rule: 90,
            entity: 90,
            stable: 90,
        },
        Instance {
            key: 1,
            rule: 0,
            entity: 80,
            stable: 80,
        },
        Instance {
            key: 2,
            rule: 70,
            entity: 0,
            stable: 70,
        },
        Instance {
            key: 3,
            rule: 60,
            entity: 60,
            stable: 0,
        },
    ];
    let cpu = *instances
        .iter()
        .min_by_key(|instance| {
            (
                instance.key,
                instance.rule,
                instance.entity,
                instance.stable,
            )
        })
        .unwrap();

    for order in [[0, 1, 2, 3], [3, 2, 1, 0], [1, 3, 0, 2]] {
        let key = order.iter().map(|&i| instances[i].key).min().unwrap();
        let rule = order
            .iter()
            .filter(|&&i| instances[i].key == key)
            .map(|&i| instances[i].rule)
            .min()
            .unwrap();
        let entity = order
            .iter()
            .filter(|&&i| instances[i].key == key && instances[i].rule == rule)
            .map(|&i| instances[i].entity)
            .min()
            .unwrap();
        let stable = order
            .iter()
            .filter(|&&i| {
                instances[i].key == key
                    && instances[i].rule == rule
                    && instances[i].entity == entity
            })
            .map(|&i| instances[i].stable)
            .min()
            .unwrap();
        assert_eq!(
            Instance {
                key,
                rule,
                entity,
                stable
            },
            cpu
        );
    }
    assert_eq!(cpu, instances[0]);
}

#[test]
fn cuda_f64_order_key_exactly_matches_rust_total_cmp() {
    let values = [
        f64::from_bits(0xfff8_0000_0000_0001), // negative quiet NaN
        f64::from_bits(0xfff0_0000_0000_0001), // negative signaling NaN
        f64::NEG_INFINITY,
        -1.0,
        f64::from_bits(0x8000_0000_0000_0001), // negative subnormal
        -0.0,
        0.0,
        f64::from_bits(0x0000_0000_0000_0001), // positive subnormal
        1.0,
        f64::INFINITY,
        f64::from_bits(0x7ff0_0000_0000_0001), // positive signaling NaN
        f64::from_bits(0x7ff8_0000_0000_0001), // positive quiet NaN
    ];
    for left in values {
        for right in values {
            assert_eq!(
                left.total_cmp(&right),
                cuda_f64_order_key(left).cmp(&cuda_f64_order_key(right)),
                "left={:#018x} right={:#018x}",
                left.to_bits(),
                right.to_bits()
            );
        }
    }
    assert!(cuda_f64_order_key(-0.0) < cuda_f64_order_key(0.0));
}

#[test]
fn frozen_demographic_model_has_fifty_million_claim_instances() {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../fixtures/demographic/benchmark/demographic_slots.no-grouped.json");
    let source = std::fs::read_to_string(path).unwrap();
    let model = sembla_ir::validate(sembla_ir::parse_json(&source).unwrap()).unwrap();
    let claims_per_slot: usize = model
        .model()
        .boxes
        .iter()
        .flat_map(|model_box| &model_box.transitions)
        .map(|transition| transition.contests.len())
        .sum();
    assert_eq!(claims_per_slot, 5);
    let instance_count = 10_000_000_usize.checked_mul(claims_per_slot).unwrap();
    assert_eq!(instance_count, 50_000_000);
    // Four SoA fields: resource/key u64 and rule/entity u32.
    assert_eq!(instance_count * (8 + 8 + 4 + 4), 1_200_000_000);
}

#[test]
fn minimum_integer_literal_remains_signed_and_generation_is_deterministic() {
    let first = generate(&minimum_integer_model()).unwrap();
    let second = generate(&minimum_integer_model()).unwrap();

    assert_eq!(first, second);
    assert!(first.source.contains("(-0x7fffffffffffffffLL - 1LL)"));
    let oversized_decimal = ["-9223372036854775808", "LL"].concat();
    assert!(!first.source.contains(&oversized_decimal));
}

#[test]
fn input_integer_ordering_promotes_both_operands_to_f64() {
    let generated = generate(&input_integer_ordering_model()).unwrap();
    assert!(generated.source.contains("(double)(9007199254740992LL)"));
    assert!(generated
        .source
        .contains("(double)((*((const long long*)(inputs"));
}

#[test]
fn shared_aggregate_is_staged_for_schedule_and_output() {
    let generated = generate(&shared_schedule_output_aggregate_model()).unwrap();
    assert_eq!(generated.aggregate_group_tables.len(), 1);
    assert_eq!(generated.state_aggregate_indices, [0]);
    assert_eq!(generated.schedule_aggregate_indices, [0]);
    assert_eq!(generated.schedule_aggregate_indices_by_rule, [vec![0]]);
    assert!(generated.effect_aggregate_indices.is_empty());
    assert_eq!(generated.output_aggregate_indices, [0]);
    assert!(generated
        .source
        .contains("aggregate_facts[aggregate_index] = code"));
    assert!(generated.source.contains("status[1] = 0ULL"));
}

#[test]
fn policy_source_contains_prospective_output_and_parallel_result_stages() {
    let generated = generate(&example_model("sir_policy.json")).unwrap();
    assert!(generated.source.contains("sembla_build_output_partials"));
    assert!(generated.source.contains("sembla_finish_outputs"));
    assert!(generated
        .source
        .contains("self_candidate = candidate_begin + local_candidate"));
    assert!(generated
        .source
        .contains("owner = (unsigned long long)blockIdx.x"));
    assert!(!generated.source.contains("long long result = a * b"));
}

#[test]
fn sir_simulation_source_matches_unchanged_checked_in_golden() {
    fn remove_between(source: &mut String, begin: &str, end: &str) {
        let begin = source.find(begin).unwrap();
        let end = source[begin..]
            .find(end)
            .map(|offset| begin + offset)
            .unwrap();
        source.replace_range(begin..end, "");
    }

    let generated = generate(&sir_model()).unwrap();
    let mut simulation = generated.source;
    let mut golden = include_str!("../tests/fixtures/sir.generated.cu").to_owned();

    // PRD 0008 replaces these regions with focused lock-free protocol
    // assertions. Excluding them from both sides keeps the pre-existing
    // broad source golden byte-identical rather than blessing unrelated
    // generated-source churn while updating a correctness protocol.
    for source in [&mut simulation, &mut golden] {
        remove_between(
            source,
            "// Records one validation failure into scratch slots",
            "__device__ __forceinline__ long long sembla_add_i64",
        );
        remove_between(
            source,
            "\nextern \"C\" __global__ void sembla_init_validation_scratch",
            "\nextern \"C\" __global__ void sembla_mark_effect_active",
        );
        remove_between(
            source,
            "\nextern \"C\" __global__ void sembla_prepare_effects",
            "\nextern \"C\" __global__ void sembla_apply_effects",
        );
    }

    let helpers_begin = simulation
        .find("__device__ __forceinline__ void sembla_atomic_min_i64")
        .unwrap();
    let helpers_end = simulation[helpers_begin..]
        .find("__device__ __forceinline__ unsigned long long sembla_f64_order_key")
        .map(|offset| helpers_begin + offset)
        .unwrap();
    simulation.replace_range(helpers_begin..helpers_end, "");
    let observation_begin = simulation
        .find("\nextern \"C\" __global__ void sembla_init_observations")
        .unwrap();
    let observation_end = simulation[observation_begin..]
        .find("\nextern \"C\" __global__ void sembla_philox_vectors")
        .map(|offset| observation_begin + offset)
        .unwrap();
    simulation.replace_range(observation_begin..observation_end, "");
    assert_eq!(simulation, golden);
}

#[test]
fn dump_is_content_addressed_and_repeatable() {
    let generated = generate(&sir_model()).unwrap();
    let directory = std::env::temp_dir().join(format!(
        "sembla-cuda-dump-{}-{}",
        std::process::id(),
        generated.source_sha256
    ));
    let _ = std::fs::remove_dir_all(&directory);
    std::env::set_var(DUMP_ENV, &directory);
    let first = generated.dump_if_requested().unwrap().unwrap();
    let second = generated.dump_if_requested().unwrap().unwrap();
    std::env::remove_var(DUMP_ENV);
    assert_eq!(first, second);
    assert_eq!(std::fs::read_to_string(first).unwrap(), generated.source);
    std::fs::remove_dir_all(directory).unwrap();
}
