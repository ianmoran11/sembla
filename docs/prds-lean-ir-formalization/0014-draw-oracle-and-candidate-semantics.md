# PRD 0014: Define draw-oracle and transition-candidate semantics

## Dependencies

PRDs 0001–0013 accepted.

Binding contract: [track README](../prds-lean-ir-formalization/README.md).

## Context

Transitions read one immutable snapshot and use addressed draws to determine race times. This is mathematical pathwise tau-leap meaning, not concrete RNG or float behavior.

## Goal

Define deterministic candidate generation, stable semantic draw coordinates and explicit hazard/draw errors.

## Requirements

1. Define `DrawCoordinate = (tick, transition identity, row identity, draw index)` and an abstract oracle returning mathematical reals. Candidate evaluation checks `0 < u ∧ u < 1`; failure produces `invalidDraw`.
2. Define a named identity-preserving model permutation/isomorphism relation and its action on coordinates.
3. Evaluate guard before hazard. False guard consults no draw; zero hazard does not fire; negative hazard is an error.
4. For positive hazard use `-log(u)/h`; a candidate exists exactly for strict `raceTime < dt`, with `dt` already proved positive.
5. Record consulted coordinates and candidate identity/race data needed by contests/traces.
6. Prove fixed-oracle determinism, race positivity/bounds, coordinate stability under the named relation, irrelevant-declaration invariance and context extensionality.
7. Add boundary, invalid-draw, zero/negative hazard, false-guard and multi-row fixtures.

## Allowed files

- `frontend/Sembla/Semantics/Random.lean`
- `frontend/Sembla/Semantics/Candidate.lean`
- `frontend/Sembla/Semantics/CandidateTests.lean`
- `frontend/Sembla/Semantics.lean`
- `frontend/Sembla.lean`
- `docs/design/lean-ir-coverage.md`
- `docs/design/lean-ir-semantics.md`

## Non-goals

- Philox, probability distributions or CTMC laws.
- Contests/effects/commit.

## Test and proof guidance

Record axioms for Mathlib logarithm lemmas through the automated audit. Do not encode dense runtime ordinals.

## Acceptance criteria

1. Candidate outcomes are deterministic for fixed inputs/oracle.
2. All coordinate, race and extensionality theorems pass the audit.
3. Strict `dt` boundary and all error branches are fixture-pinned.
4. Build, proof hygiene and full checks pass.
