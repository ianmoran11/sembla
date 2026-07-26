# Execution timing instrumentation PRDs

PRD set adding per-phase timing attribution to the run paths, so the CUDA
re-measurement produces measured milliseconds rather than inferred shares. Run
from the Sembla repository with:

```text
/piprd run docs/prds-execution-timing
```

`README.md` is ignored by `/piprd run`. Every numbered PRD must read this file
first; the constraints below are binding. When a PRD conflicts with this README,
this README wins.

## Why this folder exists

`docs/evidence/cuda-l4-20260726/` measured the CUDA backend at 5M rows over 2
ticks and produced this:

| | time |
|---|---:|
| wall clock | 10,620 ms |
| all GPU kernels combined | 9.1 ms |
| host-side CUDA API | 402 ms |
| **unaccounted (host CPU)** | **~10,200 ms** |

That last row is 96% of the run and it is not attributed to anything. It was
enough to establish "the GPU is not the constraint", but not enough to scope a
fix.

Since then `prds-host-evaluator-performance` cut host user time from 46.8 s to
5.90 s on the fixed 1M case — about 8×. The CUDA path shares that host work: its
tick loop calls `executor::observe_views` (which routes through `eval_expr`) and
`state.state_hash()` directly, and `CudaBackend::run_tick_observed` calls
`download_state_store`, which copies the full device state back **and rebuilds
the entire host `StateStore` from scratch every tick**. So the unaccounted block
has almost certainly changed shape, and re-running the same profile would only
produce a smaller unattributed remainder.

## Why timers rather than sampling

PRD 0004 in the host-evaluator folder removed a provable 8 MB-per-operand
`memcpy`. The allocator and `memmove` symbols dropped about 20% in the profile.
The measured user time did not move at all — the change was within run-to-run
scatter.

**Sample share and time cost came apart.** Scoping the next paid GPU session
from another set of sample percentages would repeat that mistake at a higher
price. These PRDs therefore instrument the phases directly and report
milliseconds.

`perf` is not installed by the Hyperstack image's cloud-init, which is a
secondary reason sampling is awkward on that host; `nsys` remains available and
still gives kernel-level detail, which direct timers cannot.

## Binding constraints

- **Instrumentation must be inert when off.** It is off by default. With the
  flag absent, every output byte, every manifest field, and every hash must be
  identical to the same build without the instrumentation. This is testable and
  every PRD here must test it.
- **Results must not change, ever.** Not with the flag on either. Timing
  observes; it does not alter what is computed, accepted, or reported. The CPU
  oracle's role (DESIGN.md §8) and §E2's determinism levels are unaffected.
- **This is not a `DECISIONS.md` §E8 flag.** §E8 governs flags that change what
  a model *means*, because a flag that alters results while being invisible to
  the manifest would falsify the §2 contract. A timing flag cannot alter
  results, so it does not belong in `FeatureSet` and does not require a
  manifest field. That reasoning is argued rather than assumed, and each PRD
  must demonstrate it with an on-versus-off byte-identity test.
- **Overhead must be negligible and bounded.** Per-tick granularity only. No
  timer inside a per-row loop, ever.

## Local versus hardware acceptance (inherited from §J14.2)

The CUDA feature cannot be built on the development Mac — there is no `nvcc`,
and `scripts/check-rust.sh` does not compile it. Following `DECISIONS.md`
§J14.2, PRDs here split acceptance:

- **Local criteria** — must pass in the managed run without a GPU: the CPU path
  instrumented and demonstrated, the default build green, the inertness test
  passing, goldens unchanged. A PRD is approvable on local criteria alone.
- **Hardware criteria** — listed in the implementation notes as *pending*, and
  executed in the next Hyperstack session. Presenting an unbuilt CUDA path as
  verified is rejected.

## PRDs

- `0001-per-phase-timing-instrumentation` — **implemented locally 2026-07-26**:
  the default-off `--timing-json` flag attributes each tick's wall time to
  named phases in both backends. CPU schema and inertness checks pass; CUDA
  hardware validation remains pending under §J14.2.

### 0001 implementation notes

The option is deliberately separate from `FeatureSet` and `--enable`.
`FeatureSet` records semantic switches under `DECISIONS.md` §E8; timing is
observational, changes no result, and therefore must not imply semantic meaning
or add a manifest field. The integration test makes that distinction
checkable by comparing stdout, results, summaries, manifest (including
`final_state_sha256`), and exported state byte-for-byte with timing off and on.
The disabled branch calls the ordinary untimed runner and owns no timing
collector.

The JSON `session.scale` is the maximum initialized table row count, so the
five-million-slot demographic case records `5000000` rather than summing its
small auxiliary tables. Durations use `std::time::Instant`, retain nanosecond
resolution internally, and are reported in milliseconds. `other` is computed
with checked duration subtraction and every tick plus the totals must reconcile
within 0.001 ms before the document is written.

The CUDA executor already synchronizes at the end of `execute_tick` through the
same-stream status device-to-host copy. No second synchronization was added,
and timing JSON records `"kernel_sync_inserted": false`; the `kernels` phase
therefore includes the existing wait without adding another perturbation.

CPU overhead was measured in-run with the release binary on the fixed
one-million-slot, 24-tick, seed-9009 case in three interleaved off/on pairs.
Median user time was 6.06 s off and 6.09 s on (+0.03 s, 0.50%); fastest user
time was 6.00 s off and 6.04 s on (+0.04 s, 0.67%). Median wall time was 7.70 s
off and 7.91 s on, with wall time noisier than user time. Every paired CSV,
summary, manifest, and stdout was byte-identical. This bounds the observed CPU
cost of per-tick timers plus session identity and JSON emission as negligible
for the target run shape.

Hardware criteria remain **pending under §J14.2**: build the release CUDA
feature on the GPU host; verify all CUDA phases with transfer and reconstruction
separate; verify reconciliation and non-negative `other`; and collect the
5M-row, 2-tick `cuda-l4-20260726` case. No local CUDA build or runtime result is
claimed.

Later PRDs are **deliberately not written.** What the instrumentation measures
decides what, if anything, comes next.

## What this is for

The immediate consumer is one Hyperstack session re-measuring the CUDA path at
the `cuda-l4-20260726` case, to answer a single question: of the wall time that
is not GPU kernels, how much is device-to-host transfer, how much is host state
reconstruction, and how much is host observation? Those three have different
fixes, and today we cannot tell them apart.
