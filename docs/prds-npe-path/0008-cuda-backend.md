# PRD 0008: CUDA native-`f64` backend

## Context

[ADR 0001](../decisions/0001-gpu-precision.md) is closed: **Strategy A, native
`f64` through CUDA**, selected from three verified full-rate H100 runs (zero
reduction error, zero winner/fired mismatches, median 0.724 ms/tick at 26M
rows). The evidence commit (`d6c545f`) validated the approach — nvcc-compiled
`f64` kernels, fixed-order two-pass reductions, coordinate Philox — on real
hardware. This PRD builds the production backend: unlike the spike's fixed
benchmark kernel, it must execute **any** validated model in the v0.1 kernel
fragment. Per `DECISIONS.md` §E9 there is exactly one GPU path; per ADR 0001
fallback is never silent.

## Goal

A `sembla-cuda` crate executes validated models (current fragment: per-row
guard/hazard evaluation, racing-clock draws, contested-resource argmin,
effects, wire tables — no birth/death) natively in `f64`, device-resident
across ticks, with per-tick state hashes **bitwise equal to the CPU oracle**.

## Specification

- New workspace crate `sembla-cuda` behind a `cuda` cargo *build* feature of
  the workspace members that reference it: `cargo build/test --workspace` on a
  machine without a CUDA toolkit stays green (the feature is off by default;
  this gates *compilation*, not semantics, so `DESIGN.md` §5.5 is not
  implicated — the executing backend is a recorded runtime selection,
  PRD 0009).
- Kernel generation: generate CUDA C from the `ValidatedModel` — one kernel
  per transition for guard+hazard+draw, plus the shared conflict-resolution
  and effect-application kernels — compiled at model load (NVRTC, or nvcc at
  build time for the fixed harness with NVRTC for model kernels; the
  implementer picks one, documents it, and there is no interpreter fallback:
  one path, `DECISIONS.md` §E9). Generated source must be deterministic
  (stable ordering) and dumpable via an env var for debugging.
- Expression semantics mirror `sembla-runtime`'s evaluator exactly — same
  operations, same `f64` semantics, same edge cases. Shared test vectors
  assert device Philox output is bit-identical to the PRD-0003 CPU
  implementation.
- Reductions and scatters use the **same fixed order** as the CPU oracle
  (fixed-shape two-pass trees, sorted scatters, lexicographic tie-breaks) so
  `f64` results are bitwise equal, as the spike's zero mirror differences
  evidenced. No atomics on any result-bearing path (Level A, `DESIGN.md`
  §5.2).
- State uploads once, stays device-resident across ticks; per-tick state
  hashing available behind a debug flag (downloads each tick) for the
  differential harness; default hashes final state only.
- Backend construction fails with an explicit diagnostic when no device or
  toolkit is present — never a silent CPU substitution (ADR 0001).
- GPU-requiring tests are `#[ignore]`; a documented script runs them
  remotely (reference the `spikes/precision/infra-hyperstack/` runbook).
  Correctness runs are valid on any CUDA GPU; only performance claims require
  full-rate hardware (README convention).

## Non-goals

CLI integration and the differential corpus (PRD 0009). Per-box heterogeneous
dispatch (the v0.5 hybrid). Birth/death, group-by views on GPU beyond what
model execution needs. wgpu/Vulkan (gated off on H100 per ADR 0001).
Performance work beyond honest measurement — the ADR gate already ran.

## Acceptance criteria

1. Without CUDA: `cargo build --workspace` and `cargo test --workspace` green
   and unchanged; the crate's non-GPU unit tests (codegen determinism,
   generated-source golden fixture for the SIR model) run everywhere.
2. Philox vectors: device output bit-identical to CPU for the checked-in
   coordinate test vectors (GPU test).
3. Oracle equality (GPU test): the SIR example at 100k persons for 200 ticks —
   per-tick state hash sequence bitwise equal to the CPU oracle's; likewise
   for the two-box `sir_policy` model and one canonical model.
4. Level A (GPU test): the same GPU run twice ⇒ byte-identical hashes.
5. No-device behavior: requesting the backend without a device produces the
   specified diagnostic (testable locally, not ignored).
6. GPU-side results are reported per the honest-reporting convention: if no
   GPU was reachable during implementation, criteria 2–4 are recorded as
   *unanswered* with the remote runbook to answer them — never simulated.
