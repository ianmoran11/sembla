# PRD 0011: Validate and evaluate finite table/reference state

## Dependencies

PRDs 0001–0010 accepted.

Binding contract: [track README](../prds-lean-ir-formalization/README.md).

## Context

PRD 0003 provides schema-indexed valid state, its extensional laws and an
explicitly unvalidated supplied-state carrier. Scalar evaluation needs a proved
conversion at that boundary plus concrete typed row/state access. Current V1
state is finite and fixed-shape; vacancy/generation are ordinary model
attributes, not framework row retirement.

## Goal

Validate supplied state into the PRD 0003 checked domain and define
deterministic finite table lookup and valid-reference behavior over successfully
validated state.

## Requirements

1. Define deterministic validation from PRD 0003's unvalidated supplied-state
   carrier into its schema-indexed valid state. Validate table/column shape,
   exact `sizeHint` row counts, scalar sorts, enum bounds, reference targets and
   reference row bounds with explicit `invalidState`/`invalidReference`
   categories and stable structural paths.
2. Define extensional table/model state operations over the PRD 0003 valid
   domains, including row lookup, attribute projection and typed reference
   dereference.
3. Successfully validated state makes stored values schema-typed and all
   references target-matched and in range. Malformed supplied state remains
   representable and returns the explicit validation errors rather than relying
   on impossible construction of a valid state.
4. Prove supplied-state validator soundness and completeness for a declarative
   validity relation. Prove lookup determinism, value typing, reference target
   typing, congruence under PRD 0003 state extensionality and frame lemmas for
   unaffected tables/rows/attributes. Consume rather than redefine PRD 0003's
   foundational row/table/model-state extensional equality laws.
5. Prove no generic retirement/liveness predicate appears in the V1 semantic
   core.
6. Add fixtures for empty tables, wrong row counts, wrong scalar/column types,
   enum bounds, cross-table references, out-of-range references and valid
   forward-reference state.

## Allowed files

- `frontend/Sembla/Semantics/StateEval.lean`
- `frontend/Sembla/Semantics/StateEvalTests.lean`
- `frontend/Sembla/Semantics.lean`
- `frontend/Sembla.lean`
- `docs/design/lean-ir-coverage.md`

## Non-goals

- Redefining the scalar/schema/state domains or their foundational
  extensionality laws from PRD 0003.
- Aggregation, state mutation or row allocation.
- Demographic vacancy/generation policy.
- Binary state-artifact parsing or Rust/runtime refinement.

## Test and proof guidance

State validation independently from lookup so malformed supplied data never
needs to inhabit the valid dependent state. Use PRD 0003 extensional equality
for congruence and frame results; preserve raw declaration order only through
its separate structural lemmas.

## Acceptance criteria

1. The supplied-state validator decides exactly the declarative validity
   relation and produces PRD 0003 valid state on success.
2. All finite state/reference operations have typed outcomes and fixtures.
3. Validator soundness/completeness, lookup determinism, typing, extensional
   congruence and frame theorems pass the audit.
4. Invalid supplied state is explicit and no retirement feature is invented.
5. Build, proof hygiene and full checks pass.
