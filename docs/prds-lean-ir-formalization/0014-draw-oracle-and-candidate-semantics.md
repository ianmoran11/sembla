# PRD 0014: Define draw-oracle and transition-candidate semantics

## Dependencies

PRDs 0001–0013 accepted.

Binding contract: [track README](../prds-lean-ir-formalization/README.md).

## Context

Transitions read one immutable snapshot and use addressed draws to determine
race times. Candidate generation consumes `Checked.Model` and its intrinsically
typed checked transition packs, never raw builder requests. This is mathematical
pathwise tau-leap meaning, not concrete RNG or float behavior.

## Goal

Define deterministic candidate generation, stable semantic draw coordinates and explicit hazard/draw errors.

## Requirements

1. Define model-local `TransitionIdentity` as the declaration-stable pair
   `(box name, transition name)` and row identity as its retained table owner plus
   typed `RowId`. Relate the transition identity to the existing encoded plan
   identity `occurrence(box.name) ++ "#" ++ transition.name`; PRD 0020 owns the
   later export-data correspondence. Checked source ordinals are retained only as
   provenance and must not enter identity.
2. Define `DrawCoordinate = (absolute tick, TransitionIdentity, row identity,
   draw index)` and an abstract oracle returning mathematical reals. Candidate
   evaluation checks `0 < u ∧ u < 1`; failure produces the local `invalidDraw`
   error.
3. Define a named identity-preserving model permutation/isomorphism relation and
   its action on coordinates. State invariance through an identity-indexed
   projection rather than literal equality of differently ordered provenance
   lists.
4. Traverse boxes, transitions and rows in preserved source order for evaluation
   and first-error precedence. Evaluate guard before hazard. False guard consults
   no draw; zero hazard does not fire; negative hazard is an error.
5. For positive hazard use `-log(u)/h`; a candidate exists exactly for strict
   `raceTime < dt`, with `dt` already proved positive.
6. Record consulted coordinates, stable identity, source-ordinal provenance,
   race data, pre-tick row/input evaluation context and the checked transition's
   exact source-ordered claim list needed by contests/traces.
7. Prove fixed-oracle determinism, race positivity/bounds, coordinate stability
   under the named relation, irrelevant-declaration invariance and context
   extensionality.
8. Add boundary, invalid-draw, zero/negative hazard, false-guard and multi-row
   fixtures, plus multiple boxes with unequal transition counts and insertion or
   reordering of an unrelated transition to prove identity stability.

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
2. All identity, coordinate, race, source-order/error and extensionality
   theorems pass the audit.
3. Strict `dt` boundary and all error branches are fixture-pinned.
4. Build, proof hygiene and full checks pass.
