# Continuous integration

GitHub Actions runs `.github/workflows/ci.yml` on every push and pull request.
It contains these checks:

- **Rust:** installs the root `rust-toolchain.toml` pin, restores the Cargo
  cache, parses the workflow YAML, runs `scripts/check-rust.sh` for formatting,
  Clippy, workspace tests, dependency/RNG policy, and lock verification, then
  runs the dependency-free Lean proof-hygiene guard. It does not repeat an
  equivalent Cargo build or test afterward.
- **Determinism:** runs `scripts/check-determinism.sh`, which executes the SIR
  model twice and compares the result CSV, summary CSV, and manifest bytes. It
  also executes the same sweep twice and compares every output and the sweep
  manifest.
- **Lean frontend:** installs the root Rust pin and
  `frontend/lean-toolchain` pin, restores the Cargo and Lake caches, and runs the
  build, elaboration, export, runtime parity, and Cargo-lock checks.
- **NPE smoke:** runs only when `calibration/**` or
  `docs/prds-npe-path/**` changes. It installs the pinned direct Python 3.12
  requirements, runs the contract/refusal tests, and performs a reduced
  one-epoch training call. It deliberately does not run SBC. The full PRD-0007
  statistical acceptance configuration remains local/manual.

The separate `.github/workflows/gpu-differential.yml` workflow is a
`workflow_dispatch`-only stub. It points operators to the PRD-0009 remote
runbook at `crates/sembla-cuda/scripts/run-differential-corpus.sh`; hosted CI
never presents the stub as GPU evidence.

## Local check contract

Run these commands from the repository root. Validation Cargo commands use the
committed lock and fail rather than regenerating it.

| Contract | Command | Environment and claim |
| --- | --- | --- |
| Fast Rust | `./scripts/check-rust.sh` | Requires the pinned Rust toolchain and Git, but not Lean. Runs formatting, Clippy, workspace tests, runtime dependency/RNG policy, and verifies `Cargo.lock` is unchanged. |
| Complete local | `./scripts/check.sh` | Requires Cargo, Git, and Lake from the pinned Rust/Lean toolchains. Runs the Rust contract, Lean proof hygiene, and full frontend parity; a missing tool is an error, never a skip. |
| Determinism | `./scripts/check-determinism.sh` | Requires the pinned Rust toolchain. Repeats CPU run and sweep workflows and compares their outputs byte-for-byte. |
| NPE smoke | `PYTHON=calibration/npe/.venv/bin/python ./scripts/check-npe-smoke.sh` | Requires the pinned Python 3.12 environment described in `calibration/npe/README.md`. This is reduced contract/training evidence, not SBC. |
| GPU manual evidence | `bash crates/sembla-cuda/scripts/run-differential-corpus.sh` | Requires a clean committed worktree and remote NVIDIA CUDA/NVRTC environment. The dispatch-only hosted workflow is a stub and is not GPU evidence. |

The Lean parity component can also be run directly when Git and both pinned
toolchains are installed:

```sh
bash frontend/scripts/check-parity.sh
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
