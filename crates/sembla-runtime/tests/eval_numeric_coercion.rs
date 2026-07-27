const EVAL_SOURCE: &str = include_str!("../src/eval.rs");

#[test]
fn owned_real_numeric_coercion_moves_its_buffer_without_cloning() {
    let function = EVAL_SOURCE
        .split_once("fn numeric_as_real(")
        .expect("numeric_as_real must remain present")
        .1
        .split_once("\n}\n")
        .expect("numeric_as_real must remain a standalone function")
        .0;

    assert!(
        function.starts_with("column: InternalColumn) -> Result<Vec<f64>, EvalError>"),
        "numeric_as_real must consume its owned column"
    );
    assert!(
        function.contains("InternalColumn::Real(values) => Ok(values)"),
        "the Real arm must move the owned Vec<f64> out of the column"
    );
    assert!(
        !function.contains("clone("),
        "numeric_as_real must not clone an owned numeric column"
    );
    assert!(
        function.contains("values[row] as f64"),
        "Int-to-Real coercion must retain its exact as-f64 conversion"
    );
    assert_eq!(
        EVAL_SOURCE.matches("numeric_as_real(").count(),
        7,
        "all six owned operand conversions and the function definition must remain visible"
    );
    assert!(
        !EVAL_SOURCE.contains("numeric_as_real(&"),
        "callers must pass ownership rather than borrowing and cloning"
    );
}
