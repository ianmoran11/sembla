const EVAL_SOURCE: &str = include_str!("../src/eval.rs");

#[test]
fn typed_snapshot_accessors_do_not_regress_into_evaluator_row_loops() {
    for accessor in ["snapshot.real(", "snapshot.int(", "snapshot.enum_index("] {
        assert!(
            !EVAL_SOURCE.contains(accessor),
            "{accessor} must not be called from eval.rs; resolve the column before iterating rows"
        );
    }
    assert_eq!(
        EVAL_SOURCE.matches("snapshot.resolve_column(").count(),
        4,
        "the three SelfAttr types and EnumIs must each resolve their column once"
    );
}

#[test]
fn zero_row_typed_column_reads_remain_lazy() {
    for empty_case in [
        "AttrType::Real if row_count == 0",
        "AttrType::Int if row_count == 0",
        "AttrType::Enum { .. } if row_count == 0",
        "if row_count == 0 {\n                return Ok(InternalColumn::Bool(Vec::new()));",
    ] {
        assert!(
            EVAL_SOURCE.contains(empty_case),
            "missing lazy zero-row case: {empty_case}"
        );
    }
}
