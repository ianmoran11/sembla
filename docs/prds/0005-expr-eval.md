# PRD 0005: Kernel expression evaluator

## Context

A Sembla tick is a bulk relational kernel (`DESIGN.md` §4.2): per-row
expressions plus group-by aggregates joined through declared Ref attributes.
The evaluator runs against the read `Snapshot` (PRD 0004) only. The key
performance/semantics idea to honor: the `Agg` form ("count rows sharing my
`employer` where `health = I`") is evaluated by **group-by then broadcast**
— aggregate once per group, not once per row — which is the exact-lumping
rewrite of `DESIGN.md` §7 applied by construction.

## Goal

An `eval` module in `sembla-runtime` that type-checks against the validated
IR (types were established in PRD 0002) and evaluates any v0.1 `Expr` for
every row of a table in one pass, deterministically.

## Specification

- `fn eval_column(expr, table, snapshot, params, agg_cache) -> ValueColumn`
  where `ValueColumn` is `Vec<f64> | Vec<i64> | Vec<bool> | Vec<u16>` and
  `params` is a resolved `ParamEnv` (θ): declared defaults overlaid with any
  per-run overrides, resolved once before tick 0 — `Expr::Param` evaluates
  as a constant from it (`DESIGN.md` §4.1; values are never inlined into
  the IR).
- Per-row forms (literals, `SelfAttr`, arithmetic, comparisons, boolean ops,
  `EnumIs`) evaluate rowwise with f64 IEEE semantics, no fast-math, no
  reassociation — evaluation order is the expression tree order (Level A
  determinism, §5.2).
- `Agg { op, table, on, filter }`: build (or reuse from `agg_cache`) a
  group-keyed accumulator over the target table — one sequential pass in row
  order (canonical order for Level A), accumulating Count/Sum per group key
  (the Ref value) — then broadcast to querying rows through their own Ref
  attr. Sums accumulate in row order (documented as the canonical CPU
  reduction order).
- `agg_cache` keyed by the aggregate's structure so identical aggregates in
  multiple transitions are computed once per tick.
- Division by zero: f64 semantics (inf/nan) — allowed, documented; guards
  should be written to avoid it, the runtime does not police it in v0.1.
- `Input { port, agg }` may be stubbed to "port tables are empty" until
  PRD 0007 wires composition in — but the code path must exist and be typed.

## Non-goals

Parallel evaluation (single-threaded is fine for the v0.1 oracle — do not
add rayon; deterministic parallelism is a later concern), transition firing
(PRD 0006), incremental/DBSP evaluation.

## Acceptance criteria

1. `cargo test --workspace` passes.
2. Unit tests cover every `Expr` variant, including nested arithmetic over
   `SelfAttr` and `EnumIs` inside `filter`, and `Param` resolution: the same
   expression evaluated under two different `ParamEnv`s yields the
   correspondingly different columns, with defaults applying when no
   override is given.
3. **Lumping equivalence test** (the §7 example, load-bearing): on a
   ~1000-row two-table fixture (Person→Employer Ref, random-ish but hardcoded
   data), a naive O(n²) reference implementation of the `Agg` count (written
   inside the test) matches the group-by evaluator exactly, row for row.
4. Aggregate caching test: two transitions sharing an identical `Agg`
   trigger one accumulator build (observable via a counter or cache
   inspection).
5. Determinism test: evaluating the same expression twice over the same
   snapshot yields identical `ValueColumn`s, including a Sum aggregate over
   f64 values chosen so that reordering would change the result (e.g. values
   of wildly different magnitudes) — asserting the canonical order is real.
6. Evaluator has no access to `WriteBuffer` (enforced by signature: it takes
   `&Snapshot`).
