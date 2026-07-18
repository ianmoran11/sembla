# PRD 0002: Declared views and summaries — IR and validator

## Context

`DESIGN.md` §4.6: what a run reports is part of the semantics and belongs in
the IR. Models declare named **views** (per-tick projections of committed
state) and **summaries** (scalars reduced over a run's views); the runner
emits declared observations generically and never knows what a model *means*.
The governing invariant: **observation is a sink** — no path from a view or
summary back to a parameter, input, hazard, transition, or wire. This PRD adds
the IR constructs and validation; PRD 0003 makes the runtime honor them and
deletes the hard-coded SIR branch. The NPE decision (`DECISIONS.md` §G5) makes
summaries load-bearing a second way: they are the conditioning data `x` the
posterior estimator trains on.

No feature flag is required (`DESIGN.md` §5.5 governs semantics *changes*):
by the sink invariant, observation cannot alter what a run computes, only what
it reports — and PRD 0003 tests that bitwise.

## Goal

`sembla-ir` models can declare views and summaries; the validator checks them;
golden fixtures freeze the encoding. No runtime behavior changes yet.

## Specification

- `Box` gains `views: Vec<ViewDecl>`; `Model` gains
  `summaries: Vec<SummaryDecl>`. Both default to empty on deserialization so
  every existing model file parses unchanged.
- `ViewDecl`: `name`, `table`, optional `filter` (Bool-typed `Expr` over one
  row of that table), `value` (Real- or Int-typed `Expr` over the row), and
  `reduce` ∈ {`sum`, `count`, `min`, `max`} (commutative monoids, `DESIGN.md`
  §4.2; `count` requires no `value`). Meaning: per tick, evaluate over the
  **committed post-tick state** and reduce to one scalar.
- `SummaryDecl`: `name`, `box`, `view`, `reduce` ∈ {`sum`, `min`, `max`,
  `last`, `argmax_tick`} folded over the run's ticks in tick order.
  `argmax_tick` yields the earliest tick achieving the maximum (deterministic
  ties).
- Validation: view names unique per box, summary names unique per model;
  table/attr references resolve; `filter` type-checks Bool and `value`
  numeric, using the existing expression type-checker; summaries reference an
  existing `box.view`. Reuse existing error conventions.
- **Sink invariant, structurally:** nothing in `Expr`, `Transition`, `Wire`,
  or port declarations can reference a view or summary — there is no syntax
  for it. State this in the rustdoc for both decl types, citing §4.6, and add
  a validator test asserting that view/summary names occupy a separate
  namespace (a transition referencing a view name as an attr still fails
  attr resolution).
- Wire-format discipline: follow the existing IR versioning convention for the
  schema change. The canonical serializer now emits the new fields; all
  checked-in `examples/*.json` and golden fixtures are re-canonicalized in
  this PRD (their `ir_hash` values change; update affected golden tests and
  say so in the implementation notes). `diff-ir` normalization handles the
  new fields.
- Add one new golden fixture: a small model exercising every `reduce` variant,
  a filtered view, and two summaries — round-trip tested byte-for-byte.

## Non-goals

Runtime evaluation and CSV output (PRD 0003). Lean surface syntax (PRD 0004).
Event streams, paged/windowed capture, adaptive triggers, external streaming
(`DESIGN.md` §4.6 exclusions). Group-by views (a view is one scalar per tick
in this PRD).

## Acceptance criteria

1. `cargo test --workspace` green; all prior tests still pass.
2. The new golden fixture round-trips byte-for-byte; existing fixtures are
   re-canonicalized and their round-trip tests pass.
3. Validator rejection tests: duplicate view name, duplicate summary name,
   unknown table, unknown attr in `filter`/`value`, non-Bool `filter`,
   `count` with a `value`, summary referencing a missing view — each with a
   distinct error message asserted.
4. Every existing example still validates; a views-free model validates and
   its behavior is untouched (no runtime change in this PRD).
5. `README.md` (root) IR-conventions section mentions views/summaries and the
   sink invariant in one sentence each.
