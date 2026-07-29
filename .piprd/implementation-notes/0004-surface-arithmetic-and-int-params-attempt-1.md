# PRD 0004 implementation notes — attempt 1

## Aggregate-in-effect determination

Effect-position aggregates are rejected at the surface with the positioned
message `aggregates are not supported in effect expressions`.

The runtime is structurally capable of evaluating them: in
`crates/sembla-runtime/src/executor.rs`, the `Effect::SetAttr` path builds every
effect column from the pre-commit `snapshot` through `eval_column` before it
queues writes (`executor.rs:745-780`). `eval_expr` dispatches both `Expr::Input`
and `Expr::Agg` (`crates/sembla-runtime/src/eval.rs:906-923`). However, a search
of runtime and integration tests found no `SetAttr` value containing either
aggregate expression kind. The PRD explicitly requires rejection when this
combination is unexercised, rather than exposing untested surface syntax. The
single shared expression elaborator still handles scalar guard, hazard, and
effect expressions; a small recursive gate rejects aggregate nodes only for the
effect context.

## Int parameter resolution

No Rust production fix was necessary. `param_value_from_json` already dispatches
`ParamType::Int` through `serde_json::Value::as_i64` to `ParamValue::Int`
(`crates/sembla-cli/src/main.rs:1448-1471`), and manifest conversion already
maps it to `ResolvedValue::Int`. The new CLI integration test pins the existing
path by overriding `retirement_months` and checking the integer value in
`resolved_theta`.

## Canonical runtime fixture

`crates/sembla-cli/tests/fixtures/arithmetic_int_increment.json` was emitted
from `Sembla.ArithmeticIntTests.incrementModel` through `Sembla.IR.toJson`, the
same canonical renderer used by `sembla-export`. An ignored, explicit
regeneration test rebuilds the Lean module, exports into a temporary path, and
compares bytes with the checked-in fixture; ordinary Rust-only checks do not
acquire a Lean dependency.

## Validation

- `./scripts/check.sh`
- `cd frontend && lake build`
- `cd frontend && bash scripts/test-negative.sh`
- `bash frontend/scripts/check-parity.sh`
- focused CLI runtime/Int-θ test and explicit ignored fixture-regeneration test
- temporary-index `git diff --cached --check`
