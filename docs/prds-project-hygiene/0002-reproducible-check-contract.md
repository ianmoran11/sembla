# PRD 0002: Make the repository check contract lock-strict and explicit

max_review_cycles: 4

## Context

Read this folder's `README.md` first. `scripts/check.sh` currently runs
`cargo clippy`, `cargo test`, and two `cargo tree` queries without `--locked`.
`frontend/scripts/check-parity.sh` likewise performs an unlocked CLI build. In
CI, these commands run before later locked build/test commands, so a stale
`Cargo.lock` can be repaired in the checkout before the purported lock check.
The same root script also calls `scripts/check-lean.sh`, which succeeds with a
warning when `lake` is unavailable, while documentation calls `check.sh` the
complete repository check. The Rust CI job then repeats build/test work already
performed by the script.

## Goal

Dependency resolution never mutates the committed lock during validation, and
there are clear, non-overlapping Rust-only and complete-repository check entry
points whose documentation and CI behavior agree.

## Requirements

1. Add `--locked` to every Cargo command in maintained check/parity scripts
   that can resolve dependencies: Clippy, tests, builds, and `cargo tree`.
   Formatting commands do not need it. Do not change dependency versions or
   `Cargo.lock`.
2. Introduce a narrowly named Rust-only check entry point, preferably
   `scripts/check-rust.sh`, containing formatting, Clippy, Rust tests, and the
   existing runtime dependency/RNG allowlist checks.
3. Make `scripts/check.sh` the strict complete local entry point: invoke the
   Rust-only checks, Lean proof hygiene, and Lean parity, and fail with an
   actionable message if a required tool is absent. A complete check must never
   silently downgrade itself.
4. Retain a separately named helper only if conditional Lean behavior is still
   useful; its name and output must say it is partial. Do not leave
   `scripts/check-lean.sh` ambiguously succeeding as a complete Lean check.
5. Update `.github/workflows/ci.yml` so the Rust job calls the Rust-only/proof
   checks and does not repeat equivalent Cargo build/test commands. The Lean job
   remains responsible for full parity.
6. Update `README.md`, `frontend/README.md`, and `docs/contributing/ci.md` with one canonical
   table or concise list: fast Rust check, complete local check, determinism,
   NPE smoke, and GPU manual evidence. Preserve existing honest environment
   caveats.
7. Add a mechanical assertion that validation leaves `Cargo.lock` unchanged
   (for example, run checks then `git diff --exit-code -- Cargo.lock`).

## Allowed files

- `scripts/check.sh`, `scripts/check-lean.sh`, `scripts/check-rust.sh` (new)
- `scripts/check-determinism.sh` only if needed for consistent lock flags
- `frontend/scripts/check-parity.sh`
- `.github/workflows/ci.yml`
- `README.md`, `frontend/README.md`, `docs/contributing/ci.md`
- Managed implementation notes/artifacts

## Non-goals

- Dependency upgrades or lockfile regeneration.
- Action SHA pinning or NPE path-filter changes (PRD 0003).
- Python dependency locking (PRD 0004).
- Changing test coverage or scientific output.

## Implementation notes

Prefer small composable shell entry points over mode flags hidden inside one
large script. Preserve strict shell options and useful failure output; do not
replace an explicit failure with environment-dependent skipping.

## Test and check guidance

Run the new Rust-only and complete entry points, then:

```bash
git diff --exit-code -- Cargo.lock
bash frontend/scripts/check-parity.sh
./scripts/check-determinism.sh
ruby scripts/check-workflow-yaml.rb
git diff --check
```

Also test the missing-Lake path in an isolated `PATH`: the complete command
must fail clearly before claiming success; the Rust-only command must remain
usable.

## Acceptance criteria

1. Every dependency-resolving Cargo invocation in maintained validation scripts
   uses `--locked`; `Cargo.lock` is byte-unchanged.
2. The Rust-only command passes without requiring Lean, while the complete
   command requires and runs Lean parity rather than silently skipping it.
3. CI no longer repeats Cargo build/test after equivalent Rust checks and still
   covers formatting, Clippy, tests, dependency policy, proof hygiene, parity,
   and determinism across its jobs.
4. README/frontend/CI documentation names the commands accurately.
5. All old-equivalent and new checks pass; protected artifacts are unchanged.
