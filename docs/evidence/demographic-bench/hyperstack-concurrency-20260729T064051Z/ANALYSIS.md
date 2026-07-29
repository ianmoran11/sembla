# H100 concurrent sweep-draw analysis

## Result

The complete-backend-per-lane spike is scientifically correct and improves
whole-sweep throughput, but it does **not** execute CUDA kernels concurrently.
It is therefore a useful lower bound, not the CUDA design to publish as a
production default.

- Repository commit: `7eb21efd029b09303e7b7d1aede4ccc9c3b30cb5`.
- Release binary SHA-256:
  `b86f69051c3f4e308ecdfdde807bbce6b21ca7ad11293f5c6fa8722dc30cfc5a`.
- GPU: NVIDIA H100 PCIe, driver 570.195.03.
- Protocol: 20 draws, 24 ticks, seed 9009, independent noise, grouped
  observations and exported pairs, three repetitions per worker count.
- Every complete output-tree comparison passed at workers 1, 2, and 4 for both
  1M and 10M. Each deliberately perturbed grouped sidecar was rejected.
- The forced schedule arm completed draw 1 before draw 0 and still published a
  byte-identical ascending-`k` output tree.
- The CPU/CUDA differential corpus passed on the same commit and H100.

## Timing

Whole-sweep seconds are the three observed repetitions. Speedups are paired by
repetition against workers 1; the table reports their median.

| scale | workers | whole-sweep seconds | median seconds | median paired speedup | median draw wall ms |
|---:|---:|---|---:|---:|---:|
| 1M | 1 | 4.963, 4.993, 4.991 | 4.991 | 1.000× | 176.2 |
| 1M | 2 | 3.756, 3.781, 3.774 | 3.774 | **1.321×** | 224.3 |
| 1M | 4 | 3.939, 3.948, 3.886 | 3.939 | **1.265×** | 455.0 |
| 10M | 1 | 36.375, 36.470, 36.401 | 36.401 | 1.000× | 1,577.8 |
| 10M | 2 | 23.469, 26.439, 24.483 | 24.483 | **1.487×** | 1,803.8 |
| 10M | 4 | 21.657, 22.003, 22.370 | 22.003 | **1.658×** | 3,060.3 |

Four lanes regress relative to two at 1M but win at 10M. Per-draw latency rises
with concurrency even when whole-sweep throughput improves, so a future policy
must optimize the declared whole-sweep objective rather than individual draw
latency.

Concurrent timing separates backend setup, execution window, and ordered
publication. Median publication is only 5–6 ms. Median backend setup rises from
1.045 s at two lanes to 1.163 s at four lanes for 1M, and from 2.700 s to
3.204 s for 10M. The throughput gain therefore comes from overlapping host-side
work and reducing idle gaps, not from hiding publication.

## Capacity

| scale | workers | median peak RSS MiB | median peak GPU memory MiB | median sampled peak GPU utilization |
|---:|---:|---:|---:|---:|
| 1M | 1 | 419.8 | 1,033 | 84% |
| 1M | 2 | 664.7 | 1,611 | 95% |
| 1M | 4 | 1,085.4 | 2,731 | 94% |
| 10M | 1 | 2,492.0 | 6,025 | 99% |
| 10M | 2 | 5,018.8 | 11,595 | 99% |
| 10M | 4 | 9,031.5 | 22,699 | 99% |

Memory scales materially with isolated lanes. Admission must preflight complete
slot capacity and fail clearly; silently reducing the requested count would make
both performance and timing records misleading.

## Nsight Systems overlap result

The 1M workers-2 trace contains 39,552 kernel intervals. Every interval uses
CUDA context `1` and stream `7`:

- summed kernel time: 307.323069 ms;
- union of kernel intervals: 307.323069 ms;
- time with two or more kernels active: **0 ms**;
- maximum simultaneous kernels: **1**;
- summed-time / union-time ratio: exactly **1.0**.

Thus, the two draw threads overlap at the host level while their CUDA kernels
serialize on one legacy/default stream. Utilization and wall-time speedups alone
would have incorrectly suggested device overlap; the trace is the deciding
evidence.

## Decision

This session admits neither a default CUDA worker count nor a production design
that creates complete default-stream backends per lane. Per `DECISIONS.md` §M1,
the next CUDA spike must share one context/program and give each isolated mutable
slot an explicitly non-blocking stream. It must repeat the same exact output,
capacity, timing, and Nsight overlap checks before a numbered CUDA-concurrency
PRD is drafted.

The CPU track remains independently positive from the 100k/1M local evidence,
but still needs its scoped 10M, second-shape, repeated, topology-controlled arms.

## Evidence delivery correction

The VM initially pushed `evidence/hyperstack-20260729T064728Z`. Independent
inspection found that the orphan-worktree cleanup used `rm -rf ./*`, which left
hidden repository files and unintentionally included `.piprd` runner state. A
credential-pattern scan found no secrets. The collector now removes every
root entry except `.git`; this locally verified evidence was republished as
`evidence/hyperstack-20260729T064728Z-clean`, and the malformed remote branch was
deleted after the clean branch was fetched and verified.
