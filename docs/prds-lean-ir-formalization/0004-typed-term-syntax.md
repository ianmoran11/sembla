# PRD 0004: Define typed expressions, aggregates, effects and claims

## Dependencies

PRDs 0001–0003 accepted.

Binding contract: [track README](../prds-lean-ir-formalization/README.md).

## Context

The checked domains can now index syntax so unresolved names and type errors are absent from evaluators.

## Goal

Define complete intrinsically typed term-level syntax and exact erasure for current V1 expressions, aggregate operators, effects and contest claims.

## Requirements

1. Define typed expression constructors for literals, parameters, self attributes, arithmetic, comparisons, Boolean operators, enum tests, input aggregates and relational aggregates.
2. Represent numeric coercion explicitly; division returns Real; assignments require exact destination sort.
3. Define typed aggregate/input snapshot signatures, output fields, effects, resource claims and race/key order values.
4. Raw key-ordering remains representable even though current surface syntax exposes race-time only; record it as raw/checkable but not surface-producible.
5. Define erasure for every constructor with exact name/order preservation.
6. Prove result-sort uniqueness, erasure constructor fidelity and impossibility of ill-typed arithmetic/effects/claims.
7. Add one typed/erased fixture per constructor and compile-fail ill-typed examples.

## Allowed files

- `frontend/Sembla/Semantics/Syntax.lean`
- `frontend/Sembla/Semantics/SyntaxTests.lean`
- `frontend/Sembla/Semantics.lean`
- `frontend/Sembla.lean`
- `docs/design/lean-ir-coverage.md`

## Non-goals

- Raw checking or evaluation.
- Views/summaries as executable observations.
- Contest winner algorithms.

## Test and proof guidance

Keep typed syntax independent of any evaluator. Compile-fail examples supplement, but do not replace, uniqueness and erasure proofs.

## Acceptance criteria

1. Every current term/effect/claim constructor has typed syntax and exact erasure.
2. Type uniqueness and fidelity theorems pass the automated audit.
3. Ill-typed assignments, aggregate values and claim keys are unconstructable.
4. Build, proof hygiene and full checks pass.
