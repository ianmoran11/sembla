# Continuous integration

GitHub Actions runs `.github/workflows/ci.yml` on every push and pull request.
It contains these checks:

- **Rust and documentation hygiene:** installs the root `rust-toolchain.toml`
  pin, restores the Cargo cache, parses the workflow YAML, runs the standard-
  library Markdown-link tests and tracked-document scan, then runs
  `scripts/check-rust.sh` for formatting, Clippy, workspace tests,
  dependency/RNG policy, and lock verification. It finishes with the
  dependency-free Lean proof-hygiene guard and does not repeat an equivalent
  Cargo build or test afterward.
- **Determinism:** runs `scripts/check-determinism.sh`, which executes the SIR
  model twice and compares the result CSV, summary CSV, and manifest bytes. It
  also executes the same sweep twice and compares every output and the sweep
  manifest.
- **Lean frontend:** installs the root Rust pin and
  `frontend/lean-toolchain` pin, restores the Cargo and Lake caches, and runs the
  build, elaboration, export, runtime parity, and Cargo-lock checks.
- **NPE smoke:** runs when `calibration/**`, `docs/prds-npe-path/**`, its
  `scripts/check-npe-smoke.sh` or `scripts/check-npe-lock.sh` harness, or
  `.github/workflows/ci.yml` changes. The direct
  `calibration/npe/requirements.txt` input and the
  `calibration/npe/requirements-ci.lock` path are also named explicitly so the
  filter remains self-testing. The job uses CPython 3.12.8 and installs the
  complete Linux/amd64 lock with `--require-hashes`; normal CI never bootstraps
  the lock generator or resolves the direct input. It runs the contract/refusal
  tests and a reduced one-epoch training call, but deliberately does not run
  SBC. The full PRD-0007 statistical acceptance configuration remains
  local/manual.

The separate `.github/workflows/gpu-differential.yml` workflow is a
`workflow_dispatch`-only stub. It points operators to the PRD-0009 remote
runbook at `crates/sembla-cuda/scripts/run-differential-corpus.sh`; hosted CI
never presents the stub as GPU evidence.

## Workflow supply-chain policy

Every non-local `uses:` reference is pinned to a reviewed upstream 40-character
commit SHA, with the corresponding release beside it as a comment. The
repository checker rejects mutable or non-SHA action references and missing
release comments. Repository-local actions may use an explicit `./` path
because their content is fixed by the checked-out commit.

`.github/dependabot.yml` requests grouped weekly updates only for the
`github-actions` ecosystem. Cargo, Python, Terraform, and Lean dependency
updates are intentionally outside this policy. The path-detection and manual
GPU-stub jobs have five-minute timeouts; their existing minimal read permissions
and the GPU workflow's dispatch-only trigger remain unchanged.

## Local check contract

Run these commands from the repository root. Validation Cargo commands use the
committed lock and fail rather than regenerating it.

| Contract | Command | Environment and claim |
| --- | --- | --- |
| Fast Rust | `./scripts/check-rust.sh` | Requires the pinned Rust toolchain and Git, but not Lean. Runs formatting, Clippy, workspace tests, runtime dependency/RNG policy, and verifies `Cargo.lock` is unchanged. |
| Documentation links | `python3 scripts/check-markdown-links.py` | Uses only the Python standard library. Checks tracked Markdown relative targets, ignores `.piprd` managed records, remote/mailto links, images, fenced examples, and pure anchors, and does not claim anchor-fragment validation. |
| Complete local | `./scripts/check.sh` | Requires Cargo, Git, Python 3, and Lake from the pinned Rust/Lean toolchains. Runs the Markdown check and its temporary-fixture tests, Rust contract, Lean proof hygiene, and full frontend parity; a missing tool is an error, never a skip. |
| Determinism | `./scripts/check-determinism.sh` | Requires the pinned Rust toolchain. Repeats CPU run and sweep workflows and compares their outputs byte-for-byte. |
| NPE smoke | `PYTHON=calibration/npe/.venv/bin/python ./scripts/check-npe-smoke.sh` | Requires the pinned Python 3.12 environment described in `calibration/npe/README.md`. This is reduced contract/training evidence, not SBC. |
| NPE lock | `./scripts/check-npe-lock.sh` | Requires Docker. In the immutable Linux/amd64 CPython 3.12.8 image, regenerates and compares the hashed lock, performs a fresh install without isolated build resolution, and runs the full reduced NPE smoke check. |
| GPU manual evidence | `bash crates/sembla-cuda/scripts/run-differential-corpus.sh` | Requires a clean committed worktree and remote NVIDIA CUDA/NVRTC environment. The dispatch-only hosted workflow is a stub and is not GPU evidence. |

The Lean parity component can also be run directly when Git and both pinned
toolchains are installed:

```sh
bash frontend/scripts/check-parity.sh
```

Workflow and Dependabot YAML are parsed; immutable action pins and comments,
the self-testing NPE filter, minimal permissions, short helper-job timeouts, and
the dispatch-only GPU trigger are asserted with:

```sh
ruby scripts/check-workflow-yaml.rb
```

During the 2026-07-23 supply-chain check, `actionlint` was not installed on the
local machine, so its result is **unanswered** rather than reported as a pass.
When available, the additional one-shot lint is:

```sh
actionlint .github/workflows/*.yml
```
