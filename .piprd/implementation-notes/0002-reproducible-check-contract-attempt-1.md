# PRD 0002 implementation notes — attempt 1

## Baseline

- Pre-PRD review HEAD: `f9fadd0379cfdd71e9786f96aaf92643eebc4f0d`.
- Baseline `Cargo.lock` SHA-256:
  `410a96ce65551f3f5f6901b3e972575090ab5253a0449643564f6692d40bbd2e`.
- Active `.piprd/**` log, state, lock, prior implementation note/review, and run
  snapshot changes were present before implementation and were not edited.

## Implementation

- Added executable `scripts/check-rust.sh` as the Rust-only contract. It runs
  formatting, locked Clippy, locked workspace tests, the exact-`libm` and
  dependency/RNG policies (using locked `cargo tree`), and a final
  `Cargo.lock` comparison against `HEAD`.
- Reworked `scripts/check.sh` into the strict complete entry point. It checks
  for Cargo, Git, and Lake before validation, then runs the Rust-only contract,
  Lean proof hygiene, full Lean/Rust parity, and a final lock comparison.
- Removed the old conditional `scripts/check-lean.sh`; no caller remains and no
  check can now report success after silently skipping Lean.
- Made the parity script's CLI build lock-strict and added an immediate
  `Cargo.lock` assertion.
- Split CI responsibilities: the Rust job runs workflow parsing,
  `check-rust.sh`, and proof hygiene without duplicate Cargo build/test steps;
  the determinism and Lean jobs retain their dedicated checks.
- Added an authoritative five-row local check matrix to `docs/ci.md` and updated
  both READMEs to distinguish Rust-only and complete validation while linking
  the determinism, reduced NPE, and manual GPU evidence contracts.

## Verification

- Missing-Lake isolated-PATH test: complete check exited 1 with an actionable
  Lake/elan message and pointed to `./scripts/check-rust.sh`; it did not claim
  success.
- Rust-only check under the same PATH without Lake: passed.
- Strict complete `./scripts/check.sh`: passed.
- Direct `bash frontend/scripts/check-parity.sh`: passed.
- `./scripts/check-determinism.sh`: passed.
- `ruby scripts/check-workflow-yaml.rb`: passed for both workflows.
- A repository-wide validation-script audit found 12 dependency-resolving
  Cargo commands and zero without `--locked`.
- CI responsibility assertions found Rust formatting/lint/tests/dependency
  policy, proof hygiene, determinism, and Lean parity covered with no duplicate
  Rust-job Cargo build/test.
- `Cargo.lock` retained its baseline SHA-256, and both the requested
  `git diff --exit-code -- Cargo.lock` assertion and the script-level
  comparisons against `HEAD` passed.
- Before/after hashes found zero changes across 304 protected tracked artifact,
  fixture, example, spike, lock, and license files, and zero changes across the
  six pre-existing active `.piprd` files.
- Shell syntax, `git diff --check`, staged/combined diff checks, and no-index
  whitespace checks for new files passed. `actionlint` remains **unanswered**
  because it is not installed locally; the repository Ruby workflow parser
  passed.
