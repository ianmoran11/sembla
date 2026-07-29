# PRD 0006 implementation notes — attempt 2

## Review blockers addressed

`verify-run` now treats `RunManifest.enabled_features` as the authoritative
runtime feature set. Replay does not accept a new CLI override and does not use
the plan identity's artifact feature list as an implicit runtime enable. Legacy
model validation, the additional runtime validation of plan models, and CPU
execution for both run and sweep replay receive the manifest-derived
`FeatureSet`.

Replay also recomputes grouped CSV records with the same
`grouped_output_records` helper used by manifest writers and compares them with
the top-level run record or each sweep execution's record. New integration
coverage pins feature-enabled replay for a legacy model run, a direct-stable
plan run, and a plan sweep. Valid-looking tampered SHA-256 digests are rejected
for both run and sweep grouped records.

## Bounded CUDA test-only scope exception

The revision request explicitly adopts the advisor's scope resolution: permit
`crates/sembla-cuda/src/codegen.rs` only for the two existing `#[cfg(test)]`
`ModelBox` literals to initialize `grouped_views: Vec::new()`. Rust struct
literals cannot inherit serde defaults, and the required workspace
`--all-targets` checks compile these tests. The two initializers are therefore a
mechanical consequence of the PRD-mandated `Box.grouped_views` field. No
production CUDA code, kernel behavior, dependencies, or numeric contracts are
changed.

## Validation

Passed:

- `cargo test --locked -p sembla-cli --test grouped_observations` (including
  legacy-model, plan, and sweep replay plus tampered grouped-hash rejection)
- `cargo test --locked -p sembla-cli --test run_manifest`
- `cargo test --locked -p sembla-cli --test sweep`
- the ignored exact Lean grouped model/plan fixture-regeneration test
- `./scripts/check.sh` (formatting, Clippy with workspace all-targets, Rust
  tests, dependency policy, Lean build/proof hygiene, negative suite, parity,
  documentation, and lock checks)
- temporary-index `git diff --cached --check` over all 41 PRD candidate files

`Cargo.lock`, frozen examples/state fixtures, historical plan fixtures, and
`DESIGN.md` remain unchanged. The real Git index remains empty.
