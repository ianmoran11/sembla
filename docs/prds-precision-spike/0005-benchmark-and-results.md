---
max_review_cycles: 3
---

# PRD 0005: Unified benchmark — throughput × accuracy matrix

## Context

PRDs 0002–0003 produced three precision strategies (`f32`, double-single, native
`f64`) plus the CPU `f64` oracle. This PRD ties them into one benchmark that, on
whatever adapter is present, measures **both axes that the decision needs**:
steady-state throughput (ms/tick, rows/sec) *and* accuracy vs the oracle
(reduction error, argmin winner-mismatch rate). It writes `spikes/precision/RESULTS.md`
with a strategy × metric matrix, honestly marking cells that could not run on the
current hardware as **unanswered** — including native `f64`, which is filled from
the PRD-0004 NVIDIA box.

Implements: `DESIGN.md` §4.2, §5.1, §5.2, §9 (throwaway-spike + honest-reporting
discipline); `docs/ROADMAP.md` v0.2 precision fork.

## Goal

`cargo run --release` runs every precision strategy available on the current
adapter over the PRD-0001 workload, measures throughput and accuracy for each,
and writes/updates `RESULTS.md` with a complete matrix plus per-strategy verdict
notes and the adapter/fp64-class metadata.

## Specification

- **Runner** (`src/bin/bench.rs` or `main.rs`): resolve the workload size for the
  adapter (PRD 0001), compute the CPU `f64` oracle once, then for each *available*
  strategy run 10 warmup + ≥100 measured ticks and record:
  - **Throughput:** median ms/tick (per-kernel via timestamp queries where
    supported, else synchronized wall-clock fallback — label which), and
    rows/sec. Break out the two hot stages: **segmented reduce** and **segmented
    argmin**, since those are what precision changes.
  - **Accuracy vs oracle:** reduction max/mean relative error; argmin winner-
    mismatch fraction; and the reduction order-sensitivity count (from the
    PRD-0001 reversed-order diagnostic) for the deterministic two-pass variants.
- **Strategy availability:** `f32` and double-single run on the portable adapter;
  native `f64` runs only where `SHADER_F64` is present (else its row is
  `unanswered on this adapter`). CUDA row filled only with `--features cuda` on a
  box with `nvcc`. The matrix always shows all strategies, marking absent ones
  explicitly.
- **`RESULTS.md` contents** (single durable numbers artifact; overwrite on each
  run, like the v0.1 spike):
  - hardware: adapter name/backend/device-type, `SHADER_F64` capability, and —
    for NVIDIA — the **fp64 throughput class** (`full-rate` vs `rate-limited`)
    and the exact GPU model, from PRD 0003/0004;
  - workload: actual `(N, G)`, downscale reason, contested-key selector, warmup/
    measured tick counts, `beta`/`dt`;
  - the **strategy × metric matrix**: rows = {`f32`, `double-single`,
    `native f64 (wgpu)`, `native f64 (CUDA)`}, columns = {ms/tick total,
    ms/tick reduce, ms/tick argmin, rows/sec, reduction rel-err, winner-mismatch
    %, order-sensitive groups}, with `unanswered` where not run;
  - the atomics-vs-deterministic-reduction choice per strategy (the Level C vs
    Level A fork, `DESIGN.md` §5.2);
  - a short per-strategy verdict paragraph.
- **Cross-machine assembly:** the dev-machine run fills `f32` + double-single and
  marks native `f64` unanswered; the PRD-0004 NVIDIA run fills the native-`f64`
  rows. `RESULTS.md` supports a merged view — document the two-run assembly and
  keep both machines' metadata blocks (do not silently overwrite the Mac block
  with the NVIDIA one).
- **Regression guard:** a fast test runs 1 tick at small scale for each available
  strategy and asserts the accuracy metrics are within the PRD-0002/0003
  thresholds, so `RESULTS.md` is never generated from a silently-broken kernel.

## Non-goals

The decision writeup and DESIGN.md amendment (PRD 0006), provisioning hardware
(0004), new precision strategies, optimizing kernels beyond hygiene, Level B
cross-model bitwise measurement.

## Acceptance criteria

1. `cargo run --release` inside `spikes/precision/` writes/updates `RESULTS.md`
   with the full strategy × metric matrix, adapter + fp64-class metadata, and
   per-strategy verdicts; strategies not runnable on the adapter show
   `unanswered`.
2. On the Metal dev machine the run completes with `f32` + double-single filled
   and native `f64` explicitly `unanswered on this adapter`.
3. Per-kernel breakdown isolates the segmented-reduce and segmented-argmin stages.
4. Accuracy numbers are computed against the PRD-0001 `f64` oracle and match the
   0002/0003 thresholds (guarded by a committed test).
5. `RESULTS.md` documents the two-run (Mac + NVIDIA) assembly and preserves both
   machines' metadata; it is committed with whatever numbers the running machine
   produced.
