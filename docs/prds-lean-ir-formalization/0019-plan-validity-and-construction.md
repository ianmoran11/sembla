# PRD 0019: Define plan validity and prove direct-plan construction

## Dependencies

PRDs 0001–0018 accepted.

Binding contract: [track README](../prds-lean-ir-formalization/README.md).

## Context

The Lean-to-IR boundary includes `ExecutablePlanV1`. Composition-linked plan correctness is deferred, but common structural validity and the current direct-stable constructor can be formalized now. The constructor canonicalizes its input and does not itself perform model checking, so its theorem must carry checked-model validity as a hypothesis.

## Goal

Define `PlanWellFormed` for the current raw plan envelope and prove soundness of Lean direct-stable plan construction.

## Requirements

1. Cover schema/identity versions, feature declarations, embedded checked-valid raw model, origin/provenance coupling, scheduler domain/algorithm and complete identity collections.
2. Cover source summaries/provenance structurally when present, without proving a linker produced them.
3. Implement a decidable structural checker and prove its soundness/completeness for `PlanWellFormed`.
4. Refactor `directStablePlan` only as needed to expose a pure proof target.
5. Prove that if `ModelWellFormed inputModel` and `directStablePlan inputModel` succeeds, the result is `PlanWellFormed`, its embedded model equals the existing `canonicalModel inputModel`, and canonicalization preserves model well-formedness. Do not claim the constructor rejects every ill-formed model.
6. Add both-origin validation fixtures, malformed envelope fixtures and direct-constructor fixtures.

## Allowed files

- `frontend/Sembla/Semantics/PlanValidity.lean`
- `frontend/Sembla/Semantics/PlanValidityTests.lean`
- `frontend/Sembla/PlanExport.lean`
- `frontend/Sembla/PlanTests.lean`
- `frontend/Sembla/Semantics.lean`
- `frontend/Sembla.lean`
- `docs/design/lean-ir-coverage.md`

## Non-goals

- Proving the composition linker constructs valid plans.
- Stable identity/canonicalization theorems owned by PRD 0020.
- JSON bytes, hashing or runtime validation.

## Test and proof guidance

A linked-origin fixture tests common structural validation only and must not be presented as linker correctness.

## Acceptance criteria

1. `PlanWellFormed` owns every common plan-envelope field.
2. Checker soundness/completeness, canonicalization preservation and the hypothesis-qualified direct-constructor theorem pass the audit.
3. Origin/provenance and scheduler/feature errors have fixtures.
4. Existing direct-plan bytes remain unchanged; build and full checks pass.
