# PRD 0010: Define scalar expression semantics

## Dependencies

PRDs 0001–0009 accepted.

Binding contract: [track README](../prds-lean-ir-formalization/README.md).

## Context

The actual frontend constructs exact raw IR through the proved PRD 0007–0009 pure builders, and their final assembly theorem establishes successful `checkModel` elaboration with exact checked erasure. This semantics layer consumes the resulting checked terms; it does not treat frontend-builder success or raw syntax as semantic input. The first meaning layer interprets scalar/local expressions while aggregates and inputs remain typed context operations owned by PRD 0012.

## Goal

Define deterministic type-preserving evaluation for literals, parameters, self attributes, arithmetic, comparisons, Booleans and enum tests with explicit errors.

## Requirements

1. Define a closed scalar `EvalError` and typed evaluation outcomes. Do not name
   it as an extensible track-wide `SemanticError`: later accepted phases own
   closed local errors, and combining APIs preserve exact child errors through
   explicit wrapper sums. Semantic evaluation errors remain distinct from
   frontend builder, declaration-checker and model-checker diagnostics.
2. Define an evaluation context indexed by the checked term's actual `RowScope`.
   It must provide typed parameter access, the scope-appropriate typed current-row
   identity/projection for either table or input scope, and abstract typed
   aggregate/input services; it may not assume every term is evaluated in an
   owning-table row.
3. Interpret exact scientific literals in `ℝ` and explicit `Int → Real`
   coercions.
4. Evaluate subexpressions left-to-right; division by zero returns the frozen
   scalar error.
5. Cover every owned constructor explicitly, including literals, parameters,
   `self`, arithmetic, comparisons, Boolean operations, enum literals/tests,
   `intToReal`, `input` and `agg`; the latter two delegate to the typed context
   interface.
6. Prove determinism, result-type preservation, outcome totality, scope safety
   and context extensionality.
7. Add fixtures for every constructor and error branch, with distinct table- and
   input-scope contexts.

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

1. Every owned expression constructor, including `enum`, `intToReal`, `input`
   and `agg`, has semantics and fixtures.
2. Determinism, type preservation, total outcome, scope safety and extensionality
   pass the audit.
3. Error order is pinned by tests and definitions.
4. Build, proof hygiene and full checks pass.
