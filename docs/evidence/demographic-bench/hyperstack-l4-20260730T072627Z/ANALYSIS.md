# Analysis: remaining CUDA readback and kernel contention

## Verdict

The leading packed-tiny-readback hypothesis is **closed for this H100/10M
shape**. Tiny D2H copies are numerous but not material: 776 copies per four-draw
arm transfer only 0.096 MB and account for 1.261 ms of sequential GPU-copy union
or 2.014 ms with four workers. The analyzer projects only 1.098 ms of
unoverlapped small-copy time across a 20-draw worker-four sweep.

The dominant remaining transfer is instead **final-state materialization**: one
480 MB pageable D2H copy per draw. Four copies transfer 1.920 GB and consume
942.260 ms sequentially or 978.416 ms with four workers. In the worker-four arm,
906.710 ms remains outside kernel intervals. Scaling total measured D2H
(906.919 ms: large plus small) from the four-draw arm to 20 draws yields 4.535 s
of projected unoverlapped D2H time; scaling the large component alone yields
4.534 s.

The next optimization should therefore trace the exact consumers of the final
state and preserve the output contract while reducing or hiding that one large
copy per draw. The preferred order is:

1. determine whether `final_state_sha256` and published artifacts require host
   materialization of every final-state byte;
2. if only the digest is required, evaluate an exact device-side final-state
   digest plus compact D2H result;
3. otherwise evaluate pinned asynchronous final-state transfer and overlap,
   with complete output-tree parity and unchanged publication ordering;
4. remeasure worker 1/2/4 only after correctness passes.

Do **not** prototype packing or double-buffering of the tiny per-tick control
copies based on this evidence. Their measured ceiling is too small.

## Evidence integrity and scope

- Repository: `e9600f31251c495268337ed6c0eb16d7fb2b838c`.
- Binary SHA-256:
  `f7a0aeb6e953b75c2d41b92ac16d2c3c17fe5a401b343fcf153b8def996af3d3`.
- Workload: 10M slots, 24 ticks, seed 9009, grouped observations, four draws.
- GPU: NVIDIA H100 PCIe, CUDA 12.8 image.
- Assertions: 7/7 PASS in `assertions.txt`.
- Remote checksums: 80/80 verified.
- Final local checksums before authored additions: 86/86 verified.
- Raw profiler reports retained: two `.nsys-rep` and five `.ncu-rep` files.
- Evidence branch: `evidence/hyperstack-20260730T073129Z`.
- Nsight Compute package: `nsight-compute-2025.2.1=2025.2.1.3-1`.
- Counter window: `RmProfilingAdminOnly: 0` while profiling and `1` after
  cleanup; the profiler binary has no residual file capability.

Nsight Systems is the timing oracle. Nsight Compute replay is used only for
occupancy, bandwidth, scheduler, and stall diagnosis. The four-draw profiler
arms are not replacements for the supported 20-draw throughput medians.

## Per-tick phase attribution

The profiler-independent single-run phase table covers all 24 ticks:

| phase | total ms | share of 481.531 ms |
|---|---:|---:|
| kernels | 481.038 | 99.898% |
| `readback_control` | 0.394 | 0.082% |
| report | 0.084 | 0.017% |
| other | 0.014 | 0.003% |
| state transfer/reconstruct/hash and view observation | 0.001 | <0.001% |

This independently confirms that compact per-tick control readback is no longer
a meaningful wall-time target. Final-state materialization occurs outside these
per-tick phase totals and is visible in Systems.

## D2H transfer decomposition

Both Systems arms perform exactly 780 D2H calls and transfer 1,920.096 MB:

| arm | class | calls | MB | copy union ms | unoverlapped ms |
|---|---|---:|---:|---:|---:|
| worker 1 | large (>=1 MB) | 4 | 1,920.000 | 942.260 | 942.260 |
| worker 1 | small (<1 MB) | 776 | 0.096 | 1.261 | 1.261 |
| worker 4 | large (>=1 MB) | 4 | 1,920.000 | 978.416 | 906.710 |
| worker 4 | small (<1 MB) | 776 | 0.096 | 2.014 | 0.220 |

Every destination is pageable host memory. Worker four overlaps 73.473 ms of
D2H with kernels, almost entirely from the large transfers. The accumulated
host-thread time inside `cuMemcpyDtoHAsync_v2` is 2,899.297 ms sequentially and
5,826.024 ms with four workers; because this is summed across threads it is not
wall time, but it confirms that the nominally asynchronous pageable calls block
host execution materially.

The profiled four-draw whole-sweep times are 11,704.844 ms at worker one and
9,846.757 ms at worker four; the worker-four execution window is 3,822.120 ms.
These setup-heavy profiler arms are diagnostic only.

## Kernel overlap and contention

Equal-work arms contain exactly 39,552 kernel launches. Worker one uses one
kernel at a time. Worker four reaches maximum kernel concurrency four:

| measure | worker 1 | worker 4 |
|---|---:|---:|
| summed kernel duration | 1,955.618 ms | 2,096.457 ms |
| kernel union duration | 1,955.618 ms | 1,956.209 ms |
| concurrent overlap (`sum - union`) | 0 | 140.248 ms |

The union is essentially unchanged while summed durations inflate by 140.839
ms. Overlap therefore hides most of the contention penalty in this arm. The
largest per-draw duration increases are:

| kernel | added ms/draw | ratio |
|---|---:|---:|
| `sembla_prepare_effects` | 12.106 | 1.156x |
| `sembla_check_candidate_errors` | 8.115 | 1.185x |
| `sembla_advance_validation_phase` | 6.035 | 2.255x |
| `sembla_observe_view` | 3.564 | 1.178x |
| `sembla_validate_claims` | 2.831 | 1.099x |

The high ratio for `sembla_advance_validation_phase` is not the largest absolute
cost. It is a very small launch: Compute records 1.56% achieved occupancy and
one active warp per SM. Its ratio alone does not justify prioritizing it.

## Nsight Compute interpretation

### `sembla_prepare_effects`

- 81.75% achieved occupancy; 52.32 active warps/SM.
- 53.38% memory/DRAM throughput and 31.19% SM throughput.
- Detailed replay: 1.09 TB/s, 53.59% L2 hit rate.
- Scheduler: 68.18% no-eligible cycles, 0.66 eligible warps/scheduler.

This kernel is neither occupancy-starved nor fully bandwidth-saturated. The
scheduler evidence indicates latency/dependency exposure despite high resident
occupancy. Its 15.6% concurrent inflation is real, but only 12.1 ms/draw.

### `sembla_check_candidate_errors`

- 78.92% achieved occupancy; 50.51 active warps/SM.
- 54.06% SM throughput, 19.05% memory throughput, 13.90% DRAM throughput.
- 18.5% concurrent duration inflation, or 8.1 ms/draw.

This is more compute-weighted than `prepare_effects`, but its absolute penalty
remains secondary to final-state D2H.

### `sembla_count_deferred`

Detailed replay records 769.67 GB/s, 11.37% L2 hit rate, 38.71% no-eligible
cycles, and 1.44 eligible warps/scheduler. This compact reduction is not the
remaining multi-second bottleneck.

## Roadmap decision

1. **Close packed tiny control readback** for this measured shape.
2. **Prioritize final-state materialization**: avoid it when the publication
   contract permits, otherwise pin and overlap it.
3. Treat `prepare_effects` and candidate-error contention as secondary targets
   after the large-copy experiment; together the top three measured penalties
   are about 26.3 ms/draw, versus roughly 226.7 ms/draw of projected
   unoverlapped D2H at worker four.
4. Preserve default worker one and explicit bounded admission.
5. Require complete output-tree parity, exact final-state hashes, raw profiler
   evidence, and worker 1/2/4 timing before accepting a readback change.
6. Do not generalize beyond this demographic/H100 shape without a materially
   different workload or device measurement.
