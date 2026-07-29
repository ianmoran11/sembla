# PRD 0003 implementation notes — attempt 1

## Input and execution path

- `sweep` now loads both legacy models and canonical executable-plan envelopes through the same `parse_input`-based executable-input helper used by `run`.
- Plan input is validated with `validate_plan`, checked for canonical bytes, and converted with `ValidatedPlan::model_with_rule_words`; the existing sweep body, backend selection, prior/theta resolution, noise modes, execution loop, and pairs export remain shared.
- No seed, reserved namespace, Philox, prior, schema, or export-format code changed.
- Legacy inputs still validate and execute through the same dense-identity model path. No `--dt` option was added to `sweep`.

## Manifest identity

- Legacy sweep run manifests retain `ir_hash` and omit `plan`/`linked_source`.
- Plan sweep run manifests omit `ir_hash`, populate `model` and `dt` from the derived model, and use `manifest::plan_identity_tuples` for the complete plan tuple and origin-specific linked-source tuple.
- Linked `two_regions` tests recompute the checked-in composition source's domain-separated SHA-256 and compare it with `linked_source.source_hash.digest`; direct-stable tests require `linked_source` to be absent.
- The existing manifest reader's all-present-or-all-absent checks are exercised against a deliberately partial plan tuple taken from a sweep manifest.
- `verify-run` now expects `ir_hash` by both input identity and manifest kind: legacy runs/sweeps and plan runs retain `Some(canonical_ir_hash)`, while plan sweeps require `None`. The comparison remains active, so a forbidden plan-sweep `ir_hash` is still a verification mismatch.
- Successful replay coverage verifies all executions and selective `--draw 1` replay for a linked two-draw independent-noise sweep, plus a complete direct-stable sweep. Existing plan-run verification coverage continues to protect its retained `ir_hash` behavior.
- Sweep currently writes one canonical `run-manifest.json`; per-draw identities/hashes remain entries in its existing `executions` array rather than separate manifest files.
- `PairsMetadata` retains its existing schema and required `ir_hash` field. For both input kinds it receives the effective flattened-model digest; this does not populate the plan sweep run manifest's omitted `ir_hash` field.

## Golden provenance

- `sweep_run_manifest_legacy_pre_prd3.json` was captured before implementation from clean PRD-0003 baseline commit `7ccfc1b` with an 80-person/8-employer/4-infected population (`synth-pop` seed 123) and `examples/sir.json --seed 9 --draws 2 --ticks 3`. Its population basename is `pop.bin`, matching the freeze test.
- The pre-existing legacy sweep CSV goldens (`sweep_draw_{0,1}_legacy.csv`, `sweep_manifest_legacy.csv`, and `sweep_summary_legacy.csv`) were verified against that baseline before editing and remain unchanged.
- The new `two_regions` plan sweep goldens use a 1,000-person/50-employer/600-infected population (`synth-pop` seed 123), CPU execution, independent noise, seed 31, two draws, and four ticks.
- The new plan pairs golden uses the same population, CPU execution, independent noise, seed 91, three draws, and eight ticks.

## Test coverage

`crates/sembla-cli/tests/sweep.rs` covers all seven requested groups:

1. byte-for-byte deterministic linked-plan sweep outputs;
2. checked draw CSV, theta manifest, aggregate summary, and canonical run-manifest goldens;
3. accepted known plan theta assignments and the unchanged unknown-name diagnostic;
4. CRN versus independent plan execution with identical theta;
5. linked/direct-stable tuple presence, absent plan `ir_hash`, checked source digest, and partial-tuple rejection;
6. all legacy sweep outputs plus the pre-change run-manifest golden;
7. checked linked-plan `(theta, x)` pairs, unchanged canonical sidecar schema, CRN warning, and summary-free plan rejection.

## Documentation

- CLI usage now says `sweep <model-or-plan.json>`.
- `docs/composition.md` contains a worked linked-plan sweep and independent-noise pairs-export example, and documents plan sweep provenance.
- Tracked calibration documentation describes pairs input rather than restricting the sweep model input, so no calibration doc change was needed.

## Local verification

- `cargo test -p sembla-cli --test sweep` passed: 16 tests, including all seven new plan/legacy groups and linked/direct-stable sweep replay.
- `cargo test -p sembla-cli --test run_manifest` passed, preserving existing plan-run verification.
- `cargo test -p sembla-cli` passed.
- `cargo fmt --all -- --check` passed.
- `./scripts/check-determinism.sh` passed with byte-identical legacy run and sweep outputs/manifests.
- `./scripts/check.sh` passed.
- `git diff --check` passed.
- No dependency manifest or `Cargo.lock` changed; no existing example, fixture, golden, or recorded evidence file changed.

## Hardware status

- Local CPU acceptance is authoritative for this PRD's checked tests.
- A CUDA plan sweep on qualified hardware remains pending; no hardware result is claimed or simulated here. It reuses PRD 0002's validated plan CUDA path and unchanged sweep backend selection.
