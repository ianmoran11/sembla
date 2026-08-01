# PRD 0018: Bind the grouped-count proof to the IR evaluator

## Dependencies

PRDs 0001–0017 accepted.

Binding contract: [track README](../prds-lean-ir-formalization/README.md).

## Context

`Sembla.LumpingProof` proves target 1a for list-level naive and grouped counting. Current V1 has `Expr.agg count` but no separate group-by/broadcast plan constructor; grouped views are observation sinks and cannot be consumed by expressions.

## Goal

Close target 1b honestly by proving the actual checked `Expr.agg count` denotation equals a grouped implementation of the same relational count, then lift that refinement through expression, observation and trace evaluation.

## Requirements

1. Define an extensional naive relational-count reference and a grouped lookup implementation over arbitrary valid finite checked tables.
2. Prove their equality by reusing or transparently generalizing `groupedCount_eq_naiveCount`.
3. Prove the checked `Expr.agg count` evaluator equals the grouped implementation under named schema/key/filter hypotheses.
4. Lift the result through containing expressions and the user-observation projection at equal state boundaries.
5. State error-domain hypotheses precisely; do not use “compatible results” or an unnamed refinement relation.
6. Add actual raw/checked `Expr.agg count` fixtures and closed examples, while keeping the central theorem quantified.
7. Update `docs/prds-proof-track/README.md`, `DESIGN.md`, `docs/ROADMAP.md` and `frontend/README.md` to mark target 1b complete only after acceptance.
8. Do not invent a grouped-plan IR constructor or claim an optimizer implementation is verified.

## Allowed files

- `frontend/Sembla/Semantics/Lumping.lean`
- `frontend/Sembla/Semantics/LumpingTests.lean`
- `frontend/Sembla/LumpingProof.lean`
- `frontend/Sembla/Semantics.lean`
- `frontend/Sembla.lean`
- `docs/prds-proof-track/README.md`
- `DESIGN.md`
- `docs/ROADMAP.md`
- `frontend/README.md`
- `docs/design/lean-ir-coverage.md`

## Non-goals

- New optimizer/query-plan syntax.
- Grouped-view feedback or runtime equivalence.

## Test and proof guidance

Closed fixtures cannot satisfy acceptance without the quantified evaluator theorem and automated axiom evidence.

## Acceptance criteria

1. Target 1b is a theorem about the actual checked aggregate evaluator.
2. The named equality/refinement and hypotheses are exact and quantified over arbitrary valid finite inputs.
3. Target 1a remains historically accurate and is reused/generalized transparently.
4. Documentation avoids optimizer/runtime overclaiming.
5. Build, proof hygiene and full checks pass.
