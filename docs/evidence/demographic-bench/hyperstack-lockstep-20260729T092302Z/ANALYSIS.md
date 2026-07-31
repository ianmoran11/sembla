# Synchronized CUDA lockstep-stream analysis

## Verdict

The synchronized non-blocking-stream arm is **negative as a production design**.
It creates real simultaneous CUDA kernel execution, but it is slower than the
previously measured independently scheduled complete-backend arm at every
concurrent point. Production sweeps remain sequential and no numbered
concurrent-draw PRD is justified by this arm.

This experiment is not fused draw-dimension batching. Each lane still owns a
complete backend/module/state allocation. A fused grid-y kernel with draw-major
state remains a materially broader, unmeasured spike; these results neither
implement nor validate it.

## Provenance and correctness

- Repository commit: `645bb7c2b8da9be063ee48470a440dee29964408`.
- Release binary SHA-256:
  `068607a6b7766b340ca1e83d43b551aff0ec46e0d24fd07bf5e890c2e30dbd33`.
- Hardware: one NVIDIA H100 PCIe GPU.
- Shapes: 1M and 10M slots, 20 draws, 24 ticks, independent noise, grouped
  observations, workers 1/2/4, three repetitions per worker count.
- All 18 complete output-tree comparisons are byte-identical to their adjacent
  sequential references.
- The deliberate grouped-sidecar perturbation is rejected at both scales.
- The schedule control proves deterministic `k % workers` lane assignment,
  overlapping per-round lane intervals, the explicit
  `cuda-lockstep-nonblocking-streams` timing identity, ascending-`k`
  publication, and byte-identical output.
- The final local `SHA256SUMS` covers and verifies all 2,317 files other than
  the manifest itself, including this analysis and teardown verification.
- This paid matrix used independent noise. CRN is not separately exercised by
  this hardware session; because the arm is performance-negative, that gap does
  not weaken its rejection, but it would block positive acceptance.

## Whole-sweep timing

Values are medians of three native `whole_sweep_wall_time_ms` measurements.
The isolated-backend comparison is the accepted adjacent H100 evidence in
[`hyperstack-concurrency-20260729T064051Z`](../hyperstack-concurrency-20260729T064051Z).

| Scale | Workers | Lockstep streams | Speedup vs lockstep workers=1 | Isolated backends | Lockstep vs isolated |
|---:|---:|---:|---:|---:|---:|
| 1M | 1 | 5.053 s | 1.000× | 4.991 s | 1.3% slower |
| 1M | 2 | 4.220 s | 1.197× | 3.774 s | 11.8% slower |
| 1M | 4 | 4.037 s | 1.252× | 3.939 s | 2.5% slower |
| 10M | 1 | 36.634 s | 1.000× | 36.401 s | 0.6% slower |
| 10M | 2 | 28.257 s | 1.296× | 24.483 s | 15.4% slower |
| 10M | 4 | 26.068 s | 1.405× | 22.003 s | 18.5% slower |

The lockstep arm improves on its own sequential reference, but synchronization
reduces the useful gain. At the capacity-relevant 10M/four-worker point, its
1.405× speedup is materially below the isolated arm's 1.654× speedup.

## Nsight Systems

The 1M/two-worker trace uses four draws and the same 24-tick model as the prior
trace.

| Metric | Isolated/default stream | Lockstep/non-blocking streams |
|---|---:|---:|
| Kernel launches | 39,552 | 39,552 |
| CUDA contexts | context 1 | context 1 |
| CUDA streams | stream 7 | streams 13 and 14 |
| Kernel time summed | 307.323 ms | 355.407 ms |
| Kernel interval union | 307.323 ms | 319.468 ms |
| Time with concurrency ≥2 | 0 ms | 35.939 ms |
| Maximum simultaneous kernels | 1 | 2 |
| Positive inter-kernel gap total | 168.988 ms | 418.451 ms |
| Median positive gap | 1.312 µs | 8.897 µs |
| p95 positive gap | 2.048 µs | 30.626 µs |

The trace directly proves overlap: streams 13 and 14 each execute 19,776
kernels, with 35.939 ms at concurrency two. That is 11.25% of the kernel
interval union. It also explains why overlap is not sufficient: summed kernel
work rises 15.6%, and synchronization/host scheduling more than doubles total
positive launch gaps. The overlapped union remains 4.0% longer than the prior
serialized kernel union.

Nsight Compute occupancy/bandwidth data was not collected. It is not required
to reject this specific arm because exact whole-sweep measurements already show
that it loses to the simpler isolated scheduler, but it remains a diagnostic
input if a later fused-kernel spike is separately approved.

## Capacity

| Scale | Workers | Peak VRAM | Peak process RSS |
|---:|---:|---:|---:|
| 1M | 1 | 1,033 MiB | 451,936 KiB |
| 1M | 2 | 1,613 MiB | 700,784 KiB |
| 1M | 4 | 2,733 MiB | 1,127,644 KiB |
| 10M | 1 | 6,025 MiB | 2,551,836 KiB |
| 10M | 2 | 11,597 MiB | 5,384,008 KiB |
| 10M | 4 | 22,701 MiB | 9,369,148 KiB |

Non-blocking streams do not change the complete-backend memory slope. The
10M/four-worker arm still requires about 22.7 GiB VRAM and 9.0 GiB RSS.

## Decision

1. Close synchronized complete-backend lockstep streams as a production
   candidate.
2. Keep the production sweep default sequential.
3. Do not draft the provisional concurrent CUDA PRD sequence from this result.
4. Treat fused draw-dimension/grid-y batching as a separate, broader Gate-1
   spike only. It must preserve draw-indexed parameters, RNG coordinates,
   mutable state, reductions, diagnostics, grouped observations, and exact
   output before it can support a numbered PRD.
5. Judge any fused spike against both sequential execution and the faster
   isolated-backend measurements above; kernel overlap alone is not acceptance.

## Teardown and artifact integrity

- Evidence branch: `evidence/hyperstack-20260729T093114Z`, commit
  `99109fe553233b4594bdd9c6cb57997709bb621c`.
- The evidence branch contains only `docs/**`, no `.piprd/**`, Terraform state,
  plan, or credential-named files.
- [`collector-driver.log`](collector-driver.log) records Terraform destroying
  the VM and SSH rule successfully.
- [`teardown-verification.txt`](teardown-verification.txt) records empty
  Terraform state, independent Hyperstack reconciliation with zero tracked VMs,
  zero `sembla-precision*` orphans, and zero unrelated account VMs.
- The same teardown record proves the consumed paid plan, ephemeral injected
  host-key material, known-host entry, and write-enabled evidence deploy key are
  absent, with no collector/watchdog process remaining.
