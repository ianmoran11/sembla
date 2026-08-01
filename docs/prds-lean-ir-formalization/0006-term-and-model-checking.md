# PRD 0006: Check terms and assemble checked models

## Dependencies

PRDs 0001–0005 accepted.

Binding contract: [track README](../prds-lean-ir-formalization/README.md).

## Context

Declaration checking provides resolved contexts. The remaining raw expressions, transitions, observations and model-level references must be elaborated to typed syntax and assembled into one checked model.

## Goal

Complete raw-to-checked model elaboration with proved soundness, completeness and exact erasure.

## Requirements

1. Define declarative judgments for expressions, aggregate scopes, effects, contests, outputs, views, grouped views, summaries and structurally valid wire endpoints/schemas.
2. Implement terminating elaboration into typed terms and checked declarations using PRD 0005 contexts.
3. Enforce guard/filter Boolean types, hazard Real type, aggregate/input scope, exact effect destination type, Ref-write claim requirements and orderable compatible contest keys.
4. Assemble `Checked.Model` and define `ModelWellFormed` as the complete current foundational judgment.
5. Prove term-checker soundness/completeness, checked result-sort uniqueness, whole-model checker soundness/completeness and exact raw erasure.
6. Prove checking the erasure of a checked model returns an equivalent checked model under a named structural equivalence defined here.
7. Add positive fixtures for every constructor and negative fixtures for every term/model error category.

## Allowed files

- `frontend/Sembla/Semantics/CheckTerms.lean`
- `frontend/Sembla/Semantics/CheckModel.lean`
- `frontend/Sembla/Semantics/CheckModelTests.lean`
- `frontend/Sembla/Semantics.lean`
- `frontend/Sembla.lean`
- `docs/design/lean-ir-coverage.md`

## Non-goals

- Evaluation or frontend macro refactoring.
- Composition-source checking/linking.
- Behavioral meaning for wires.

## Test and proof guidance

Define checked-model equivalence before using it in round-trip theorems. Do not weaken exact raw erasure by introducing normalization.

## Acceptance criteria

1. `checkModel` decides exactly `ModelWellFormed` for the owned V1 model fragment.
2. Soundness, completeness, type uniqueness, exact erasure and checked round-trip theorems pass the audit.
3. Constructor/error fixture coverage is complete.
4. Build, proof hygiene and full checks pass.
