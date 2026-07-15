---
max_review_cycles: 3
---

# PRD 0001: Precision-spike scaffold, hot-path workload, CPU `f64` oracle

## Context

The v0.1 GPU throughput spike answered "is the kernel shape fast?" but only in
`f32`, leaving the production-`f64` question open (`spikes/gpu-throughput/RESULTS.md`,
`DESIGN.md` §9). This PRD set answers "which precision strategy should the v0.2
GPU backend adopt?" by benchmarking three strategies on the **precision-critical**
hot path. This first PRD builds the standalone crate, defines the workload and
its parameters, and — critically — implements the **CPU `f64` scalar oracle**
that is the accuracy ground truth for every later PRD. No GPU code lands here
beyond adapter enumeration; the point is a trustworthy reference and a stable
workload definition the precision kernels are scored against.

Implements: `DESIGN.md` §4.2 (relational kernel fragment: group-by monoid
reduction, map, Philox-by-coordinate), §5.1 (argmin conflict resolution with
lexicographic tie-break), §5.2 (determinism levels); `docs/ROADMAP.md` v0.2
precision fork.

## Goal

A standalone crate `spikes/precision/` that (a) compiles outside the root
workspace, (b) defines the real-valued segmented-reduce + segmented-argmin
workload with documented sizing, and (c) computes the CPU `f64` reference result
(segmented sums, per-resource argmin winners, fired flags) used as ground truth
by PRDs 0002–0005.

## Specification

- **Crate layout.** `spikes/precision/Cargo.toml` with an empty `[workspace]`
  table so the root workspace ignores it (mirror `spikes/gpu-throughput/`).
  Dependencies limited to what the portable path needs now (`bytemuck`,
  `pollster`, `wgpu`); no dependency on any `sembla-*` crate. Confirm
  `cargo build --workspace` at the repo root does **not** build this crate.
- **Workload definition** (`src/workload.rs`), one simulated tick over the
  SIR-shaped data, deliberately **real-valued** so precision bites (unlike the
  v0.1 spike's integer counts):
  - `N` person rows (default 26_000_000, auto-downscaled to adapter limits with
    the reason recorded), `G` employer groups (default 1_300_000, ~20/group),
    contiguous group layout (persons sorted by employer) so segmented kernels are
    well-defined.
  - Per person: `employer: u32`, `health: u32` enum (`S=0,I=1,R=2`), and a real
    susceptibility weight `w ∈ (0,1)` generated deterministically from Philox
    coordinates (document the reserved coordinate scheme; `w` is the value whose
    reduction precision is under test).
  - **Segmented reduce (real-valued monoid):** per employer, `sum_g = Σ w_i`
    over infectious persons in group `g`. This is the floating-point
    commutative-monoid reduction whose precision and order-sensitivity are the
    Level A/B question.
  - **Map:** per susceptible person, hazard `λ_i = beta * sum_{employer_i} /
    groupSize`, uniform `u_i` from Philox, exponential race time
    `t_i = -ln(1 - u_i) / λ_i`. Real-valued; `beta`, `dt` are config constants.
  - **Segmented argmin (conflict resolution):** over a contested key covering
    ~10% of rows (document the selector, as the v0.1 spike does), pick the
    winner per key by argmin over `t_i` with the exact lexicographic tie-break
    `(t_bits, rule_id, entity_id)` from `DESIGN.md` §5.1. Losers are suppressed.
- **CPU `f64` oracle** (`src/oracle.rs`): a straightforward scalar `f64`
  reimplementation of the three stages above, computed in a **fixed, documented
  reduction order** (ascending entity id within group) so it is a single
  well-defined reference. Exposes, per tick: the vector of segmented sums (as
  `f64`), the per-key winner entity id, and the fired-flag vector. This file
  must not use any GPU type and must not import a `sembla-*` crate.
- **Sizing + adapter probe** (`src/lib.rs`): enumerate the default wgpu adapter,
  record name/backend/device-type and `SHADER_F64` (`Features::SHADER_F64`)
  capability, and compute the safe `(N, G)` for its buffer/memory limits with the
  downscale reason. No compute dispatch yet — just the probe and sizing, reused
  by later PRDs.
- **Determinism instrumentation hook.** The oracle also computes the segmented
  sums a second time in a **reversed** reduction order and reports, per group,
  whether the `f64` result differs bitwise — the baseline "how order-sensitive is
  this reduction even at `f64`?" signal that PRDs 0002–0003 compare their GPU
  strategies against.

## Non-goals

GPU compute kernels (PRD 0002+), double-single or native-`f64` arithmetic
(0002/0003), terraform (0004), the benchmark harness and RESULTS matrix (0005),
any integration with `sembla-runtime` or the IR, CUDA.

## Acceptance criteria

1. `cargo build` and `cargo test` succeed inside `spikes/precision/`; the root
   `cargo build --workspace` does not compile the spike (assert in the PRD notes
   which mechanism excludes it).
2. The workload and its sizing/downscale logic are covered by a test at a small
   scale (e.g. 10k rows / 500 groups) with contiguous group layout asserted.
3. The CPU `f64` oracle is deterministic: two runs at fixed seed produce
   byte-identical segmented sums, winners, and fired flags (asserted).
4. The reversed-order reduction diagnostic runs and reports the count of
   order-sensitive groups at the test scale (asserted to be well-defined, not a
   specific value).
5. Adapter probe prints adapter name/backend and `SHADER_F64` capability, and the
   computed safe `(N, G)` with any downscale reason.
