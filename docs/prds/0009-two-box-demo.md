# PRD 0009: Two-box feedback demo — SIR + policy controller

## Context

The v0.1 composition deliverable (`DESIGN.md` §9): the SIR population box
wired in a feedback loop with a small policy box that reads aggregate
infections and feeds back a contact-rate modifier. This exercises table-typed
ports, the one-tick delay, traced/feedback structure, and "a composed system
is a system" — on the real model rather than the PRD-0007 toy. It is also the
first real **common-random-numbers counterfactual** (`DESIGN.md` §5.3) demo.

## Goal

A checked-in two-box SIR+policy model, a CRN counterfactual comparison
utility, and documentation that walks a reader through both.

## Specification

- `examples/sir_policy.json`: box `population` = PRD-0008 SIR, with the
  `infect` hazard multiplied by an `Input` aggregate `restriction_modifier`
  (defaulting to 1.0 at tick 0 — document tick-0 semantics per PRD 0007).
  Box `policy`: a 1-row table with attrs `mode: Enum{Open, Restricted}` and
  `modifier: Real`; transitions switch mode when the received infection count
  crosses thresholds up/down (hysteresis: trigger-on and trigger-off
  thresholds differ, so it doesn't oscillate every tick), setting `modifier`
  to 1.0 / 0.4 respectively. Wires: population infection count → policy;
  policy modifier → population.
- Thresholded policy transitions should fire *deterministically* when their
  condition holds — use the defined semantics for that (a hazard large
  enough that `p ≈ 1` per tick is acceptable if documented, or an explicit
  `Immediate` hazard convenience added to the IR with
  `t = 0` race time — implementer's choice; if the IR gains `Immediate`,
  update PRD-0002 golden fixtures and validator tests accordingly).
- `sembla compare --population ... --seed ... --ticks ... --out compare.csv`
  supporting two contrast shapes under the **same seed** (CRN — identical
  coordinates ⇒ identical draws, `DESIGN.md` §5.3):
  - **model contrast**: `sembla compare <modelA.json> <modelB.json> ...`
  - **parameter contrast**: `sembla compare <model.json> --params-a a.json
    --params-b b.json ...` — same IR, two θ vectors (paired sensitivity /
    prior-predictive contrasts).
  Both emit per-tick side-by-side aggregates and their differences, with the
  resolved θ of each arm echoed in the header.
- `docs/examples/sir_policy.md`: what the feedback loop does, the one-tick
  delay caveat (§10.7), and a CRN comparison of the policy model vs. the
  no-policy PRD-0008 model, with the interpretation (differences are
  attributable to policy, same shocks hit both runs).

## Non-goals

More than two boxes, entity-level cross-boundary messages, plotting,
statistical analysis of the comparison beyond the CSV.

## Acceptance criteria

1. `cargo test --workspace` passes, including all earlier PRDs' tests
   (if `Immediate` was added, PRD-0002 fixtures/tests updated coherently).
2. Integration test at 100k persons: the policy box switches to `Restricted`
   within a documented tick range under fixed seed/parameters, and the
   epidemic's peak infected count is lower than the no-policy PRD-0008 run
   under the same seed (CRN paired comparison inside the test).
3. Hysteresis test: over 200 ticks the policy mode changes at most a
   documented small number of times (no per-tick oscillation).
4. Determinism: `sembla compare` run twice produces byte-identical output;
   the comparison CSV's per-run columns for the *baseline* model are
   identical to a standalone PRD-0008 run at the same seed (CRN sanity: the
   baseline is unaffected by being run alongside a variant).
5. One-tick delay is asserted: the tick the policy fires and the first tick
   the population's effective hazard changes differ by exactly one (test).
6. Parameter-contrast test: `sembla compare` on the PRD-0008 SIR model with
   `beta` lowered in arm B (same seed) shows a strictly lower final attack
   rate in arm B, and repeat invocations are byte-identical.
7. `docs/examples/sir_policy.md` exists with runnable commands, including
   one parameter-contrast example.
