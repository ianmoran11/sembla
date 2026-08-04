# PRD 0012: Define aggregate and output-materialization semantics

## Dependencies

PRDs 0001–0011 accepted.

Binding contract: [track README](../prds-lean-ir-formalization/README.md).

## Context

The scalar evaluator delegates aggregate/input operations to a typed interface. This PRD supplies finite-table meanings and output materialization against an abstract input snapshot; composition will later construct those snapshots.

## Goal

Complete relational/input aggregate evaluation and the deterministic, type-preserving materialization of checked output declarations. Frontend construction of raw output builders remains closed in PRD 0009.

## Requirements

1. Define `InputSnapshot` as exactly one finite table for every checked
   `InputId`, each with exactly its declared schema and an arbitrary finite row
   cardinality supplied by the semantic environment; do not infer cardinality
   from `IR.PortDecl` and do not define wire delivery.
2. Define filters, count, numeric sum, current foreign-key/self-key joins and
   input aggregates.
3. Close PRD 0010's abstract service boundary: construct concrete typed
   aggregate/input evaluator services and input-scoped contexts from
   `ValidModelState`, parameters, an `InputSnapshot` and the scope-appropriate
   typed table/input row cursor. Support every checked aggregate nesting admitted
   by PRD 0006 through a structurally well-founded evaluator design, and prove
   `.agg`/`.input` evaluation agrees with these definitions in every supported
   `RowScope`.
4. Use source row order operationally but prove count/sum results invariant under
   a named traversal-order permutation that preserves the same typed `RowId`
   identities and expression meaning. A stronger model isomorphism must define
   explicit reference transport and may not compete with PRD 0014's model
   relation.
5. Define the denotation and materialization of checked output declarations
   corresponding to raw `IR.OutputBuilder` values, from an explicitly supplied
   state boundary. Every checked output materializes exactly one row, with
   fields in checked/source order and exactly its declared schema. Do not add or
   reopen frontend builder construction. The later handoff fixes that
   composition supplies post-commit state.
6. Propagate left-to-right semantic errors and define empty count/sum as their
   correctly typed zero.
7. Prove determinism, type preservation, scope correctness, evaluator-service
   agreement, permutation laws and exact output cardinality/schema preservation.
8. Add empty, filtered, joined, nested-relational, table/input-scope,
   input-snapshot, error and one-row output fixtures.

## Allowed files

- `frontend/Sembla/Semantics/Aggregate.lean`
- `frontend/Sembla/Semantics/Output.lean`
- `frontend/Sembla/Semantics/AggregateTests.lean`
- `frontend/Sembla/Semantics.lean`
- `frontend/Sembla.lean`
- `docs/design/lean-ir-coverage.md`

## Non-goals

- Views/summaries or wire/mailbox semantics.
- General query optimization.

## Test and proof guidance

Name the permutation relation and hypotheses explicitly; declaration-order fixtures do not prove invariance.

## Acceptance criteria

1. Every aggregate/input/output constructor has semantics and fixtures.
2. Determinism, typing, scope, service-agreement, permutation and exact
   one-row-schema theorems pass the audit.
3. Input evaluation depends only on the supplied snapshot interface, and the
   concrete services close every PRD 0010 aggregate/input callback.
4. Build, proof hygiene and full checks pass.
