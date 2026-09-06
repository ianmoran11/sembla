# CUDA runtime review — 2026-09-05

Status: implementation, local checks, H100 differential corpus, before/after
sweeps, and the frozen validation gate passed. Evidence was retrieved and
verified; the VM was destroyed and temporary credentials were cleaned up.

Raw evidence is retained in
[`hyperstack-l4-20260905T121420Z`](../evidence/demographic-bench/hyperstack-l4-20260905T121420Z/README.md).
The remote measurements are also on branch
`evidence/hyperstack-20260905T133320Z`, commit
`07c859507c9b674dbf3f5fa6d95f9bf8725404c4`. Local closeout records are additional
to that remote snapshot; `SHA256SUMS.remote` remains unchanged.

## Same-host before/after measurements

The approved session compares baseline
`58e191aa3071310230070fee9d2df965d01681e8` with CUDA-only candidate
`4458a9de2f7477b34d0bb5b7585685185e65ea63` on one H100 PCIe. Both arms use the
same model and initial state, 24 ticks, 20 draws, seed 9009, and grouped
observations. Each baseline/current pair ran consecutively. CPU runtime code
is identical between these two commits; the separate local CPU improvements
are excluded from this experiment.

| Slots | Baseline CUDA sweep | Candidate CUDA sweep | Wall-time reduction | Baseline CPU sweep | Candidate CPU sweep |
|---|---:|---:|---:|---:|---:|
| 1M | 4.17 s | 3.99 s | 4.3% | 152.62 s | 150.78 s |
| 10M | 26.52 s | 25.35 s | 4.4% | 1737.86 s | 1757.91 s |

These are whole-process times, including setup. This is **one sweep pair per
scale**, not a replicated estimate of a general speedup. The unchanged CPU
arms moved by about −1.2% at 1M and +1.2% at 10M, illustrating host variation.
Later-draw median CUDA times were 133.97 → 129.06 ms at 1M and
1050.24 → 1014.80 ms at 10M. Those draw timings use 3 ms polling of published
draw files, rather than CUDA event timing; draws also have distinct seeds.

The new 5M-row, two-tick no-grouped profile contains **336 kernel launches**,
including two deferred-count launches totalling 0.375 ms. The retained reference
has 340 launches, including six deferred-count launches totalling 1.126 ms.
Winner initialization remains two launches but touches half as many rows in
this model (0.077 ms versus the reference's 0.151 ms). The no-grouped tick loop
took 10.831 ms and the grouped loop 14.662 ms. The reference profile comes from
a different H100 session: its durations explain the mechanism, while the
adjacent sweeps above supply the controlled before/after wall measurements.

Independent verification checked all **103 files per sweep tree**: baseline
versus candidate for each backend, and CPU versus CUDA for each version, at
both scales. Same-backend comparisons are byte-exact. Cross-backend comparisons
normalize only the established `backend_identity` manifest field. Exported
pairs also match, and the deliberately corrupted sidecar was rejected.

The separate frozen 10M-row, 24-tick no-grouped gate measured CUDA at
6.28/6.32/6.37 s and CPU at 52.86/49.94/49.93 s. The median ratio is 7.902×,
passing its 3× gate. Results, summaries, and execution hash tuples are
byte-identical across all six runs. This gate compares backends at the
candidate commit; it does not measure the candidate's speedup over baseline.

The [analysis JSON](../evidence/demographic-bench/hyperstack-l4-20260905T121420Z/review/sweep-analysis.json)
contains raw wall time, process RSS, first-draw time, later-draw medians, and
comparison counts. Reproduce the checksum and output verification with:

```sh
evidence=docs/evidence/demographic-bench/hyperstack-l4-20260905T121420Z
python3 "$evidence/review/verify.py" "$evidence"
```

## Reference profile used to select the change

The starting reference is the retained H100 session
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
The reference [kernel summary](../evidence/demographic-bench/hyperstack-l4-20260905T025910Z/profile/nsys-kern-sum.txt)
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

## Implementation: remove work for uncontested tables

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
The full GPU differential corpus also passed, including the new sparse-resource
test's CPU report/state comparisons, reset, and two fused draw slots.

## Further opportunities

| Priority | Source | Next experiment and constraint |
|---|---|---|
| 1 | Whole-command initialization and finalization | Add separate timing for state validation/copies/packing, NVRTC, upload, and final hash. Backend construction clones initial state and compiles per backend. The short tick profile cannot attribute the remaining whole-command time; measure before adding a persistent cache. |
| 2 | Deferred flag allocation and initialization | Flags still reserve one byte per candidate per global table. Reuse the contest-target metadata to compact this dimension. Unlike the measured change, this needs generated-kernel indexing changes; preserve global-table report order and fused-slot isolation. |
| 3 | Effect preparation and validation launches | Effect preparation accounts for 80 of 340 launches and 15.6% of kernel time in the reference trace. Investigate statically proving harmless cases or combining work. Preserve first-error ordering, duplicate-write diagnostics, full-column validation, and rollback. Later validation passes already return early on success. |
| 4 | Grouped extrema and histograms | Extrema currently perform global min/max atomics per row and band axis; grouped/legacy-enum counts use global atomics per selected row. Compare block reductions or bounded shared histograms, with a fallback for large key spaces. Obtain a grouped kernel trace first: the retained kernel summary is no-grouped. |
| 5 | Fired counts and effect-active flags | Separate full winner scans count fired rows and set an active flag. `mark_effect_active` issues an atomic for every winning row. Reuse a reduction or reduce within blocks, preserving the point at which effect validation consumes the flag. |
| 6 | Group aggregates outside the demographic case | `build_aggregate_partials` currently admits one worker and scans rows serially. Filtered Count is a promising exact parallel case. Ordered Real sums and checked Int sums cannot be reordered indiscriminately; use an aggregate-heavy workload and preserve error semantics. |

The measured improvement applies to the demographic workload above. The saved
allocation and the reference's 9.9% deferred-count kernel share do not predict
whole-command gains for other models.

## Hardware validation protocol

The session follows [`RUNBOOK.md`](../../spikes/precision/infra-hyperstack/RUNBOOK.md)
and the authoritative [provisioning README](../../spikes/precision/infra-hyperstack/README.md),
using the clean committed CUDA-only checkout and exact baseline listed above.

The existing collector runs without changing the frozen protocol:

```sh
BENCH_CORPUS=1 BENCH_PROFILE=1 BENCH_SWEEP=1 \
BENCH_SWEEP_BASELINE_COMMIT=58e191aa3071310230070fee9d2df965d01681e8 \
  bash run-demographic-benchmark.sh
```

The differential corpus and the added sparse-resource GPU test passed.
The retained-backend stage compared baseline/current CPU and CUDA sweeps at
1M and 10M slots, 20 draws, 24 ticks, seed 9009, with grouped observations,
complete output-tree comparisons, and a deliberately corrupted comparator
control. The frozen gate used three CPU/CUDA replicates at 10M slots and
24 ticks. Profile launches are separate from performance measurements.

The user approved the saved plan for one CANADA-1 H100 PCIe at $2.50/hour plus
$0.006720430/hour for the public IP. Provisioning began at approximately
12:11 UTC on September 5, with an independent destroy watchdog. The collector
finished successfully and destroyed the VM. At 13:34:49 UTC,
[provider verification](../evidence/demographic-bench/hyperstack-l4-20260905T121420Z/review/provider-verification.json)
confirmed an HTTP 200 response with zero session, project, or other VMs, and
Terraform state contained no paid resources. This is approximately $3.50 at
the approved hourly rate; it is a runtime estimate, not an invoice amount.

The [closeout record](../evidence/demographic-bench/hyperstack-l4-20260905T121420Z/review/session-closeout.json)
records watchdog shutdown, disposable key revocation, removal of the generated
console-password Keychain item, cleared session credentials, and restoration
of the pre-session local Terraform configuration. No paid resource remains.
