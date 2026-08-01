# PRD 0002: Stabilize raw IR/plan coverage

## Dependencies

PRD 0001 accepted.

Binding contract: [track README](../prds-lean-ir-formalization/README.md).

## Context

`frontend/Sembla/IR.lean`, composition-source structures and `ExecutablePlanV1` are serialization-friendly raw contracts. The track cannot claim completeness without an exhaustive owned inventory.

## Goal

Create a machine-checked constructor/field classification and raw fixtures without changing public structures or bytes.

## Requirements

1. Add `Sembla.Semantics.Raw` as imports/aliases and exhaustive classifiers, avoiding duplicate raw definitions.
2. Add `docs/design/lean-ir-coverage.md` covering every current field/constructor in raw model, composition source, plan, identity and provenance structures.
3. Give each item one **primary owning PRD** plus any later theorem dependencies. Classify it as semantic, structural, observational, provenance-only or rejected by the Lean frontend.
4. Add total pattern matches that fail compilation when a covered inductive gains an unclassified constructor.
5. Add raw fixtures covering all variants, including priors, nested aggregates, grouped views, multi-claim transitions, key-ordering raw syntax, both plan origins and source summaries/version fields.
6. Preserve current canonical exports and bytes. Stop on a raw-contract discrepancy.

## Allowed files

- `frontend/Sembla/Semantics/Raw.lean`
- `frontend/Sembla/Semantics/RawTests.lean`
- `frontend/Sembla/Semantics.lean`
- `frontend/Sembla.lean`
- `docs/design/lean-ir-coverage.md`
- `docs/design/lean-ir-semantics.md`

## Non-goals

- Checked types, resolution or behavioral meaning.
- Raw schema changes.
- JSON/hash proofs.

## Test and proof guidance

Use exhaustive definitions and `#guard` fixtures. The coverage document must link each item to a concrete definition/test and primary PRD.

## Acceptance criteria

1. Every current field/constructor has exactly one primary owner and no unexplained classification.
2. Exhaustiveness checks catch a temporary unclassified sentinel/constructor change, followed by cleanup.
3. Existing canonical export fixtures remain unchanged.
4. Build, automated proof audit, proof hygiene and full checks pass.
