# PRD 0004: Run CUDA sweep draws concurrently on free-running non-blocking streams

## Status

Implemented and locally approved in `d862a91`; hardware-verified on H100 at
`d72057f`. All acceptance criteria pass. Evidence and calculations:
[`hyperstack-supported-concurrency-20260730T022957Z`](../evidence/demographic-bench/hyperstack-supported-concurrency-20260730T022957Z).

## Context

Read `docs/prds-sweep-throughput/README.md` first; its constraints bind,
including the binding contract for future PRDs and the ascending-`k`
publication obligation.

CUDA Gate 1 is discharged. Four concurrency designs were measured on one H100
at 1M and 10M slots, 20 draws, 24 ticks:

| design | 10M, 2 lanes | 10M, 4 lanes | verdict |
|---|---:|---:|---|
| isolated default-stream backends | 24.663 s (1.485×) | 22.196 s (1.656×) | host pipelining only; kernels serialized |
| lockstep non-blocking streams | 28.448 s (1.294×) | 26.380 s (1.396×) | real overlap, barrier gaps; closed |
| fused grid-y batching | 56.173 s (0.663×) | 64.576 s (0.577×) | slower than sequential; closed |
| **free-running non-blocking streams** | **23.207 s (1.598×)** | **21.247 s (1.745×)** | **passed Gate 1** |

All seconds and speedups in that table use one common aggregation (external
wall, ratio of medians) per the freestream `ANALYSIS.md` cross-design table;
the per-arm analyses quote slightly different paired-internal bases.

Evidence: `docs/evidence/demographic-bench/hyperstack-freestream-20260729T152534Z/`
(timing: independent noise, three repetitions, Nsight showing 39,552 kernels
split evenly across non-default streams 13/14 with 29.470 ms of real overlap)
and `docs/evidence/demographic-bench/hyperstack-crn-20260729T221234Z/`
(correctness: CRN noise, every output tree byte-equal at workers 1/2/4, both
scales, negative controls rejected). Complete output trees — manifests,
summaries, grouped sidecars, exported pairs, `final_state_sha256` — are
byte-identical to sequential for this design in every measured arm under
both noise modes.

The proven mechanism already exists as hidden, default-off spike code:
`SEMBLA_SWEEP_SPIKE_CUDA_FREE_STREAMS=1` gives each lane an isolated retained
backend on an explicitly non-blocking stream, constructed, used, and dropped on
its worker thread; draws are claimed dynamically with no tick barriers; one
coordinator publishes ascending `k` (`crates/sembla-cli/src/main.rs`, the
`SweepConcurrencyMode::CudaFreeNonblocking` arm).

What does not exist is a **supported interface**: the controls are hidden
environment variables, there is no capacity admission, and nothing stops a
user requesting more lanes than the device can hold (10M/four lanes measured
22,701 MiB VRAM and 9,161,224 KiB RSS, about 9.4 GB). This PRD promotes the
measured mechanism to an explicit, bounded, default-off interface. It does
not change the mechanism.

## Goal

A sweep on `--backend cuda` may run draws concurrently with an explicit,
bounded worker count, defaulting to 1 (sequential), producing output
byte-identical to sequential under both noise modes, with clear failure when
the requested capacity does not fit.

## Specification

### 1. Supported interface: `--draw-workers N`, CUDA only

Add `--draw-workers <N>` to `sweep`, default 1. N ≥ 2 requires `--backend
cuda`; on `--backend cpu` it fails clearly before scientific output (the CPU
track is deferred — see Non-goals). N greater than the draw count fails.
N = 1 is exactly today's sequential path, including the retained-backend
lifecycle from PRD 0001.

The hidden spike environment variables remain as test/evidence seams only.
The supported flag and `SEMBLA_SWEEP_SPIKE_CUDA_FREE_STREAMS` select the same
mechanism; setting the flag while the hidden seams request a different worker
count via `SEMBLA_SWEEP_SPIKE_DRAW_WORKERS` fails rather than silently
preferring one. The lockstep and fused hidden controls remain
experimental and mutually exclusive with this flag; they are not promoted and
must not be reachable through it.

### 2. Bounded admission with capacity preflight — this is the criterion

Admission is explicit and bounded. Before any lane is constructed, estimate
device memory for N lanes and fail clearly if it does not fit. The measured
shape of the cost is: one base allocation plus a near-linear per-lane cost
(1M: 1,033 → 1,613 → 2,733 MiB for 1/2/4 lanes; 10M: 6,025 → 11,597 →
22,701 MiB). The estimator must be **conservative** — never underestimate —
derived from the model's buffer census and state size rather than tuned to the
measured points, and checked against free device memory at run time with a
documented safety margin.

Failure modes, all before scientific output: insufficient device memory,
unsupported backend, worker count above draw count, zero or malformed count.
**No silent cap, no silent reduction, no fallback to sequential, no migration
to another device.** If the preflight cannot establish a conservative bound
for a model shape, it fails the same way rather than guessing.

RSS scales with lanes too (10M/four lanes: 9,161,224 KiB); the preflight
must state its host-memory assumption or bound, not only the device one.

### 3. Lane lifecycle and stream discipline

Each lane constructs, uses, and drops its own retained `CudaBackend` on its
worker thread. CUDA contexts are thread-current; a backend constructed on the
coordinator and moved to a worker fails with `CUDA_ERROR_INVALID_CONTEXT`
(recorded in the rejected
`hyperstack-concurrency-20260729T062402Z` diagnostic). Each lane uses an
explicitly non-blocking stream — the default stream is synchronizing and was
measured to serialize all kernels (0 ms overlap in the isolated-backend
trace). Lanes are retained and reset between claimed draws per PRD 0001; no
per-draw construction, state cloning, or NVRTC recompilation. Backend identity
is captured once per lane and all lanes must agree; identity is recorded in
the manifest as today.

The ordinary generated kernel ABI is used unchanged. No fused grid-y
transformation, no kernel changes, no tick barriers, no cross-lane
synchronization.

### 4. Scheduling and deterministic publication

Draws are claimed dynamically (`next_draw` fetch-add) so a slow draw does not
hold up its peers; completion order may invert. Publication is the
coordinator's alone and is strictly ascending `k`: files, pairs, summaries,
`all_series`, manifest executions, and timing records. Parameter sampling and
replica seeds remain pure functions of `k` — never of admission, completion,
lane, thread, or stream order. Theta-file assignment order is unchanged.

Failure semantics match the binding contract: workers return isolated
results; after all lanes resolve, the coordinator publishes the contiguous
successful prefix in ascending `k` and reports the lowest failing `k`; higher
speculative results remain unpublished.

### 5. Truthful timing

The concurrency timing document (`sembla-sweep-concurrency-spike-timing-v1`
or a promoted successor) records `execution_mode`, requested and effective
worker counts, setup/execution-window/publication phases, and per-draw lane
and start/finish offsets in ascending `k`. One shared number must never be
presented as independent per-draw truth (the fused arm's lesson). Whole-sweep
wall remains the headline number.

### 6. Byte-identity and the comparator

Every scientific output byte-identical to sequential under both noise modes,
including grouped sidecars, exported pairs and their metadata, summaries,
manifests, and `final_state_sha256`. The comparison is the complete output
tree, file set first and then bytes, and the comparator must be demonstrated
failing on a deliberate perturbation (§M4) — both already exist in
`scripts/run-sweep-concurrency-spike.py` and are required to keep passing.

## Allowed files

- `crates/sembla-cli/src/main.rs`
- `crates/sembla-cuda/src/backend.rs` (minor: capacity-estimate support only)
- `crates/sembla-cli/tests/**`, `crates/sembla-cuda/tests/**` (tests only)
- `scripts/run-sweep-concurrency-spike.py` (drive the supported flag)
- `spikes/precision/infra-hyperstack/run-demographic-benchmark.sh` — the
  hardware criteria need a stage that exercises the **supported flag**, not
  the hidden env; same limited-authorisation shape as PRD 0001's collector
  exception: an opt-in stage, no change to the frozen §L4 protocol, teardown,
  or Terraform handling
- `spikes/precision/infra-hyperstack/RUNBOOK.md` — same authorisation; a new
  collector stage that is not documented is a stage nobody will run (PRD
  0001's lesson). The stage is named `BENCH_CONCURRENCY_SUPPORTED=1` in the
  criteria below; document it here.
- `docs/prds-sweep-throughput/README.md` (status notes only)
- `docs/evidence/**` (new evidence only)

**If a required gate fails on files outside this list, stop and report it** —
`DECISIONS.md` §M2. Do not hard-code this PRD's own path in any criterion
(§M5); refer to "this PRD at its current path". Run
`python3 scripts/check-prd-allowlist.py` on this PRD before queueing.

## Non-goals

- **No CPU concurrency.** CPU Gate 1 is parked by operator decision
  (CUDA-capable production). The conditional `0002`/`0003` entries are
  deferred, not negative; this PRD is self-contained for the CUDA track.
- No promotion of lockstep streams or fused grid-y; both are closed with
  evidence.
- No `auto` worker count and no universal default. Default stays 1. No policy
  claim beyond the demographic model family and H100-class device measured;
  broader claims need second-shape evidence per the gate.
- No kernel/codegen changes, no CUDA Graphs, no packed-readback work — those
  are separate measured-or-unmeasured questions and would make this result
  unattributable.
- No manifest schema change, no change to draw selection, parameter sampling,
  CRN semantics, or §E2 levels. No multi-GPU. No new dependencies.

## Acceptance criteria

**Local (required for approval):**

1. `--draw-workers` parsing and validation: default 1; CPU backend, zero,
   malformed, above-draw-count, and conflicting flag/env combinations all
   fail clearly before scientific output, covered by tests.
2. The capacity estimator has unit tests proving it is conservative against
   the measured 1M/10M points (estimate ≥ observed at every measured arm) and
   that preflight failure precedes any lane construction. The mechanism for
   injecting a fake memory limit in tests is stated.
3. Worker-count construction test: N lanes construct exactly N retained
   backends, not one per draw (mirrors the PRD 0001 construction counter).
4. Draw independence: draw `k` run alone is byte-identical to draw `k` run
   alongside others, for contests and grouped observations under both noise
   modes, driven through the supported flag.
5. Forced completion inversion publishes ascending `k` with byte-identical
   output (existing delay seam), driven through the supported flag.
6. Every existing golden byte-identical; `git diff --stat` shows none of
   them. No manifest schema change.
7. `cargo test --locked` and `scripts/check-rust.sh` green; CUDA-feature
   `cargo check`/`clippy` green without claiming GPU execution.
8. `python3 scripts/check-markdown-links.py` passes, and
   `python3 scripts/check-prd-allowlist.py` on this PRD reports no file named
   in an imperative sentence missing from the allowed list.
9. The timing document records execution mode, requested/effective workers,
   and the three phases; a test asserts the schema.

**Hardware (§J14.2, and per §M3 the command must exist):**

10. `cargo build --release --features cuda` on a GPU host.
11. The `BENCH_CONCURRENCY_SUPPORTED=1` collector stage drives the
    **supported flag** at workers 1/2/4,
    1M and 10M, three repetitions independent noise plus one CRN repetition:
    complete output trees byte-equal to sequential, negative control rejected.
12. Capacity-failure demonstration: a requested worker count sized to exceed
    device memory fails with the preflight error and writes no scientific
    output.
13. Nsight Systems trace at two lanes showing kernels on distinct
    non-default streams; reported as evidence, not as a success criterion —
    whole-sweep wall and byte-parity decide.

**Local criteria 1–9 were sufficient for implementation approval.** Hardware
criteria 10–13 were subsequently executed on H100 at `d72057f`; all pass.
The supported collector completed the independent/CRN matrix, capacity-failure
arm, and Nsight trace, with evidence at
[`hyperstack-supported-concurrency-20260730T022957Z`](../evidence/demographic-bench/hyperstack-supported-concurrency-20260730T022957Z).
The earlier local approval was not presented as GPU evidence.

## Note on expectations

The Gate 1 spike measured 1.598×/1.745× (10M) and 1.266×/1.270× (1M) for
this mechanism on H100. The completed supported-interface session independently
measured 1.584×/1.756× (10M) and 1.275×/1.278× (1M). The agreement confirms
that the production flag and capacity preflight preserve the spike result
rather than inheriting it by assumption (§M1).

Memory is the binding constraint, not compute: 22,701 MiB VRAM at 10M/four
lanes. The value of this PRD at 10M is real but bounded (≤ ~1.75× at four
lanes, with 2→4 lanes already flattening); anyone expecting near-linear
scaling should read the idle-gap and contention analysis in the freestream
`ANALYSIS.md` before requesting more lanes.
