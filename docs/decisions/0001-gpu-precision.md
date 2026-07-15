# ADR 0001: GPU precision strategy

- **Status:** Measured; decision pending full-rate NVIDIA confirmation
- **Date:** 2026-07-16
- **Decision scope:** v0.2 GPU backend precision and reachable determinism levels
- **Evidence:** [`spikes/precision/RESULTS.md`](../../spikes/precision/RESULTS.md)

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

The durable matrix is
[`spikes/precision/RESULTS.md`](../../spikes/precision/RESULTS.md). The recorded
run used an Apple M2 Pro integrated GPU with Metal, the full
`(N, G) = (26,000,000, 1,300,000)` workload, `beta = 0.35`, `dt = 0.25`, 10
warmup ticks, and 100 measured ticks. Metal's advertised timestamp support did
not return every stage pair, so the artifact correctly labels these medians as
synchronized wall-clock fallback measurements.

| Strategy | Total ms/tick | Rows/sec | Reduction relative error, max / mean | Winner mismatch | Result |
|---|---:|---:|---:|---:|---|
| `f32` | 10.711042 | 2,427,401,667.709 | `1.714669e-7` / `3.216831e-8` | 0.000000% | answered on Apple M2 Pro |
| double-single | 13.868958 | 1,874,690,225.466 | `1.096998e-14` / `1.206441e-15` | 0.000000% | answered on Apple M2 Pro |
| native `f64` (wgpu) | unanswered | unanswered | unanswered | unanswered | Metal has no `SHADER_F64` |
| native `f64` (CUDA) | unanswered | unanswered | unanswered | unanswered | CUDA was not enabled or run |

On this workload, double-single cost 1.295× the `f32` total time: a 29.5%
latency increase and 22.8% throughput loss, retaining 77.2% of `f32` throughput
(72.10 versus 93.36 ticks/sec). It reduced max and mean reduction error by about
15.6 million× and 26.7 million× respectively. Both measured paths had zero
winner mismatches, but that finite observation is not proof of equivalence for
all models. The 276,097 order-sensitive groups in both rows reinforce the need
for a fixed reduction order; they are not winner mismatches.

No NVIDIA machine was measured. Consequently there is **no measured NVIDIA fp64
throughput class or native-`f64` timing**. The Apple metadata's conservative
`rate-limited / unknown` label is not a commodity-NVIDIA measurement and cannot
be extrapolated to A100/V100/H100 full-rate behavior. Both native rows that bear
on Strategy A remain explicitly unanswered.

## Options

### A — native `f64`

Native `f64` is the closest representation to the CPU oracle and makes Level A
plausible with the spike's fixed-order two-pass reductions on the same binary
and GPU model. Level B remains unproven because cross-hardware bitwise behavior
was not measured. The strategy excludes Metal and portable-WGSL-only devices and
its throughput depends strongly on the GPU's fp64:fp32 ratio.

There is no concrete throughput or accuracy number for A yet. Treating the
Apple result, a commodity GPU ratio, or a model-name lookup as a full-rate result
would be an unsupported extrapolation.

### B — double-single

Double-single is portable in representation and delivered approximately 48-bit
accuracy on Metal. Its measured end-to-end cost was 1.295× `f32`, not the
previously estimated 3–4× arithmetic cost. It met the spike's strict-arithmetic
probe, `1e-10` max-reduction-error threshold, and winner threshold on this
machine.

Fixed ordering plus a strict arithmetic compilation path makes Level A plausible
on a supported binary/GPU pair. Level B is only a candidate: it requires pinned
operations and cross-hardware bitwise tests that the spike did not perform.
Double-single is not native `f64`; selecting it would require the final numeric
contract to state an oracle-relative tolerance rather than claim `f64`
equivalence.

### C — tiered precision by contract

The measured `f32` path was fastest and had zero observed winner mismatches, but
its max reduction error was `1.714669e-7`. It does **not** satisfy the current
`f64` convention. A tiered backend would keep the CPU `f64` interpreter as the
semantic oracle and explicitly define where reduced precision is permitted.

Level C is the natural baseline when atomics and summation-order jitter are
allowed. Fixed-order kernels can make Level A reproducibility plausible for the
reduced contract on one binary/GPU model, but not `f64` equivalence. Level B is
out unless a software-pinned representation is separately demonstrated.

## Decision

**No A/B/C strategy is selected yet.** The evidence is measured but not decisive
because neither native-`f64` path ran and no full-rate NVIDIA GPU was measured.
The next decision action is mandatory and precise: run PRD 0004 with
`gpu_class = "full_rate"` on one verified full-rate NVIDIA machine, then run the
complete PRD-0005 benchmark three times on that same machine. Use the full
`(26,000,000, 1,300,000)` workload when it fits and identical parameters for
every strategy and invocation. Apple and NVIDIA timings must not be compared as
if they came from one machine.

The rendered merged matrix is a cross-machine presentation and is **not** an
input to the performance gate: by design it selects Development rows for `f32`
and double-single and NVIDIA rows for native `f64`. Each invocation instead
stores all four local rows in the marker-delimited JSON at
`machines.nvidia.strategies`. Those NVIDIA-local rows are the decision evidence.

Preserve three independent runs as follows:

1. Make three byte-identical copies of the Mac-containing `RESULTS.md` at three
   distinct absolute paths on the NVIDIA machine.
2. For invocation `i`, set `SPIKE_RESULTS_PATH` to copy `i`, run the PRD-0004
   wrapper once, and redirect its stdout and stderr to a persistent run-specific
   log outside the wrapper. The wrapper's own transcript is temporary.
3. Retrieve all three result files and logs before teardown. Never reuse one
   results path: updating the fixed `nvidia` machine key replaces that file's
   previous NVIDIA run.
4. In every file verify the same repository commit, exact GPU model, driver,
   `full-rate` fp64 class, actual `(N, G)`, `beta`, `dt`, and strategy
   availability. A mismatch invalidates the three-run comparison.
5. Read timing and accuracy from each file's
   `machines.nvidia.strategies[*].status`; specifically, use
   `status.timing.total_ms` for the performance rule. Record all three inputs and
   their computed medians in this ADR when the gate is executed.

The PRD-0005 state does not include fired-flag or arithmetic-mirror counters.
Before the decision run, make its supplemental diagnostics emit and preserve,
for each candidate, the count of `GpuTickResult.fired` values that differ from
the CPU oracle at the benchmark tick; native `f64` must also emit unexplained
arithmetic-mirror differences. The existing native guard already asserts both
diagnostics, but the portable guard does not yet score fired flags, so B and C
cannot qualify until that portable counter is added to the decision evidence.
Any absent or failed diagnostic leaves the candidate unqualified rather than
treating a missing counter as zero.

Apply this decision rule. Numerical and guard conditions must pass in all three
runs; performance comparisons use the median of the three NVIDIA-local
`total_ms` values:

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
   and performance gates on the decision machine. The current Apple result is
   supporting evidence, not a same-machine gate input.
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

Record the named native backend when A is selected. Restricted full-rate
hardware is accepted when A is the sole qualifying precise path, or when A earns
the specified material gain over qualifying B. Thresholds are not relaxed
silently.

The 20% native-over-double-single preference and 75% throughput floor are
engineering policy: they quantify the trade required to surrender portability
or throughput. They are intentionally falsifiable, not claims about unmeasured
hardware.

## Consequences

Until the full-rate gate is executed:

- the CPU `f64` interpreter remains the production semantics oracle and the
  numeric contract is unchanged;
- v0.2 must not commit to a precision-dependent GPU backend;
- `f32` is a performance baseline, not an implementation of the `f64` contract;
- fixed-order double-single is the measured provisional precise candidate, not
  the selected strategy;
- Level A is plausible for fixed-order A or B on one binary/GPU model, Level B
  remains unproven, and C supplies only its explicitly reduced contract.

After the gate, v0.2 must build exactly the strategy selected by the rule, retain
differential tests against the CPU `f64` oracle, and amend `DESIGN.md` §5.2 with
the selected representation and tolerance. Choosing A accepts full-rate-NVIDIA
hardware restrictions; choosing B accepts emulation cost and a tolerance-based
contract; choosing C accepts a deliberately reduced GPU numeric contract.
