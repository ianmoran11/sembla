# PRD 0002 implementation notes — attempt 1

## CUDA rule-identity audit

Audited with `rg -n 'rule_id|rule_word' crates/sembla-cuda/src/backend.rs crates/sembla-cuda/src/codegen.rs crates/sembla-cuda/src/lib.rs` after the implementation.

### Stable word: Philox key material

- `crates/sembla-cuda/src/lib.rs:21-42`: `PhiloxCoordinate` and its constructor now name the Philox coordinate `rule_word` rather than `rule_id`.
- `crates/sembla-cuda/src/backend.rs:458-461`: the host-side `philox_vectors` input array reads `PhiloxCoordinate.rule_word` into counter word 1.
- `crates/sembla-cuda/src/codegen.rs:1308`: the generated transition race-time call `sembla_exp(seed, tick, ..., row, draw, lambda)` now bakes `validated.rule_word` into counter word 1.
- `crates/sembla-cuda/src/codegen.rs:1949-1996`: generic `sembla_philox`/`sembla_uniform`/`sembla_exp` and the explicit vector-test kernel consume the supplied rule coordinate; they do not derive or recompute model identities. There is no separate model-derived effect-draw Philox call in the current CUDA codegen.

### Stable word: ordering/tie-break keys

- `crates/sembla-cuda/src/codegen.rs:1454`: initializes each generated `best_rule_*` from `validated.rule_word`.
- `crates/sembla-cuda/src/codegen.rs:1494,1496`: both real-key and non-real-key lexicographic comparisons use `other.rule_word`.
- `crates/sembla-cuda/src/codegen.rs:1498`: winner replacement stores `other.rule_word`.
- `crates/sembla-cuda/src/codegen.rs:1503`: the self-winner comparison uses `validated.rule_word`.
- The unused dense-rule argument to `claim_key` was removed so it cannot be mistaken for ordering identity.

### Dense ordinal: indexing, layout, specialization, dispatch, and diagnostics

All remaining production `rule_id` uses are intentionally dense ordinals:

- `crates/sembla-cuda/src/backend.rs:393-403`: indexes candidate offsets and reports fired counts by dense rule.
- `crates/sembla-cuda/src/backend.rs:599-613`: selects the generated validation branch/kernel by dense rule.
- `crates/sembla-cuda/src/backend.rs:629`: row-count error diagnostics.
- `crates/sembla-cuda/src/backend.rs:659-682`: candidate-buffer offset lookup and claim-validation dispatch.
- `crates/sembla-cuda/src/backend.rs:713`: first dense rule to candidate-range offset.
- `crates/sembla-cuda/src/codegen.rs:146-153,242,248,256,264,271`: `AggUse::Schedule/Effect` specialization bookkeeping.
- `crates/sembla-cuda/src/codegen.rs:1033-1034,1129-1140`: aggregate-rule lookup and winner/candidate-offset indexing.
- `crates/sembla-cuda/src/codegen.rs:1218,1228`: validation-kernel dense selector argument and comparison.
- `crates/sembla-cuda/src/codegen.rs:1229`: candidate-offset diagnostic identity.
- `crates/sembla-cuda/src/codegen.rs:1293,1297`: per-dense-rule kernel symbol and candidate-buffer offset.
- `crates/sembla-cuda/src/codegen.rs:1318,1331`: claim-validation dense selector argument and comparison.
- `crates/sembla-cuda/src/codegen.rs:1337,1340`: claim error candidate offsets.
- `crates/sembla-cuda/src/codegen.rs:1417`: incompatible-claim left/right candidate offsets.
- `crates/sembla-cuda/src/codegen.rs:1436`: resolver candidate-range bounds and row derivation.
- `crates/sembla-cuda/src/codegen.rs:1486`: other-rule candidate-buffer offset (adjacent ordering comparisons use `rule_word`).
- `crates/sembla-cuda/src/codegen.rs:1536-1537`: effect-validation winner lookup and candidate identity.
- `crates/sembla-cuda/src/codegen.rs:1556,1560`: enum/ref effect diagnostics through candidate offsets.
- `crates/sembla-cuda/src/codegen.rs:1580-1581`: winner/candidate-buffer layout.
- `crates/sembla-cuda/src/codegen.rs:1599`: write-conflict owner and diagnostic rule ordinal.

Test-only `rule_id` references in `codegen.rs:2128-2296` construct a valid stable plan and assert that Philox/tie-break strings contain nonordinal words while kernel names and candidate offsets retain dense ordinals.

Legacy codegen remains byte-identical because legacy validation sets `rule_word == rule_id`; the checked-in `sir.generated.cu` snapshot was not changed and its equality test passes. No example, CSV/hash golden, plan fixture, or recorded differential-evidence file changed. The codegen assertions added here are unit tests for the new parameter split, not changes to frozen goldens.

## CLI, corpus, and local acceptance

- Plan `run --backend cuda` now reaches the existing CUDA backend through `ValidatedPlan::model_with_rule_words`; plan `--dt` rejection is unchanged.
- `diff-backends` routes both legacy models and canonical validated plans through the shared run-input loader and reuses the existing backend execution and byte-comparison contract.
- `--all-plan-fixtures` enumerates exactly the sorted top-level `fixtures/plans/*.plan.json` and `fixtures/plans/linked/*.plan.json` set (13 files); invalid/golden subdirectories are not traversed. It rejects a positional path, `--all-examples`, `--params`, and `--dt`.
- Numeric population handling is unchanged from `run`: composed tables honor nonzero authored `size_hint`s; the supplied population initializes tables without such a hint.
- GPU-gated plan differential tests use the existing ignored-test convention and skip cleanly locally.
- The remote runbook now captures separate legacy and plan corpus logs; the harness documents the composition corpus and DECISIONS.md §J14 split.

Local evidence:

- `./scripts/check.sh` passed after the implementation, including CUDA host/codegen tests.
- `cargo build -p sembla-cuda` passed explicitly.
- `cargo test -p sembla-cuda --lib` passed (16 tests).
- `cargo test -p sembla-cli --bin sembla` passed (19 tests).
- `cargo test -p sembla-cli --test validate` passed (12 tests).
- `cargo test -p sembla-cli --features cuda --test gpu_differential` passed locally with all 3 hardware tests cleanly ignored.
- No-GPU smoke checks for plan `run --backend cuda`, single-plan `diff-backends`, and `--all-plan-fixtures` reached the existing `cuda backend unavailable` diagnostic; plan `--dt` retained its frozen rejection.

## Pending hardware acceptance (not run or claimed locally)

- **Pending:** On qualified CUDA hardware, run `crates/sembla-cuda/scripts/run-differential-corpus.sh` and confirm the full legacy and 13-plan composition corpora pass CPU-vs-CUDA comparison.
- **Pending:** Confirm the legacy corpus results/evidence remain unchanged on hardware after the identity split.
