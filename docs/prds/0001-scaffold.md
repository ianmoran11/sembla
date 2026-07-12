---
max_review_cycles: 3
---

# PRD 0001: Rust workspace scaffold

## Context

Sembla v0.1 (see `DESIGN.md` §2, §9) is a simulation framework: a Lean
frontend (added later, PRD 0010) emits a JSON IR executed by a deterministic
Rust CPU interpreter. This PRD creates the Rust skeleton everything else
builds on. No simulation logic yet.

## Goal

A clean Cargo workspace with three crates, a CI-style check script, and
repository hygiene files, so subsequent PRDs only add code, not structure.

## Deliverables

- Cargo workspace at the repo root with members:
  - `crates/sembla-ir` (lib): will hold IR types (PRD 0002). For now: crate
    with a `VERSION: &str` const and one passing unit test.
  - `crates/sembla-runtime` (lib): will hold the interpreter (PRDs 0003–0007).
    Same placeholder standard.
  - `crates/sembla-cli` (bin, binary name `sembla`): depends on both libs;
    `sembla --version` prints the workspace version.
- `scripts/check.sh`: runs `cargo fmt --check`, `cargo clippy --workspace
  --all-targets -- -D warnings`, `cargo test --workspace`. Exits nonzero on
  any failure.
- `.gitignore` covering Rust (`target/`), Lean (`.lake/`), and editor noise.
- Top-level `README.md` (brief: what Sembla is, link to `DESIGN.md`, how to
  build/test, workspace layout).
- `rust-toolchain.toml` pinning a stable toolchain.

## Non-goals

Lean project (PRD 0010), GitHub Actions workflows, any IR or runtime logic.

## Acceptance criteria

1. `cargo build --workspace` succeeds from a fresh checkout.
2. `cargo test --workspace` passes with at least one test per crate.
3. `./scripts/check.sh` exits 0, and exits nonzero if a test is made to fail
   (verify by inspection of the script logic).
4. `cargo run -p sembla-cli -- --version` prints a version string.
5. `README.md` exists and links to `DESIGN.md`; `.gitignore` excludes
   `target/` and `.lake/`.
