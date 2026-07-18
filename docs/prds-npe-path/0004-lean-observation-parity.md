# PRD 0004: Lean frontend observation parity

## Context

The IR gained views/summaries in PRD 0002; the Lean DSL must be able to
declare them or Lean-authored models cannot report anything. Two binding
rules: frontend correctness is **parity with already-proven fixtures**
(`DECISIONS.md` §I1), and **no inert syntax** (`DESIGN.md` §5.5) — the
frontend either fully elaborates observation declarations into the IR or
rejects them with a diagnostic; it never accepts-and-ignores.

## Goal

The Lean DSL declares views and summaries; the exporter emits IR
byte-identical to the PRD-0002/0003 fixtures; malformed declarations fail
elaboration with named errors.

## Specification

- Surface syntax for view and summary declarations, consistent with the
  existing DSL's style (the implementer designs the concrete syntax; it must
  cover: view name, source table, optional filter predicate, value
  expression or `count`, the four view reductions, and the five summary
  reductions including `argmax_tick`).
- Elaboration to `ViewDecl`/`SummaryDecl`, preserving declaration order
  (order is semantic for output columns, PRD 0003).
- Negative elaboration tests: unknown table, unknown attribute, non-Boolean
  filter, `count` with a value expression, summary referencing an undeclared
  view — each fails with a diagnostic naming the offending declaration.
- Parity: extend the Lean-authored SIR model with the `S`/`I`/`R` views and
  at least two summaries; its exported canonical JSON must equal the migrated
  `examples/sir.json` fixture byte-for-byte. The PRD-0002 golden fixture
  model (every reduce variant) is also authored in Lean and exported to
  byte-equality.
- Structure widgets are untouched; if the state-diagram widget breaks on
  models containing views, fix the props derivation, but no new widget is
  built.

## Non-goals

An observation/plot widget. Behavior widgets. Any IR change. Custom syntax
beyond what the IR constructs need.

## Acceptance criteria

1. The frontend's existing build and test entrypoints (see
   `frontend/README.md`) pass with the new tests included.
2. Byte-parity: both Lean-authored models export canonical JSON equal to
   their checked-in fixtures.
3. All negative elaboration tests fail elaboration with the specified
   diagnostics (asserted, not eyeballed).
4. `cargo test --workspace` is unaffected and green.
5. `frontend/README.md` documents the observation syntax with one example.
