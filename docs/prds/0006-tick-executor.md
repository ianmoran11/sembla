# PRD 0006: Tick executor — racing clocks and conflict resolution

## Context

This is the semantic heart of the runtime (`DESIGN.md` §4.3, §5.1). Ideal
semantics: a CTMC — every enabled transition runs an exponential clock, the
earliest wins. Executed semantics: **tau-leaping** — rates frozen at tick
start, everything sampled inside the tick window fires, contested resources
resolved by argmin, losers re-race next tick, no within-tick cascades. All
randomness through the PRD-0003 coordinate API; all reads from the PRD-0004
`Snapshot`; all writes to the `WriteBuffer`; expressions via PRD 0005.

## Goal

`run_tick(model, state, params, seed, tick) -> TickReport` executing one
single-box tick end-to-end, bitwise-deterministically, plus a `run(model,
state, params, seed, n_ticks)` driver and the saturation diagnostic —
where `params` is the resolved θ (`ParamEnv`, PRD 0005), fixed for the whole
run.

## Specification

Per tick, in this order:

1. **Freeze**: take the `Snapshot`.
2. **Evaluate**: for each transition (in `rule_id` order): guard column,
   hazard column (λ per model-time unit).
3. **Sample races**: for each row with guard true and λ > 0, sample
   `t = exp_f64(seed, tick, rule_id, entity_id, draw_idx=0, λ)`. The
   transition is a *candidate* iff `t < model.dt`.
4. **Resolve conflicts** (`DESIGN.md` §5.1): group candidates by contested
   resource (evaluate each claim's `resource` expr to a (table, row) key).
   Per resource, winner = argmin by ordering key — `RaceTime` uses `t`,
   `Key(expr)` uses the evaluated key — with the deterministic lexicographic
   tie-break `(key, rule_id, entity_id)`. Non-contesting candidates all fire.
   A transition claiming multiple resources must win **all** of them.
   Resolution must be implemented as a sort or sequential min-scan in
   canonical order (never via hash-map grouping order).
5. **Apply effects**: winners' `Effect` lists write to the `WriteBuffer`.
   Two winners writing the same cell is impossible by construction when
   claims are declared correctly; the executor must nonetheless detect a
   same-cell double-write and return an error naming both transitions (this
   is the §5.1 coverage check's runtime backstop).
6. **Commit** and emit `TickReport { tick, fired: Vec<(rule_id, count)>,
   deferred_per_resource_table: Vec<(table, deferred_count)> }` — the
   deferred counts are the **saturation diagnostic** (§5.1, §6): losers that
   were candidates but lost a race. The `run` driver logs a warning when any
   table's deferred count exceeds 10% of its fired count.

## Non-goals

Multi-box execution and inputs (PRD 0007), scheduled clocks / non-exponential
durations, birth/death, parallelism (single-threaded oracle), Levels B/C.

## Acceptance criteria

1. `cargo test --workspace` passes.
2. **Analytic hazard test**: one table, 100k rows, single transition
   `Calm → Agitated` at constant λ with dt such that p = 1 − exp(−λ·dt) ≈
   0.1: over one tick the fired count is within 3σ of the binomial
   expectation; over many ticks the survival curve matches exp(−λt) within
   3σ per tick.
3. **Determinism test**: 50 ticks on `examples/two_state.json` (1000 rows,
   seeded initial data) run twice from scratch ⇒ identical state hash
   (PRD 0004) at every tick, and identical `TickReport`s.
4. **Conflict test**: a fixture where two "hire" transitions contest the same
   worker rows (RaceTime ordering): exactly one wins per contested worker per
   tick; a hand-computed 3-row micro-case (fixed seed, expected winners
   precomputed from the RNG in the test itself) matches exactly; tie-break
   path covered by a case with equal keys.
5. **Key-ordered conflict test**: same fixture with `Key(expr)` (FIFO-style
   ordering by an attribute): the lowest-key candidate wins regardless of
   race times.
6. **Double-write backstop test**: a deliberately mis-declared model
   (two transitions writing one row's attr without a shared claim) produces
   the defined error, not silent last-write-wins.
7. Saturation diagnostic appears in `TickReport` and the >10% warning fires
   in a test with an oversubscribed resource.
8. `sembla run <model.json> --seed N --ticks K --population <spec>` runs the
   two-state example and prints per-tick fired counts (population spec format
   is the implementer's choice — simplest thing that works; PRD 0008 replaces
   it).
