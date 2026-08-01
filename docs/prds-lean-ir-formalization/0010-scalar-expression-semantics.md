# PRD 0010: Define scalar expression semantics

## Dependencies

PRDs 0001–0009 accepted.

Binding contract: [track README](../prds-lean-ir-formalization/README.md).

## Context

Checked syntax exists and the actual frontend constructs it through proved pure functions. The first meaning layer interprets scalar/local expressions while aggregates and inputs remain typed context operations owned by PRD 0012.

## Goal

Define deterministic type-preserving evaluation for literals, parameters, self attributes, arithmetic, comparisons, Booleans and enum tests with explicit errors.

## Requirements

1. Define `SemanticError` categories and typed evaluation outcomes.
2. Define an evaluation context for parameters, current row attributes and abstract typed aggregate/input services.
3. Interpret exact scientific literals in `ℝ` and explicit numeric coercions.
4. Evaluate subexpressions left-to-right; division by zero returns the frozen error.
5. Cover literals, parameters, self attributes, arithmetic, comparisons, Boolean operations and enum tests; delegate aggregate/input nodes to the typed context interface.
6. Prove determinism, result-type preservation, outcome totality and context extensionality.
7. Add fixtures for every constructor and error branch.

## Allowed files

- `frontend/Sembla/Semantics/Eval.lean`
- `frontend/Sembla/Semantics/EvalTests.lean`
- `frontend/Sembla/Semantics.lean`
- `frontend/Sembla.lean`
- `docs/design/lean-ir-coverage.md`

## Non-goals

- Table traversal, transitions or draws.
- Float/transcendental implementation behavior.

## Test and proof guidance

Do not inherit Lean's total `x / 0` behavior; the evaluator must implement the explicit error contract.

## Acceptance criteria

1. Every owned expression constructor has semantics and fixtures.
2. Determinism, type preservation, total outcome and extensionality pass the audit.
3. Error order is pinned by tests and definitions.
4. Build, proof hygiene and full checks pass.
