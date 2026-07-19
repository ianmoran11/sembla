#![cfg(feature = "cuda")]

use sembla_cuda::{generate, CudaBackend, HashMode};
use sembla_runtime::eval::ParamEnv;
use sembla_runtime::executor::run_tick;
use sembla_runtime::state::{ColumnData, ColumnInit, StateStore, TableInit};

fn claim_overflow_model() -> sembla_ir::ValidatedModel {
    let source = r#"{"name":"claims","dt":1.0,"params":[],"boxes":[{"name":"world","tables":[{"name":"Worker","size_hint":1,"attrs":[]},{"name":"Applicant","size_hint":2,"attrs":[{"name":"worker","ty":{"kind":"ref","table":"Worker"}},{"name":"priority","ty":{"kind":"int"}},{"name":"state","ty":{"kind":"enum","variants":["Waiting","Done"]}}]}],"transitions":[{"name":"finish","table":"Applicant","guard":{"kind":"enum_is","attr":"state","variant":"Waiting"},"hazard":{"kind":"real","value":1.0},"effects":[{"kind":"set_attr","attr":"state","value":{"kind":"enum","variant":"Done"}}],"contests":[{"resource":{"kind":"self_attr","name":"worker"},"ordering":{"kind":"key","expr":{"kind":"mul","lhs":{"kind":"self_attr","name":"priority"},"rhs":{"kind":"int","value":2}}}}]}],"inputs":[],"outputs":[],"views":[]}],"wires":[],"summaries":[]}"#;
    sembla_ir::validate(sembla_ir::parse_json(source).unwrap()).unwrap()
}

fn claim_overflow_state() -> Vec<TableInit> {
    vec![
        TableInit::new("world", "Worker", 1, Vec::new()),
        TableInit::new(
            "world",
            "Applicant",
            2,
            vec![
                ColumnInit::new("worker", ColumnData::Ref(vec![0, 0])),
                ColumnInit::new("priority", ColumnData::Int(vec![i64::MAX, 1])),
                ColumnInit::new("state", ColumnData::Enum(vec![0, 0])),
            ],
        ),
    ]
}

fn incompatible_claim_model(key_enabled: bool) -> sembla_ir::ValidatedModel {
    let enabled = if key_enabled { "true" } else { "false" };
    let source = r#"{"name":"incompatible_claims","dt":1.0,"params":[],"boxes":[{"name":"world","tables":[{"name":"Worker","size_hint":1,"attrs":[]},{"name":"Applicant","size_hint":1,"attrs":[{"name":"worker","ty":{"kind":"ref","table":"Worker"}},{"name":"priority","ty":{"kind":"int"}}]}],"transitions":[{"name":"race","table":"Applicant","guard":{"kind":"bool","value":true},"hazard":{"kind":"real","value":1e300},"effects":[],"contests":[{"resource":{"kind":"self_attr","name":"worker"},"ordering":{"kind":"race_time"}}]},{"name":"priority","table":"Applicant","guard":{"kind":"bool","value":KEY_ENABLED},"hazard":{"kind":"real","value":1e300},"effects":[],"contests":[{"resource":{"kind":"self_attr","name":"worker"},"ordering":{"kind":"key","expr":{"kind":"self_attr","name":"priority"}}}]}],"inputs":[],"outputs":[],"views":[]}],"wires":[],"summaries":[]}"#
        .replace("KEY_ENABLED", enabled);
    sembla_ir::validate(sembla_ir::parse_json(&source).unwrap()).unwrap()
}

fn incompatible_claim_state() -> Vec<TableInit> {
    vec![
        TableInit::new("world", "Worker", 1, Vec::new()),
        TableInit::new(
            "world",
            "Applicant",
            1,
            vec![
                ColumnInit::new("worker", ColumnData::Ref(vec![0])),
                ColumnInit::new("priority", ColumnData::Int(vec![0])),
            ],
        ),
    ]
}

fn minimum_integer_model() -> sembla_ir::ValidatedModel {
    let source = r#"{"name":"minimum_integer","dt":1.0,"params":[],"boxes":[{"name":"world","tables":[{"name":"Person","size_hint":1,"attrs":[{"name":"x","ty":{"kind":"int"}}]}],"transitions":[{"name":"set_minimum","table":"Person","guard":{"kind":"lt","lhs":{"kind":"int","value":-9223372036854775808},"rhs":{"kind":"self_attr","name":"x"}},"hazard":{"kind":"real","value":1e300},"effects":[{"kind":"set_attr","attr":"x","value":{"kind":"int","value":-9223372036854775808}}],"contests":[]}],"inputs":[],"outputs":[],"views":[]}],"wires":[],"summaries":[]}"#;
    sembla_ir::validate(sembla_ir::parse_json(source).unwrap()).unwrap()
}

fn minimum_integer_state() -> Vec<TableInit> {
    vec![TableInit::new(
        "world",
        "Person",
        1,
        vec![ColumnInit::new("x", ColumnData::Int(vec![0]))],
    )]
}

fn prospective_output_overflow_model() -> sembla_ir::ValidatedModel {
    let source = r#"{"name":"prospective_output_overflow","dt":1.0,"params":[],"boxes":[{"name":"source","tables":[{"name":"Group","size_hint":1,"attrs":[]},{"name":"Person","size_hint":2,"attrs":[{"name":"group","ty":{"kind":"ref","table":"Group"}},{"name":"x","ty":{"kind":"int"}}]}],"transitions":[{"name":"clear","table":"Person","guard":{"kind":"bool","value":true},"hazard":{"kind":"real","value":1e300},"effects":[{"kind":"set_attr","attr":"x","value":{"kind":"int","value":0}}],"contests":[]}],"inputs":[],"outputs":[{"name":"totals","schema":[{"name":"total","ty":{"kind":"int"}}],"builder":{"kind":"per_table","table":"Person","fields":[{"name":"total","op":{"kind":"sum","value":{"kind":"agg","op":{"kind":"sum","value":{"kind":"self_attr","name":"x"}},"table":"Person","on":{"fk_attr":"group","self_fk_attr":"group"},"filter":{"kind":"bool","value":true}}},"filter":null}]}}],"views":[]},{"name":"sink","tables":[],"transitions":[],"inputs":[{"name":"totals","schema":[{"name":"total","ty":{"kind":"int"}}]}],"outputs":[],"views":[]}],"wires":[{"from":{"box":"source","port":"totals"},"to":{"box":"sink","port":"totals"}}],"summaries":[]}"#;
    sembla_ir::validate(sembla_ir::parse_json(source).unwrap()).unwrap()
}

fn prospective_output_overflow_state() -> Vec<TableInit> {
    vec![
        TableInit::new("source", "Group", 1, Vec::new()),
        TableInit::new(
            "source",
            "Person",
            2,
            vec![
                ColumnInit::new("group", ColumnData::Ref(vec![0, 0])),
                ColumnInit::new("x", ColumnData::Int(vec![i64::MAX, 1])),
            ],
        ),
    ]
}

fn transition_only_aggregate_model() -> sembla_ir::ValidatedModel {
    let source = r#"{"name":"transition_only_aggregate","dt":1.0,"params":[],"boxes":[{"name":"world","tables":[{"name":"Group","size_hint":1,"attrs":[]},{"name":"Person","size_hint":2,"attrs":[{"name":"group","ty":{"kind":"ref","table":"Group"}},{"name":"x","ty":{"kind":"int"}}]}],"transitions":[{"name":"fill","table":"Person","guard":{"kind":"ge","lhs":{"kind":"agg","op":{"kind":"sum","value":{"kind":"self_attr","name":"x"}},"table":"Person","on":{"fk_attr":"group","self_fk_attr":"group"},"filter":{"kind":"bool","value":true}},"rhs":{"kind":"int","value":0}},"hazard":{"kind":"real","value":1e300},"effects":[{"kind":"set_attr","attr":"x","value":{"kind":"int","value":9223372036854775807}}],"contests":[]}],"inputs":[],"outputs":[],"views":[]}],"wires":[],"summaries":[]}"#;
    sembla_ir::validate(sembla_ir::parse_json(source).unwrap()).unwrap()
}

fn transition_only_aggregate_state() -> Vec<TableInit> {
    vec![
        TableInit::new("world", "Group", 1, Vec::new()),
        TableInit::new(
            "world",
            "Person",
            2,
            vec![
                ColumnInit::new("group", ColumnData::Ref(vec![0, 0])),
                ColumnInit::new("x", ColumnData::Int(vec![0, 0])),
            ],
        ),
    ]
}

fn effect_aggregate_model(enabled: bool) -> sembla_ir::ValidatedModel {
    let enabled = if enabled { "true" } else { "false" };
    let source = r#"{"name":"effect_aggregate","dt":1.0,"params":[],"boxes":[{"name":"world","tables":[{"name":"Group","size_hint":1,"attrs":[]},{"name":"Person","size_hint":2,"attrs":[{"name":"group","ty":{"kind":"ref","table":"Group"}},{"name":"x","ty":{"kind":"int"}},{"name":"y","ty":{"kind":"int"}}]}],"transitions":[{"name":"maybe","table":"Person","guard":{"kind":"bool","value":EFFECT_ENABLED},"hazard":{"kind":"real","value":1e300},"effects":[{"kind":"set_attr","attr":"y","value":{"kind":"agg","op":{"kind":"sum","value":{"kind":"self_attr","name":"x"}},"table":"Person","on":{"fk_attr":"group","self_fk_attr":"group"},"filter":{"kind":"bool","value":true}}}],"contests":[]}],"inputs":[],"outputs":[],"views":[]}],"wires":[],"summaries":[]}"#
        .replace("EFFECT_ENABLED", enabled);
    sembla_ir::validate(sembla_ir::parse_json(&source).unwrap()).unwrap()
}

fn effect_aggregate_state(x: [i64; 2]) -> Vec<TableInit> {
    vec![
        TableInit::new("world", "Group", 1, Vec::new()),
        TableInit::new(
            "world",
            "Person",
            2,
            vec![
                ColumnInit::new("group", ColumnData::Ref(vec![0, 0])),
                ColumnInit::new("x", ColumnData::Int(x.to_vec())),
                ColumnInit::new("y", ColumnData::Int(vec![0, 0])),
            ],
        ),
    ]
}

fn input_integer_ordering_model() -> sembla_ir::ValidatedModel {
    let source = r#"{"name":"input_integer_ordering","dt":1.0,"params":[],"boxes":[{"name":"source","tables":[{"name":"Event","size_hint":1,"attrs":[{"name":"amount","ty":{"kind":"int"}}]}],"transitions":[],"inputs":[],"outputs":[{"name":"events","schema":[{"name":"amount","ty":{"kind":"int"}}],"builder":{"kind":"per_table","table":"Event","fields":[{"name":"amount","op":{"kind":"sum","value":{"kind":"self_attr","name":"amount"}},"filter":null}]}}],"views":[]},{"name":"sink","tables":[{"name":"Agent","size_hint":1,"attrs":[{"name":"state","ty":{"kind":"enum","variants":["Off","On"]}}]}],"transitions":[{"name":"activate","table":"Agent","guard":{"kind":"gt","lhs":{"kind":"input","port":"events","agg":{"op":{"kind":"count"},"filter":{"kind":"gt","lhs":{"kind":"self_attr","name":"amount"},"rhs":{"kind":"int","value":9007199254740992}}}},"rhs":{"kind":"int","value":0}},"hazard":{"kind":"real","value":1e300},"effects":[{"kind":"set_attr","attr":"state","value":{"kind":"enum","variant":"On"}}],"contests":[]}],"inputs":[{"name":"events","schema":[{"name":"amount","ty":{"kind":"int"}}]}],"outputs":[],"views":[]}],"wires":[{"from":{"box":"source","port":"events"},"to":{"box":"sink","port":"events"}}],"summaries":[]}"#;
    sembla_ir::validate(sembla_ir::parse_json(source).unwrap()).unwrap()
}

fn input_integer_ordering_state() -> Vec<TableInit> {
    vec![
        TableInit::new(
            "source",
            "Event",
            1,
            vec![ColumnInit::new(
                "amount",
                ColumnData::Int(vec![9_007_199_254_740_993]),
            )],
        ),
        TableInit::new(
            "sink",
            "Agent",
            1,
            vec![ColumnInit::new("state", ColumnData::Enum(vec![0]))],
        ),
    ]
}

fn sequential_group_sum_model() -> sembla_ir::ValidatedModel {
    let source = r#"{"name":"sequential_group_sum","dt":1.0,"params":[],"boxes":[{"name":"world","tables":[{"name":"Group","size_hint":1,"attrs":[]},{"name":"Person","size_hint":4,"attrs":[{"name":"group","ty":{"kind":"ref","table":"Group"}},{"name":"x","ty":{"kind":"real"}},{"name":"state","ty":{"kind":"enum","variants":["Off","On"]}}]}],"transitions":[{"name":"activate","table":"Person","guard":{"kind":"gt","lhs":{"kind":"agg","op":{"kind":"sum","value":{"kind":"self_attr","name":"x"}},"table":"Person","on":{"fk_attr":"group","self_fk_attr":"group"},"filter":{"kind":"bool","value":true}},"rhs":{"kind":"real","value":0.0}},"hazard":{"kind":"real","value":1e300},"effects":[{"kind":"set_attr","attr":"state","value":{"kind":"enum","variant":"On"}}],"contests":[]}],"inputs":[],"outputs":[],"views":[]}],"wires":[],"summaries":[]}"#;
    sembla_ir::validate(sembla_ir::parse_json(source).unwrap()).unwrap()
}

fn sequential_group_sum_state() -> Vec<TableInit> {
    vec![
        TableInit::new("world", "Group", 1, Vec::new()),
        TableInit::new(
            "world",
            "Person",
            4,
            vec![
                ColumnInit::new("group", ColumnData::Ref(vec![0; 4])),
                ColumnInit::new("x", ColumnData::Real(vec![1.0e16, 1.0, -1.0e16, 1.0])),
                ColumnInit::new("state", ColumnData::Enum(vec![0; 4])),
            ],
        ),
    ]
}

fn sequential_output_sum_model() -> sembla_ir::ValidatedModel {
    let source = r#"{"name":"sequential_output_sum","dt":1.0,"params":[],"boxes":[{"name":"source","tables":[{"name":"Event","size_hint":4,"attrs":[{"name":"x","ty":{"kind":"real"}}]}],"transitions":[],"inputs":[],"outputs":[{"name":"total","schema":[{"name":"x","ty":{"kind":"real"}}],"builder":{"kind":"per_table","table":"Event","fields":[{"name":"x","op":{"kind":"sum","value":{"kind":"self_attr","name":"x"}},"filter":null}]}}],"views":[]},{"name":"sink","tables":[],"transitions":[],"inputs":[{"name":"total","schema":[{"name":"x","ty":{"kind":"real"}}]}],"outputs":[],"views":[]}],"wires":[{"from":{"box":"source","port":"total"},"to":{"box":"sink","port":"total"}}],"summaries":[]}"#;
    sembla_ir::validate(sembla_ir::parse_json(source).unwrap()).unwrap()
}

fn sequential_output_sum_state() -> Vec<TableInit> {
    vec![TableInit::new(
        "source",
        "Event",
        4,
        vec![ColumnInit::new(
            "x",
            ColumnData::Real(vec![1.0e16, 1.0, -1.0e16, 1.0]),
        )],
    )]
}

fn sequential_int_overflow_model() -> sembla_ir::ValidatedModel {
    let source = r#"{"name":"sequential_int_overflow","dt":1.0,"params":[],"boxes":[{"name":"world","tables":[{"name":"Group","size_hint":1,"attrs":[]},{"name":"Person","size_hint":3,"attrs":[{"name":"group","ty":{"kind":"ref","table":"Group"}},{"name":"x","ty":{"kind":"int"}}]}],"transitions":[{"name":"observe","table":"Person","guard":{"kind":"ge","lhs":{"kind":"agg","op":{"kind":"sum","value":{"kind":"self_attr","name":"x"}},"table":"Person","on":{"fk_attr":"group","self_fk_attr":"group"},"filter":{"kind":"bool","value":true}},"rhs":{"kind":"int","value":0}},"hazard":{"kind":"real","value":1.0},"effects":[],"contests":[]}],"inputs":[],"outputs":[],"views":[]}],"wires":[],"summaries":[]}"#;
    sembla_ir::validate(sembla_ir::parse_json(source).unwrap()).unwrap()
}

fn sequential_int_overflow_state() -> Vec<TableInit> {
    vec![
        TableInit::new("world", "Group", 1, Vec::new()),
        TableInit::new(
            "world",
            "Person",
            3,
            vec![
                ColumnInit::new("group", ColumnData::Ref(vec![0; 3])),
                ColumnInit::new("x", ColumnData::Int(vec![i64::MAX, 1, -1])),
            ],
        ),
    ]
}

fn claim_before_later_guard_error_model() -> sembla_ir::ValidatedModel {
    let source = r#"{"name":"claim_before_later_guard_error","dt":1.0,"params":[],"boxes":[{"name":"world","tables":[{"name":"Worker","size_hint":1,"attrs":[]},{"name":"Applicant","size_hint":1,"attrs":[{"name":"worker","ty":{"kind":"ref","table":"Worker"}},{"name":"priority","ty":{"kind":"int"}},{"name":"x","ty":{"kind":"int"}}]}],"transitions":[{"name":"claim_first","table":"Applicant","guard":{"kind":"bool","value":true},"hazard":{"kind":"real","value":1e300},"effects":[],"contests":[{"resource":{"kind":"self_attr","name":"worker"},"ordering":{"kind":"key","expr":{"kind":"mul","lhs":{"kind":"self_attr","name":"priority"},"rhs":{"kind":"int","value":2}}}}]},{"name":"guard_second","table":"Applicant","guard":{"kind":"gt","lhs":{"kind":"mul","lhs":{"kind":"self_attr","name":"x"},"rhs":{"kind":"int","value":2}},"rhs":{"kind":"int","value":0}},"hazard":{"kind":"real","value":1e300},"effects":[],"contests":[]}],"inputs":[],"outputs":[],"views":[]}],"wires":[],"summaries":[]}"#;
    sembla_ir::validate(sembla_ir::parse_json(source).unwrap()).unwrap()
}

fn claim_before_later_guard_error_state() -> Vec<TableInit> {
    vec![
        TableInit::new("world", "Worker", 1, Vec::new()),
        TableInit::new(
            "world",
            "Applicant",
            1,
            vec![
                ColumnInit::new("worker", ColumnData::Ref(vec![0])),
                ColumnInit::new("priority", ColumnData::Int(vec![i64::MAX])),
                ColumnInit::new("x", ColumnData::Int(vec![i64::MAX])),
            ],
        ),
    ]
}

fn scalar_before_later_aggregate_error_model() -> sembla_ir::ValidatedModel {
    let source = r#"{"name":"scalar_before_later_aggregate_error","dt":1.0,"params":[],"boxes":[{"name":"world","tables":[{"name":"Group","size_hint":1,"attrs":[]},{"name":"Person","size_hint":2,"attrs":[{"name":"group","ty":{"kind":"ref","table":"Group"}},{"name":"x","ty":{"kind":"int"}}]}],"transitions":[{"name":"scalar_first","table":"Person","guard":{"kind":"gt","lhs":{"kind":"mul","lhs":{"kind":"self_attr","name":"x"},"rhs":{"kind":"int","value":2}},"rhs":{"kind":"int","value":0}},"hazard":{"kind":"real","value":1e300},"effects":[],"contests":[]},{"name":"aggregate_second","table":"Person","guard":{"kind":"ge","lhs":{"kind":"agg","op":{"kind":"sum","value":{"kind":"self_attr","name":"x"}},"table":"Person","on":{"fk_attr":"group","self_fk_attr":"group"},"filter":{"kind":"bool","value":true}},"rhs":{"kind":"int","value":0}},"hazard":{"kind":"real","value":1e300},"effects":[],"contests":[]}],"inputs":[],"outputs":[],"views":[]}],"wires":[],"summaries":[]}"#;
    sembla_ir::validate(sembla_ir::parse_json(source).unwrap()).unwrap()
}

fn scalar_before_later_aggregate_error_state() -> Vec<TableInit> {
    vec![
        TableInit::new("world", "Group", 1, Vec::new()),
        TableInit::new(
            "world",
            "Person",
            2,
            vec![
                ColumnInit::new("group", ColumnData::Ref(vec![0; 2])),
                ColumnInit::new("x", ColumnData::Int(vec![i64::MAX, 1])),
            ],
        ),
    ]
}

fn losing_row_effect_overflow_model() -> sembla_ir::ValidatedModel {
    let source = r#"{"name":"effects","dt":1.0,"params":[],"boxes":[{"name":"world","tables":[{"name":"Person","size_hint":2,"attrs":[{"name":"state","ty":{"kind":"enum","variants":["Off","On"]}},{"name":"x","ty":{"kind":"int"}}]}],"transitions":[{"name":"double","table":"Person","guard":{"kind":"enum_is","attr":"state","variant":"On"},"hazard":{"kind":"real","value":1e300},"effects":[{"kind":"set_attr","attr":"x","value":{"kind":"mul","lhs":{"kind":"self_attr","name":"x"},"rhs":{"kind":"int","value":2}}}],"contests":[]}],"inputs":[],"outputs":[],"views":[]}],"wires":[],"summaries":[]}"#;
    sembla_ir::validate(sembla_ir::parse_json(source).unwrap()).unwrap()
}

fn losing_row_effect_state() -> Vec<TableInit> {
    vec![TableInit::new(
        "world",
        "Person",
        2,
        vec![
            ColumnInit::new("state", ColumnData::Enum(vec![0, 1])),
            ColumnInit::new("x", ColumnData::Int(vec![i64::MAX, 1])),
        ],
    )]
}

fn filtered_aggregate_overflow_model() -> sembla_ir::ValidatedModel {
    let source = r#"{"name":"aggregate_errors","dt":1.0,"params":[],"boxes":[{"name":"world","tables":[{"name":"Group","size_hint":1,"attrs":[]},{"name":"Person","size_hint":2,"attrs":[{"name":"group","ty":{"kind":"ref","table":"Group"}},{"name":"include","ty":{"kind":"enum","variants":["No","Yes"]}}]}],"transitions":[{"name":"never","table":"Person","guard":{"kind":"bool","value":false},"hazard":{"kind":"div","lhs":{"kind":"agg","op":{"kind":"sum","value":{"kind":"mul","lhs":{"kind":"int","value":9223372036854775807},"rhs":{"kind":"int","value":2}}},"table":"Person","on":{"fk_attr":"group","self_fk_attr":"group"},"filter":{"kind":"enum_is","attr":"include","variant":"Yes"}},"rhs":{"kind":"real","value":1.0}},"effects":[],"contests":[]}],"inputs":[],"outputs":[],"views":[]}],"wires":[],"summaries":[]}"#;
    sembla_ir::validate(sembla_ir::parse_json(source).unwrap()).unwrap()
}

fn prospective_nested_output_model() -> sembla_ir::ValidatedModel {
    let source = r#"{"name":"nested_output","dt":1.0,"params":[],"boxes":[{"name":"source","tables":[{"name":"Group","size_hint":1,"attrs":[]},{"name":"Person","size_hint":2,"attrs":[{"name":"group","ty":{"kind":"ref","table":"Group"}},{"name":"x","ty":{"kind":"real"}},{"name":"state","ty":{"kind":"enum","variants":["Off","On"]}}]}],"transitions":[{"name":"update","table":"Person","guard":{"kind":"enum_is","attr":"state","variant":"On"},"hazard":{"kind":"real","value":1e300},"effects":[{"kind":"set_attr","attr":"x","value":{"kind":"real","value":2.0}}],"contests":[]}],"inputs":[],"outputs":[{"name":"totals","schema":[{"name":"total","ty":{"kind":"real"}}],"builder":{"kind":"per_table","table":"Person","fields":[{"name":"total","op":{"kind":"sum","value":{"kind":"agg","op":{"kind":"sum","value":{"kind":"self_attr","name":"x"}},"table":"Person","on":{"fk_attr":"group","self_fk_attr":"group"},"filter":{"kind":"bool","value":true}}},"filter":null}]}}],"views":[]},{"name":"sink","tables":[],"transitions":[],"inputs":[{"name":"totals","schema":[{"name":"total","ty":{"kind":"real"}}]}],"outputs":[],"views":[]}],"wires":[{"from":{"box":"source","port":"totals"},"to":{"box":"sink","port":"totals"}}],"summaries":[]}"#;
    sembla_ir::validate(sembla_ir::parse_json(source).unwrap()).unwrap()
}

fn prospective_nested_output_state() -> Vec<TableInit> {
    vec![
        TableInit::new("source", "Group", 1, Vec::new()),
        TableInit::new(
            "source",
            "Person",
            2,
            vec![
                ColumnInit::new("group", ColumnData::Ref(vec![0, 0])),
                ColumnInit::new("x", ColumnData::Real(vec![1.0, 1.0])),
                ColumnInit::new("state", ColumnData::Enum(vec![1, 1])),
            ],
        ),
    ]
}

fn filtered_aggregate_state() -> Vec<TableInit> {
    vec![
        TableInit::new("world", "Group", 1, Vec::new()),
        TableInit::new(
            "world",
            "Person",
            2,
            vec![
                ColumnInit::new("group", ColumnData::Ref(vec![0, 0])),
                ColumnInit::new("include", ColumnData::Enum(vec![0, 1])),
            ],
        ),
    ]
}

#[test]
fn semantic_gpu_fixtures_validate_without_a_device() {
    let _ = claim_overflow_model();
    let _ = losing_row_effect_overflow_model();
    let _ = filtered_aggregate_overflow_model();

    let incompatible = incompatible_claim_model(true);
    let params = ParamEnv::defaults(&incompatible);
    let mut state = StateStore::new(&incompatible, incompatible_claim_state()).unwrap();
    assert!(run_tick(&incompatible, &mut state, &params, 13, 0)
        .unwrap_err()
        .to_string()
        .contains("incompatible claim ordering"));

    let inactive = incompatible_claim_model(false);
    let params = ParamEnv::defaults(&inactive);
    let mut state = StateStore::new(&inactive, incompatible_claim_state()).unwrap();
    run_tick(&inactive, &mut state, &params, 13, 0).unwrap();

    let minimum = minimum_integer_model();
    let params = ParamEnv::defaults(&minimum);
    let mut state = StateStore::new(&minimum, minimum_integer_state()).unwrap();
    run_tick(&minimum, &mut state, &params, 17, 0).unwrap();
    assert_eq!(
        state.snapshot().int("world", "Person", "x", 0).unwrap(),
        i64::MIN
    );

    let prospective = prospective_output_overflow_model();
    let generated = generate(&prospective).unwrap();
    assert!(generated.schedule_aggregate_indices.is_empty());
    assert!(generated.effect_aggregate_indices.is_empty());
    assert_eq!(generated.output_aggregate_indices, [0]);
    let params = ParamEnv::defaults(&prospective);
    let mut state = StateStore::new(&prospective, prospective_output_overflow_state()).unwrap();
    run_tick(&prospective, &mut state, &params, 19, 0).unwrap();
    assert_eq!(state.snapshot().int("source", "Person", "x", 0).unwrap(), 0);
    assert_eq!(
        state
            .snapshot()
            .input_table("sink", "totals")
            .unwrap()
            .columns,
        vec![ColumnData::Int(vec![0])]
    );

    let transition_only = transition_only_aggregate_model();
    let generated = generate(&transition_only).unwrap();
    assert_eq!(generated.schedule_aggregate_indices, [0]);
    assert!(generated.effect_aggregate_indices.is_empty());
    assert!(generated.output_aggregate_indices.is_empty());
    let params = ParamEnv::defaults(&transition_only);
    let mut state = StateStore::new(&transition_only, transition_only_aggregate_state()).unwrap();
    run_tick(&transition_only, &mut state, &params, 21, 0).unwrap();
    assert_eq!(
        state.snapshot().int("world", "Person", "x", 0).unwrap(),
        i64::MAX
    );

    let inactive_effect = effect_aggregate_model(false);
    let generated = generate(&inactive_effect).unwrap();
    assert!(generated.schedule_aggregate_indices.is_empty());
    assert_eq!(generated.effect_aggregate_indices, [0]);
    assert!(generated.output_aggregate_indices.is_empty());
    assert!(generated.source.contains("sembla_mark_effect_aggregates"));
    assert!(generated.source.contains("active[0] = 1U"));
    let params = ParamEnv::defaults(&inactive_effect);
    let mut state =
        StateStore::new(&inactive_effect, effect_aggregate_state([i64::MAX, 1])).unwrap();
    run_tick(&inactive_effect, &mut state, &params, 23, 0).unwrap();
    assert_eq!(state.snapshot().int("world", "Person", "y", 0).unwrap(), 0);

    let active_effect = effect_aggregate_model(true);
    let params = ParamEnv::defaults(&active_effect);
    let mut state = StateStore::new(&active_effect, effect_aggregate_state([2, 3])).unwrap();
    run_tick(&active_effect, &mut state, &params, 25, 0).unwrap();
    assert_eq!(state.snapshot().int("world", "Person", "y", 0).unwrap(), 5);

    let input_ordering = input_integer_ordering_model();
    let params = ParamEnv::defaults(&input_ordering);
    let mut state = StateStore::new(&input_ordering, input_integer_ordering_state()).unwrap();
    run_tick(&input_ordering, &mut state, &params, 29, 0).unwrap();
    run_tick(&input_ordering, &mut state, &params, 29, 1).unwrap();
    assert_eq!(
        state
            .snapshot()
            .enum_index("sink", "Agent", "state", 0)
            .unwrap(),
        0
    );

    let sequential_group = sequential_group_sum_model();
    let generated = generate(&sequential_group).unwrap();
    assert!(!generated.source.contains("midpoint"));
    let params = ParamEnv::defaults(&sequential_group);
    let mut state = StateStore::new(&sequential_group, sequential_group_sum_state()).unwrap();
    run_tick(&sequential_group, &mut state, &params, 31, 0).unwrap();
    assert_eq!(
        state
            .snapshot()
            .enum_index("world", "Person", "state", 0)
            .unwrap(),
        1
    );

    let sequential_output = sequential_output_sum_model();
    let generated = generate(&sequential_output).unwrap();
    assert!(!generated.source.contains("midpoint"));
    let params = ParamEnv::defaults(&sequential_output);
    let mut state = StateStore::new(&sequential_output, sequential_output_sum_state()).unwrap();
    run_tick(&sequential_output, &mut state, &params, 33, 0).unwrap();
    assert_eq!(
        state
            .snapshot()
            .input_table("sink", "total")
            .unwrap()
            .columns,
        vec![ColumnData::Real(vec![1.0])]
    );

    let sequential_int = sequential_int_overflow_model();
    let params = ParamEnv::defaults(&sequential_int);
    let mut state = StateStore::new(&sequential_int, sequential_int_overflow_state()).unwrap();
    assert!(run_tick(&sequential_int, &mut state, &params, 35, 0)
        .unwrap_err()
        .to_string()
        .contains("overflow"));

    let ordered_errors = claim_before_later_guard_error_model();
    let params = ParamEnv::defaults(&ordered_errors);
    let mut state =
        StateStore::new(&ordered_errors, claim_before_later_guard_error_state()).unwrap();
    assert!(run_tick(&ordered_errors, &mut state, &params, 37, 0)
        .unwrap_err()
        .to_string()
        .contains("overflow"));

    let aggregate_order = scalar_before_later_aggregate_error_model();
    let generated = generate(&aggregate_order).unwrap();
    assert_eq!(
        generated.schedule_aggregate_indices_by_rule,
        [vec![], vec![0]]
    );
    let params = ParamEnv::defaults(&aggregate_order);
    let mut state = StateStore::new(
        &aggregate_order,
        scalar_before_later_aggregate_error_state(),
    )
    .unwrap();
    assert!(run_tick(&aggregate_order, &mut state, &params, 39, 0)
        .unwrap_err()
        .to_string()
        .contains("overflow"));

    let model = prospective_nested_output_model();
    let params = ParamEnv::defaults(&model);
    let mut state = StateStore::new(&model, prospective_nested_output_state()).unwrap();
    run_tick(&model, &mut state, &params, 11, 0).unwrap();
    let snapshot = state.snapshot();
    let input = snapshot.input_table("sink", "totals").unwrap();
    assert_eq!(input.columns, vec![ColumnData::Real(vec![8.0])]);
}

#[test]
#[ignore = "requires a CUDA GPU; run crates/sembla-cuda/scripts/run-gpu-tests.sh"]
fn incompatible_claim_error_is_deterministic_and_ignores_disabled_candidates() {
    let model = incompatible_claim_model(true);
    let initial = incompatible_claim_state();
    let params = ParamEnv::defaults(&model);

    let mut cpu = StateStore::new(&model, initial.clone()).unwrap();
    assert!(run_tick(&model, &mut cpu, &params, 13, 0)
        .unwrap_err()
        .to_string()
        .contains("incompatible claim ordering"));

    let mut gpu = CudaBackend::new(&model, initial, &params, 13, HashMode::FinalOnly)
        .expect("CUDA device, driver, and NVRTC are required");
    let expected = gpu.run(1).unwrap_err().to_string();
    assert!(expected.contains("incompatible claim ordering"));
    for _ in 0..4 {
        assert_eq!(gpu.run(1).unwrap_err().to_string(), expected);
    }

    let model = incompatible_claim_model(false);
    let initial = incompatible_claim_state();
    let params = ParamEnv::defaults(&model);
    let mut cpu = StateStore::new(&model, initial.clone()).unwrap();
    run_tick(&model, &mut cpu, &params, 13, 0).unwrap();
    let expected = cpu.state_hash();

    let actual = CudaBackend::new(&model, initial, &params, 13, HashMode::EveryTick)
        .expect("CUDA device, driver, and NVRTC are required")
        .run(1)
        .unwrap()
        .per_tick_state_hashes[0];
    assert_eq!(actual, expected);
}

#[test]
#[ignore = "requires a CUDA GPU; run crates/sembla-cuda/scripts/run-gpu-tests.sh"]
fn sequential_group_and_output_reductions_match_cpu() {
    for (model, initial, seed) in [
        (
            sequential_group_sum_model(),
            sequential_group_sum_state(),
            31,
        ),
        (
            sequential_output_sum_model(),
            sequential_output_sum_state(),
            33,
        ),
    ] {
        let params = ParamEnv::defaults(&model);
        let mut cpu = StateStore::new(&model, initial.clone()).unwrap();
        run_tick(&model, &mut cpu, &params, seed, 0).unwrap();
        let expected = cpu.state_hash();
        let actual = CudaBackend::new(&model, initial, &params, seed, HashMode::EveryTick)
            .expect("CUDA device, driver, and NVRTC are required")
            .run(1)
            .unwrap()
            .per_tick_state_hashes[0];
        assert_eq!(actual, expected);
    }

    let model = sequential_int_overflow_model();
    let initial = sequential_int_overflow_state();
    let params = ParamEnv::defaults(&model);
    let mut cpu = StateStore::new(&model, initial.clone()).unwrap();
    assert!(run_tick(&model, &mut cpu, &params, 35, 0)
        .unwrap_err()
        .to_string()
        .contains("overflow"));
    let mut gpu = CudaBackend::new(&model, initial, &params, 35, HashMode::FinalOnly)
        .expect("CUDA device, driver, and NVRTC are required");
    assert!(gpu.run(1).unwrap_err().to_string().contains("aggregate"));
}

#[test]
#[ignore = "requires a CUDA GPU; run crates/sembla-cuda/scripts/run-gpu-tests.sh"]
fn earlier_claim_error_precedes_later_guard_error() {
    let model = claim_before_later_guard_error_model();
    let initial = claim_before_later_guard_error_state();
    let params = ParamEnv::defaults(&model);
    let mut cpu = StateStore::new(&model, initial.clone()).unwrap();
    assert!(run_tick(&model, &mut cpu, &params, 37, 0)
        .unwrap_err()
        .to_string()
        .contains("overflow"));

    let mut gpu = CudaBackend::new(&model, initial, &params, 37, HashMode::FinalOnly)
        .expect("CUDA device, driver, and NVRTC are required");
    assert!(gpu
        .run(1)
        .unwrap_err()
        .to_string()
        .contains("claim expression overflowed"));
}

#[test]
#[ignore = "requires a CUDA GPU; run crates/sembla-cuda/scripts/run-gpu-tests.sh"]
fn earlier_scalar_error_precedes_later_aggregate_error() {
    let model = scalar_before_later_aggregate_error_model();
    let initial = scalar_before_later_aggregate_error_state();
    let params = ParamEnv::defaults(&model);
    let mut cpu = StateStore::new(&model, initial.clone()).unwrap();
    assert!(run_tick(&model, &mut cpu, &params, 39, 0)
        .unwrap_err()
        .to_string()
        .contains("overflow"));

    let mut gpu = CudaBackend::new(&model, initial, &params, 39, HashMode::FinalOnly)
        .expect("CUDA device, driver, and NVRTC are required");
    assert!(gpu
        .run(1)
        .unwrap_err()
        .to_string()
        .contains("candidate 0 overflowed Int"));
}

#[test]
#[ignore = "requires a CUDA GPU; run crates/sembla-cuda/scripts/run-gpu-tests.sh"]
fn aggregate_use_staging_matches_cpu_error_liveness() {
    for (model, initial, seed) in [
        (
            prospective_output_overflow_model(),
            prospective_output_overflow_state(),
            19,
        ),
        (
            transition_only_aggregate_model(),
            transition_only_aggregate_state(),
            21,
        ),
        (
            effect_aggregate_model(false),
            effect_aggregate_state([i64::MAX, 1]),
            23,
        ),
        (
            effect_aggregate_model(true),
            effect_aggregate_state([2, 3]),
            25,
        ),
    ] {
        let params = ParamEnv::defaults(&model);
        let mut cpu = StateStore::new(&model, initial.clone()).unwrap();
        run_tick(&model, &mut cpu, &params, seed, 0).unwrap();
        let expected = cpu.state_hash();
        let actual = CudaBackend::new(&model, initial, &params, seed, HashMode::EveryTick)
            .expect("CUDA device, driver, and NVRTC are required")
            .run(1)
            .unwrap()
            .per_tick_state_hashes[0];
        assert_eq!(actual, expected);
    }
}

#[test]
#[ignore = "requires a CUDA GPU; run crates/sembla-cuda/scripts/run-gpu-tests.sh"]
fn input_integer_ordering_matches_cpu_f64_semantics() {
    let model = input_integer_ordering_model();
    let initial = input_integer_ordering_state();
    let params = ParamEnv::defaults(&model);
    let mut cpu = StateStore::new(&model, initial.clone()).unwrap();
    let mut expected = Vec::new();
    for tick in 0..2 {
        run_tick(&model, &mut cpu, &params, 29, tick).unwrap();
        expected.push(cpu.state_hash());
    }
    assert_eq!(
        cpu.snapshot()
            .enum_index("sink", "Agent", "state", 0)
            .unwrap(),
        0
    );

    let actual = CudaBackend::new(&model, initial, &params, 29, HashMode::EveryTick)
        .expect("CUDA device, driver, and NVRTC are required")
        .run(2)
        .unwrap()
        .per_tick_state_hashes;
    assert_eq!(actual, expected);
}

#[test]
#[ignore = "requires a CUDA GPU; run crates/sembla-cuda/scripts/run-gpu-tests.sh"]
fn minimum_integer_literal_matches_cpu_signed_semantics() {
    let model = minimum_integer_model();
    let initial = minimum_integer_state();
    let params = ParamEnv::defaults(&model);

    let mut cpu = StateStore::new(&model, initial.clone()).unwrap();
    run_tick(&model, &mut cpu, &params, 17, 0).unwrap();
    assert_eq!(
        cpu.snapshot().int("world", "Person", "x", 0).unwrap(),
        i64::MIN
    );
    let expected = cpu.state_hash();

    let actual = CudaBackend::new(&model, initial, &params, 17, HashMode::EveryTick)
        .expect("CUDA device, driver, and NVRTC are required")
        .run(1)
        .unwrap()
        .per_tick_state_hashes[0];
    assert_eq!(actual, expected);
}

#[test]
#[ignore = "requires a CUDA GPU; run crates/sembla-cuda/scripts/run-gpu-tests.sh"]
fn checked_multiply_and_eager_claim_errors_match_cpu() {
    let model = claim_overflow_model();
    let initial = claim_overflow_state();
    let params = ParamEnv::defaults(&model);

    let mut cpu = StateStore::new(&model, initial.clone()).unwrap();
    let cpu_error = run_tick(&model, &mut cpu, &params, 7, 0).unwrap_err();
    assert!(cpu_error.to_string().contains("overflow"));

    let mut gpu = CudaBackend::new(&model, initial, &params, 7, HashMode::FinalOnly)
        .expect("CUDA device, driver, and NVRTC are required");
    let gpu_error = gpu.run(1).unwrap_err();
    assert!(gpu_error
        .to_string()
        .contains("claim expression overflowed"));
}

#[test]
#[ignore = "requires a CUDA GPU; run crates/sembla-cuda/scripts/run-gpu-tests.sh"]
fn losing_row_effect_is_evaluated_when_transition_has_a_winner() {
    let model = losing_row_effect_overflow_model();
    let initial = losing_row_effect_state();
    let params = ParamEnv::defaults(&model);

    let mut cpu = StateStore::new(&model, initial.clone()).unwrap();
    assert!(run_tick(&model, &mut cpu, &params, 9, 0)
        .unwrap_err()
        .to_string()
        .contains("overflow"));

    let mut gpu = CudaBackend::new(&model, initial, &params, 9, HashMode::FinalOnly)
        .expect("CUDA device, driver, and NVRTC are required");
    assert!(gpu
        .run(1)
        .unwrap_err()
        .to_string()
        .contains("effect overflowed"));
}

#[test]
#[ignore = "requires a CUDA GPU; run crates/sembla-cuda/scripts/run-gpu-tests.sh"]
fn filtered_group_sum_evaluates_values_before_selection() {
    let model = filtered_aggregate_overflow_model();
    let initial = filtered_aggregate_state();
    let params = ParamEnv::defaults(&model);

    let mut cpu = StateStore::new(&model, initial.clone()).unwrap();
    assert!(run_tick(&model, &mut cpu, &params, 3, 0)
        .unwrap_err()
        .to_string()
        .contains("overflow"));

    let mut gpu = CudaBackend::new(&model, initial, &params, 3, HashMode::FinalOnly)
        .expect("CUDA device, driver, and NVRTC are required");
    assert!(gpu.run(1).unwrap_err().to_string().contains("aggregate"));
}

#[test]
#[ignore = "requires a CUDA GPU; run crates/sembla-cuda/scripts/run-gpu-tests.sh"]
fn nested_wire_aggregate_uses_prospective_state() {
    let model = prospective_nested_output_model();
    let initial = prospective_nested_output_state();
    let params = ParamEnv::defaults(&model);

    let mut cpu = StateStore::new(&model, initial.clone()).unwrap();
    run_tick(&model, &mut cpu, &params, 11, 0).unwrap();
    let expected = cpu.state_hash();

    let actual = CudaBackend::new(&model, initial, &params, 11, HashMode::EveryTick)
        .expect("CUDA device, driver, and NVRTC are required")
        .run(1)
        .unwrap()
        .per_tick_state_hashes[0];
    assert_eq!(actual, expected);
}
