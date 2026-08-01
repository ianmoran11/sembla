# PRD 0011: Define finite table and reference semantics

## Dependencies

PRDs 0001–0010 accepted.

Binding contract: [track README](../prds-lean-ir-formalization/README.md).

## Context

Scalar evaluation needs concrete typed row/state access. Current V1 state is finite and fixed-shape; vacancy/generation are ordinary model attributes, not framework row retirement.

## Goal

Define deterministic finite table lookup and valid-reference behavior over supplied checked state.

## Requirements

1. Define extensional table/model state operations over PRD 0003 domains.
2. Define row lookup, attribute projection and typed reference dereference.
3. Valid checked state makes all stored references in-range and schema-matched; malformed supplied state produces explicit `invalidState`/`invalidReference` errors.
4. Prove lookup determinism, value typing, reference target typing, extensional equality and frame lemmas for unaffected tables/rows/attributes.
5. Prove no generic retirement/liveness predicate appears in the V1 semantic core.
6. Add fixtures for empty tables, bounds, cross-table references and malformed supplied state.

## Allowed files

- `frontend/Sembla/Semantics/StateEval.lean`
- `frontend/Sembla/Semantics/StateEvalTests.lean`
- `frontend/Sembla/Semantics.lean`
- `frontend/Sembla.lean`
- `docs/design/lean-ir-coverage.md`

## Non-goals

- Aggregation, state mutation or row allocation.
- Demographic vacancy/generation policy.

## Test and proof guidance

State mathematical equality extensionally; preserve raw order only through separate structural lemmas.

## Acceptance criteria

1. All finite state/reference operations have typed outcomes and fixtures.
2. Determinism, typing, extensionality and frame theorems pass the audit.
3. Invalid supplied state is explicit and no retirement feature is invented.
4. Build, proof hygiene and full checks pass.
