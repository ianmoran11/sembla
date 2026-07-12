use sembla_ir::{parse_json, validate, AttrType, ParamValue};
use sembla_runtime::eval::{ParamEnv, ParamOverride};
use sembla_runtime::executor::run_tick;
use sembla_runtime::state::{ColumnData, ColumnInit, StateStore, TableInit};

fn load(source: &str) -> sembla_ir::ValidatedModel {
    validate(parse_json(source).unwrap()).unwrap()
}

fn zero_state(model: &sembla_ir::ValidatedModel) -> StateStore {
    let tables = model
        .model()
        .boxes
        .iter()
        .flat_map(|model_box| {
            model_box.tables.iter().map(|table| {
                let rows = usize::try_from(table.size_hint).unwrap();
                let columns = table
                    .attrs
                    .iter()
                    .map(|attr| {
                        let data = match &attr.ty {
                            AttrType::Real => ColumnData::Real(vec![0.0; rows]),
                            AttrType::Int => ColumnData::Int(vec![0; rows]),
                            AttrType::Enum { .. } => ColumnData::Enum(vec![0; rows]),
                            AttrType::Ref { .. } => ColumnData::Ref(vec![0; rows]),
                        };
                        ColumnInit::new(&attr.name, data)
                    })
                    .collect();
                TableInit::new(&model_box.name, &table.name, rows, columns)
            })
        })
        .collect();
    StateStore::new(model, tables).unwrap()
}

#[test]
fn tick_zero_inputs_are_empty_and_delivery_has_exactly_one_tick_delay() {
    let model = load(include_str!("../../../examples/two_box.json"));
    let params = ParamEnv::defaults(&model);
    let mut state = zero_state(&model);

    // Every declared input exists as a schema-carrying, zero-row table at tick 0.
    assert_eq!(
        state
            .snapshot()
            .input_table("population", "control")
            .unwrap()
            .row_count,
        0
    );
    assert_eq!(
        state
            .snapshot()
            .input_table("controller", "infection")
            .unwrap()
            .row_count,
        0
    );

    let tick_zero = run_tick(&model, &mut state, &params, 9, 0).unwrap();
    assert_eq!(tick_zero.fired, vec![(0, 0), (1, 1)]);
    assert_eq!(
        state
            .snapshot()
            .enum_index("population", "Person", "health", 0),
        Ok(0)
    );
    // Controller's committed modifier was copied only after all boxes committed.
    let control = state
        .snapshot()
        .input_table("population", "control")
        .unwrap()
        .clone();
    assert_eq!(control.row_count, 1);
    assert_eq!(
        control.column("modifier"),
        Some(&ColumnData::Real(vec![1.0]))
    );

    let tick_one = run_tick(&model, &mut state, &params, 9, 1).unwrap();
    assert_eq!(tick_one.fired, vec![(0, 16), (1, 0)]);
    for row in 0..16 {
        assert_eq!(
            state
                .snapshot()
                .enum_index("population", "Person", "health", row),
            Ok(1)
        );
    }
}

#[test]
fn composed_feedback_is_hash_and_report_deterministic_for_fifty_ticks() {
    let model = load(include_str!("../../../examples/two_box.json"));
    let params = ParamEnv::defaults(&model);
    let mut lhs = zero_state(&model);
    let mut rhs = zero_state(&model);
    for tick in 0..50 {
        let lhs_report = run_tick(&model, &mut lhs, &params, 123, tick).unwrap();
        let rhs_report = run_tick(&model, &mut rhs, &params, 123, tick).unwrap();
        assert_eq!(lhs_report, rhs_report);
        assert_eq!(lhs.state_hash(), rhs.state_hash());
    }
}

#[test]
fn moving_the_box_boundary_preserves_every_table_bitwise() {
    let composed = load(include_str!("../../../examples/two_box.json"));
    let merged = load(include_str!("../../../examples/two_box_merged.json"));
    let composed_params = ParamEnv::defaults(&composed);
    let merged_params = ParamEnv::defaults(&merged);
    let mut composed_state = zero_state(&composed);
    let mut merged_state = zero_state(&merged);

    // Both fixtures declare `infect` before `enable`, so their model-global
    // rule_ids are 0 and 1 in both layouts. Person and Controller populations
    // are also identical, hence row-index entity_ids and Philox coordinates
    // align exactly. Their same-schema `group` references are initialized to
    // row zero. The merged fixture has no ports or wires: its Expr::Agg reads
    // the tick-start Person/Controller snapshot, matching wire delivery's
    // uniform one-tick delay without retaining the box boundary.
    for tick in 0..50 {
        let composed_report =
            run_tick(&composed, &mut composed_state, &composed_params, 77, tick).unwrap();
        let merged_report = run_tick(&merged, &mut merged_state, &merged_params, 77, tick).unwrap();
        assert_eq!(composed_report.fired, merged_report.fired);
        assert_eq!(
            composed_state
                .snapshot()
                .table_hash("population", "Person")
                .unwrap(),
            merged_state
                .snapshot()
                .table_hash("merged", "Person")
                .unwrap()
        );
        assert_eq!(
            composed_state
                .snapshot()
                .table_hash("controller", "Controller")
                .unwrap(),
            merged_state
                .snapshot()
                .table_hash("merged", "Controller")
                .unwrap()
        );
    }
}

#[test]
fn output_failure_rolls_back_state_and_inputs_and_store_is_reusable() {
    let model = load(
        r#"{
          "name": "fallible_output",
          "dt": 1.0,
          "params": [{
            "name": "amount",
            "ty": "int",
            "default": { "kind": "int", "value": 9223372036854775807 },
            "prior": null
          }],
          "boxes": [{
            "name": "box",
            "tables": [{
              "name": "Row",
              "size_hint": 2,
              "attrs": [{ "name": "marker", "ty": { "kind": "int" } }]
            }],
            "transitions": [{
              "name": "mark",
              "table": "Row",
              "guard": { "kind": "bool", "value": true },
              "hazard": { "kind": "real", "value": 1e300 },
              "effects": [{
                "kind": "set_attr",
                "attr": "marker",
                "value": { "kind": "int", "value": 1 }
              }],
              "contests": []
            }],
            "inputs": [{
              "name": "loop",
              "schema": [{ "name": "total", "ty": { "kind": "int" } }]
            }],
            "outputs": [{
              "name": "loop",
              "schema": [{ "name": "total", "ty": { "kind": "int" } }],
              "builder": {
                "kind": "per_table",
                "table": "Row",
                "fields": [{
                  "name": "total",
                  "op": {
                    "kind": "sum",
                    "value": { "kind": "param", "name": "amount" }
                  },
                  "filter": null
                }]
              }
            }]
          }],
          "wires": [{
            "from": { "box": "box", "port": "loop" },
            "to": { "box": "box", "port": "loop" }
          }]
        }"#,
    );
    let mut state = zero_state(&model);
    let before_hash = state.state_hash();
    let before_input = state.snapshot().input_table("box", "loop").unwrap().clone();

    let error = run_tick(&model, &mut state, &ParamEnv::defaults(&model), 4, 0).unwrap_err();
    assert!(error.to_string().contains("output integer sum overflow"));
    assert_eq!(state.state_hash(), before_hash);
    assert_eq!(
        state.snapshot().input_table("box", "loop").unwrap(),
        &before_input
    );
    assert_eq!(state.snapshot().int("box", "Row", "marker", 0), Ok(0));

    let safe_params = ParamEnv::resolve(
        &model,
        &[ParamOverride::new("amount", ParamValue::Int { value: 0 })],
    )
    .unwrap();
    run_tick(&model, &mut state, &safe_params, 4, 0).unwrap();
    assert_eq!(state.snapshot().int("box", "Row", "marker", 0), Ok(1));
    assert_eq!(
        state
            .snapshot()
            .input_table("box", "loop")
            .unwrap()
            .column("total"),
        Some(&ColumnData::Int(vec![0]))
    );
}
