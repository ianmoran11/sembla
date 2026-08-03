# PRD 0012: Define aggregate and output-materialization semantics

## Dependencies

PRDs 0001–0011 accepted.

Binding contract: [track README](../prds-lean-ir-formalization/README.md).

## Context

The scalar evaluator delegates aggregate/input operations to a typed interface. This PRD supplies finite-table meanings and output materialization against an abstract input snapshot; composition will later construct those snapshots.

## Goal

Complete relational/input aggregate evaluation and the deterministic, type-preserving materialization of checked output declarations. Frontend construction of raw output builders remains closed in PRD 0009.

## Requirements

1. Define typed input snapshots as finite tables supplied by the semantic environment; do not define wire delivery.
2. Define filters, count, numeric sum, current foreign-key/self-key joins and input aggregates.
3. Use source row order operationally but prove count/sum results invariant under named row permutations when expression meaning is permutation-stable.
4. Define the denotation and materialization of checked output declarations corresponding to raw `IR.OutputBuilder` values, from an explicitly supplied state boundary. Do not add or reopen frontend builder construction. The later handoff fixes that composition supplies post-commit state.
5. Propagate left-to-right semantic errors and define empty count/sum as zero.
6. Prove determinism, type preservation, scope correctness, permutation laws and output-schema preservation.
7. Add empty, filtered, joined, input-snapshot, error and output fixtures.

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
2. Determinism, typing, scope, permutation and schema theorems pass the audit.
3. Input evaluation depends only on the supplied snapshot interface.
4. Build, proof hygiene and full checks pass.
