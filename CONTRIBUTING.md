# Contributing to Sembla

Sembla spans Rust, Lean, and a quarantined Python calibration environment. Keep
changes narrow, use the pinned environments, and treat checked-in evidence as
part of the reproducibility contract. The maintained check matrix is in
[`docs/ci.md`](docs/ci.md).

## Pinned environments

- **Rust:** install Rustup and use the repository's `rust-toolchain.toml`, which
  pins Rust 1.79.0 with `rustfmt` and Clippy. Run Cargo commands from the
  repository root and retain the committed `Cargo.lock`; validation commands
  use `--locked`.
- **Lean:** install Elan. Commands under `frontend/` use
  `frontend/lean-toolchain`, which pins Lean 4.13.0 and supplies Lake. Do not
  substitute a globally selected Lean release.
- **Python:** the reviewed Linux NPE environment is CPython 3.12.8 with the
  complete hashed lock in `calibration/npe/requirements-ci.lock`. Follow
  [`calibration/npe/README.md`](calibration/npe/README.md); do not replace the
  lock installation with an unconstrained resolver run. On the documented
  Linux target, create it with:

  ```sh
  python3.12 -m venv calibration/npe/.venv
  PYTHON=calibration/npe/.venv/bin/python \
    calibration/npe/install-requirements-ci.sh
  ```

## Checks

Run the smallest contract appropriate while developing, then the complete
contract before submitting a repository-wide change:

```sh
./scripts/check-rust.sh       # formatting, Clippy, Rust tests, dependency policy
./scripts/check.sh            # documentation + strict Rust + Lean + parity
python3 scripts/check-markdown-links.py
python3 scripts/check-cargo-metadata.py
./scripts/check-determinism.sh
bash frontend/scripts/check-parity.sh
```

The directly runnable Markdown checker uses only the Python standard library.
It checks relative targets in tracked Markdown, excludes managed `.piprd`
records, and does not test remote URLs or anchor fragments. The complete check
runs both its temporary-fixture unit tests and the repository scan.

`./scripts/check.sh` fails when a required pinned tool is unavailable; it does
not silently skip Python or Lean. The determinism command byte-compares repeated
CPU run and sweep outputs. The direct parity command byte-compares Lean exports
and canonical fixtures and verifies their Rust-side contracts.

For NPE dependency or calibration-path changes, also run the immutable
Linux/amd64 lock validation and reduced smoke test described in the NPE README:

```sh
./scripts/check-npe-lock.sh
```

Before handing off a change, run `git diff --check` and inspect the complete
staged and unstaged diff. Additional workflow and manual GPU checks, including
their environment requirements, are documented in the
[check matrix](docs/ci.md#local-check-contract).

## Package publishing

Publishing is intentionally disabled for every workspace crate. Do not relax
that manifest policy or add release automation as part of an unrelated change.
A separate release PRD must cover package ownership, crate READMEs, API
stability, versioning, and release provenance before any `cargo publish`
attempt or registry upload is authorized.

## Frozen artifacts and regeneration

Checked-in examples, fixtures, golden files, portable bundles, calibration/NPE
artifacts, and GPU evidence are reviewed evidence. In particular, files under
`calibration/npe/artifacts/` encode scientific inputs and results, and their
seeds, hashes, schemas, thresholds, and output formats are part of the contract.
Do not hand-edit, opportunistically refresh, or regenerate these files merely to
make a test pass.

Regeneration requires explicit authorization from the active PRD or a
maintainer that names the affected artifact family. When regeneration is
authorized:

1. use the documented generator and pinned environment;
2. preserve declared seeds, identity rules, schemas, thresholds, and formats
   unless the authorization explicitly changes them;
3. record the exact command, environment, and relevant source/input hashes;
4. inspect the byte-level diff and run every associated determinism, parity, and
   validation check.

If authorization is absent or the documented environment is unavailable, stop
and report the result as blocked or unanswered rather than manufacturing new
evidence.

## Managed PRD runs and agent output

During a managed `/piprd` run, implement only the active PRD and leave review
and commit handling to the runner. Do not manually stage active `.piprd`
runtime files. Existing `.piprd` run state, logs, locks, reviews, advice,
snapshots, implementation notes, and other managed records are durable workflow
state: do not rewrite, delete, clean, or absorb them into an unrelated change.

`.pi-subagents` contains disposable raw agent transcripts. Never commit that
directory or copy raw transcripts into repository documentation or artifacts.
Keep implementation changes within the active PRD's allowed files and report
scope or validation blockers instead of bypassing the managed workflow.
