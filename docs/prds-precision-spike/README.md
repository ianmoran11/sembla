# Precision spike PRDs (GPU `f64` decision)

Ordered PRD set for the **1-week GPU precision spike** called for in
[`docs/ROADMAP.md`](../ROADMAP.md) as the gate on the v0.2 GPU backend. Designed
to be run by [pi-piprd](https://github.com/ianmoran11/pi-piprd)
(`/piprd run docs/prds-precision-spike/0001-spike-scaffold.md`). This README is
excluded from runs.

## Why this spike exists

The v0.1 GPU throughput spike (`spikes/gpu-throughput/RESULTS.md`) proved the
tick *kernel shape* is fast (~74 ticks/sec at 26M rows on an Apple M2 Pro) **but
only in `f32`** — portable WGSL on Metal exposes no shader `f64`. Sembla's
production numeric contract is `f64` (`DESIGN.md` §9 conventions), so
production-`f64` throughput is **unanswered**, and that gap gates the whole v0.2
backend and the reachability of determinism Levels A/B (`DESIGN.md` §5.2, §10.3).

This spike measures the three candidate precision strategies from the roadmap on
the **precision-critical hot path** — the real-valued segmented reduction and the
segmented-argmin conflict resolution (`DESIGN.md` §4.2, §5.1) — and produces a
written recommendation for the v0.2 precision contract:

- **A. Native `f64`** on capable hardware (NVIDIA via wgpu/Vulkan `SHADER_F64`,
  and a CUDA reference variant).
- **B. Double-single** (compensated `f32`-pair, ~48-bit mantissa) in portable
  WGSL — runs on the existing Metal dev machine.
- **C. Tiered precision by contract** — `f32`/mixed fast path with `f64` only
  where it provably matters (reductions, argmin keys), CPU `f64` oracle as
  ground truth.

## Authority

`DESIGN.md` at the repository root is the design authority. Every PRD cites the
sections it implements. Where a PRD and `DESIGN.md` conflict, flag it in the
implementation notes and follow `DESIGN.md`.

## Run order

| # | PRD | Layer |
|---|-----|-------|
| 0001 | Spike scaffold, hot-path workload, CPU `f64` oracle | spike infra |
| 0002 | Portable WGSL kernels: `f32` baseline + double-single | spike |
| 0003 | Native-`f64` kernels: wgpu `SHADER_F64` + CUDA variant | spike |
| 0004 | Terraform: CUDA-capable NVIDIA GPU instance | infra |
| 0005 | Unified benchmark: throughput × accuracy matrix | spike |
| 0006 | Decision report + `DESIGN.md`/roadmap amendment | doc |

## What this spike is and is not

- **Throwaway measurement, like the v0.1 GPU spike.** The spike crate lives in
  `spikes/precision/`, is a standalone Cargo package (its own empty
  `[workspace]`), is **never** a member of the root workspace, and is never
  depended on by any production crate. `cargo build --workspace` at the repo root
  must not compile it.
- **Not integrated** with `sembla-runtime` or the IR. The accuracy oracle is a
  local scalar `f64` reimplementation of this spike's hot path — do **not** link
  the runtime crate. Philox parity (if used) is checked against copied
  known-answer vectors, exactly as the v0.1 spike does.
- **Its only durable artifacts** are `spikes/precision/RESULTS.md` (the numbers)
  and `docs/decisions/0001-gpu-precision.md` (the decision). The kernels are
  scaffolding for those two documents.

## Global conventions (binding on all PRDs)

- **Standalone crate:** `spikes/precision/` with its own `Cargo.toml` carrying an
  empty `[workspace]` table (or root-workspace `exclude`) so the root workspace
  ignores it. wgpu compute shaders for the portable paths; an optional CUDA
  variant (built via a feature flag / separate `build.rs`, only where a CUDA
  toolkit is present) for the native-`f64` reference.
- **The precision-critical workload** (not the throughput kernel of the v0.1
  spike): a **real-valued** segmented reduction and a segmented argmin, so
  precision actually bites. Integer counts are exact and are not the question;
  see PRD 0001 for the exact workload.
- **Ground truth is a CPU `f64` scalar oracle** computed inside the spike. Every
  GPU precision strategy is scored against it for both throughput and accuracy.
- **Honest hardware reporting** (inherited from the v0.1 spike): record adapter
  name/backend, `SHADER_F64` capability, and — for native `f64` on NVIDIA — the
  device's **fp64:fp32 throughput ratio class** (full-rate A100/V100/H100-class
  at ~1:2 vs. rate-limited T4/L4/A10-class at ~1:32–1:64). A rate-limited number
  must be labelled as such and must not be extrapolated as if it were full-rate.
  If a path cannot run on the available adapter, mark that cell **unanswered**
  rather than reporting a misleading number.
- **Determinism framing:** the segmented reduction's *order sensitivity* is the
  Level A/B question (`DESIGN.md` §5.2); the argmin *winner* correctness is the
  Level C-vs-correctness question (a wrong winner is a wrong simulation, not
  jitter). Both are measured explicitly.
- **Testing:** every PRD lands with the spike's `cargo test` green inside
  `spikes/precision/`, and the root `cargo build --workspace` unaffected.
