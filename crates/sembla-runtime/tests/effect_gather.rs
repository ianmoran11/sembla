const EVAL_SOURCE: &str = include_str!("../src/eval.rs");
const EXECUTOR_SOURCE: &str = include_str!("../src/executor.rs");

fn section<'a>(source: &'a str, start: &str, end: &str) -> &'a str {
    let (_, tail) = source
        .split_once(start)
        .unwrap_or_else(|| panic!("missing section start: {start}"));
    let (body, _) = tail
        .split_once(end)
        .unwrap_or_else(|| panic!("missing section end: {end}"));
    body
}

#[test]
fn effects_use_gathered_winner_rows_with_full_column_fallbacks() {
    let staging = section(
        EXECUTOR_SOURCE,
        "let mut winner_rows = None",
        "\n    let mut fired = transitions",
    );
    assert!(staging.contains("eval_gather("));
    assert!(staging.contains("eval_typed_ref_gather("));
    assert!(staging.contains("eval_column("));
    assert!(staging.contains("eval_typed_ref_column("));
    assert!(staging.contains("winner_offset"));
    assert!(staging.contains("candidate.row"));
}

#[test]
fn gather_eligibility_reuses_the_conservative_row_infallible_predicate() {
    let gather = section(
        EVAL_SOURCE,
        "pub fn eval_gather(",
        "\n/// Evaluates a Ref-typed root expression",
    );
    assert!(gather.contains("prepare_gather("));

    let predicate = section(
        EVAL_SOURCE,
        "fn expr_is_row_infallible(",
        "\nfn prepare_node",
    );
    assert!(predicate.contains("Expr::Input { .. } | Expr::Agg { .. } => false"));
    assert!(predicate.contains("== RuntimeType::Real"));
}
