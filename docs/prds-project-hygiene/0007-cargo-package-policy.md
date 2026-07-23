# PRD 0007: Make Cargo publication policy and package metadata explicit

max_review_cycles: 3

## Context

Read this folder's `README.md` first. The four workspace crates inherit version,
edition, and dual license, but publication is left at Cargo's permissive default
and workspace metadata omits repository and minimum Rust version. The crate
manifests also lack descriptions. There is no release/publishing workflow or
PRD authorizing publication, so an accidental `cargo publish` attempt should
fail until a deliberate release track reverses that policy.

## Goal

All workspace packages explicitly refuse publication for now and expose enough
consistent metadata for tools and contributors, without changing versions,
dependencies, lock resolution, or runtime behavior.

## Requirements

1. In `[workspace.package]`, add the canonical repository URL, the minimum Rust
   version matching `rust-toolchain.toml`, and `publish = false` if Cargo permits
   that field to be inherited on the pinned toolchain.
2. Make each of the four crate packages inherit or explicitly set the no-publish,
   repository, and rust-version policy. Add one accurate, restrained package
   description per crate.
3. Do not change package names, versions, edition, features, dependencies,
   binary/library targets, or `Cargo.lock`.
4. Add a small metadata assertion to the repository checks that parses
   `cargo metadata --locked --no-deps` and verifies every workspace member has:
   dual-license metadata, repository URL, rust-version, description, and
   `publish == []`/the Cargo representation of `false`. Keep the assertion
   dependency-free (standard Python/Ruby or a small shell helper).
5. Document in `CONTRIBUTING.md` that publishing is intentionally disabled and
   requires a separate release PRD covering package ownership, crate READMEs,
   API stability, versioning, and release provenance.

## Allowed files

- `Cargo.toml`
- `crates/sembla-cli/Cargo.toml`
- `crates/sembla-cuda/Cargo.toml`
- `crates/sembla-ir/Cargo.toml`
- `crates/sembla-runtime/Cargo.toml`
- One dependency-free metadata-check script under `scripts/`
- Canonical check entry points and `.github/workflows/ci.yml` only to invoke the
  metadata assertion
- `CONTRIBUTING.md`
- Managed implementation notes/artifacts

## Non-goals

- Publishing crates or adding release automation.
- Version bumps, API changes, dependency changes, or lock regeneration.
- Adding per-crate READMEs solely to satisfy a future registry.
- Changing spike crates outside the root workspace.

## Implementation notes

Use workspace inheritance where supported by Cargo 1.79 and verify the resolved
JSON metadata rather than only manifest spelling. If `publish.workspace` is not
supported for this field, set `publish = false` explicitly in each crate.

## Test and check guidance

Run:

```bash
cargo metadata --locked --no-deps --format-version 1
# Run the new metadata assertion directly.
git diff --exit-code -- Cargo.lock
./scripts/check.sh
bash frontend/scripts/check-parity.sh
git diff --check
```

Optionally demonstrate that `cargo publish --dry-run -p sembla-ir` refuses due
to policy, but do not upload or contact a registry.

## Acceptance criteria

1. All four workspace packages explicitly resolve to `publish = false`, the
   repository URL, pinned minimum Rust version, dual license, and a useful
   description in Cargo metadata.
2. The metadata assertion fails on a temporary missing field and passes on the
   repository.
3. Package versions, dependencies, targets, features, and `Cargo.lock` are
   unchanged.
4. Contribution docs state the deliberate no-publish policy and the requirements
   for a future release decision.
5. Repository, parity, and diff checks pass.
