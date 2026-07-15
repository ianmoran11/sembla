---
max_review_cycles: 3
---

# PRD 0002: Portable WGSL kernels — `f32` baseline + double-single

## Context

Strategy **B** (double-single) and the `f32` baseline both run on the existing
Apple M2 / Metal dev machine, so they can be measured without new hardware. This
PRD implements both in portable WGSL against the PRD-0001 workload and scores
them for **accuracy** against the CPU `f64` oracle. Double-single (a.k.a. df64 /
compensated `f32`-pair, Dekker/Knuth two-sum + Dekker product) gives ~48 bits of
mantissa using only `f32` ops — the portable candidate for hitting production
precision without shader `f64`.

Implements: `DESIGN.md` §4.2, §5.1, §5.2; `docs/ROADMAP.md` v0.2 option B and the
`f32` baseline it is compared against.

## Goal

Two WGSL implementations of the segmented-reduce + map + segmented-argmin hot
path — one in single `f32`, one in double-single — that run on the portable
adapter, plus accuracy tests scoring each against the PRD-0001 `f64` oracle for
(a) reduction relative error and (b) argmin winner-mismatch rate.

## Specification

- **Double-single primitives** (`src/wgsl/df64.wgsl`, included into the kernel
  module): `df64` as `vec2<f32>` (hi, lo); implement `two_sum`, `quick_two_sum`,
  `two_prod` (or FMA-free Dekker split), `df_add`, `df_mul`, `df_div` (enough for
  the reduction and the hazard/race arithmetic). Document each and note that
  correctness depends on the adapter **not** contracting to FMA or enabling
  fast-math — assert the shader is compiled without fast-math relaxations and
  record whether the adapter honors it (this is itself a Level B finding,
  `DESIGN.md` §10.3).
- **Segmented reduction, two variants:**
  - `f32`: per-employer `Σ w_i` accumulated in `f32` (atomics or two-pass
    segmented — note which; atomics on `f32` are the Level C fork, `DESIGN.md`
    §5.2, and require a compare-exchange loop since WGSL lacks native `f32`
    atomic add — implement and note it).
  - `df64`: the same reduction in double-single, in a **fixed segmented order**
    (two-pass, not atomics) so it is order-deterministic — the Level A-style
    reference for portability. Document the pass structure.
- **Map + race:** hazard `λ`, uniform from the WGSL Philox (reuse/port the v0.1
  spike's `philox4x32-10`; validate against ≥4 copied known-answer vectors as the
  v0.1 spike does — do not link the runtime), and `t_i = -ln(1-u)/λ` in each
  precision. `ln` in double-single may use an `f32` `log` refined by one
  Newton/Taylor correction step; document the accuracy of the chosen `ln`.
- **Segmented argmin:** winner per contested key by argmin over `t_i` with the
  `(t_bits, rule_id, entity_id)` tie-break, computed from each precision's `t_i`.
  For `df64`, the argmin comparison uses the double-single `t`; for `f32`, the
  single `t`.
- **Accuracy scoring** (in tests, against the PRD-0001 oracle at a scale that
  fits the dev adapter, e.g. 1M rows):
  - reduction: max and mean relative error of segmented sums per strategy;
  - argmin: fraction of contested keys whose **winner entity differs** from the
    `f64` oracle's winner — the correctness-critical metric (an `f32` near-tie
    that flips the winner is a wrong simulation, not jitter);
  - report both per strategy; assert `df64` reduction error ≤ a documented
    threshold well below `f32`'s, and that `df64` winner-mismatch rate ≤ `f32`'s.
- **Steady-state throughput hooks** are added but the full benchmark matrix is
  PRD 0005; here just expose per-strategy dispatch entry points and a 1-tick
  correctness smoke path.

## Non-goals

Native `f64` (PRD 0003), CUDA, terraform, the full throughput matrix and
RESULTS.md (0005), the decision writeup (0006), optimizing double-single beyond a
correct, documented implementation.

## Acceptance criteria

1. `cargo test` inside `spikes/precision/` passes; root `cargo build --workspace`
   still excludes the spike.
2. WGSL Philox known-answer test passes against ≥4 copied CPU-derived vectors.
3. Both `f32` and `df64` hot paths run 1 tick on the available adapter and their
   outputs are scored against the PRD-0001 `f64` oracle; the accuracy report
   (reduction error + winner-mismatch rate per strategy) is asserted and printed.
4. `df64` is measurably closer to the `f64` oracle than `f32` on both metrics, by
   the documented thresholds.
5. Fast-math / FMA-contraction status of the compiled shader is recorded (it
   determines whether `df64` is trustworthy on this adapter).

## Implementation notes

- Both strategies use a deterministic two-pass reduction: two ascending-row
  partials per employer, then an ordered partial-0/partial-1 merge. No floating
  atomics are used.
- The 1M-row accuracy guard fixes seed `0x0123456789abcdfc`, tick 7, and `dt=100`.
  That seed includes a copied Philox near-tie whose 24-bit uniforms tie but whose
  53-bit uniforms do not. The double-single guards are max relative reduction
  error `<= 1e-10`, max/mean error each `<= 1%` of f32, and a strictly lower
  winner-mismatch rate.
- Apple M2 Pro / Metal results: f32 max/mean reduction error
  `1.394316e-7` / `3.214444e-8`, 1/50,000 winner mismatches; double-single
  `9.292057e-15` / `1.205181e-15`, 0/50,000 mismatches.
- wgpu 0.20 exposes no public fast-math control. The spike pins the published
  `wgpu-hal 0.21.1` source (upstream commit
  `14a7698d16f0f5bcdf8cd6d515952441d4bd2585`) and minimally patches its Metal
  compiler options with `set_fast_math_enabled(false)`. The WGSL has no atomic
  rounding fences. Trust requires strict mode to be requested on Metal, no
  observed contraction or reassociation, and preserved two-sum/Dekker-product
  residuals; unsupported backends are not assumed strict.
