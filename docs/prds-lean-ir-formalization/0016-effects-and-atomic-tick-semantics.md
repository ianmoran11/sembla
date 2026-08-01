# PRD 0016: Define effects, simultaneous commit and atomic ticks

## Dependencies

PRDs 0001–0015 accepted.

Binding contract: [track README](../prds-lean-ir-formalization/README.md).

## Context

Candidate and contest results are read from one snapshot. Effects must also evaluate against that snapshot, then commit together so no within-tick cascade or list-order race exists.

## Goal

Define accepted write sets, explicit conflicts, simultaneous state commit and one complete non-composed tick.

## Requirements

1. Evaluate every accepted candidate's effects against the pre-tick state.
2. Represent writes by resolved destination row/attribute and typed value.
3. Detect two accepted writes to the same destination as `conflictingWrites`; do not choose by list order.
4. Commit conflict-free writes simultaneously and preserve every unaffected location.
5. Define one atomic tick from checked model, state, parameters, abstract input snapshot and draw oracle to next state/events or explicit error.
6. Prove write typing, conflict freedom on successful commit, frame laws, simultaneous-commit characterization, no read-after-write cascade, state well-formedness preservation and fixed-oracle tick determinism.
7. Add no-fire, uncontested, contested loser, multiple-effect, disjoint-write and conflicting-write fixtures.

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
2. Typing, frame, atomicity, no-cascade, preservation and determinism theorems pass the audit.
3. Conflicts are explicit and never resolved incidentally.
4. Build, proof hygiene and full checks pass.
