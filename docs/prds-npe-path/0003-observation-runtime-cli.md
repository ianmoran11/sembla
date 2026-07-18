# PRD 0003: Observation runtime and SIR-branch retirement

## Context

`DESIGN.md` §4.6 records v0.1's known violation: the CLI branches on a
hard-coded SIR box name to pick its output columns, and `sembla sweep` refuses
models that are not SIR-shaped. Declared views/summaries (PRD 0002) are the
fix, and **retiring that branch is their acceptance test** — acceptance is
deletion. The sink invariant must hold bitwise: enabling, disabling, or
filtering observation cannot change state, draws, conflict resolution, or any
scheduling decision.

## Goal

The runtime evaluates declared views per tick and summaries per run; the CLI
reports every model generically; `optional_sir_box_name`, the SIR CSV branch,
and the sweep refusal are deleted; the SIR examples keep byte-identical output
through declared views.

## Specification

- `sembla-runtime`: evaluate each box's views against the committed post-tick
  state, in declaration order, returning `f64`/`i64` scalars in the tick
  report; fold summaries across ticks (tick order; `argmax_tick` ties →
  earliest). Evaluation must not touch the RNG, the conflict engine, or state
  — enforce by taking `&StateStore`.
- CLI `run` output for a model **with views**: keep the `# params=` and
  `# dt=` header lines, then
  `tick,<view columns in declaration order>,<fired_<transition_name> per rule
  in rule order>,deferred_total`. Summaries go to `<out>.summaries.csv`
  (`name,value`, declaration order), hashed into stdout alongside the
  existing hashes and recorded in the PRD-0001 manifest as an
  `observation` hash field (append-only, beside its algorithm ID).
- A model with **no views** keeps the existing generic state-count/firing CSV
  (it is model-agnostic; document that it is the no-views default, not a
  special case).
- Migrate `examples/sir.json` and `examples/sir_policy.json`: declare views
  `S`, `I`, `R` (count of `person` rows where `health` equals the variant) so
  the run CSV reproduces the legacy
  `tick,S,I,R,fired_infect,fired_recover,deferred_total` output
  **byte-for-byte** (transition names already yield the legacy fired-column
  names). Check in the previous output as a golden file and diff against it.
- `sembla sweep`: delete the "requires a SIR box" refusal. Per-draw series and
  `summary.csv` quantiles are computed over the model's reported per-tick
  columns, whatever they are. The SIR sweep fixtures keep byte-identical
  output.
- Delete `optional_sir_box_name`, `sir_box_name`, `run_sir_results_csv`,
  `sir_counts`, and their tests. Add a guard test asserting no string literal
  naming a model box (`"sir"`, `"population"`) survives in `sembla-cli`
  source outside `#[cfg(test)]` fixtures and docs.
- Sink test (the §4.6 invariant, bitwise): run the SIR example, then run a
  copy with views/summaries removed, then a copy with an extra view added —
  all three produce identical per-tick `final_state_sha256` sequences and
  identical firing behavior; only reported columns differ.

## Non-goals

Lean surface syntax (PRD 0004). `(θ, x)` export (PRD 0006). Event streams or
paging. New view kinds beyond PRD 0002. Changing canonical-model CSV formats
(they are views-free and keep the generic default).

## Acceptance criteria

1. `cargo test --workspace` green; all prior tests still pass.
2. Golden test: migrated `examples/sir.json` run output is byte-identical to
   the checked-in legacy CSV; `sir_policy` and the SIR sweep fixtures
   likewise.
3. Deletion verified: the four symbols are gone; the guard test passes; no
   model name appears in `sembla-cli` (ROADMAP v0.3 exit criterion 1).
4. Sink test as specified: adding, removing, or disabling observation leaves
   every per-tick state hash bitwise unchanged.
5. `sembla sweep` completes on a views-free canonical model (previously
   refused) and its output is deterministic across two invocations.
6. Summaries: an integration test computes `peak_I = max(I)` and
   `peak_tick = argmax_tick(I)` on a small SIR run and matches hand-computed
   values from the per-tick CSV.
7. `docs/examples/sir.md` and `docs/examples/canonical-models.md` updated
   (views declaration, summaries file, the no-views default).
