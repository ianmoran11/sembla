# CUDA validation-parallelism PRDs

Ordered PRD set that makes the CUDA backend usable for models with contests and
`Ref` dereferences. Run from the Sembla repository with:

```text
/piprd run docs/prds-cuda-validation-parallelism
```

`README.md` is ignored by `/piprd run`. Every numbered PRD must read this file
first; the constraints below are binding. When a PRD conflicts with this README,
this README wins.

## The measured problem

On 2026-07-25, on one Hyperstack host (NVIDIA H100 PCIe, AMD EPYC 9554, CUDA
12.8, driver 570.195.03), the `demographic_slots` no-grouped model at 10,000,000
slots for 24 ticks measured:

| Backend | Wall time | s/tick |
|---|---:|---:|
| CUDA (H100 PCIe) | 1h 30m 08s | 225.3 |
| CPU (EPYC 9554, same host) | 7m 19s | 18.3 |

**The GPU is 12.3× slower than the CPU beside it**, on the same binary, commit,
seed, and state artifact, differing only in `--backend`. Evidence:
`docs/evidence/demographic-bench/hyperstack-cuda-10m-20260725/` and
`docs/evidence/demographic-bench/hyperstack-cpu-10m-20260725/`.

**Evidence status correction (2026-07-25).** The checked-in CUDA arm records
commit `f81fef9d90ad`; the CPU arm records `8857cb839220`. No `crates/**`, Cargo
manifest/lockfile, or fixture files changed between those commits, and the
benchmark-script delta affects only the CUDA zero-tick export step. The stored
pair therefore supports the diagnostic 12.3× baseline but does not substantiate
the same-commit, same-artifact sentence above and is not §L4 gate evidence. The
frozen §L4 protocol remains unchanged.

For contrast, ADR 0001 measured ~1,380 ticks/sec for SIR at 26M rows on the same
GPU class. SIR declares no contests and dereferences no `Ref`, so it generates
none of the code below.

## The cause

`crates/sembla-cuda/src/codegen.rs` emits four kernels whose bodies open with

```c
if (blockIdx.x != 0 || threadIdx.x != 0 || status[0] != 0ULL) return;
```

and which then contain `for (unsigned long long row = 0; row < row_counts[...];
++row)`:

- `sembla_validate_claims`
- `sembla_validate_transition`
- `sembla_validate_effects`
- `sembla_validate_outputs`

One GPU thread therefore walks every row, once per claim and per fallible
expression, every tick. A single GPU thread has no instruction-level parallelism
and no latency hiding, so it is far slower at this than one CPU core. The
simulation kernels themselves (`resolve_conflicts`, `apply_effects`,
`build_aggregate_partials`, `finish_aggregates`, `build_output_partials`,
`finish_outputs`) are already properly parallel. **The defect is in validation,
not in execution.**

The loops are emitted conditionally, per claim and per expression that can fail,
which is why the cost appears exactly for the model class the forward roadmap is
built on: slot claiming, generation-safe references, household links, and
identity-preserving migration all dereference `Ref` or contest a resource.

## Authority and scope

- `DESIGN.md` (§4.2 kernel fragment, §5.2 precision, §8 two execution paths),
  `DECISIONS.md` (§E2 determinism levels, §E3 conflict resolution, §J14 GPU
  evidence discipline, §K6 grouped observations), and
  [ADR 0001](../decisions/0001-gpu-precision.md) all bind.
- **Validation stays a separate pass.** Fusing per-row validation into the
  execution kernels is rejected for this track: it entangles two concerns that
  are currently independent and easy to reason about, for a speedup that is not
  needed to clear the bar below. Recorded as a decision, not left implicit.
- **No new model semantics.** This folder changes how existing checks execute,
  never what they mean, when they run, or what they accept.

## Frozen and untouchable

`examples/**`, every CSV/hash golden, the CUDA differential evidence, plan and
source schemas, version strings, the `SEMBLA_POP` and `sembla.state/v1` formats,
Philox layout, and the conflict argmin semantics. A diff to any of them is a
failed PRD.

## Determinism and diagnostics are the hard constraint

The current code reports the **first failing candidate in row order**
(`status[1] = candidate`). Diagnostics are part of the observable contract, not
an implementation detail: the CPU oracle is ground truth and the differential
harness compares contract diagnostics, so a parallel implementation that reports
*a* failing candidate instead of *the same* one is a regression even when it is
"more correct". Parallel reduction to the minimum failing index is required, and
must be demonstrated by test rather than asserted.

Determinism Level A (bitwise, same binary and GPU model) is unchanged. Nothing
here may make results depend on block count, launch configuration, or scheduling
order.

## Deferred, with triggers

- **CUDA support for grouped observations** (§K9, follow-up folder). This track
  must not become that one; the demographic model enters the differential corpus
  in its no-grouped configuration only.
- **Hoisting validation out of the per-tick path.** Tempting, and the largest
  possible win, but it requires proving the checked property cannot change
  within a tick — and Track R deliberately makes `Ref` mutable. Trigger: an
  accepted proof obligation, not a performance argument.
- **`Expr::Tick` / derived age.** Separate §K2 trigger. Note that the ageing
  cost share measured 11.6% (M2 Pro) and 12.2% (EPYC) at 10M, both above the
  10% threshold, on unreplicated single runs. That is this track's business only
  insofar as its benchmark should report the share.

## PRDs

- `0001-decision-record-and-baseline` — record the finding and the three design
  decisions in `DECISIONS.md`; freeze the benchmark case and the numeric gate.
  Documentation only.
- `0002-parallel-validation-kernels` — grid-stride the four row loops with
  deterministic minimum-index failure reporting.
- `0003-diagnostic-equality-tests` — negative models must report identical
  status codes and candidate indices on CPU and CUDA.
- `0004-differential-corpus-membership` — existing corpus byte-unchanged; the
  demographic no-grouped model joins it, closing the coverage gap that let this
  regression pass every differential test.
- `0005-measure-and-publish` — re-run the frozen case, commit evidence, correct
  `docs/demographic-benchmark.md` and the forward roadmap's scale note.

## Acceptance notes

Per §J14.2, PRDs split acceptance into **local criteria** (must pass without a
GPU: compilation, codegen text assertions, corpus listing, graceful skips,
legacy goldens unchanged) and **hardware criteria** (executed on a rented GPU
and recorded as evidence). Codegen changes are testable locally by asserting on
the emitted CUDA source; only the timing gate needs hardware.
