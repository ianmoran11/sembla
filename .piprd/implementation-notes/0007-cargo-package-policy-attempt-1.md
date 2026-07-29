# PRD 0007 implementation — attempt 1

## Baseline and scope

- Started from `5f6cb5f2623d36883330767ff99d8e7cd9942358` (`Implement 0006-text-policy-and-doc-link-check`) with an empty real index.
- Preserved active `.piprd/**` managed state and the owner-authorized local exclusion for `docs/australian-population-use-case.md`.
- A temporary-index emulation of piprd staging contains exactly the nine allowed implementation files: `.github/workflows/ci.yml`, `CONTRIBUTING.md`, `Cargo.toml`, the four root-workspace crate manifests, `scripts/check-cargo-metadata.py`, and `scripts/check.sh`.

## Implementation

- Added workspace-level inheritance for the canonical repository URL, Rust `1.79.0` minimum matching `rust-toolchain.toml`, and `publish = false`.
- Made all four root-workspace crates inherit repository, rust-version, and publish policy, and added restrained package descriptions.
- Added `scripts/check-cargo-metadata.py`, a Python-standard-library assertion over `cargo metadata --locked --no-deps --format-version 1`. It verifies the exact four workspace members, dual license, repository, pinned minimum Rust version, non-empty descriptions, and Cargo's `publish = false` representation (`[]`). Its `--metadata-file` option supports isolated negative fixtures without editing manifests.
- Wired the assertion into `scripts/check.sh` and the existing CI package/documentation hygiene step.
- Documented the intentional no-publish policy and the package ownership, crate README, API stability, versioning, and release-provenance requirements for a separate future release PRD.

## Metadata and compatibility evidence

- Cargo 1.79 accepts `publish.workspace = true` and resolves every package to `publish: []`, repository `https://github.com/ianmoran11/sembla`, rust-version `1.79.0`, license `MIT OR Apache-2.0`, and its package-specific description.
- Comparing resolved metadata before and after found package names, versions, editions, features, dependencies, and targets unchanged.
- `Cargo.lock` remains byte-identical with SHA-256 `410a96ce65551f3f5f6901b3e972575090ab5253a0449643564f6692d40bbd2e`.
- A temporary saved metadata fixture with `sembla-ir.repository` removed failed with exit 1 and the diagnostic `sembla-ir: repository must be "https://github.com/ianmoran11/sembla"; found null`; repository metadata passes.

## Validation

- `cargo metadata --locked --no-deps --format-version 1` — passed.
- `python3 -B scripts/check-cargo-metadata.py` — passed for all four packages.
- Temporary missing-field metadata fixture — failed as required with exit 1 and field/package evidence.
- `./scripts/check.sh` — passed, including metadata, Markdown, Rust, Lean proof-hygiene, parity, and lock checks.
- `bash frontend/scripts/check-parity.sh` — passed directly; canonical fixtures and bundles remained byte-identical.
- `ruby scripts/check-workflow-yaml.rb` — parsed both workflows and Dependabot configuration.
- `git diff --exit-code -- Cargo.lock`, `git diff --check`, and temporary-index `git diff --cached --check` — passed.
- `actionlint` is not installed, so that optional workflow lint remains **UNANSWERED**; no available failure was ignored.
- No Python cache artifacts were created.
