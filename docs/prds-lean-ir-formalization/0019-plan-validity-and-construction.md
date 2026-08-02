# PRD 0019: Define plan validity and prove direct-plan construction

## Dependencies

PRDs 0001–0018 accepted.

Binding contract: [track README](../prds-lean-ir-formalization/README.md).

## Context

The Lean-to-IR boundary includes `ExecutablePlanV1`. Composition-linked plan correctness is deferred, but common structural validity, raw wire validity and the current direct-stable constructor can be formalized now. PRD 0006 deliberately preserves wires without validating them. The constructor canonicalizes its input and does not itself perform model or wire checking, so its theorem must carry both validity hypotheses.

## Goal

Define `PlanWellFormed` for the current raw plan envelope and prove soundness of Lean direct-stable plan construction.

## Requirements

1. Define an independent `WiresWellFormed` judgment for `IR.Model.wires`. Every endpoint box resolves; a source port resolves in that box's output namespace; a target port resolves in that box's input namespace; source and target schemas are structurally exactly equal in attribute order, names, types, enum order and raw reference spellings; and each input endpoint has at most one source. One output may fan out. This is raw structural validity only, not delivery or linker behavior.
2. Cover schema/identity versions, feature declarations, embedded `ModelWellFormed ∧ WiresWellFormed` raw model, origin/provenance coupling, scheduler domain/algorithm and complete identity collections.
3. Cover source summaries/provenance structurally when present, without proving a linker produced them.
4. Implement terminating decidable structural checkers and prove soundness/completeness for `WiresWellFormed` and `PlanWellFormed`, with structured endpoint/schema/fan-in failures.
5. Refactor `directStablePlan` only as needed to expose a pure proof target.
6. Prove that if `ModelWellFormed inputModel`, `WiresWellFormed inputModel` and `directStablePlan inputModel` succeeds, the result is `PlanWellFormed`, its embedded model equals the existing `canonicalModel inputModel`, and canonicalization preserves both model-local and wire well-formedness. Do not claim the constructor rejects every ill-formed model.
7. Add both-origin validation fixtures, malformed envelope fixtures, direct-constructor fixtures and single-defect wire fixtures for unknown source/target boxes, wrong-direction or unknown ports, ordered schema mismatch and duplicate input fan-in. Include an accepted output fan-out fixture.

## Allowed files

- `frontend/Sembla/Semantics/PlanValidity.lean`
- `frontend/Sembla/Semantics/PlanValidityTests.lean`
- `frontend/Sembla/PlanExport.lean`
- `frontend/Sembla/PlanTests.lean`
- `frontend/Sembla/Semantics.lean`
- `frontend/Sembla.lean`
- `docs/design/lean-ir-coverage.md`

## Non-goals

- Proving the composition linker constructs valid plans or assigning wire delivery behavior.
- Stable identity/canonicalization theorems owned by PRD 0020.
- JSON bytes, hashing or runtime validation.

## Test and proof guidance

A linked-origin fixture tests common structural validation only and must not be presented as linker correctness.

## Acceptance criteria

1. `WiresWellFormed` owns every raw wire endpoint, direction, exact ordered schema and fan-in invariant; `PlanWellFormed` owns every common plan-envelope field and requires both model-local and wire validity for its embedded model.
2. Wire/plan checker soundness and completeness, canonicalization preservation of both judgments and the doubly hypothesis-qualified direct-constructor theorem pass the audit.
3. Origin/provenance, scheduler/feature and every single-defect wire error family have structured fixtures; output fan-out is accepted.
4. Existing direct-plan bytes remain unchanged; build, proof hygiene and full checks pass.
