use sembla_ir::{AggJoin, AggOp, Attr, AttrType, Box as IrBox, Expr, Model, Table};
use sembla_runtime::eval::{eval_column, AggCache, EvalTable, ParamEnv, ValueColumn};
use sembla_runtime::state::{ColumnData, ColumnInit, StateStore, TableInit};

const EVAL_SOURCE: &str = include_str!("../src/eval.rs");

#[test]
fn typed_snapshot_accessors_do_not_regress_into_evaluator_row_loops() {
    for accessor in [
        "snapshot.real(",
        "snapshot.int(",
        "snapshot.enum_index(",
        "snapshot.reference(",
    ] {
        assert!(
            !EVAL_SOURCE.contains(accessor),
            "{accessor} must not be called from eval.rs; resolve the column before iterating rows"
        );
    }
    assert_eq!(
        EVAL_SOURCE.matches("snapshot.resolve_column(").count(),
        12,
        "whole-column and prepared-tile SelfAttr/EnumIs paths plus aggregate reads must each resolve once"
    );
}

#[test]
fn zero_row_typed_column_reads_remain_lazy() {
    for empty_case in [
        "AttrType::Real if row_count == 0",
        "AttrType::Int if row_count == 0",
        "AttrType::Enum { .. } if row_count == 0",
        "AttrType::Ref { .. } if row_count == 0",
        "if row_count == 0 {\n                return Ok(InternalColumn::Bool(Vec::new()));",
    ] {
        assert!(
            EVAL_SOURCE.contains(empty_case),
            "missing lazy zero-row case: {empty_case}"
        );
    }
}

fn attr(name: &str, ty: AttrType) -> Attr {
    Attr {
        name: name.into(),
        ty,
    }
}

fn aggregate_model(target_fk_type: AttrType) -> sembla_ir::ValidatedModel {
    sembla_ir::validate(Model {
        name: "aggregate_resolution_fixture".into(),
        dt: 1.0,
        params: vec![],
        boxes: vec![IrBox {
            name: "world".into(),
            tables: vec![
                Table {
                    name: "Group".into(),
                    size_hint: 1,
                    attrs: vec![],
                },
                Table {
                    name: "Target".into(),
                    size_hint: 2,
                    attrs: vec![attr("group", target_fk_type)],
                },
                Table {
                    name: "Query".into(),
                    size_hint: 1,
                    attrs: vec![attr(
                        "group",
                        AttrType::Ref {
                            table: "Group".into(),
                        },
                    )],
                },
            ],
            transitions: vec![],
            inputs: vec![],
            outputs: vec![],
            views: vec![],
            grouped_views: vec![],
        }],
        wires: vec![],
        summaries: vec![],
    })
    .expect("aggregate resolution fixture must validate")
}

#[test]
fn all_false_aggregate_filters_do_not_resolve_the_target_reference_column() {
    let eval_model = aggregate_model(AttrType::Ref {
        table: "Group".into(),
    });
    let state_model = aggregate_model(AttrType::Int);
    let store = StateStore::new(
        &state_model,
        vec![
            TableInit::new("world", "Group", 1, vec![]),
            TableInit::new(
                "world",
                "Target",
                2,
                vec![ColumnInit::new("group", ColumnData::Int(vec![0, 0]))],
            ),
            TableInit::new(
                "world",
                "Query",
                1,
                vec![ColumnInit::new("group", ColumnData::Ref(vec![0]))],
            ),
        ],
    )
    .expect("mismatched fixture state must initialize against its own model");
    let params = ParamEnv::defaults(&eval_model);
    let snapshot = store.snapshot();

    for (op, expected) in [
        (AggOp::Count, ValueColumn::Int(vec![0])),
        (
            AggOp::Sum {
                value: Box::new(Expr::Int { value: 7 }),
            },
            ValueColumn::Int(vec![0]),
        ),
        (
            AggOp::Sum {
                value: Box::new(Expr::Real { value: 7.0 }),
            },
            ValueColumn::Real(vec![0.0]),
        ),
    ] {
        let mut cache = AggCache::new(&eval_model, &snapshot, &params);
        let result = eval_column(
            &Expr::Agg {
                op,
                table: "Target".into(),
                on: AggJoin {
                    fk_attr: "group".into(),
                    self_fk_attr: "group".into(),
                },
                filter: Box::new(Expr::Bool { value: false }),
            },
            EvalTable::new(&eval_model, "world", "Query").unwrap(),
            &snapshot,
            &params,
            &mut cache,
        )
        .expect("an all-false filter must not inspect the target FK column");
        assert_eq!(result, expected);
    }
}
