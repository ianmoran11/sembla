# PRD 0020: Prove identity, canonicalization and structural export-data laws

## Dependencies

PRDs 0001–0019 accepted.

Binding contract: [track README](../prds-lean-ir-formalization/README.md).

## Context

Plan validity exists, but stable identity bijections, canonical ordering and the structural data passed to JSON export need separate proofs. Byte encoding and cryptographic implementation remain outside the assurance boundary.

## Goal

Prove direct-plan stable identity and canonicalization laws and connect a well-formed plan to its structural JSON-value representation.

## Requirements

1. Define named relations for semantic raw equality, canonical scientific equality and identity-preserving model isomorphism.
2. Prove direct-plan leaf/transition/wire/domain identity uniqueness and complete declaration/identity bijections.
3. Prove canonicalization idempotence and stability under the named identity-preserving source relation.
4. Cover both plan origins structurally, but prove construction-specific laws only for `directStablePlan`; linker-origin construction remains follow-on composition work.
5. Define or expose the structural JSON-value/export-data function and prove field/tag/list correspondence to `PlanWellFormed`.
6. Prove provenance-only fields are excluded from the named foundational behavioral projection; stable semantic identities are not provenance-only.
7. Preserve existing canonical byte/hash fixtures as regression tests, without claiming encoder/hash proofs.

## Allowed files

- `frontend/Sembla/Semantics/PlanIdentity.lean`
- `frontend/Sembla/Semantics/PlanIdentityTests.lean`
- `frontend/Sembla/PlanExport.lean`
- `frontend/Sembla/PlanJson.lean`
- `frontend/Sembla/PlanTests.lean`
- `frontend/Sembla/Semantics.lean`
- `frontend/Sembla.lean`
- `docs/design/lean-ir-coverage.md`
- `docs/design/lean-ir-semantics.md`

## Non-goals

- Byte-level JSON encoder correctness, SHA proofs or linker correctness.
- Runtime identity/refinement.

## Test and proof guidance

Keep structural/canonical equalities separate from pathwise behavioral equality. Every relation must be defined before theorem use.

## Acceptance criteria

1. Uniqueness, bijection, idempotence and named-relation stability theorems pass the audit.
2. Structural export-data correspondence covers every plan field.
3. Existing bytes/hashes are unchanged and accurately described as tests.
4. Build, proof hygiene, parity and full checks pass.
