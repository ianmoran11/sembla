# Contributing to Sembla

This repository contains Sembla's Rust backend and a quarantined Python
calibration environment. The Lean frontend is maintained independently in
[`ianmoran11/sembla-lean`](https://github.com/ianmoran11/sembla-lean). Keep
changes narrow, use the pinned environments, and treat checked-in evidence as
part of the reproducibility contract. The maintained check matrix is in
[`docs/contributing/ci.md`](docs/contributing/ci.md).

## Pinned environments

- **Rust:** install Rustup and use the repository's `rust-toolchain.toml`, which
  pins Rust 1.79.0 with `rustfmt` and Clippy. Run Cargo commands from the
  repository root and retain the committed `Cargo.lock`; validation commands
  use `--locked`.
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
./scripts/check.sh            # documentation, architecture, and strict Rust
python3 scripts/check-markdown-links.py
./scripts/check-abs-data.sh    # ABS extracts, readers, reconciliation (offline)
python3 scripts/check-cargo-metadata.py
./scripts/check-determinism.sh
```

`./scripts/check-abs-data.sh` runs the ABS pipeline's reader and extract
tests, verifies the pinned cache without touching the network, reconciles
the committed extracts, and confirms they regenerate byte-identically. It
uses only the Python standard library and needs no virtual environment.

For Australian population foundation changes, `data/abs/rates.py` is the sole
generator of the four complete v2 parameter tables and the non-production Lean
ParamDecl reference exported by `scripts/export-frontend-data.sh`.
`Parameters.lean` is a frontend compatibility projection and must never be
overwritten implicitly by backend generation. In tests,
redirect `--parameter-tables`, `--lean-parameters`, `--params-dir`, and
`--report` to temporary paths. The module map, semantic order, 17/360 split,
seven published-zero exceptions, frozen fixture hashes, and hard failure policy
are documented in the
[frontend maintenance README](https://github.com/ianmoran11/sembla-lean/blob/main/Sembla/Models/AustralianPopulation/README.md).

Export the generated parameter-family tables and structural reference for a
frontend compatibility change with an explicit destination:

```sh
./scripts/export-frontend-data.sh /tmp/sembla-frontend-data
```

Never locate or overwrite a sibling frontend checkout implicitly. The
frontend repository owns its exact-byte pins and direct Lean elaboration
checks. See the [indexed-family guide](docs/guides/indexed-parameter-families.md).

The directly runnable Markdown checker uses only the Python standard library.
It checks relative targets in tracked Markdown, excludes managed `.piprd`
records, and does not test remote URLs or anchor fragments. The complete check
runs both its temporary-fixture unit tests and the repository scan.

`./scripts/check.sh` fails when a required pinned tool is unavailable; it does
not silently skip Python or Rust checks. The determinism command byte-compares
repeated CPU run and sweep outputs. Cross-repository compatibility is run by
the frontend repository against the backend commit recorded in its pin.

For NPE dependency or calibration-path changes, also run the immutable
Linux/amd64 lock validation and reduced smoke test described in the NPE README:

```sh
./scripts/check-npe-lock.sh
```

Before handing off a change, run `git diff --check` and inspect the complete
staged and unstaged diff. Additional workflow and manual GPU checks, including
their environment requirements, are documented in the
[check matrix](docs/contributing/ci.md#local-check-contract).

## Local cache cleanup

Preview the repository's closed allowlist of rebuildable local caches before
removing anything:

```sh
bash scripts/clean-local.sh
```

The default dry run reports whether each allowed path exists and its approximate
size. Review that output first; deletion requires a separate explicit command:

```sh
bash scripts/clean-local.sh --apply
```

Cleanup is intentionally limited to the root Rust `target/`, root
`.pytest_cache/`, the NPE `.venv/`, and Python
`__pycache__` directories below `calibration/npe/`. It refuses symlinked or
out-of-root candidates, protected paths, and any candidate containing tracked
files; it never delegates to `git clean`.

Applying cleanup trades disk space for rebuild time. Rust outputs must be
rebuilt, and removing `calibration/npe/.venv/` requires reinstalling the
pinned Python dependencies before running NPE checks. Managed `.piprd` state,
agent transcripts, fixtures, examples, scientific artifacts and evidence, and
Terraform material are outside the allowlist and are never cleanup targets.

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
evidence. Source-owned generated modules are not frozen scientific evidence:
when their documented generator changes, regenerate them in the same change
and prove exact reproducibility. This exception does not authorize refreshing
any model, plan, state, golden, calibration, or evidence fixture.

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
