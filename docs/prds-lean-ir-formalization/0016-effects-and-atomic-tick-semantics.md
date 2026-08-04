# PRD 0016: Define effects, simultaneous commit and atomic ticks

## Dependencies

PRDs 0001–0015 accepted.

Binding contract: [track README](../prds-lean-ir-formalization/README.md).

## Context

Candidate and contest results are read from one snapshot. Effects must also evaluate against that snapshot, then commit together so no within-tick cascade or list-order race exists.

## Goal

Define accepted write sets, explicit conflicts, simultaneous state commit and one complete non-composed tick.

## Requirements

1. Evaluate every accepted candidate's source-ordered effects against the same
   retained pre-tick state/input context. Preserve effect source order and
   source-ordinal provenance for evaluation/errors/events; it must never resolve
   a write conflict.
2. Prove the static-to-dynamic Ref-write bridge: the checked effect RHS and its
   structurally matched claim resource evaluate to the same typed reference in
   that context, an accepted Ref write's candidate won that evaluated resource,
   and deferred/losing candidates emit no writes.
3. Represent writes by resolved destination row/attribute and typed value.
4. Detect two accepted writes to the same destination as the local
   `conflictingWrites` error; detection is invariant under write-list
   permutation and must not choose by list order.
5. Commit conflict-free writes simultaneously and preserve every unaffected
   location.
6. Define one atomic tick from checked model, state, parameters, abstract input
   snapshot and draw oracle to next state/events or an explicit wrapper sum that
   preserves the exact candidate, contest, effect and commit errors.
7. Prove write typing, claim/writer authorization, losing-candidate suppression,
   conflict freedom on successful commit, frame laws, simultaneous-commit
   characterization, no read-after-write cascade, state well-formedness
   preservation and fixed-oracle tick determinism.
8. Add no-fire, uncontested, accepted claimed-Ref write, losing claimed-Ref
   write, multiple-effect, disjoint-write, and two accepted writes to one Ref
   destination/conflicting-write fixtures.

## Allowed files

- `frontend/Sembla/Semantics/Effect.lean`
- `frontend/Sembla/Semantics/Tick.lean`
- `frontend/Sembla/Semantics/TickTests.lean`
- `frontend/Sembla/Semantics.lean`
- `frontend/Sembla.lean`
- `docs/design/lean-ir-coverage.md`

## Non-goals

- Multi-tick traces, wires or outputs delivery.
- Runtime scheduling/parallelism.

## Test and proof guidance

State frame and atomicity extensionally over finite state. Permutation fixtures do not replace theorems.

## Acceptance criteria

1. Every effect field has pathwise meaning and fixtures.
2. Typing, claim/write authorization, loser suppression, frame, atomicity,
   no-cascade, preservation and determinism theorems pass the audit.
3. Conflicts are explicit and never resolved incidentally.
4. Build, proof hygiene and full checks pass.
