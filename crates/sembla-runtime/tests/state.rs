use sembla_runtime::state::{ColumnData, ColumnInit, StateStore, TableInit};

fn two_state_model() -> sembla_ir::ValidatedModel {
    let model = sembla_ir::parse_json(include_str!("../../../examples/two_state.json"))
        .expect("two-state fixture must parse");
    sembla_ir::validate(model).expect("two-state fixture must validate")
}

fn two_state_init(values: Vec<u16>) -> Vec<TableInit> {
    vec![TableInit::new(
        "population",
        "Person",
        values.len(),
        vec![ColumnInit::new("mood", ColumnData::Enum(values))],
    )]
}

fn fixed_moods() -> Vec<u16> {
    (0..100).map(|row| (row % 2) as u16).collect()
}

fn all_types_model() -> sembla_ir::ValidatedModel {
    let source = r#"
    {
      "name": "all_types",
      "dt": 1.0,
      "params": [],
      "boxes": [
        {
          "name": "network",
          "tables": [
            {
              "name": "Node",
              "size_hint": 3,
              "attrs": [
                {"name": "weight", "ty": {"kind": "real"}},
                {"name": "rank", "ty": {"kind": "int"}},
                {"name": "color", "ty": {"kind": "enum", "variants": ["Red", "Blue"]}}
              ]
            },
            {
              "name": "Edge",
              "size_hint": 2,
              "attrs": [
                {"name": "to", "ty": {"kind": "ref", "table": "Node"}}
              ]
            }
          ],
          "transitions": [],
          "inputs": [],
          "outputs": []
        },
        {
          "name": "other",
          "tables": [
            {
              "name": "Node",
              "size_hint": 1,
              "attrs": [
                {"name": "value", "ty": {"kind": "int"}}
              ]
            }
          ],
          "transitions": [],
          "inputs": [],
          "outputs": []
        }
      ],
      "wires": []
    }
    "#;
    sembla_ir::validate(sembla_ir::parse_json(source).expect("model must parse"))
        .expect("model must validate")
}

fn all_types_init() -> Vec<TableInit> {
    vec![
        TableInit::new(
            "network",
            "Node",
            3,
            vec![
                ColumnInit::new("weight", ColumnData::Real(vec![1.5, -0.0, 3.25])),
                ColumnInit::new("rank", ColumnData::Int(vec![-2, 0, 9])),
                ColumnInit::new("color", ColumnData::Enum(vec![0, 1, 0])),
            ],
        ),
        TableInit::new(
            "network",
            "Edge",
            2,
            vec![ColumnInit::new("to", ColumnData::Ref(vec![0, 2]))],
        ),
        TableInit::new(
            "other",
            "Node",
            1,
            vec![ColumnInit::new("value", ColumnData::Int(vec![42]))],
        ),
    ]
}

#[test]
fn initializes_two_state_and_reports_enum_and_ref_rows() {
    let model = two_state_model();
    let store = StateStore::new(&model, two_state_init(vec![0; 100]))
        .expect("100-row two-state population must initialize");
    assert_eq!(store.snapshot().row_count("population", "Person"), Ok(100));
    assert_eq!(
        store
            .snapshot()
            .enum_index("population", "Person", "mood", 99),
        Ok(0)
    );

    let mut invalid_enum = vec![0; 100];
    invalid_enum[73] = 2;
    let error = StateStore::new(&model, two_state_init(invalid_enum)).unwrap_err();
    assert_eq!(
        error.to_string(),
        "box 'population', table 'Person', column 'mood', row 73: enum index 2 is out of bounds for 2 variants"
    );

    let ref_model = all_types_model();
    let mut invalid_ref = all_types_init();
    invalid_ref[1].columns[0].data = ColumnData::Ref(vec![0, 3]);
    let error = StateStore::new(&ref_model, invalid_ref).unwrap_err();
    assert_eq!(
        error.to_string(),
        "box 'network', table 'Edge', column 'to', row 1: reference index 3 is out of bounds for target table 'Node' with 3 rows"
    );
}

#[test]
fn writes_are_invisible_until_commit_and_populations_remain_fixed() {
    let model = all_types_model();
    let mut store = StateStore::new(&model, all_types_init()).expect("state must initialize");
    let initial_hash = store.state_hash();

    {
        let (old, mut writes) = store.buffers().expect("write buffer must prepare");
        writes
            .set_real("network", "Node", "weight", 0, 7.0)
            .unwrap();
        writes.set_int("network", "Node", "rank", 1, 11).unwrap();
        writes.set_enum("network", "Node", "color", 2, 1).unwrap();
        writes.set_ref("network", "Edge", "to", 0, 2).unwrap();
        writes.set_int("other", "Node", "value", 0, 99).unwrap();

        assert_eq!(old.real("network", "Node", "weight", 0), Ok(1.5));
        assert_eq!(old.int("network", "Node", "rank", 1), Ok(0));
        assert_eq!(old.enum_index("network", "Node", "color", 2), Ok(0));
        assert_eq!(old.reference("network", "Edge", "to", 0), Ok(0));
        assert_eq!(old.int("other", "Node", "value", 0), Ok(42));
        assert_eq!(old.state_hash(), initial_hash);
    }

    assert_eq!(store.state_hash(), initial_hash);
    store.commit().expect("prepared writes must commit");
    let new = store.snapshot();
    assert_eq!(new.real("network", "Node", "weight", 0), Ok(7.0));
    assert_eq!(new.int("network", "Node", "rank", 1), Ok(11));
    assert_eq!(new.enum_index("network", "Node", "color", 2), Ok(1));
    assert_eq!(new.reference("network", "Edge", "to", 0), Ok(2));
    assert_eq!(new.int("other", "Node", "value", 0), Ok(99));
    assert_eq!(new.row_count("network", "Node"), Ok(3));
    assert_ne!(new.state_hash(), initial_hash);
    assert_eq!(
        store.commit().unwrap_err().to_string(),
        "cannot commit state: no write buffer has been prepared"
    );
}

#[test]
fn canonical_hash_is_golden_deterministic_and_cell_sensitive() {
    let model = two_state_model();
    let expected = [
        0xd3, 0x91, 0xa9, 0x4f, 0xaa, 0xd7, 0x62, 0x9c, 0x65, 0x87, 0x91, 0x90, 0x0c, 0x6f, 0x81,
        0xae, 0xc1, 0x17, 0x24, 0xd0, 0x6d, 0xa1, 0x55, 0x70, 0x40, 0x8f, 0x9a, 0xb5, 0x5b, 0x76,
        0x1a, 0x33,
    ];
    let first = StateStore::new(&model, two_state_init(fixed_moods())).unwrap();
    let second = StateStore::new(&model, two_state_init(fixed_moods())).unwrap();

    assert_eq!(first.state_hash(), expected);
    assert_eq!(second.state_hash(), expected);

    for row in 0..100 {
        let mut changed = StateStore::new(&model, two_state_init(fixed_moods())).unwrap();
        {
            let mut writes = changed.write_buffer().unwrap();
            writes
                .set_enum("population", "Person", "mood", row, 1 - (row % 2) as u16)
                .unwrap();
        }
        changed.commit().unwrap();
        assert_ne!(
            changed.state_hash(),
            expected,
            "row {row} did not affect hash"
        );
    }
}

#[test]
fn every_physical_cell_encoding_affects_the_hash() {
    let model = all_types_model();
    let baseline = StateStore::new(&model, all_types_init())
        .unwrap()
        .state_hash();

    let mut real = StateStore::new(&model, all_types_init()).unwrap();
    real.write_buffer()
        .unwrap()
        .set_real("network", "Node", "weight", 1, 0.0)
        .unwrap();
    real.commit().unwrap();
    assert_ne!(real.state_hash(), baseline);

    let mut int = StateStore::new(&model, all_types_init()).unwrap();
    int.write_buffer()
        .unwrap()
        .set_int("network", "Node", "rank", 0, -1)
        .unwrap();
    int.commit().unwrap();
    assert_ne!(int.state_hash(), baseline);

    let mut enumeration = StateStore::new(&model, all_types_init()).unwrap();
    enumeration
        .write_buffer()
        .unwrap()
        .set_enum("network", "Node", "color", 0, 1)
        .unwrap();
    enumeration.commit().unwrap();
    assert_ne!(enumeration.state_hash(), baseline);

    let mut reference = StateStore::new(&model, all_types_init()).unwrap();
    reference
        .write_buffer()
        .unwrap()
        .set_ref("network", "Edge", "to", 0, 1)
        .unwrap();
    reference.commit().unwrap();
    assert_ne!(reference.state_hash(), baseline);
}

#[test]
fn write_time_enum_and_ref_bounds_name_the_cell() {
    let model = all_types_model();
    let mut store = StateStore::new(&model, all_types_init()).unwrap();
    {
        let mut writes = store.write_buffer().unwrap();
        assert_eq!(
            writes
                .set_enum("network", "Node", "color", 1, 2)
                .unwrap_err()
                .to_string(),
            "box 'network', table 'Node', column 'color', row 1: enum index 2 is out of bounds for 2 variants"
        );
        assert_eq!(
            writes
                .set_ref("network", "Edge", "to", 0, 3)
                .unwrap_err()
                .to_string(),
            "box 'network', table 'Edge', column 'to', row 0: reference index 3 is out of bounds for target table 'Node' with 3 rows"
        );
    }
}
