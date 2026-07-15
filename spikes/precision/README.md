# Precision strategy spike

Standalone throwaway crate for choosing the v0.2 GPU precision strategy. PRD
0001 provides the stable real-valued workload, adapter sizing probe, and scalar
CPU `f64` oracle. PRD 0002 adds portable WGSL `f32` and double-single GPU paths;
PRD 0003 adds capability-gated native WGSL/Vulkan and optional CUDA `f64` paths.

## Workspace isolation

The empty `[workspace]` table in this crate's `Cargo.toml` makes
`spikes/precision/` a nested workspace root. The repository root also lists only
its three production crates as workspace members. Consequently, root
`cargo build --workspace` does not discover or compile this spike; run Cargo
commands from this directory to build it.

## Workload contract

Defaults are 26,000,000 person rows and 1,300,000 contiguous employer groups
(20 rows/group). Each person has `employer: u32`, SIR `health: u32`, and a
Philox-generated binary64 susceptibility `weight` in `(0, 1)`. A tick performs:

1. fixed-order segmented sums of infectious weights;
2. susceptible hazard/race mapping with `lambda = beta * sum / group_size` and
   `t = -ln(1-u) / lambda`;
3. segmented argmin for rows selected by `entity_id % 10 == 5`, keyed by
   employer and ordered exactly by `(t_bits, rule_id, entity_id)`.

An eligible unselected row fires directly. For selected rows, only the per-key
winner fires. The oracle also repeats each group sum in descending entity order
and records bitwise differences from its canonical ascending order.

Static weights reserve Philox coordinates
`(tick=0, rule_id=0xffff_fe00, entity_id, draw_idx=0)`. Infection races use
`(tick, rule_id=0, entity_id, draw_idx=0)`. The key is `(seed_lo, seed_hi)` and
counter words are `(tick, rule_id, entity_id, draw_idx)`.

## Probe and sizing

```console
cargo run
```

The probe prints adapter name/backend/device type, `SHADER_F64`, safe `(N, G)`,
and any downscale reason. The sizing model checks the largest 8-byte storage
binding and a documented resident footprint of 32 bytes/person plus 12
bytes/employer. Because wgpu exposes no portable heap-budget query, aggregate
resident bytes are conservatively bounded by `min(max_buffer_size, 1 GiB)`.
Software adapters additionally use a 200,000-row functional safety cap.

## Portable precision kernels

`src/wgsl/portable.wgsl` provides the `f32` and double-single entry points;
`src/wgsl/df64.wgsl` supplies the Knuth/Dekker primitives. Both reductions are
atomics-free and deterministic:

1. pass 1 scans two fixed ascending-row halves per employer into partial sums;
2. pass 2 merges partial 0 followed by partial 1;
3. the row-parallel map computes Philox uniforms and exponential race times;
4. one invocation per employer scans contested rows and applies the
   precision-specific `(time, rule_id, entity_id)` key.

`PortableRunner::{dispatch_reduction_only, dispatch_map_argmin_only,
dispatch_tick_only}` retain buffers for later steady-state timing. The
`dispatch_f32` and `dispatch_df64` methods add correctness readback.

The double-single logarithm starts with WGSL `log(x_hi)` and adds one Newton
correction `(x-exp(y0))/exp(y0)` in double-single. Its omitted term is quadratic
in the f32 intrinsic residual (approximately `1e-14` for a few-ulp residual).

### Accuracy guard

The committed smoke path uses 1,000,000 rows / 50,000 groups at tick 7. Seed
`0x0123456789abcdfc` contains a documented real Philox near-tie: entities
756845 and 756855 have identical high 24 uniform bits but distinct 53-bit
uniforms. `dt=100` admits both clocks, making the f32 winner error deterministic.
The assertions require:

- double-single max reduction relative error `<= 1e-10`;
- double-single max and mean reduction errors each `<= 1%` of f32;
- double-single winner-mismatch rate strictly below f32.

On the Apple M2 Pro / Metal implementation run, f32 reported max/mean reduction
errors `1.394316e-7` / `3.214444e-8` and 1/50,000 winner mismatches;
double-single reported `9.292057e-15` / `1.205181e-15` and 0/50,000.

### Strict Metal compilation and FMA finding

wgpu 0.20 has no public switch for disabling backend fast math. This standalone
spike therefore pins the published `wgpu-hal 0.21.1` source under
`vendor/wgpu-hal/` (upstream commit
`14a7698d16f0f5bcdf8cd6d515952441d4bd2585`) and applies one Metal-only change:
`CompileOptions::set_fast_math_enabled(false)`. See
`vendor/wgpu-hal/SEMBLA-PATCH.md` for provenance. The WGSL uses ordinary
Knuth/Dekker operations without atomic rounding fences.

The compiled-shader behavior probe requires the unfenced multiply-add and sum
to show neither contraction nor reassociation, and requires the expected
Knuth-two-sum and Dekker-product residuals. Double-single is marked trustworthy
only when strict compilation was requested on the supported Metal backend and
all probes pass. Other backends are reported unsupported rather than assumed to
honor a strict mode they do not expose.

## Native f64 strategies

`src/wgsl/f64_native.wgsl` is a separate shader module. It is created only when
the selected adapter is Vulkan and advertises `Features::SHADER_F64`; device
creation explicitly requests that feature. Metal therefore reports
`native_f64: unsupported` without parsing or compiling the f64 source. A local
Naga test still parses and validates the source with the `FLOAT64` capability,
so shader syntax is checked without suitable hardware.

The WGSL path uses `f64` weights, partials, sums, race times, `beta`, and `dt`.
Its reduction is the same two ascending halves plus ordered merge used by the
CUDA reference and `f64_mirror.rs`. Because all race times are non-negative,
numeric f64 ordering is identical to ordering the IEEE `t_bits`; rule and entity
IDs complete the lexicographic tie-break. `NativeF64Runner` exposes retained
reduction-only, map/argmin-only, full-tick, and correctness-readback dispatches.
The accuracy report distinguishes oracle sum differences explained by the fixed
reduction tree from unexplained GPU/mirror differences and requires exactly zero
winner and fired-flag mismatches.

### fp64 throughput class

Every supported result includes the exact device name, an fp64:fp32 ratio when
known, `full-rate` or `rate-limited`, evidence, and a full-rate extrapolation
guard. CUDA uses the authoritative
`cudaDevAttrSingleToDoublePrecisionPerfRatio`. The wgpu fallback recognizes
NVIDIA A100/V100/H100/H200/GH200 as approximately 1:2 full-rate, T4 as
approximately 1:32, and L4/A10/A10G/RTX/GTX/GeForce as conservatively 1:64.
Unknown models are also conservatively rate-limited with an unknown ratio.
Only the full-rate class permits full-rate extrapolation; all other output says
`full-rate-extrapolation=refused`.

The model table follows NVIDIA's published device specifications and exists only
because wgpu exposes no throughput-ratio property. CUDA's runtime attribute
overrides the name lookup, including when it contradicts a familiar model name.

### Optional CUDA reference

`src/cuda/f64_native.cu` implements Philox, the fixed two-pass reduction,
double-precision map/race, and exact `(t_bits, rule_id, entity_id)` argmin. Its C
ABI copies the same Rust workload to device memory and returns sums, winners,
and fired flags for host-side oracle scoring. The build intentionally omits
fast math and passes `--fmad=false`, `--prec-div=true`, and `--prec-sqrt=true`.

```console
cargo test                         # Metal: native_f64 unsupported; CUDA disabled
cargo test --features cuda         # no nvcc: cuda: toolkit-absent, successful no-op
WGPU_BACKEND=vulkan cargo test native_f64 -- --nocapture
cargo test --features cuda -- --nocapture
```

`build.rs` probes `NVCC`, then `$CUDA_HOME/bin/nvcc`, then `nvcc` on `PATH`.
When the feature is enabled but none executes, no CUDA symbols or libraries are
linked. A detected but broken toolkit fails the build rather than being
misreported as absent. Finding `nvcc` does not imply a CUDA device is available;
that runtime case is separately reported as `cuda: device-unavailable`.
