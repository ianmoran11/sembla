# PRD 0004 implementation notes — attempt 1

## Scope

- Routed both `compare` forms through the existing `read_executable_input` / `parse_input` path, so legacy inputs preserve their validated-model path and plans receive validation, canonical-byte enforcement, and stable `rule_word`s.
- Added an early identity-scheme check for two-file contrasts. A legacy/plan pair names both paths and returns before reading population bytes, resolving theta, executing either arm, or creating the CSV/manifest sidecar.
- Kept the compare CSV schema, shared seed, backend selection, field extraction, hashes, and manifest format unchanged. No `--dt`, seed, namespace, schema, or dependency change was made.
- Updated both compare forms in `USAGE` to name model-or-plan inputs.

## Exact CRN evidence

- `shared_population_is_exactly_crn_paired_across_different_linked_plans` compares the linked `solo_population` and unwired `independent_epidemic_policy` plans. It first checks the shared population transition identity/rule-word records, then mechanically checks every tick's S/I/R values, infect/recover firings, deferred count, and all three difference columns. Two independent invocations and the checked golden are byte-identical.
- The frozen linked `epidemic_policy` fixture exposes only population-side `beta`/`gamma`; its policy thresholds are literals, so it cannot express the PRD's required policy-side theta contrast. The user explicitly selected the proposed resolution: add a test-only linked plan under `crates/sembla-cli/tests/fixtures/` rather than alter the frozen fixture or weaken the causal test.
- `epidemic_policy_threshold.source.json` and its canonical linked plan add only the integer `restriction_threshold` parameter in the policy guard. Existing identities, rule words, wire delays, and runtime semantics are otherwise inherited from the frozen fixture. The plan's embedded source digest is SHA-256 over `sembla.source-artifact/v1 || 0x00 || canonical source bytes`, and the test recomputes it.
- With thresholds 500 versus 1000, seed 55, and the showcase population, population columns are exactly equal at ticks 0 and 1 and first differ at tick 2, after both one-tick wires. The test pins tick 2 and byte-compares two executions with `compare_policy_threshold.csv`.

## Goldens and demo

- Added plan-vs-plan golden `compare_solo_independent.csv`.
- Added params-form delayed-policy golden `compare_policy_threshold.csv`.
- Added params-form demo golden `compare_demo_counterfactual.csv`; the test runs the committed `demo_counterfactual_outbreak/executable-plan.json` twice under `control_beta` 0.45 versus 0.9. No demo source, plan, report, bundle, or existing demo golden changed.
- Mixed legacy/plan rejection asserts the exact deterministic message, empty stdout, and absence of both requested CSV and sidecar.

## Documentation

- Added runnable two-file noninterference, delayed-policy params, and committed-demo params commands to `docs/composition.md`.
- The walkthrough explains why stable content-addressed identities make cross-model CRN pairing principled and cites DECISIONS.md §J4/§J14.

## Validation

- `cargo test -p sembla-cli --test compare`: passed, 4 tests.
- `cargo test -p sembla-cli`: passed, including unchanged legacy compare tests and all prior plan/sweep coverage.
- `cargo fmt --all -- --check`: passed.
- `./scripts/check.sh`: passed, including all Rust, Lean, parity, fixture, and proof-hygiene checks.
- `./scripts/check-determinism.sh`: passed; legacy run/sweep artifacts remain byte-identical.
- `cd frontend && lake build`: passed.
- `cd frontend && bash scripts/check-parity.sh`: passed.
- `git diff --check`: passed.
- A plan-vs-plan compare with `--backend cuda` reached the selected backend and failed only with the expected local `cuda backend unavailable: crate built without the 'cuda' feature`; it did not take a legacy-only rejection path. Qualified-hardware execution remains pending and is not claimed.

## Protected contracts

- No existing file under `examples/**`, `fixtures/**`, the demo tree, or existing golden sets changed.
- No `Cargo.toml`, `Cargo.lock`, dependency, schema, identity, prior, seed, reserved namespace, Philox, output-format, or CUDA implementation change was made.
