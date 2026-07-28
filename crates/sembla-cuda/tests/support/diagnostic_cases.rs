use std::path::{Path, PathBuf};

use sembla_runtime::state::{ColumnData, ColumnInit, TableInit};

pub const FAILING_ROWS: [usize; 3] = [2, 5, 7];
pub const GEOMETRIES: [(u32, u32); 4] = [(1, 1), (1, 32), (3, 4), (4, 128)];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CaseKind {
    ClaimKey,
    TransitionGuard,
    Effect,
    Output,
}

#[derive(Clone, Copy, Debug)]
pub struct DiagnosticCase {
    pub name: &'static str,
    pub model_path: &'static str,
    pub kind: CaseKind,
    pub expected_status: (u64, u64),
    pub expected_cpu_error: &'static str,
    pub expected_cuda_error: &'static str,
}

pub const CASES: [DiagnosticCase; 4] = [
    DiagnosticCase {
        name: "claim-key-overflow",
        model_path: "fixtures/validation-negative/claim_key_overflow.json",
        kind: CaseKind::ClaimKey,
        expected_status: (10, 2),
        expected_cpu_error: "integer arithmetic overflow at row 2",
        expected_cuda_error: "candidate 2 claim expression overflowed Int",
    },
    DiagnosticCase {
        name: "transition-guard-overflow",
        model_path: "fixtures/validation-negative/transition_guard_overflow.json",
        kind: CaseKind::TransitionGuard,
        expected_status: (3, 2),
        expected_cpu_error: "integer arithmetic overflow at row 2",
        expected_cuda_error: "candidate 2 overflowed Int",
    },
    DiagnosticCase {
        name: "effect-int-overflow",
        model_path: "fixtures/validation-negative/effect_int_overflow.json",
        kind: CaseKind::Effect,
        expected_status: (5, 2),
        expected_cpu_error: "integer arithmetic overflow at row 2",
        expected_cuda_error: "candidate 2 effect overflowed Int",
    },
    DiagnosticCase {
        name: "output-expression-overflow",
        model_path: "fixtures/validation-negative/output_expression_overflow.json",
        kind: CaseKind::Output,
        expected_status: (9, 1),
        expected_cpu_error: "integer arithmetic overflow at row 2",
        expected_cuda_error: "wire output field 1 overflowed Int",
    },
];

pub fn repository_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("sembla-cuda must live under crates/")
        .to_path_buf()
}

pub fn load_model(case: &DiagnosticCase) -> sembla_ir::ValidatedModel {
    let source = std::fs::read_to_string(repository_root().join(case.model_path))
        .unwrap_or_else(|error| panic!("{}: failed to read model: {error}", case.name));
    sembla_ir::validate(sembla_ir::parse_json(&source).unwrap())
        .unwrap_or_else(|error| panic!("{}: invalid model: {error}", case.name))
}

pub fn initial_state(case: &DiagnosticCase) -> Vec<TableInit> {
    let failing = vec![1, 1, i64::MAX, 1, 1, i64::MAX, 1, i64::MAX];
    match case.kind {
        CaseKind::ClaimKey => vec![TableInit::new(
            "world",
            "Person",
            8,
            vec![
                ColumnInit::new("mate", ColumnData::Ref(vec![0; 8])),
                ColumnInit::new("priority", ColumnData::Int(failing)),
            ],
        )],
        CaseKind::TransitionGuard => vec![TableInit::new(
            "world",
            "Person",
            8,
            vec![ColumnInit::new("x", ColumnData::Int(failing))],
        )],
        CaseKind::Effect => vec![TableInit::new(
            "world",
            "Person",
            8,
            vec![
                ColumnInit::new("x", ColumnData::Int(failing)),
                ColumnInit::new("y", ColumnData::Int(vec![0; 8])),
            ],
        )],
        CaseKind::Output => vec![TableInit::new(
            "source",
            "Person",
            8,
            vec![ColumnInit::new("x", ColumnData::Int(failing))],
        )],
    }
}
