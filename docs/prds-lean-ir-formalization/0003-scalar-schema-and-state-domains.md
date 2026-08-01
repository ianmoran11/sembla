# PRD 0003: Define scalar, schema and finite-state domains

## Dependencies

PRDs 0001–0002 accepted.

Binding contract: [track README](../prds-lean-ir-formalization/README.md).

## Context

Reasoning needs typed values, resolved finite names and valid finite state before typed expression syntax can be introduced.

## Goal

Define the foundational checked domains and prove their lookup/type invariants, without defining expression or transition behavior.

## Requirements

1. Define scalar sorts and values for mathematical reals, integers, Booleans, finite enums and typed table references.
2. Define unique parameter/table/attribute schemas and finite resolved identifiers.
3. Define finite row ordinals, typed rows and model state. References are table-matched in-range ordinals; do not add active/retired-row machinery.
4. Retain declaration order separately from extensional finite lookup where later outputs expose order.
5. Define erasure to raw parameter/table/schema structures.
6. Prove value-sort uniqueness, resolved lookup uniqueness, finite-index bounds, impossible cross-schema references and exact schema/state erasure laws.
7. Add positive examples for every sort and compile-fail examples for cross-schema values/references.

## Allowed files

- `frontend/Sembla/Semantics/Types.lean`
- `frontend/Sembla/Semantics/State.lean`
- `frontend/Sembla/Semantics/TypesTests.lean`
- `frontend/Sembla/Semantics.lean`
- `frontend/Sembla.lean`
- `docs/design/lean-ir-coverage.md`

## Non-goals

- Typed expression syntax or checking.
- State evolution, row allocation, vacancy or retirement.
- Priors as distributions.

## Test and proof guidance

Prefer finite/dependent types that make invalid references unconstructable. Do not use runtime row-liveness conventions.

## Acceptance criteria

1. All scalar/schema/state coverage items owned by this PRD have definitions and exact erasure.
2. Required uniqueness, bounds and cross-schema impossibility theorems pass the automated axiom audit.
3. Invalid examples fail by typing rather than dynamic assertions.
4. Build, proof hygiene and full checks pass.
