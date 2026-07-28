# PRD 0012 implementation notes

## Bundle and provenance implementation

- Added the canonical four-file bundle builder in `frontend/Sembla/Composition/Bundle.lean` and `sembla-link <source> --bundle <dir>` emission in `frontend/LinkMain.lean`.
- The bundle-root payload is implemented independently in Lean and Rust from the frozen rule: canonical manifest bytes without `bundle_integrity`, then each of the three named paths in lexicographic byte order, `0x00`, and raw SHA-256(file bytes). The resulting payload is domain-hashed under `sembla.bundle-root/v1`.
- Added strict Rust `sembla bundle-verify`: schema/encoding/domain checks, canonical manifest bytes, source/envelope/semantic hashes, report-sensitive bundle integrity, full plan validation/canonicality, and manifest/embedded-provenance agreement.
- Added the checked `fixtures/bundles/epidemic_policy/` bundle and append-only four-file parity.
- Extended `verify-run` through the existing legacy/plan parse dispatch. Plan verification recomputes the semantic hash and both provenance tuples while retaining the legacy replay path.

## Architecture §26.2 acceptance sweep

Every criterion is classified; none is silent.

1. **Met.** `Composition.Json.parse`/`render` and `Composition.SourceTests` parse and round-trip canonical JSON independently of Lean surface syntax; PRD 0006 pins hand-authored source fixtures.
2. **Met.** `Composition.linkV1`, structured `LinkErrorV1`, `LinkTests`, and PRDs 0007–0009 pin the canonical signature, stage ordering, deterministic errors, and identity ordering.
3. **Met.** `SpecStatic.denoteSourceStatic` and the plan denotation in PRD 0010 are independent executable definitions, guarded by `SpecTests`.
4. **Met.** `SpecStatements.lean`, `linkV1_produces_valid_plan`, and `frontend/scripts/check-proofs.sh` typecheck the validity/preservation statements and make admitted versus executable proof status explicit (PRD 0010; DECISIONS §J13).
5. **Deferred.** Linked and `direct_stable` plans have mandatory identity maps and raw legacy retains `sembla.identity/legacy-positional-v1`; the criterion's normalized-legacy-plan clause is explicitly deferred by DECISIONS §J7 and is not accepted as a V1 origin.
6. **Met.** Repeated and nested occurrence, transition, wire, mailbox, and draw identities are pinned by PRD 0009 `LinkTests` plus the `two_regions`/`two_independent_regions` plan and runtime goldens.
7. **Met.** PRD 0008 mailbox construction and tests pin the owner wire occurrence and both endpoint occurrences, including fan-out.
8. **Met.** `crates/sembla-cli/tests/run_manifest.rs::linked_and_direct_plan_manifests_preserve_end_to_end_provenance` proves linked all-present and direct all-absent provenance; legacy manifest goldens remain tuple-free. Every plan retains its execution identity map.
9. **Met.** `crates/sembla-cli/tests/bundle.rs::plan_copied_out_of_bundle_runs_without_source_or_manifest` runs a detached plan, while plan validation tests pin all execution relationships.
10. **Met.** Plan identity maps and run-manifest plan tuples carry canonical empty enabled features; only linked plans/manifests carry source/linker provenance (`plan_validation`, `manifest` unit tests, and the new end-to-end test).
11. **Met.** The golden bundle pins algorithms, domains, digests, schemas, encoding, linker semantics, identity scheme, source-map schema, and bundle schema; run-manifest tuple tests pin the relevant plan/source versions and hash records.
12. **Met.** Existing plan validation tests reject unknown schemas, identity schemes, features, domains, and rule-word collisions; `bundle.rs` adds strict bundle version/domain validation and named corruption failures.
13. **Met.** PRD 0005 direct exports enter `validate_plan` and the same runtime plan path under `direct_stable`; direct plan validation/run tests pin the explicit stable identity interpretation.
14. **Met.** `frontend/scripts/check-parity.sh`, legacy CLI/run-manifest goldens, and `./scripts/check.sh` retain byte-compatible legacy behavior. The final `git diff 1d19fb3 -- examples/` spot-check is empty.
15. **Deferred.** Lean-authored and hand-authored source twins are byte-equal and link equally under PRD 0011, but the criterion's independent non-Lean source producer is explicitly deferred by DECISIONS §J12; no non-Lean producer is claimed.
16. **Met.** PRDs 0007–0009 `LinkTests`, checked composition-source/linked-plan fixtures, and CLI composition tests cover product, repeated composites, delay, fan-out, exposure, nesting, identity, source maps, and noninterference.
17. **Met.** The parity script invokes Rust `sembla validate` for every linked golden plan, and the repository check runs the Rust plan validation corpus.
18. **Deferred.** DECISIONS §J13 explicitly defers full behavioral preservation beyond the executable CPU observation contract. Plan/CUDA composition integration is a PRD 0012 non-goal; no CUDA differential composition corpus is claimed.
19. **Met.** `bundle.rs::checked_and_moved_bundles_verify_in_deterministic_order` copies the four-file bundle to a new directory and verifies the relationship; the detached-plan test separately proves standalone execution.
20. **Met.** DECISIONS §J12 and the positioned negative suite reject deferred constructs rather than accepting inert syntax; `docs/composition.md` labels CPU-only plan execution and historical relinking as current limits/future work.

## Documentation

- Added `docs/composition.md` covering component/root authoring, source export, single-file and bundle linking, verification, plan validation/run/replay, stable identity grammar and refactor effects, origin distinctions, and run-manifest tuples.
- Added only the requested dated/header and §4.4 pointers to `DESIGN.md`, replaced only the stale implementation-status paragraph in the Option D architecture, and made minimal feature/example additions to the root README.

## Validation

All required checks passed on the final implementation workspace:

- `./scripts/check.sh`
- `cd frontend && lake build`
- `cd frontend && bash scripts/test-negative.sh`
- `bash frontend/scripts/check-parity.sh`
- `git diff --check`
- focused `cargo test -p sembla-cli --test bundle --test run_manifest --test validate`
- final `git diff 1d19fb3 -- examples/` spot-check (empty)

Parity regenerated the bundle in a temporary directory, compared all four files byte-for-byte, proved non-empty-directory refusal, and passed Rust `bundle-verify`. No implementation change was staged or committed.
