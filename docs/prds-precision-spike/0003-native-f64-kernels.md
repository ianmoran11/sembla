---
max_review_cycles: 3
---

# PRD 0003: Native-`f64` kernels — wgpu `SHADER_F64` + CUDA reference

## Context

Strategy **A** (native `f64`) is the cleanest semantics but is **not available on
the Apple M2 / Metal dev machine** (`SHADER_F64` unsupported — the finding that
motivated this whole spike). Native `f64` needs an NVIDIA GPU, reached two ways:
wgpu on the Vulkan backend requesting `Features::SHADER_F64`, and — as the honest
reference, since wgpu's `f64` maturity varies — an optional CUDA variant. This
PRD implements both paths so they are *buildable and unit-tested locally* and
*run for real* on the hardware PRD 0004 provisions. On an adapter without
`SHADER_F64`, the path must **gracefully mark itself unanswered**, exactly as the
v0.1 spike does for `f64`.

Crucially, the spike must record the device's **fp64 throughput class**: NVIDIA
datacenter-compute GPUs (A100/V100/H100) run `f64` at ~1:2 of `f32`, while
commodity GPUs (T4/L4/A10/gaming) rate-limit `f64` to ~1:32–1:64. A "native
`f64`" number is only meaningful with that ratio attached.

Implements: `DESIGN.md` §4.2, §5.1, §5.2, §10.3; `docs/ROADMAP.md` v0.2 option A.

## Goal

A native-`f64` implementation of the PRD-0001 hot path in (a) WGSL requesting
`SHADER_F64` on a Vulkan/NVIDIA adapter and (b) an optional CUDA kernel behind a
feature flag, each producing results scored against the CPU `f64` oracle, with
graceful "unanswered" behavior where `f64` is unsupported.

## Specification

- **wgpu native-`f64` path** (`src/wgsl/f64_native.wgsl` + Rust dispatch):
  - Request `Features::SHADER_F64` at device creation; if the adapter lacks it,
    do **not** error — record `native_f64: unsupported` and skip, so the crate
    still builds and the 0002 paths still run (the Metal dev machine hits this
    branch).
  - Segmented reduction in native `f64` (two-pass, fixed order — the Level A
    reference); map/race in `f64`; segmented argmin over `f64` `t_i` with the
    same lexicographic tie-break.
- **CUDA reference variant** (optional, `--features cuda`): a `.cu` kernel for the
  same hot path in `double`, compiled only when a CUDA toolkit is detected
  (`build.rs` probes `nvcc`; absent ⇒ feature is a no-op that records
  `cuda: toolkit-absent`). This exists because it is the least-ambiguous native
  `f64` measurement and cross-checks the wgpu/Vulkan `f64` path. Keep it minimal:
  same workload, same tie-break, host-side comparison to the oracle.
- **fp64 throughput class detection/recording:** query and record the device name
  and, where available, the fp64:fp32 ratio (from device properties or a
  documented lookup by GPU model); classify as `full-rate` (~1:2) or
  `rate-limited` (~1:32+). This classification is mandatory in the output — a
  rate-limited number must never be extrapolated as if full-rate.
- **Accuracy:** native `f64` is expected to match the CPU `f64` oracle to within
  reduction-order effects only. Assert winner-mismatch rate is ~0 (any nonzero is
  a reduction-order artifact and must be explained), and quantify residual
  reduction differences vs the oracle's fixed order.
- **Local testability without a GPU:** all `f64` scalar arithmetic used by the
  kernels has a Rust `f64` mirror that is unit-tested on the dev machine (the
  kernels themselves are exercised only where a capable adapter exists). On the
  Metal dev machine, `cargo test` passes with the native paths reporting
  `unsupported`/`toolkit-absent` rather than failing.

## Non-goals

Provisioning the NVIDIA hardware (PRD 0004 — this PRD only *runs on* it), the
throughput matrix and RESULTS.md (0005), the decision writeup (0006), multi-GPU,
optimizing the CUDA kernel beyond a correct reference, Level B portable-bitwise
across two different NVIDIA models (noted as future work, not measured here).

## Acceptance criteria

1. `cargo build` and `cargo test` succeed inside `spikes/precision/` on the dev
   machine, with the native-`f64` and CUDA paths reporting `unsupported` /
   `toolkit-absent` gracefully (no failure); root `cargo build --workspace`
   still excludes the spike.
2. On an adapter exposing `SHADER_F64`, the native-`f64` hot path runs 1 tick and
   is scored against the CPU `f64` oracle with ~0 winner mismatches (deviations
   explained as reduction-order artifacts).
3. The CUDA variant builds when `nvcc` is present (`--features cuda`) and is a
   documented no-op otherwise.
4. Device fp64 throughput class (`full-rate` vs `rate-limited`) is detected/
   recorded alongside the device name; the output refuses to extrapolate a
   rate-limited number as full-rate.
5. The Rust `f64` mirrors of the kernel arithmetic are unit-tested and pass on
   the dev machine.
