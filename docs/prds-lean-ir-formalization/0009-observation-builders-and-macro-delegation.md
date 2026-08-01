# PRD 0009: Add observation builders and delegate thin macros

## Dependencies

PRDs 0001–0008 accepted.

Binding contract: [track README](../prds-lean-ir-formalization/README.md).

## Context

Core and transition builders exist, but the real frontend path remains macro-driven. This final builder slice covers model-local outputs/views/summaries and moves semantic construction behind the pure APIs.

## Goal

Complete model-local pure builders and make current macros thin parsing/diagnostic adapters while preserving public syntax and export fixtures.

## Requirements

1. Add pure builders for input/output declarations, output fields/builders, scalar views, grouped views and summaries.
2. Prove checker acceptance and exact erasure for each builder.
3. Refactor `frontend/Sembla/DSL.lean` so parameter/table/model, transition/effect/contest and observation semantic decisions delegate to PRDs 0007–0009 builders.
4. Preserve syntax parsing and translate structured builder/checker failures to existing diagnostic categories/positions.
5. Existing composition/wire macros may remain compatibility-tested adapters; do not introduce composition checking or proof claims.
6. Preserve canonical model/export bytes and all positioned negative tests.
7. Document parsing, macro expansion and diagnostic rendering as trusted/tested rather than verified.

## Allowed files

- `frontend/Sembla/Frontend/Builders/Observation.lean`
- `frontend/Sembla/Frontend/Builders/ObservationTests.lean`
- `frontend/Sembla/Frontend/Builders.lean`
- `frontend/Sembla/DSL.lean`
- `frontend/Sembla/CommandFrontendTests.lean`
- `frontend/Sembla.lean`
- `frontend/README.md`
- `docs/design/lean-ir-coverage.md`

## Non-goals

- Verifying Lean metaprograms.
- Composition-source/linker refactoring.
- Behavioral semantics.

## Test and proof guidance

Run direct builder tests and the existing elaboration/export suites. No canonical fixture regeneration is permitted without an explicit schema decision.

## Acceptance criteria

1. All current model-local semantic construction delegates to pure proved builders.
2. Builder theorems pass the audit; macros are documented honestly.
3. Positive/negative elaboration and canonical exports are unchanged.
4. Build, proof hygiene, parity and full checks pass.
