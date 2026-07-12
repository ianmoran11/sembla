# PRD 0007: Composition — boxes, wires, one-tick delay

## Context

Composition is the feature v0.1 exists to prove (`DESIGN.md` §4.4, §9): boxes
are Moore machines with **table-typed ports**, wires carry a table per tick,
and everything — across wires and within boxes alike — observes a **uniform
one-tick delay** (read-old/write-new). The payoff to be demonstrated:
**moving a box boundary never changes observable semantics** (refactoring
invariance, theorem target #2 in §7).

## Goal

Extend the executor to multi-box models: build output tables at end of tick,
deliver them as input tables at the start of the *next* tick, evaluate
`Input` expressions against them — and prove boundary invariance with a
bitwise test.

## Specification

- Execution order per global tick: all boxes read their `Snapshot`s and
  input tables (delivered from the previous tick; empty tables at tick 0),
  run PRD-0006 steps 1–5 independently in box declaration order, then all
  boxes commit, then output tables for the next tick are built from the
  *committed* state per each `OutputDecl.builder` (v0.1 builders: per-table
  aggregate rows — e.g. `count where health = I` — per PRD 0002).
- `Input { port, agg }` in expressions evaluates aggregates over the current
  tick's received input table (Count/Sum over its columns, optional filter).
- Wires are delivered by schema-checked copy; no shared references between
  box states — boxes may not read each other's state, only wires
  (§4.4: no globals).
- RNG coordinates are model-global (`rule_id` is already model-wide per
  PRD 0002), so identically-declared transitions draw identical randomness
  regardless of which box they sit in — this is what makes the boundary
  invariance test possible at the bitwise level.
- The state hash (PRD 0004) extends canonically over multiple boxes (box
  declaration order) and in-flight wire tables.

## Non-goals

General wiring-diagram language, nested boxes (flat box list + wires is v0.1
composition — nesting is v0.2), cross-boundary entity-level messaging
(aggregate builders only in v0.1), cycles *within* a tick (impossible by
construction given the delay).

## Acceptance criteria

1. `cargo test --workspace` passes; all PRD-0006 single-box tests still pass
   (a single-box model is the degenerate multi-box case).
2. `examples/two_box.json` checked in: box A (population with one hazard
   transition whose rate is scaled by an `Input` aggregate) wired in a
   feedback loop with box B (a 1-row "controller" table whose transition
   reads A's infection count output and sets a modifier attribute exported
   back to A). Runs 50 ticks deterministically (hash-identical repeat runs).
3. **One-tick delay test**: a step change in box B's modifier at tick k
   affects box A's hazard at tick k+1, not k (asserted on fired counts with
   a deterministic construction, e.g. λ toggled between 0 and a huge value).
4. **Boundary invariance test (load-bearing)**: `examples/two_box.json` and a
   checked-in hand-merged single-box equivalent `examples/two_box_merged.json`
   (same tables, same transitions in the same declaration order, wires
   replaced by same-schema internal aggregate reads with the same one-tick
   delay) produce **bitwise-identical per-table state hashes at every tick
   for 50 ticks** under the same seed. The test documents in comments why
   the construction aligns rule_ids and entity_ids.
5. Tick-0 semantics (empty input tables) is explicit, documented, and covered
   by a test.
6. `sembla run` executes multi-box models and reports per-box fired counts.
