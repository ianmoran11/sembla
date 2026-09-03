use super::*;
use sembla_ir::{validate, Attr, Box as ModelBox, Effect, Model, Table, Transition};
use sembla_runtime::core::{ColumnData, ColumnInit, StateStore, TableInit};

#[test]
fn scratch_reuses_storage_and_sizes_only_written_columns() {
    const ATTRS: usize = 128;
    const ROWS: usize = 130;

    DOUBLE_WRITE_SCRATCH.with(|scratch| {
        *scratch.borrow_mut() = DoubleWriteScratch::default();
    });
    let attrs = (0..ATTRS)
        .map(|index| Attr {
            name: format!("field_{index}"),
            ty: AttrType::Int,
        })
        .collect::<Vec<_>>();
    let model = validate(Model {
        name: "bitmap-scratch".to_owned(),
        dt: 1.0,
        params: Vec::new(),
        boxes: vec![ModelBox {
            name: "world".to_owned(),
            tables: vec![Table {
                name: "Item".to_owned(),
                size_hint: ROWS as u64,
                attrs: attrs.clone(),
            }],
            transitions: vec![Transition {
                name: "write-one-of-many".to_owned(),
                table: "Item".to_owned(),
                guard: Expr::Bool { value: true },
                hazard: Expr::Real { value: 1.0e300 },
                effects: vec![Effect::SetAttr {
                    attr: "field_0".to_owned(),
                    value: Expr::Int { value: 1 },
                }],
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
    .unwrap();
    let columns = attrs
        .iter()
        .map(|attr| ColumnInit::new(&attr.name, ColumnData::Int(vec![0; ROWS])))
        .collect();
    let mut state =
        StateStore::new(&model, vec![TableInit::new("world", "Item", ROWS, columns)]).unwrap();
    let params = ParamEnv::defaults(&model);

    run_tick(&model, &mut state, &params, 7, 0).unwrap();
    let first = DOUBLE_WRITE_SCRATCH.with(|scratch| {
        let scratch = scratch.borrow();
        assert_eq!(scratch.destination_slots.len(), 1);
        assert_eq!(
            scratch.columns.len(),
            1,
            "127 unwritten columns need no bitmap"
        );
        assert_eq!(scratch.words.len(), ROWS.div_ceil(u64::BITS as usize));
        assert_eq!(scratch.touched_words.len(), scratch.words.len());
        (
            scratch.words.as_ptr(),
            scratch.words.capacity(),
            scratch.touched_words.as_ptr(),
            scratch.touched_words.capacity(),
        )
    });

    run_tick(&model, &mut state, &params, 7, 1).unwrap();
    DOUBLE_WRITE_SCRATCH.with(|scratch| {
        let scratch = scratch.borrow();
        assert_eq!(scratch.destination_slots.len(), 1);
        assert_eq!(scratch.columns.len(), 1);
        assert_eq!(scratch.words.len(), ROWS.div_ceil(u64::BITS as usize));
        assert_eq!(
            (
                scratch.words.as_ptr(),
                scratch.words.capacity(),
                scratch.touched_words.as_ptr(),
                scratch.touched_words.capacity(),
            ),
            first,
            "bitmap and touched-word storage must be reused across ticks"
        );
    });
}
