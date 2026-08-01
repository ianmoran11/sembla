# PRD 0008: Add pure transition, effect and contest builders

## Dependencies

PRDs 0001–0007 accepted.

Binding contract: [track README](../prds-lean-ir-formalization/README.md).

## Context

Transitions combine typed guards/hazards, effects and resource claims. Keeping their construction pure makes the user frontend rely on the same checker contract later used by semantics.

## Goal

Build model-local transition declarations through pure functions with proved checker acceptance and exact erasure.

## Requirements

1. Add builders for typed/raw expressions used by transitions, transitions, set-attribute effects and current contest declarations.
2. Preserve the current surface restriction to race-time contests while retaining checked support for raw key ordering.
3. Enforce guard/hazard/effect/claim constraints through typed inputs or checker results, not duplicated ad hoc rules.
4. Prove successful builder outputs are accepted and erase exactly.
5. Add direct fixtures for ordinary transitions, multiple effects, multiple claims and every rejection category.
6. Leave macros unchanged until PRD 0009.

## Allowed files

- `frontend/Sembla/Frontend/Builders/Transition.lean`
- `frontend/Sembla/Frontend/Builders/TransitionTests.lean`
- `frontend/Sembla/Frontend/Builders.lean`
- `frontend/Sembla.lean`
- `docs/design/lean-ir-coverage.md`

## Non-goals

- Transition execution or contest winners.
- Key-ordering surface syntax.
- Composition/wire builders.

## Test and proof guidance

Use checker theorems from PRD 0006 rather than reproving typing independently.

## Acceptance criteria

1. Every current model-local transition/effect/contest form has a pure builder or explicit surface rejection.
2. Soundness and erasure theorems pass the audit.
3. Fixtures include multi-claim transitions without choosing their later winner semantics.
4. Build, proof hygiene and full checks pass.
