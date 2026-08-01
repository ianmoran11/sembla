# PRD 0001: Freeze the semantic charter, module map and proof policy

## Dependencies

None. The track README is binding.

Binding contract: [track README](../prds-lean-ir-formalization/README.md).

## Context

The raw Lean IR has no complete meaning function, while maintained prose distinguishes ideal CTMC semantics from executable tau-leap behavior. Later PRDs require fixed semantic choices, a deliberate Mathlib dependency and an objective proof gate before definitions land.

## Goal

Create the maintained charter and module skeleton, pin Mathlib, and automate the track's proof/axiom policy without claiming that semantics exists yet.

## Requirements

1. Add `docs/design/lean-ir-semantics.md`, copying the README's accepted architecture and frozen decision table with citations to current code/decisions. Stop on a conflict instead of resolving it silently.
2. Document the distinction among raw validity, checked values, dynamic semantic errors, macro diagnostics, pathwise tau-leap meaning and deferred ideal CTMC meaning.
3. Add a reviewed exact module map for PRDs 0002–0021. If it differs from their allowed-file lists, amend those PRDs before enqueueing 0002.
4. Add a Mathlib version compatible with the pinned Lean toolchain to `frontend/lakefile.toml` and lock it in `frontend/lake-manifest.json`.
5. Add empty importable `Sembla.Semantics` and `Sembla.Frontend.Builders` umbrellas; do not add placeholder propositions.
6. Extend proof hygiene and add `frontend/Sembla/Semantics/ProofAudit.lean` plus a deterministic command that enumerates every theorem/lemma in covered namespaces and rejects axioms outside `{propext, Classical.choice, Quot.sound}`.
7. Mechanically ban the tokens/declaration forms listed in the README proof policy for covered modules.
8. Update maintained design/frontend indexes only to describe the proposed architecture and proof gate accurately.

## Allowed files

- `frontend/lakefile.toml`
- `frontend/lake-manifest.json`
- `frontend/Sembla.lean`
- `frontend/Sembla/Semantics.lean`
- `frontend/Sembla/Semantics/ProofAudit.lean`
- `frontend/Sembla/Frontend/Builders.lean`
- `frontend/scripts/check-proofs.sh`
- `scripts/check.sh`
- `docs/design/lean-ir-semantics.md`
- `docs/design/README.md`
- `DESIGN.md`, `DECISIONS.md`, `docs/ROADMAP.md`, `frontend/README.md`
- this track's README and numbered PRDs only if the reviewed module map requires path amendments

## Non-goals

- Semantic domains, evaluators or theorem placeholders.
- Changing raw IR, syntax or exports.
- Composition or runtime work.

## Test and proof guidance

Demonstrate that temporary forbidden declarations and a theorem depending on an unapproved axiom make the audit fail, then remove them completely. Build once from a clean dependency resolution where practical.

## Acceptance criteria

1. The charter freezes every README decision or stops on a documented conflict.
2. Mathlib resolves reproducibly and `cd frontend && lake build` passes.
3. The automated audit covers all named theorems/lemmas in the declared namespaces and enforces the exact policy.
4. Proof-guard negative self-tests are recorded and cleaned up.
5. No document claims that semantics has already been completed.
6. From the repository root, proof hygiene and `bash scripts/check.sh` pass.
