# ADR 0001: GPU precision strategy

- **Status:** Accepted — Strategy A, native `f64` through CUDA
- **Date:** 2026-07-18
- **Decision scope:** v0.2 GPU backend precision and reachable determinism levels
- **Evidence:** [`spikes/precision/RESULTS.md`](../../spikes/precision/RESULTS.md) and the [verified three-run H100 bundle](../../spikes/precision/evidence/hyperstack-h100-20260718/README.md)

## Context

The v0.1 throughput spike established that the hot-path kernel shape is fast on
an Apple M2 Pro, but portable WGSL/Metal exposes no shader `f64`. Its `f32`
throughput therefore did not answer whether a production GPU backend can preserve
Sembla's CPU `f64` semantics at an acceptable cost. That gap gates v0.2: the
precision representation controls both the hardware that the backend can use and
which determinism guarantees in `DESIGN.md` §5.2 are credible.

The precision spike compared three strategies:

- **A — native `f64`** through wgpu/Vulkan and an independent CUDA reference;
- **B — double-single**, an approximately 48-bit `f32`-pair representation;
- **C — tiered precision**, retaining the CPU `f64` interpreter as truth while an
  explicitly reduced GPU contract permits `f32` or mixed precision.

Strategy letters in this record are not determinism levels. A strategy must
separately state whether it can deliver Level A, B, or C.

## Evidence

The canonical merged result is
[`spikes/precision/RESULTS.md`](../../spikes/precision/RESULTS.md), byte-identical
to verified H100 run 1. The complete three-run bundle, external completion logs,
hardware report, bootstrap diagnostics, trusted host-key evidence, and computed
summary are tracked under
[`spikes/precision/evidence/hyperstack-h100-20260718/`](../../spikes/precision/evidence/hyperstack-h100-20260718/README.md).

Both machines ran the full `(N, G) = (26,000,000, 1,300,000)` workload with
`beta = 0.35`, `dt = 0.25`, 10 warmup ticks, and 100 measured ticks. The earlier
Apple M2 Pro result remains useful supporting evidence:

| Strategy | Apple total ms/tick | Apple rows/sec | Reduction relative error, max / mean | Winner mismatch |
|---|---:|---:|---:|---:|
| `f32` | 10.711042 | 2,427,401,667.709 | `1.714669e-7` / `3.216831e-8` | 0.000000% |
| double-single | 13.868958 | 1,874,690,225.466 | `1.096998e-14` / `1.206441e-15` | 0.000000% |

The binding gate used three NVIDIA-local rows from one `n3-H100x1` machine,
not the rendered cross-machine matrix. The device was an NVIDIA H100 PCIe with
driver `570.195.03`; CUDA reported an FP32:FP64 performance ratio of `2:1`, so
the machine was verified as full-rate. GPU timestamp-query results were:

| Run | `f32` ms/tick | double-single ms/tick | CUDA `f64` ms/tick |
|---|---:|---:|---:|
| 1 | 0.817456000 | 0.829600000 | 0.724384010 |
| 2 | 0.818064000 | 0.832112000 | 0.724880010 |
| 3 | 0.817936000 | 0.829472000 | 0.722815990 |
| **median** | **0.817936000** | **0.829600000** | **0.724384010** |

The median CUDA path delivered 35,892,564,781.780 rows/sec, retained 112.915%
of `f32` throughput, and used 88.562% of its time. In all three runs CUDA
native `f64` had zero max and mean reduction error, zero winner mismatches, zero
fired mismatches, and zero unexplained fixed-tree arithmetic-mirror differences.
Its guard passed every time.

Double-single produced usable timings but failed its guard in every run: its
one-million-row winner mismatch rate (`0.00002`) did not improve on `f32`, its
full-workload row had one fired mismatch, and NVIDIA/Vulkan did not provide a
trustworthy strict-arithmetic path. The `f32` full-workload row also had one
fired mismatch. Native `f64` through wgpu was unavailable on this exact H100:
the observed wgpu 0.20 Vulkan pipeline produced an NVIDIA NVVM compiler failure
and unsafe teardown behavior, so the implementation gates it before pipeline
creation. CUDA remained independently available and passed.

## Options

### A — native `f64`

Native `f64` is the closest representation to the CPU oracle. On the verified
full-rate H100, the CUDA implementation passed every numerical and guard check
in all three runs and exceeded the same-machine performance floor. CUDA is the
selected production backend and accuracy reference.

The wgpu/Vulkan implementation remains a separately reported path, but it is
unavailable on the observed H100 compiler stack. Strategy A therefore accepts a
CUDA and full-rate-NVIDIA deployment requirement. Fixed-order two-pass
reductions make Level A plausible on the same pinned binary and GPU model;
Level B remains unproven because cross-hardware bitwise behavior was not tested.

### B — double-single

Double-single remains strong supporting evidence on Metal, where it delivered
approximately 48-bit reduction accuracy while retaining 77.2% of `f32`
throughput. It did not transfer to the tested NVIDIA/Vulkan compiler path: its
strict-arithmetic probe was untrustworthy, its guard failed, and its full row
had the same reduction error and fired mismatch as `f32`.

Strategy B therefore does not qualify. This is a portability result, not a claim
that the representation is universally defective: selecting it on another
backend would require a new gate demonstrating strict operations and the stated
numerical contract there.

### C — tiered precision by contract

The H100 `f32` path was a fast baseline, but its max reduction error was
`1.714669e-7` and its full-workload row had one fired mismatch. It does not
satisfy the unchanged `f64` convention or the proposed reduced contract's
zero-fired-mismatch condition.

Strategy C is unnecessary because A qualifies, and it would not independently
qualify from this evidence. Any future tiered backend must remain explicitly
reduced precision, retain the CPU `f64` oracle, and pass a separately accepted
contract; it must not be called native-`f64` equivalence.

## Precommitted decision rule

The following rule was committed before the H100 evidence was collected.
Numerical and guard conditions must pass in all three runs; performance
comparisons use the median of the three NVIDIA-local `total_ms` values:

1. A precise candidate qualifies numerically only when every run reports max
   reduction relative error `<= 1e-10` and zero winner mismatches, its recorded
   supplemental fired-mismatch count is zero, and all applicable guard and
   arithmetic-mirror assertions pass. Double-single must also pass the strict-
   arithmetic probe.
2. A production candidate meets the performance floor only when it retains at
   least 75% of same-machine `f32` throughput, equivalently
   `median(T_candidate) <= (4/3) * median(T_f32)`.
3. **A qualifies** only if the CUDA reference qualifies and a named production
   native path (CUDA or wgpu/Vulkan) independently passes both the numerical and
   performance gates. CUDA remains the accuracy reference when wgpu is chosen.
4. **B qualifies** only if double-single passes its numerical, strict-arithmetic,
   and performance gates on the decision machine. The Apple result is supporting
   evidence, not a same-machine gate input.
5. **C qualifies** only if neither precise strategy qualifies and a separate
   reduced-precision contract is accepted. That contract must require max
   reduction relative error `<= 2e-7`, zero winner and fired-flag mismatches
   across both this benchmark and the v0.2 differential corpus, at least 75% of
   same-machine `f32` throughput, and tolerances for all other numeric state. It
   must not call the GPU path `f64`.

| A status | B status | Same-machine result | Selection |
|---|---|---|---|
| qualifies | does not qualify | not applicable | **A** |
| qualifies | qualifies | `median(T_A) <= 0.80 * median(T_B)` | **A** |
| qualifies | qualifies | A is less than 20% faster | **B** |
| does not qualify | qualifies | not applicable | **B** |
| does not qualify | does not qualify | C passes its reduced contract | **C** |
| does not qualify | does not qualify | C does not qualify | **blocked** |

The 20% native-over-double-single preference and 75% throughput floor are
engineering policy. Thresholds are not relaxed after observing measurements.

## Decision

**Select Strategy A: native `f64`, with CUDA as the named production backend.**
The full-rate gate was executed three times on the same NVIDIA H100 PCIe at
commit `d6c545f63a89135d01addeea42b9fbe44fac897a`. Artifact verification required
distinct run IDs, generated times, result hashes, and bound external completion
logs while holding hardware, driver, workload, availability, and provenance
constant.

The binding rule resolves as follows:

1. CUDA passed the numerical gate in every run: max reduction error, winner
   mismatches, fired mismatches, and unexplained arithmetic-mirror differences
   were all zero, and every CUDA guard passed.
2. CUDA passed the performance floor. Its median `0.724384010` ms/tick is below
   the permitted `(4/3) * 0.817936000` ms/tick and retains 112.915% of
   same-machine `f32` throughput.
3. CUDA is both the accuracy reference and the named production native path, so
   A qualifies even though the independently reported wgpu path is unavailable
   on this exact H100 compiler stack.
4. B does not qualify because its guard and strict-arithmetic requirements
   failed; the full-workload fired mismatch independently confirms the failure.
5. C is not considered for selection because A qualifies. The measured `f32`
   row also fails C's zero-fired-mismatch requirement.

The three result hashes are
`68e0acd5a9aeb4c624693f3e81319f4e7a502331d27c5bc758b5a9d0439b7e69`,
`f6b18ebbbe9d5cedc54119e3126b6153a74d08d59f96a305d21ee103157a98fe`, and
`e941ff74d1fffc377392233d3cc2dfec6d29fd6befd7989b3ffaab26ca94fb47`.
The tracked evidence bundle is authoritative for per-run details.

## Consequences

- v0.2 must implement the production GPU precision path with CUDA native `f64`
  on verified full-rate NVIDIA hardware.
- The CPU `f64` interpreter remains the semantics oracle, and every production
  kernel must be differentially tested against it. Selection of native `f64`
  does not remove oracle-relative diagnostics.
- The numeric contract remains `f64`; no reduced-precision tolerance is adopted.
- `f32` remains a performance baseline only. Double-single remains useful Metal
  evidence but is not the selected production representation.
- The production scheduler must preserve the fixed-order reduction and
  lexicographic winner rules measured here. This supports Level A only for the
  same pinned binary and GPU model. Level B remains unproven pending
  cross-hardware bitwise evidence.
- Runtime manifests must name the CUDA backend, native-`f64` representation,
  exact GPU/driver, determinism level, and any fallback. Silent fallback to
  wgpu, double-single, or `f32` is prohibited.
- Supporting non-NVIDIA or non-full-rate hardware requires an explicit future
  backend decision and a new qualification gate; it must not weaken this
  decision silently.
