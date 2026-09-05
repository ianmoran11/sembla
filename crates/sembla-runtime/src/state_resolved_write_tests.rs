use super::*;

fn state_data() -> StateData {
    StateData {
        tables: vec![
            TableState {
                box_name: "world".to_owned(),
                name: "Node".to_owned(),
                row_count: 2,
                columns: vec![ColumnState::Enum {
                    name: "color".to_owned(),
                    variant_count: 2,
                    values: vec![0, 1],
                }],
            },
            TableState {
                box_name: "world".to_owned(),
                name: "Edge".to_owned(),
                row_count: 1,
                columns: vec![ColumnState::Ref {
                    name: "to".to_owned(),
                    target_table: 0,
                    values: vec![0],
                }],
            },
        ],
    }
}

#[test]
fn resolved_writes_preserve_row_enum_and_ref_validation() {
    let mut state = state_data();
    let (color, target) = {
        let snapshot = Snapshot {
            state: &state,
            inputs: &[],
        };
        (
            snapshot
                .resolve_write_column("world", "Node", "color")
                .unwrap(),
            snapshot
                .resolve_write_column("world", "Edge", "to")
                .unwrap(),
        )
    };
    let mut writes = WriteBuffer { state: &mut state };

    assert_eq!(
        writes
            .set_resolved_real(color, 2, 0.0)
            .unwrap_err()
            .to_string(),
        "box 'world', table 'Node', column 'color', row 2: row index is out of bounds for 2 rows"
    );
    assert_eq!(
        writes
            .set_resolved_enum(color, 1, 2)
            .unwrap_err()
            .to_string(),
        "box 'world', table 'Node', column 'color', row 1: enum index 2 is out of bounds for 2 variants"
    );
    assert_eq!(
        writes
            .set_resolved_ref(target, 0, 2)
            .unwrap_err()
            .to_string(),
        "box 'world', table 'Edge', column 'to', row 0: reference index 2 is out of bounds for target table 'Node' with 2 rows"
    );
}

fn refresh_model() -> ValidatedModel {
    let source = r#"
    {
      "name": "refresh",
      "dt": 1.0,
      "params": [],
      "boxes": [{
        "name": "world",
        "tables": [
          {"name": "Node", "size_hint": 3, "attrs": [
            {"name": "real", "ty": {"kind": "real"}},
            {"name": "int", "ty": {"kind": "int"}},
            {"name": "enum", "ty": {"kind": "enum", "variants": ["a", "b"]}}
          ]},
          {"name": "Edge", "size_hint": 2, "attrs": [
            {"name": "to", "ty": {"kind": "ref", "table": "Node"}}
          ]}
        ],
        "transitions": [],
        "inputs": [{
          "name": "incoming",
          "schema": [{"name": "value", "ty": {"kind": "int"}}]
        }],
        "outputs": []
      }],
      "wires": []
    }
    "#;
    sembla_ir::validate(sembla_ir::parse_json(source).unwrap()).unwrap()
}

fn refresh_initial(offset: i64) -> Vec<TableInit> {
    vec![
        TableInit::new(
            "world",
            "Node",
            3,
            vec![
                ColumnInit::new("real", ColumnData::Real(vec![offset as f64, 1.0, 2.0])),
                ColumnInit::new("int", ColumnData::Int(vec![offset, offset + 1, offset + 2])),
                ColumnInit::new("enum", ColumnData::Enum(vec![0, 1, 0])),
            ],
        ),
        TableInit::new(
            "world",
            "Edge",
            2,
            vec![ColumnInit::new("to", ColumnData::Ref(vec![0, 2]))],
        ),
    ]
}

fn allocation_signature(state: &StateData) -> Vec<(usize, usize)> {
    state
        .tables
        .iter()
        .flat_map(|table| &table.columns)
        .map(|column| match column {
            ColumnState::Real { values, .. } => (values.as_ptr() as usize, values.capacity()),
            ColumnState::Int { values, .. } => (values.as_ptr() as usize, values.capacity()),
            ColumnState::Enum { values, .. } => (values.as_ptr() as usize, values.capacity()),
            ColumnState::Ref { values, .. } => (values.as_ptr() as usize, values.capacity()),
        })
        .collect()
}

fn input_allocation_signature(inputs: &[InputTable]) -> Vec<(usize, usize)> {
    inputs
        .iter()
        .flat_map(|input| &input.columns)
        .map(|column| match column {
            ColumnData::Real(values) => (values.as_ptr() as usize, values.capacity()),
            ColumnData::Int(values) => (values.as_ptr() as usize, values.capacity()),
            ColumnData::Enum(values) => (values.as_ptr() as usize, values.capacity()),
            ColumnData::Ref(values) => (values.as_ptr() as usize, values.capacity()),
        })
        .collect()
}

#[test]
fn backend_refresh_retains_current_and_next_column_allocations() {
    let model = refresh_model();
    let mut store = StateStore::new(&model, refresh_initial(0)).unwrap();
    let current_before = allocation_signature(&store.current);
    let next_before = allocation_signature(&store.next);

    store
        .refresh_backend_state(&model, &refresh_initial(10))
        .unwrap();
    assert_eq!(allocation_signature(&store.current), current_before);
    assert_eq!(allocation_signature(&store.next), next_before);
    assert_eq!(store.snapshot().int("world", "Node", "int", 2), Ok(12));

    store
        .refresh_backend_state(&model, &refresh_initial(20))
        .unwrap();
    assert_eq!(allocation_signature(&store.current), current_before);
    assert_eq!(allocation_signature(&store.next), next_before);
    {
        let _writes = store.write_buffer().unwrap();
    }
    assert_eq!(allocation_signature(&store.next), next_before);
}

#[test]
fn draw_reset_restores_both_buffers_and_empty_inputs_without_reallocation() {
    let model = refresh_model();
    let initial = refresh_initial(0);
    let mut store = StateStore::new(&model, initial.clone()).unwrap();
    let current_before = allocation_signature(&store.current);
    let next_before = allocation_signature(&store.next);

    store.inputs[0].row_count = 3;
    store.inputs[0].columns[0] = ColumnData::Int(Vec::with_capacity(8));
    let ColumnData::Int(values) = &mut store.inputs[0].columns[0] else {
        unreachable!()
    };
    values.extend([4, 5, 6]);
    let inputs_before = input_allocation_signature(&store.inputs);

    for offset in [10, 20, 0] {
        store
            .reset_backend_draw(&model, &refresh_initial(offset))
            .unwrap();
        assert_eq!(allocation_signature(&store.current), current_before);
        assert_eq!(allocation_signature(&store.next), next_before);
        assert_eq!(input_allocation_signature(&store.inputs), inputs_before);
        assert_eq!(store.inputs[0].row_count, 0);
        assert_eq!(store.inputs[0].columns[0].len(), 0);
        assert_eq!(store.snapshot().int("world", "Node", "int", 0), Ok(offset));
        assert_eq!(store.current, store.next);
    }
    assert_eq!(
        store.current,
        StateStore::new(&model, initial).unwrap().current
    );
}

#[test]
fn draw_reset_validation_is_constructor_equivalent_and_atomic() {
    let model = refresh_model();
    let mut store = StateStore::new(&model, refresh_initial(7)).unwrap();
    store.inputs[0].row_count = 1;
    store.inputs[0].columns[0] = ColumnData::Int(vec![99]);
    let before = store.clone();
    let mut malformed = refresh_initial(0);
    malformed[0].columns[2].data = ColumnData::Enum(vec![0, 2, 0]);

    let reset_error = store
        .reset_backend_draw(&model, &malformed)
        .unwrap_err()
        .to_string();
    let constructor_error = StateStore::new(&model, malformed).unwrap_err().to_string();
    assert_eq!(reset_error, constructor_error);
    assert_eq!(store.current, before.current);
    assert_eq!(store.next, before.next);
    assert_eq!(store.inputs, before.inputs);
    assert_eq!(store.write_prepared, before.write_prepared);
}
