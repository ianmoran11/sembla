# PRD 0021: Complete the coverage audit and composition handoff

## Dependencies

PRDs 0001–0020 accepted.

Binding contract: [track README](../prds-lean-ir-formalization/README.md).

## Context

The foundation must close with adversarial evidence, not an assumption that accumulated modules cover V1. Composition is deliberately deferred until these definitions and relations exist.

## Goal

Demonstrate complete foundational Lean-side coverage, audit proof/trust claims, and write an implementation-ready scope charter for a separate future composition formalization track without adding composition semantics now.

## Requirements

1. Complete `docs/design/lean-ir-coverage.md`: every current field/constructor must link to raw classification, checked representation/rejection, checker theorem, semantics/structural erasure, fixtures and proof owner.
2. Add aggregate compile-time coverage guards so a new constructor cannot bypass classification/tests.
3. Run the automated theorem/axiom inventory and audit for hidden opaque propositions, forbidden declarations and undocumented classical assumptions.
4. Add `docs/design/lean-composition-formalization-scope.md` documenting, without Lean declarations:
   - checked source coverage for components, instances, bindings, summaries, boundaries, wires, schema/linker/identity versions, required/enabled features, `outer_dt`, and global tau-leap scheduler restrictions;
   - post-commit output materialization, next-tick delivery, unwired-empty inputs, fan-out, single-source inputs and delayed feedback;
   - independent hierarchical-source and flat-plan denotations using the foundation;
   - static preservation, one-tick correspondence and finite-trace preservation theorem decomposition;
   - the existing full pathwise observation fields; and
   - literal identity preservation for linking versus boundary-refactoring theorems modulo an explicit bijective renaming of leaf/transition/wire/mailbox/draw identities.
5. Record later, separate extension questions for ideal CTMC semantics and model algebra; do not add interfaces or constructors in Lean.
6. Correct maintained documentation so every claim is labeled proved, tested, trusted or future.
7. Obtain independent review for missing constructors, circular definitions, runtime leakage and overclaiming; resolve blockers.
8. Update this track's status only after all evidence is present. Do not enqueue a composition PRD automatically.

## Allowed files

- `frontend/Sembla/Semantics/CoverageAudit.lean`
- `frontend/Sembla/Semantics/ProofAudit.lean`
- `frontend/Sembla/Semantics.lean`
- `frontend/Sembla.lean`
- `frontend/scripts/check-proofs.sh`
- `scripts/check.sh`
- `docs/design/lean-ir-coverage.md`
- `docs/design/lean-ir-semantics.md`
- `docs/design/lean-composition-formalization-scope.md`
- `docs/design/README.md`
- `DESIGN.md`, `DECISIONS.md`, `docs/ROADMAP.md`, `docs/overview.md`, `frontend/README.md`
- this track's `README.md`
- `docs/prds/TRACKS.md`

## Non-goals

- Composition/linker semantic definitions or proofs.
- CTMC/model-algebra implementation.
- Runtime, float, RNG or JSON implementation verification.

## Test and proof guidance

Exercise coverage guards with a temporary sentinel, then clean it completely. Reviewers inspect definitions/theorem statements, not only green commands.

## Acceptance criteria

1. No current V1 field/constructor lacks an owner, invariant/meaning and evidence.
2. The theorem inventory satisfies the exact proof policy with no omission.
3. The composition charter is precise enough to draft fine-grained follow-on PRDs but adds no composition semantics.
4. Independent review reports no blocker or overclaim.
5. Build, proof hygiene and full checks pass.
