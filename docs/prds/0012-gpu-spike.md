---
max_review_cycles: 3
---

# PRD 0012: GPU throughput spike (throwaway)

## Context

v0.1 deliberately ships without a GPU backend (`DESIGN.md` §9): the risk to
retire now is **throughput**, not compilability — the semantics was designed
GPU-legal by construction (§4.2). This spike measures whether the core tick
workload hits credible ticks/sec at Australian-population scale. It is
**explicitly throwaway**: it lives outside the Cargo workspace, is never
depended on, and its only durable artifact is a results document.

## Goal

A standalone benchmark in `spikes/gpu-throughput/` (own crate, wgpu compute
shaders for portability) that runs the SIR tick's kernel skeleton at 26M rows
and records measured throughput in `spikes/gpu-throughput/RESULTS.md`.

## Specification

- Own `Cargo.toml` (NOT a workspace member; add an explicit `[workspace]`
  empty table or exclude it so the root workspace ignores it).
- **Precision exception for this throwaway spike:** if the selected portable
  WGSL adapter does not expose shader `f64`, the benchmark may use `f32` for
  hazard/race arithmetic. Record the adapter capability and precision, treat
  the measured rate only as directional kernel-shape evidence, and mark
  production-`f64` throughput unanswered. This does not relax the production
  numeric contract.
- Workload per simulated tick, on GPU end to end (26M `person` rows, 1.3M
  `employer` groups, data resident on device; sized down automatically if
  the adapter's memory requires — record actual sizes):
  1. **Philox4x32-10 in WGSL** keyed by the PRD-0003 coordinate scheme;
     validated in a test against at least 4 known-answer vectors computed by
     the CPU implementation (copy the expected values into the spike's test;
     do not link the runtime crate).
  2. **Aggregate**: per-employer count of infectious persons (segmented
     reduction or atomics — implementer's choice; note which was used, since
     it's the Level A vs C fork, §5.2).
  3. **Map**: per-person hazard λ from the broadcast count, exponential race
     sample, candidate flag.
  4. **Segmented argmin** by a contested-resource key over ~10% of rows
     (simulating conflict resolution), with the lexicographic tie-break.
  5. State write (enum column update) + a device-side counter of fired rows.
- Measure: steady-state ms/tick (median over ≥ 100 ticks after warmup),
  broken down per kernel via timestamp queries where the adapter supports
  them; plus rows/sec.
- Run on whatever adapter is available (`wgpu` auto-select); record adapter
  name/backend. If only a software adapter (e.g. lavapipe) is available,
  run at reduced scale, record that fact prominently, and mark the
  throughput question **unanswered** in RESULTS.md rather than reporting
  misleading numbers.
- `RESULTS.md` must state: hardware, sizes, ms/tick per kernel, total,
  extrapolation to 26M if run smaller, the atomics-vs-deterministic-reduction
  choice made, and a one-paragraph verdict against the v0.1 success
  criterion #4 (`DESIGN.md` §9): is a credible ticks/sec at 26M rows
  plausible, and what looks like the bottleneck for the real v0.2 backend.

## Non-goals

Integration with `sembla-runtime` or the IR, multi-GPU, Level B portability,
optimizing beyond obvious kernel hygiene (this is a measurement, not a
product), CUDA-specific code.

## Acceptance criteria

1. `cargo build` and `cargo test` succeed inside `spikes/gpu-throughput/`;
   the root workspace build remains unaffected (`cargo build --workspace` at
   the repo root does not compile the spike).
2. The WGSL Philox known-answer test passes against the CPU-derived vectors.
3. A correctness smoke test runs 1 tick at 10k rows on the available adapter
   and cross-checks fired-candidate counts against a scalar CPU
   reimplementation inside the spike (exact match on candidate flags given
   identical draws).
4. `cargo run --release` executes the benchmark and writes/updates
   `RESULTS.md` with all fields specified above, including adapter identity
   and the explicit verdict paragraph (or the explicit "unanswered on this
   hardware" statement for software adapters).
5. `RESULTS.md` is committed with the numbers from the machine the spike ran
   on.
