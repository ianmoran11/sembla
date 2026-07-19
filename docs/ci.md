# Continuous integration

GitHub Actions runs `.github/workflows/ci.yml` on every push and pull request.
It contains these checks:

- **Rust:** installs the root `rust-toolchain.toml` pin, restores the Cargo
  cache, runs `scripts/check.sh`, and then explicitly builds and tests the
  workspace.
- **Determinism:** runs `scripts/check-determinism.sh`, which executes the SIR
  model twice and compares the result CSV, summary CSV, and manifest bytes. It
  also executes the same sweep twice and compares every output and the sweep
  manifest.
- **Lean frontend:** installs the `frontend/lean-toolchain` pin, restores the
  Lake cache, and runs the existing build, elaboration, export, and runtime
  parity checks.
- **NPE smoke:** runs only when `calibration/**` or
  `docs/prds-npe-path/**` changes. It installs the exact Python 3.12
  dependencies, runs the contract/refusal tests, and performs a reduced
  one-epoch training call. It deliberately does not run SBC. The full PRD-0007
  statistical acceptance configuration remains local/manual.

The separate `.github/workflows/gpu-differential.yml` workflow is a
`workflow_dispatch`-only stub. It points operators to the PRD-0009 remote
runbook at `crates/sembla-cuda/scripts/run-differential-corpus.sh`; hosted CI
never presents the stub as GPU evidence.

## Local equivalents

From the repository root, the Rust and determinism checks are:

```sh
./scripts/check.sh
cargo build --workspace --locked
cargo test --workspace --locked
./scripts/check-determinism.sh
```

With `elan` installed, run the same Lean command as CI:

```sh
bash frontend/scripts/check-parity.sh
```

For the NPE smoke test, create the pinned environment described in
`calibration/npe/README.md`, then run:

```sh
PYTHON=calibration/npe/.venv/bin/python ./scripts/check-npe-smoke.sh
```

Workflow YAML is parsed, and the dispatch-only GPU trigger is asserted, with:

```sh
ruby scripts/check-workflow-yaml.rb
```

At implementation time on 2026-07-19, `actionlint` was not installed on the
local machine, so its result is **unanswered** rather than reported as a pass.
When available, the additional one-shot lint is:

```sh
actionlint .github/workflows/*.yml
```
