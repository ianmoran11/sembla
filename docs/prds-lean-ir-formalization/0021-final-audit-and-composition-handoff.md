# PRD 0021: Complete the coverage audit and composition handoff

## Dependencies

PRDs 0001–0020 accepted.

Binding contract: [track README](../prds-lean-ir-formalization/README.md).

## Context

The foundation must close with adversarial evidence, not an assumption that accumulated modules cover V1. Composition is deliberately deferred until these definitions and relations exist.

## Goal

Demonstrate complete foundational Lean-side coverage, audit proof/trust claims, and write an implementation-ready scope charter for a separate future composition formalization track without adding composition semantics now.

## Requirements

1. Complete `docs/design/lean-ir-coverage.md`. Every non-composition field/constructor must link to both raw classifications, checked representation/rejection, checker theorem, semantics/structural erasure, fixtures and proof owner. Builder-owned forms must additionally link to the PRD 0007–0009 pure-builder soundness/completeness/failure evidence, source-order/attachment evidence and final-assembly `checkModel`/exact-erasure theorem. Record executable evidence that current model-local macros delegate to that final assembly boundary, and classify parsing, token-to-path mapping and positioned diagnostic rendering explicitly as trusted/tested rather than proved. Composition-source-only fields/constructors must instead link to both raw classifications, fixtures, their explicit future-composition owner and the exact deferred obligation in the composition handoff charter; they must not claim a current-track checked representation, checker theorem or semantics that this track deliberately excludes.
2. Add aggregate compile-time coverage guards so neither a new inductive constructor nor a new structure field can bypass classification/tests.
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

Exercise coverage guards with a temporary inductive constructor and a temporary structure field, then clean both completely. Reviewers inspect definitions/theorem statements and deferred-composition rows, not only green commands.

## Acceptance criteria

1. No current V1 field/constructor lacks an owner and evidence. Every non-composition item has a current-track invariant/meaning; every builder-owned item additionally has pure-construction, final-assembly and macro-delegation evidence with the trusted diagnostic boundary stated exactly; every composition-source-only item has an explicit deferred invariant/meaning in the future-composition handoff and makes no false current-track proof claim.
2. The theorem inventory satisfies the exact proof policy with no omission.
3. The composition charter is precise enough to draft fine-grained follow-on PRDs but adds no composition semantics.
4. Independent review reports no blocker or overclaim.
5. Build, proof hygiene and full checks pass.
