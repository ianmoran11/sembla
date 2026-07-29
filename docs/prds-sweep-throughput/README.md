# Sweep throughput PRDs

Make batch runs cheap. Run from the Sembla repository with:

```text
/piprd run docs/prds-sweep-throughput
```

`README.md` is ignored by `/piprd run`. Every numbered PRD must read this file
first; the constraints below are binding.

## Why this folder exists

The driver workflow is **uncertainty and sensitivity analysis** — many draws
over the same model and population, differing only in parameters and seed.
Before 0001, every draw constructed a new backend, recompiled the CUDA model
through NVRTC, loaded and uploaded the state, and cloned the full host initial
state — about 458 MiB at 10M slots. None of that work is inherently per draw.

0001 replaced that lifecycle with one retained CPU/CUDA backend that is reset
and reseeded per draw. The current loop now deliberately executes draws in
ascending `k` through that one mutable object. That fixed repeated setup and
established the draw-independence seam on which the next candidate — several
isolated retained backends running simultaneously — depends.

## Measured current state

The corrected H100 session at commit
`5616dbe56cddb26e6a6541bead3572639827a8c2` is recorded in
[`hyperstack-l4-20260729T022057Z`](../evidence/demographic-bench/hyperstack-l4-20260729T022057Z/).
For 20 CUDA draws over 24 ticks:

| scale | whole sweep | draw 0 | median later draw |
|---:|---:|---:|---:|
| 1M | 4.987 s | 1.431 s | 0.175 s |
| 10M | 37.722 s | 6.362 s | 1.615 s |

The immediately preceding control-count implementation baseline
`d598342e9b28f242758aad392253a26c690bccef` measured 12.115/2.419/0.496 s at
1M and 210.570/15.021/10.290 s at 10M for the same three columns. The current
whole sweep is therefore 2.43× faster at 1M and 5.58× at 10M; median later draws
are 2.83× and 6.37× faster respectively.

Those comparisons measure the device-side control-count change, not 0001 in
isolation. Their purpose here is to freeze the corrected sequential baseline
from which concurrent draws must be measured. They also show why projection is
not evidence: the old 100-draw estimate in this README did not predict the
measured scale dependence.

The workflow is now fast enough that concurrency may matter at small scales,
but that is a hypothesis. §M1 requires a direct overlapping same-result arm
before a new PRD may claim a gain.

## Binding constraints

- **Results must not change.** Every golden, every CSV and hash golden, the
  frozen demographic state fixture, every run and sweep manifest including
  `final_state_sha256`, and the tracked CUDA differential evidence must be
  **byte-identical**. A sweep's outputs must not depend on how many draws
  preceded it in the same process.
- **Draw independence is the property being relied on and must be proved, not
  assumed.** A retained backend introduces exactly one hazard: state leaking
  from draw *n* into draw *n+1*. Every PRD here must show that draw *k* run
  alone is byte-identical to draw *k* run after *k−1* others.
- **§E2 determinism levels are unchanged.** Reuse must not alter which draws are
  taken, only what is rebuilt between them.
- **The CRN/independent noise distinction is semantic.** `NoiseMode::Crn` reuses
  one seed; `Independent` derives a per-draw replica seed. A backend that takes
  its seed at construction cannot simply be reused across `Independent` draws —
  re-seeding must be explicit and correct, not incidental.
- **Backend identity checking must not be weakened.** The sweep asserts the
  backend device identity is stable across draws (`main.rs:1843`). Retaining one
  backend makes that trivially true, which removes a real check. State what
  replaces it.
- **No new dependencies.**

## What is already built and should be reused

`prds-cuda-host-path/0001` added an in-place `StateStore` refresh that reuses
column allocations and runs the same validation as the constructor. It exists to
avoid exactly the reallocation the draw loop does per draw. Prefer reusing it
over writing a second mechanism.

## Local versus hardware acceptance (inherited from §J14.2)

Most of this folder is CPU-side and fully testable locally: the draw loop, the
state reset, the manifest contents, byte-identical outputs. The CUDA lifecycle
change cannot be compiled on the development Mac.

Per §M3, a PRD here that defers a hardware criterion must name the command that
will run it. `BENCH_CORPUS=1 BENCH_PROFILE=1 bash run-demographic-benchmark.sh`
is the collector; a sweep-specific measurement needs its own stage, and adding
that stage is **in scope for the PRD that needs it**.

## Measurement protocol

Wall time for a whole sweep is the headline — that is what the workflow waits
on. Report per-draw cost as total ÷ draws, and separately report the cost of
draw 0 against the median draw, since the whole point is that draw 0 pays a
setup the others should not.

`--timing-json` per-phase instrumentation exists for a single run. If it does
not aggregate usefully over a sweep, say so rather than reporting a per-tick
table that hides the setup being removed.

Concurrent execution adds two requirements. Report throughput and the latency
distribution as well as whole-sweep wall time: overlapping per-draw durations
may sum to more than wall time. The existing `sembla-sweep-timing-v1` schema is
therefore valid only for sequential execution unless a later PRD explicitly
versions it with queue, start, finish, requested/effective concurrency, and
whole-sweep fields. Do not silently reinterpret its current fields.

Every performance comparison uses adjacent, otherwise identical arms and
compares complete normalized output trees. It must include grouped sidecars,
summaries, pairs, manifests, and final-state hashes. Per §M4, the comparator is
not trusted until a deliberately perturbed copy makes it fail.

## Before running a PRD from here

```sh
python3 scripts/check-prd-allowlist.py <the-PRD-at-its-current-path>
```

It lists every repo path the PRD names that its allowed-file list does not
cover. Most will be read-only context; the point is that the list is short
enough to eyeball. `0001` stalled a managed run for five attempts on exactly
this defect — it required a sweep stage in the collector while omitting the
collector from its own allowed files — and this check reproduces that finding in
under a second.

Never put a hard-coded self-path in an acceptance criterion. PRDs move into
`docs/prds-run-queue/` by design; the command above must be run by the operator
against the PRD where it currently lives.

## PRDs

- `0001-reuse-the-backend-across-draws` — implemented 2026-07-28: construct
  one retained CPU/CUDA backend, reset and reseed it per draw, and capture
  identity once. Local correctness and initial CPU evidence are recorded in
  [`../evidence/sweep-backend-reuse-20260728.md`](../evidence/sweep-backend-reuse-20260728.md).
  Corrected-current 1M/10M GPU evidence, complete CPU/CUDA output-tree parity,
  the negative comparator control, and final checksums are recorded in
  [`hyperstack-l4-20260729T022057Z`](../evidence/demographic-bench/hyperstack-l4-20260729T022057Z/).

- `0004-run-cuda-draws-concurrently` — drafted 2026-07-29 after CUDA Gate 1
  passed for the free-running non-blocking-stream design; see the sequence
  status below.

The concurrent-draw candidate was scoped below so the measurement could answer
the architectural questions before an implementation specification froze them;
that measurement is now complete and `0004` is the result.

## Concurrent-draw track — scoped, not yet drafted

This work stays in `docs/prds-sweep-throughput/`; do not create a second folder.
It directly extends 0001's retained-backend lifecycle and inherits this folder's
draw-independence, seed, identity, output, and timing contracts.

### What exists today

CPU execution is already parallel *within* a draw. `tick_worker_count` defaults
to `available_parallelism()` (or `SEMBLA_EVAL_THREADS`), and fixed row-tile
tasks run through `std::thread::scope`. That is not inter-draw parallelism.

The sweep itself is explicit and sequential:

```rust
// Deliberately sequential: declaration order within each k, then k order.
for draw in 0..draw_count {
```

It owns one mutable `SweepBackend`; the CPU variant owns one mutable
`StateStore`, and the CUDA variant owns one `CudaBackend`, one default stream,
and one complete mutable allocation set. `run_draw` takes `&mut self`.
Concurrent draws are therefore not a `par_iter` change and may never share that
mutable object.

### Gate 0 — corrected sequential evidence complete

The checksummed
[`hyperstack-l4-20260729T022057Z`](../evidence/demographic-bench/hyperstack-l4-20260729T022057Z/)
session satisfies this gate:

- all arms identify current commit
  `5616dbe56cddb26e6a6541bead3572639827a8c2`;
- the differential corpus exits 0 and grouped CPU/CUDA parity covers all five
  primary/summary/grouped files;
- the 1M and 10M sequential baseline/current sweep timings above are complete;
- at both scales, 103 normalized CPU/CUDA output files are exact after backend
  identity normalization, and the 1M grouped-sidecar perturbation is rejected;
- the grouped 5M/2-tick CUDA profile reports 20.814 ms total,
  0.029 ms `readback_control`, and 0.009 ms `report`, with its timing self-check
  reconciled;
- all 1,368 entries in `SHA256SUMS` pass;
- evidence was delivered to `evidence/hyperstack-20260729T034132Z`; and
- Terraform state, the Hyperstack account reconciliation, and the independent
  watchdog all report clean teardown.

These results are the sequential baseline. A profile or occupancy estimate may
identify concurrency as a candidate, but cannot size it.

### Gate 1 — direct same-result spikes before any numbered PRD

Per `DECISIONS.md` §M1, concurrency must be measured by a runnable arm that
executes the proposed shape and asserts the same complete result. This is a
spike/evidence task, not a production PRD and not permission to change default
behaviour.

The runnable driver is `scripts/run-sweep-concurrency-spike.py`. It invokes the
hidden `SEMBLA_SWEEP_SPIKE_DRAW_WORKERS` seam, keeps timing outside scientific
output directories, records requested/effective lanes and resource samples,
compares complete output trees byte-for-byte, and proves its comparator with one
deliberate CSV perturbation. Example shapes are:

```sh
cargo build --release --locked -p sembla-cli --features cuda

python3 scripts/run-sweep-concurrency-spike.py \
  --binary target/release/sembla \
  --model <model.json> --population <state> \
  --backend cuda --output-root <evidence-dir> \
  --workers 1 2 4 --draws 20 --ticks 24 --noise independent
```

For CPU add `--cpu-total-threads <physical-budget>`; the driver divides that
budget across active draws and exports the resulting `SEMBLA_EVAL_THREADS` per
arm. The output root must not exist, so an arm cannot silently reuse evidence.

Measure CPU and CUDA separately; a positive result for one does not authorize
the other. Preliminary 100k/1M CPU arms are recorded in
[`sweep-concurrency-spike-20260729`](../evidence/sweep-concurrency-spike-20260729/):
workers 1/2/4 are byte-identical and four lanes improve whole-sweep wall by
3.22–3.29× under a fixed ten-thread budget. That is a feasibility result, not a
completed gate: repeated 10M/second-shape CPU evidence remains open.

The CUDA complete-backend lower-bound arm is recorded in
[`hyperstack-concurrency-20260729T064051Z`](../evidence/demographic-bench/hyperstack-concurrency-20260729T064051Z/).
Across three repetitions, workers 1/2/4 are byte-identical at 1M and 10M. Median
whole-sweep speedups are 1.321×/1.265× at 1M and 1.487×/1.658× at 10M for two/four
workers. Capacity reaches 22,699 MiB VRAM and 9,032 MiB RSS at 10M/four workers.
However, Nsight reports all 39,552 traced kernels on context 1, stream 7, with
zero time at concurrency ≥2. The gain is host-side overlap while CUDA kernels
serialize. This closes the complete-default-stream design and triggers the
scoped shared-context/non-blocking-stream prototype; CUDA Gate 1 is not complete
until that direct arm is measured.

The synchronized lockstep-stream arm is recorded in
[`hyperstack-lockstep-20260729T092302Z`](../evidence/demographic-bench/hyperstack-lockstep-20260729T092302Z/).
`--cuda-lockstep-streams` gives each active lane an explicitly non-blocking CUDA
stream and advances equal-sized lane groups through tick boundaries together.
It remains hidden and default-off; production sweeps remain sequential.

All 18 complete output-tree comparisons pass across workers 1/2/4, three
repetitions, and the 1M/10M shapes. Ratio-of-median whole-sweep speedups for
two/four workers are 1.197×/1.252× at 1M and 1.296×/1.405× at 10M. Using the
same aggregation, these are weaker than the isolated scheduler's 1.322×/1.267×
and 1.487×/1.654× results. At 10M/four workers, lockstep execution is 18.5%
slower than isolated execution and still uses 22,701 MiB VRAM and 9,369,148 KiB
RSS. The hardware matrix used independent noise only; this is sufficient to
reject a slower arm but would not satisfy the CRN requirement for a positive
Gate-1 result.

Nsight directly proves simultaneous execution rather than serialization:
39,552 kernels run on context 1, split evenly between streams 13 and 14, with
35.939 ms at concurrency two and maximum concurrency two. However, only 11.25%
of the kernel interval union overlaps. Summed kernel time rises from 307.323 ms
to 355.407 ms relative to the prior trace, while total positive inter-kernel
gaps rise from 168.988 ms to 418.451 ms. Real overlap therefore does not make
this synchronized complete-backend design competitive. The arm is negative and
closed as a production candidate.

The remaining CUDA experiment is now a hidden fused grid-y spike. With
`SEMBLA_SWEEP_SPIKE_CUDA_FUSED_DRAWS=1|2|4`, one context, generated module, and
default stream retain draw-major mutable arenas. Every existing logical phase
launch keeps its x geometry and adds `gridDim.y = active_slots`; `blockIdx.y`
selects an isolated slot. Phase boundaries and within-draw reductions remain
unchanged, transition kernels load an independent per-slot seed, and the Philox
draw coordinate remains literal `0U`. A final partial chunk uses its actual
active width rather than padding or dropping draws.

This is default-off Gate-1 evidence code, not a production interface or
permission for a numbered PRD. It is mutually exclusive with multi-backend
workers, lockstep streams, and draw-delay controls. The adjacent sequential reference and fused capacities can be driven with:

```sh
cargo test --release --locked -p sembla-cli --features cuda \
  --test gpu_differential \
  fused_grid_y_sweep_matches_sequential_cuda_with_partial_tail \
  -- --ignored --exact --nocapture

python3 scripts/run-sweep-concurrency-spike.py \
  --binary target/release/sembla \
  --model <model.json> --population <state> \
  --backend cuda --cuda-fused-grid-y \
  --output-root <new-fused-evidence-dir> \
  --workers 1 2 4 --draws 20 --ticks 24 --noise independent \
  --repetitions 3

# Corresponding independent-backend comparator; omit fused mode only.
python3 scripts/run-sweep-concurrency-spike.py \
  --binary target/release/sembla \
  --model <model.json> --population <state> \
  --backend cuda \
  --output-root <new-independent-evidence-dir> \
  --workers 1 2 4 --draws 20 --ticks 24 --noise independent \
  --repetitions 3
```

The ignored hardware test covers both CRN and independent noise, grouped and
contest output, capacities 1/2/4, and a partial tail. Hardware acceptance also
requires explicit capacity outcomes and a repeated whole-sweep win over both
the adjacent sequential and separately measured isolated-backend arms.
One launch per logical phase must be confirmed with Nsight; launch reduction or
occupancy alone is not success.

The completed fused arm is recorded in
[`hyperstack-fused-20260729T121444Z`](../evidence/demographic-bench/hyperstack-fused-20260729T121444Z/).
The capacity-four/two-active-draw compute-sanitizer shakedown reports zero errors,
and every complete output-tree comparison passes. Nsight confirms 19,776 kernel
launches with `gridDim.y = 2`, one context, and one stream—half the corresponding
four-draw complete-backend launch count. Correctness and actual fused execution
are therefore established.

Performance is decisively negative. At 1M, sequential/fused-1/fused-2/fused-4
median wall times are 5.161/7.754/7.466/13.439 seconds. At 10M they are
37.268/56.501/56.173/64.576 seconds. Capacity two is 44.7% and 50.7% slower than
sequential, while capacity four reaches 22,697 MiB VRAM and 10,150,528 KiB RSS.
It is also substantially slower than the prior isolated-backend and lockstep
arms. Fused grid-y batching is closed as a production candidate.

**CPU arm**

- Use isolated retained CPU backends for active draws.
- Measure outer concurrency 1, 2, and 4 with an explicit total CPU budget.
- Compare inner worker count 1 and budgeted intra-draw workers so nested
  `available_parallelism()` pools cannot oversubscribe the host unnoticed.
- Record CPU topology, affinity/NUMA policy, RSS, whole-sweep wall, throughput,
  and p50/p95 draw latency.

**CUDA arm**

- First measure isolated complete backends to bound benefit and memory cost.
- If default streams, repeated NVRTC, or separate contexts serialize/confound
  that arm, measure the smallest shared-context, non-blocking-stream prototype
  that can distinguish real kernel overlap from time slicing.
- Record compile/setup count and time, stream/context shape, peak VRAM/RSS,
  GPU utilization, event or `nsys` overlap evidence, whole-sweep wall,
  throughput, and p50/p95 draw latency.
- Request concurrency 1, 2, and 4 only where allocation preflight succeeds. An
  OOM is evidence, not a reason to silently reduce the requested count.

Both arms use at least the 1M and 10M demographic shapes, 24 ticks, at least 20
draws, and adjacent sequential/concurrent arms. Independent noise is the timing
case; CRN and independent noise are both correctness cases. Any worker policy
intended to apply beyond this model needs a second materially different shape,
reported separately rather than averaged.

The spike must prove:

- draw `k` alone equals draw `k` after and alongside other draws;
- parameter sampling and replica seeds remain pure functions of `k`;
- complete file sets and bytes equal the sequential arm, including grouped
  sidecars and `final_state_sha256`;
- a perturbed comparison fails; and
- setup, steady-state execution, publication, and resource use are separately
  visible.

The outcome is recorded independently for CPU and CUDA as one of:

1. negative — close that backend's concurrency track;
2. positive — a bounded backend-pool implementation is justified; or
3. blocked — compilation/module/state separation must be measured first.

Do not choose a universal worker count or an `auto` policy from one scale.
The complete-backend/default-stream, complete-backend/lockstep-stream, and
fused grid-y CUDA mechanisms are all negative and closed. The fourth and final
scheduling/stream quadrant — **free-running non-blocking streams** — is
recorded in
[`hyperstack-freestream-20260729T152534Z`](../evidence/demographic-bench/hyperstack-freestream-20260729T152534Z/):
`SEMBLA_SWEEP_SPIKE_CUDA_FREE_STREAMS=1` gives each lane an isolated retained
backend on an explicitly non-blocking stream, claims draws dynamically with no
tick barriers, and publishes ascending `k`.

The result is **conditionally positive**. All 18 complete-tree comparisons
are byte-equal, negative controls are rejected, and the schedule control
proves `cuda-free-nonblocking-streams` with a forced completion inversion.
Nsight shows 39,552 kernels split evenly across non-default streams 13/14
with 29.470 ms of real overlap (max concurrency 2) and no barrier gaps.
Median whole-sweep speedups over workers 1 are 1.266x/1.270x at 1M and
**1.598x/1.745x at 10M** — the fastest measured CUDA design at 10M and
effectively tied with isolated default-stream backends at 1M (10M: isolated
1.487x/1.658x, lockstep 1.296x/1.405x, fused negative). Memory
remains draw-major: 22,701 MiB VRAM and 9,161,224 KiB RSS at 10M/four lanes.

The gate is not fully discharged: like the lockstep arm, this matrix used
independent noise only, and the gate requires CRN as a correctness case.
`0004-run-cuda-draws-concurrently` may be drafted only after a CRN hardware
correctness arm passes for this design, and any production form needs explicit
capacity admission rather than an `auto` worker count. CPU Gate 1 remains
separate and open.

**Gate 1 discharged (CUDA).** The CRN correctness arm is recorded in
[`hyperstack-crn-20260729T221234Z`](../evidence/demographic-bench/hyperstack-crn-20260729T221234Z/):
workers 1/2/4 at 1M and 10M under CRN noise, one repetition per scale, every
complete output-tree comparison byte-equal, both negative controls rejected.
With independent noise as the timing case and both noise modes as correctness
cases, all Gate-1 requirements measured to date pass for the free-running
non-blocking-stream design. Drafting `0004-run-cuda-draws-concurrently` under
the conditional sequence below is now justified, subject to the binding
contract (bounded explicit admission, isolated per-lane state, `k`-derived
seeds, ascending-`k` publication, default-off). CPU Gate 1 remains separate
and open.

### Binding contract for future PRDs

Any numbered PRD drafted after Gate 1 inherits these constraints:

- Default concurrency remains 1 unless a later measured decision changes it.
- Every active draw owns isolated mutable state, parameters, seed/tick,
  diagnostics, observations, and scratch. A stream-slot design also requires a
  non-blocking stream per draw; a fused design may use one stream only if every
  mutable region and kernel coordinate remains explicitly draw-indexed.
- A lane is retained and reset between assigned draws; concurrency must not
  reintroduce per-draw construction, state cloning, or NVRTC compilation.
- Theta and execution seed are derived from `k`, never from admission,
  completion, lane, thread, or stream order.
- Admission is bounded. A slow low-`k` draw may not cause an unbounded queue of
  completed higher-`k` outputs or multiply host/device memory without limit.
- Workers return isolated results. One coordinator publishes files, pairs,
  summaries, `all_series`, and `RunManifest.executions` in ascending `k`.
- Observable failure matches sequential order: publish only the contiguous
  successful prefix and report the lowest failing `k` after all lower draws
  have resolved. Higher speculative results remain unpublished.
- Requested unsupported concurrency fails clearly before scientific output. No
  silent fallback, implicit cap, cross-device migration, or identity change.
- Existing manifests and scientific outputs remain byte-identical. Scheduler
  provenance belongs in a versioned timing/evidence record unless a separate
  semantic decision authorizes a manifest change.
- No new dependencies. No lock that serializes all CUDA work may be presented as
  concurrency.

### Conditional PRD sequence after Gate 1

The evidence decides which entries exist and their final numbering. If both
backends pass Gate 1, the tentative draft names are
`0002-bounded-ordered-draw-scheduler`,
`0003-control-cpu-draw-resources`,
`0004-run-cuda-draws-concurrently`, and
`0005-publish-concurrency-defaults`. Remove and renumber conditional entries
rather than creating placeholder PRDs when one backend's evidence is negative.
The expected order is:

1. **Bounded ordered draw scheduler.** Add fixed-window admission, isolated
   worker results, ascending-`k` publication, deterministic prefix/error
   behaviour, bounded reordering storage, and timing that remains truthful
   under overlap. Completion-order inversion and injected-error tests are
   mandatory. If only one backend measured positively, reject concurrency on
   the other rather than widening this PRD.
2. **CPU resource control and backend lanes — conditional.** Only if the CPU
   arm wins: thread an explicit inner-worker budget through runtime execution,
   retain one resettable `StateStore` per lane, prevent nested oversubscription,
   and prove bit identity across outer/inner partitions.
3. **CUDA fused draw slots — closed.** The fused grid-y arm measured
   negative (slower than sequential at every capacity), so this conditional
   entry is retired; the measured free-running non-blocking-stream design is
   the CUDA mechanism instead, specified in `0004`.
4. **Measured defaults and publication — conditional.** Decide whether the
   option remains explicit or gains an `auto` policy only after repeated
   multi-shape evidence. Dynamic throttling is not part of the first delivery.

Do not manufacture a mechanism PRD when its prerequisite measurement is
negative. The complete-backend/default-stream, complete-backend/lockstep-stream,
and fused grid-y arms were all measured negative and remain closed; none of
them may be repackaged into a concurrency PRD.

**Sequence status (2026-07-29).** The free-running non-blocking-stream arm
passed Gate 1, so the CUDA track proceeds and the CPU track is parked by
operator decision (CUDA-capable production):

- `0002-bounded-ordered-draw-scheduler` — **deferred**. The scheduler contract
  (dynamic admission, isolated results, ascending-`k` publication, truthful
  timing) is subsumed into `0004` for the CUDA track; a standalone scheduler
  PRD returns only if the CPU track reopens.
- `0003-control-cpu-draw-resources` — **deferred** pending CPU Gate 1.
- [`0004-run-cuda-draws-concurrently`](0004-run-cuda-draws-concurrently.md) —
  **drafted**. Self-contained for the CUDA track: supported `--draw-workers`
  interface, bounded capacity preflight, per-lane non-blocking streams, and
  the scheduler contract, on the measured free-streams mechanism.
- `0005-publish-concurrency-defaults` — **conditional** as written: any
  default or `auto` policy waits for repeated multi-shape evidence.

### Required acceptance evidence shape

Each implementation PRD records a narrative summary plus machine-readable raw
records sufficient to recompute every headline number. At minimum include:

- repository/dirty state and release binary SHA-256;
- CPU/GPU/driver/CUDA/topology and tool versions;
- model/state hashes, scale, ticks, draws, seed, noise mode, and features;
- exact commands and adjacent arm order;
- requested/effective concurrency and backend/stream/context/compile counts;
- whole-sweep wall, setup/compile, draw-0, later-draw p50/p95, and draws/second;
- peak VRAM and RSS, headroom/OOM, and utilization/overlap evidence;
- complete normalized file-tree equality and hashes;
- draw-alone/schedule-independence matrices for contest/grouped cases under CRN
  and independent noise;
- comparator perturbation and expected nonzero result; and
- local versus hardware criteria, with every deferred hardware criterion naming
  the exact runnable collector command per §M3.

Do not regenerate or bless changed goldens. Negative and inconclusive results
are first-class evidence.

### Global non-goals

Until separate measured work authorizes them, this track excludes multi-GPU or
distributed scheduling, kernel algorithm changes, RNG coordinates or draw-set
changes, calibration-method changes, resumable/atomic sweep directories,
graceful CUDA kernel cancellation, automatic VRAM throttling, adaptive
self-tuning, changes to `run`/`compare`/differential paths, and concurrent final
artifact writes from workers.
