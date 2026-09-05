# CUDA runtime review — 2026-09-05

Status: candidate implementation and local validation. New GPU correctness and
performance results are pending the runbook's paid-plan approval.

## Evidence used

The current reference is the retained H100 session
[`hyperstack-l4-20260905T025910Z`](../evidence/demographic-bench/hyperstack-l4-20260905T025910Z/README.md),
running commit `1688e51ac107511908d762e389bb26f4850fd747`. The CUDA production
code at review baseline `58e191aa3071310230070fee9d2df965d01681e8` is unchanged
from that measured commit.

The frozen 10M-slot, 24-tick no-grouped command took 6.060–6.210 seconds
(median 6.110). The separate 5M-slot, two-tick profile measured 11.409 ms for
the tick loop, of which 11.363 ms was assigned to the device execution and
observation phase. The grouped profile measured 14.880 ms, including 14.841 ms
in that phase. These are different workload sizes and timing boundaries;
do not divide the tick profile by the frozen whole-command time.

The older host-observation bottleneck is no longer the current tick profile.
The latest [kernel summary](../evidence/demographic-bench/hyperstack-l4-20260905T025910Z/profile/nsys-kern-sum.txt)
contains 340 launches and 11.405 ms of summed kernel duration. Its main costs
are effect preparation (15.6%), deferred counting (9.9%), scalar observations
(9.0%), conflict resolution (8.1%), and fired counting (7.4%). These are kernel
shares, not whole-command speedup predictions.

The retained `nsys-api-sum.txt` contains an export warning rather than a usable
API summary. A read-only query of the retained `profile-cuda.sqlite` instead
finds nine D2H API calls totalling 129.386 ms, including a largest call of
117.904 ms. This includes finalization outside the tick timers. It is not
evidence that tiny per-tick readbacks dominate, and does not reopen the retired
packed-control or scalar device-SHA experiments.

## Implemented candidate: remove work for uncontested tables

Code generation now retains the sorted, deduplicated global table indices
targeted by contest expressions, using the same resolved types as the generated
conflict kernel. The backend uses that metadata in two places:

- Winner buffers reserve rows only for contest targets. Global table offsets
  still address each target's contiguous segment; unrelated tables reserve no
  winner slots. The existing capacity estimator uses the reduced allocation.
- Deferred counts are reduced only for contest targets. Initialization still
  zeroes all table counts, preserving report shape and omission of zero counts.

In the demographic model, only `slot_resource` is contested. At 10M slots,
winner storage falls from `(20,000,004 × 16)` to `(10,000,000 × 16)` bytes:
**160,000,064 bytes saved per CUDA worker**, or 640,000,256 bytes across four
workers. Winner initialization touches the same smaller range. Deferred-count
launches fall from three to one per tick, eliminating two scans of the complete
candidate array. The per-candidate deferred buffer itself is unchanged.

Generated CUDA source is unchanged: the same kernels receive smaller winner
buffers and the corresponding offsets. An unused tuple component in grouped
source generation was also removed. No checked-in model, scientific output,
hash domain, or generated-source golden was regenerated.

Tests cover duplicate target tables reached through distinct reference
attributes, two boxes with repeated table names, multiple targets, empty
targets, and models without contests. The hardware test compares CPU reports
and state hashes through multiple ticks, reseeding, and two fused draw slots.
The differential collector invokes it before measuring performance.

Local validation passed: `./scripts/check-rust.sh`,
`./scripts/check-determinism.sh`, the CUDA-feature library tests (38 passed,
three hardware tests ignored), shell syntax, and `git diff --check`.
GPU execution remains unverified for this candidate.

## Further opportunities

| Priority | Source | Next experiment and constraint |
|---|---|---|
| 1 | Whole-command initialization and finalization | Add separate timing for state validation/copies/packing, NVRTC, upload, and final hash. Backend construction clones initial state and compiles per backend. The short tick profile cannot attribute the remaining whole-command time; measure before adding a persistent cache. |
| 2 | Effect preparation and validation launches | Effect preparation accounts for 80 of 340 launches and 15.6% of kernel time in the retained trace. Investigate statically proving harmless cases or combining work. Preserve first-error ordering, duplicate-write diagnostics, full-column validation, and rollback. Later validation passes already return early on success. |
| 3 | Grouped extrema and histograms | Extrema currently perform global min/max atomics per row and band axis; grouped/legacy-enum counts use global atomics per selected row. Compare block reductions or bounded shared histograms, with a fallback for large key spaces. Obtain a grouped kernel trace first: the retained kernel summary is no-grouped. |
| 4 | Fired counts and effect-active flags | Separate full winner scans count fired rows and set an active flag. `mark_effect_active` issues an atomic for every winning row. Reuse a reduction or reduce within blocks, preserving the point at which effect validation consumes the flag. |
| 5 | Group aggregates outside the demographic case | `build_aggregate_partials` currently admits one worker and scans rows serially. Filtered Count is a promising exact parallel case. Ordered Real sums and checked Int sums cannot be reordered indiscriminately; use an aggregate-heavy workload and preserve error semantics. |

Do not infer a whole-command improvement from the saved allocation or the
9.9% deferred-count kernel share. The candidate needs a same-host comparison.

## Hardware validation plan

Follow [`RUNBOOK.md`](../../spikes/precision/infra-hyperstack/RUNBOOK.md) and
the authoritative [provisioning README](../../spikes/precision/infra-hyperstack/README.md).
Use a clean committed CUDA-only candidate checkout, with review baseline
`58e191aa3071310230070fee9d2df965d01681e8` for before/after measurements.

The existing collector supports the required sequence without changing the
frozen protocol:

```sh
BENCH_CORPUS=1 BENCH_PROFILE=1 BENCH_SWEEP=1 \
BENCH_SWEEP_BASELINE_COMMIT=58e191aa3071310230070fee9d2df965d01681e8 \
  bash run-demographic-benchmark.sh
```

The differential corpus and the added sparse-resource GPU test must pass.
The retained-backend stage compares baseline/current CPU and CUDA sweeps at
1M and 10M slots, 20 draws, 24 ticks, seed 9009, with grouped observations,
complete output-tree comparisons, and a deliberately corrupted comparator
control. The frozen gate retains three CPU/CUDA replicates at 10M slots and
24 ticks. Profile launches are separate from performance measurements.

Inspect the new kernel trace for one deferred-count launch per tick and the
smaller winner initialization. Report whole-command and retained-sweep times
separately, retain raw measurements and checksums, and report a neutral or
negative result if the saved device work is not visible in wall time.

Live read-only discovery on 2026-09-05 found CANADA-1 H100 PCIe availability at
$2.50/hour plus $0.006720430/hour for the public IP. Provider reconciliation
and Terraform state both reported no VMs. Plan for roughly 2–3 hours including
the CPU sweep arms, with a four-hour destroy watchdog. Pricing and stock must
be rechecked when preparing the exact saved plan. No paid resource has been
created for this review.
