# PRD 0007: Add pure parameter, table and model builders

## Dependencies

PRDs 0001–0006 accepted.

Binding contract: [track README](../prds-lean-ir-formalization/README.md).

## Context

Semantic construction currently occurs inside macros. This first builder slice covers declaration contexts without crossing into transition or composition behavior.

## Goal

Provide pure builders for parameters, table schemas and model shells, with proofs that successful results are accepted by the checker.

## Requirements

1. Add pure structured builder errors separate from macro diagnostics.
2. Add builders for parameters/defaults/priors, attributes/enums/refs, tables and model metadata including exact positive `dt`.
3. Return raw fragments accepted by PRD 0005 or checked fragments that erase exactly to the intended raw forms.
4. Prove builder soundness and exact erasure for every successful constructor.
5. Add direct builder fixtures matching current canonical frontend declarations.
6. Do not refactor macros yet; PRD 0009 performs the coordinated delegation after all model-local builders exist.

## Allowed files

- `frontend/Sembla/Frontend/Builders/Core.lean`
- `frontend/Sembla/Frontend/Builders/CoreTests.lean`
- `frontend/Sembla/Frontend/Builders.lean`
- `frontend/Sembla.lean`
- `docs/design/lean-ir-coverage.md`

## Non-goals

- Transition/effect/contest or observation builders.
- Macro changes.
- Wires/composition sources.

## Test and proof guidance

Compare erasures with existing raw fixture values, not only pretty-printed forms. All preservation theorems enter the automated audit.

## Acceptance criteria

1. Every owned builder succeeds exactly for the documented checked fragment.
2. Builder soundness and erasure theorems pass the audit.
3. Fixtures cover successful and rejected construction.
4. Existing frontend behavior is untouched; build and full checks pass.
