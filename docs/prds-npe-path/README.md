# Sembla NPE-path PRDs

Ordered PRD set implementing the 2026-07-18 roadmap amendment: v0.2's run
manifest and CUDA backend, the promoted v0.3 observation work, and v0.4's
amortized-NPE calibration path. Designed to be run by
[pi-piprd](https://github.com/ianmoran11/pi-piprd)
(`/piprd run docs/prds-npe-path`). This README is excluded from runs.

## Authority

`DESIGN.md` at the repository root is the design authority, as amended
2026-07-18 (§4.6, §5.4, §10.4–§10.6). Binding companions: `DECISIONS.md` §G5
(amortized NPE, external workflow, replica-noise rule),
[ADR 0001](../decisions/0001-gpu-precision.md) (CUDA native `f64` selected;
no silent fallback), and `docs/ROADMAP.md` as amended (v0.3 re-cut). Every PRD
cites the sections it implements. Where a PRD and DESIGN.md conflict, flag it
in the implementation notes and follow DESIGN.md.

## Run order

| # | PRD | Layer |
|---|-----|-------|
| 0001 | Run manifest + `verify-run` | CLI/contract |
| 0002 | Declared views & summaries: IR + validator | IR |
| 0003 | Observation runtime + SIR-branch retirement | runtime/CLI |
| 0004 | Lean frontend observation parity | frontend |
| 0005 | Sweep: independent-noise mode + `--theta-file` | CLI |
| 0006 | `(θ, x)` training-pairs export | CLI |
| 0007 | NPE reference pipeline (`sbi`, external) | calibration |
| 0008 | CUDA native-`f64` backend | GPU |
| 0009 | Differential harness + backend selection | GPU/CLI |
| 0010 | CI workflows | infra |

**Why this order.** 0001–0007 are CPU-side and deliver the NPE loop end-to-end
on the oracle (the sweep runner is fast enough for the small reference
configs). 0008–0009 then accelerate the same loop per ADR 0001; they need
remote NVIDIA hardware for their GPU-side acceptance, so they come late enough
that a hardware stall cannot block the calibration path. 0010 pins everything
in CI.

## Global conventions (binding on all PRDs)

All conventions in `docs/prds/README.md` remain binding (crate layout,
`f64` numerics, Level A determinism, no `HashMap` order leaks, all randomness
through the Philox coordinate API, `rule_id` assignment, the canonical state
hash, tests stay green across PRDs, golden fixtures freeze wire formats).
Additional conventions for this set:

- **Manifest discipline (DESIGN.md §5.4):** every hash is stored beside a
  named algorithm ID; schema versions are explicit and per-concern; optional
  fields are append-only; related optional fields form all-present-or-
  all-absent tuples that readers must reject when partial. The manifest is one
  file — no archives, no event capture.
- **Observation is a sink (DESIGN.md §4.6):** no view or summary may influence
  state, draws, draw coordinates, conflict resolution, or scheduling. Adding,
  removing, or disabling observation must leave state hashes bitwise
  unchanged, and PRDs test this. Views/summaries need **no** feature flag:
  they cannot change what a run computes, only what it reports.
- **Reserved RNG namespaces (cumulative):** prior/parameter draws use
  `rule_id = u32::MAX` (v0.1 PRD 0013). Replica-seed derivation uses
  `rule_id = u32::MAX - 1` (PRD 0005 here). Validator-assigned rule IDs count
  from zero and can never collide with reserved namespaces.
- **No silent fallback (ADR 0001):** the executing backend is selected
  explicitly. If a requested backend is unavailable, fail with a diagnostic —
  never substitute another backend, precision, or representation. The
  manifest records what actually executed.
- **CUDA build gating:** `cargo build --workspace` and
  `cargo test --workspace` must stay green on machines without a CUDA toolkit
  or GPU. GPU-requiring tests are `#[ignore]` with a documented remote
  execution path (the `spikes/precision/infra-hyperstack/` runbook is the
  reference). Correctness runs are valid on any CUDA-capable NVIDIA GPU
  (native `f64` is exact regardless of fp64 rate — only slower); performance
  statements come only from verified full-rate hardware. This does not weaken
  ADR 0001's full-rate production requirement.
- **Python quarantine (mirrors v0.1 §I5):** the NPE pipeline lives in
  `calibration/npe/`, outside the Cargo workspace, never imported by Rust
  code, with pinned dependencies. It consumes only exported artifacts
  (PRD 0006) — never Sembla internals.
- **Honest reporting (spike culture):** an acceptance criterion that cannot be
  executed in the implementing environment is reported as *unanswered* with
  the reason and the documented path to answer it — never approximated,
  extrapolated, or fabricated.
