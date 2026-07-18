# PRD 0009: Differential harness and backend selection

## Context

v0.2's definition (ROADMAP): a production GPU backend *differentially tested
against the CPU oracle* — every example model runs on both paths and state
hashes match where the determinism level promises bitwise equality (they do:
CUDA native `f64` with fixed-order kernels, ADR 0001). The PRD-0001 manifest
must record the backend that actually executed, and fallback is prohibited
from being silent.

## Goal

`--backend cpu|cuda` across the CLI, truthful manifest recording, and a
differential corpus command that proves oracle equality over every checked-in
example.

## Specification

- `run`, `sweep`, and `compare` accept `--backend cpu|cuda` (default `cpu`).
  Requesting `cuda` where unavailable exits nonzero with the PRD-0008
  diagnostic — no fallback, no warning-and-continue (ADR 0001).
- Manifest backend-identity tuple records reality:
  `{"backend": "cpu-oracle", ...}` or
  `{"backend": "cuda-native-f64", "precision": "f64", "fell_back": false}`,
  plus append-only `gpu_model` and `driver_version` fields (all-or-nothing
  with the cuda backend value). `verify-run` on a CUDA-produced manifest
  re-runs on the recorded backend.
- `sembla diff-backends <model.json> [run args]`: runs the model on both
  backends with per-tick hashing enabled, compares per-tick state hashes,
  results bytes, and summaries bytes; exit 0 only on full bitwise equality,
  else a first-divergence report (tick, hash pair). Corpus mode
  `--all-examples` iterates every `examples/*.json`.
- Differential corpus test (`#[ignore]`, GPU): `diff-backends --all-examples`
  passes. Record the run per the evidence culture: a dated markdown note
  under `spikes/precision/evidence/` naming commit, GPU, driver, and the
  corpus result (the lightweight sibling of the ADR bundle, not a new
  format).
- Throughput, informational only: `diff-backends` prints ticks/sec per
  backend. On a full-rate machine, run the 26M-row workload shape once and
  record the number in the evidence note beside the ADR's 1,380.5 ticks/sec
  reference. No performance gate — the ADR gate already ran.
- Documentation: the harness docs carry the agreed hardware paragraph —
  correctness CI may run on any CUDA-capable NVIDIA GPU because native `f64`
  is exact regardless of fp64 rate; performance statements come only from
  verified full-rate hardware; neither weakens ADR 0001's full-rate
  production requirement.

## Non-goals

Level B (cross-hardware bitwise — explicitly unproven, ADR 0001). Multi-GPU.
Per-box dispatch. CI wiring (PRD 0010). Tolerance-based comparison paths
(nothing ships that needs one: the selected contract is bitwise).

## Acceptance criteria

1. `cargo test --workspace` green without CUDA; all prior tests pass.
2. Backend selection tests (local): default is `cpu`; `--backend cuda`
   without a device exits nonzero with the specified diagnostic; the manifest
   from a CPU run records the cpu tuple exactly.
3. GPU tests (`#[ignore]`, remote): the differential corpus passes; a CUDA
   run's manifest records the cuda tuple with `gpu_model`/`driver_version`;
   `verify-run` round-trips it; Level A repeat run is byte-identical.
4. The evidence note exists with commit, GPU, driver, corpus verdict, and the
   informational throughput — or records *unanswered* with the runbook if no
   GPU was reachable (honest-reporting convention).
5. Partial GPU tuple (e.g. `gpu_model` without `driver_version`) is rejected
   by the manifest reader (test).
6. Harness docs contain the hardware paragraph; `docs/examples/sir.md` shows
   a `--backend cuda` invocation.
