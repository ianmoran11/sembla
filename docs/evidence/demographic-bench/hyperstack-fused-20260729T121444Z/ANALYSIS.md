# Fused grid-y CUDA sweep analysis

## Verdict

The fused grid-y draw spike is **correct but negative**. It does not justify a
numbered CUDA-concurrency PRD or a production default.

Repository commit: `133f87b8dd31bce9254a1be31fe3a3d3bb772443`.
Hardware: one NVIDIA H100 PCIe. Each scale used 20 independent-noise draws,
24 ticks, capacities 1/2/4, and three repetitions. Every candidate was compared
with a separately executed sequential CUDA reference from the same repetition.

## Correctness and shakedown

- The capacity-four/two-active-draw, two-tick shakedown matched the sequential
  output tree exactly under compute-sanitizer memcheck.
- Compute-sanitizer reported `ERROR SUMMARY: 0 errors`.
- All 24 recorded complete-tree comparisons (12 per scale, including adjacent
  references) are equal. This covers manifests, summaries, grouped outputs,
  exported pairs, and final-state hashes.
- Both deliberate negative controls changed one grouped output and were rejected.
- Schedule-control output matched the adjacent sequential reference, and its ten
  capacity-two chunk records cover draws 0 through 19 in ascending order.
- The final local `SHA256SUMS` verifies all 2,994 evidence files.

## Whole-sweep results

Times below are medians of three external wall-clock measurements.

| population | arm | median wall | vs sequential | peak GPU memory | peak RSS |
|---:|---|---:|---:|---:|---:|
| 1M | sequential CUDA | 5.161 s | 1.000x | 1,033 MiB | 430,896 KiB |
| 1M | fused capacity 1 | 7.754 s | 0.666x | 1,033 MiB | 620,336 KiB |
| 1M | fused capacity 2 | 7.466 s | 0.691x | 1,609 MiB | 807,892 KiB |
| 1M | fused capacity 4 | 13.439 s | 0.384x | 2,729 MiB | 1,182,944 KiB |
| 10M | sequential CUDA | 37.268 s | 1.000x | 6,025 MiB | 2,552,056 KiB |
| 10M | fused capacity 1 | 56.501 s | 0.660x | 6,025 MiB | 4,524,732 KiB |
| 10M | fused capacity 2 | 56.173 s | 0.663x | 11,593 MiB | 6,399,912 KiB |
| 10M | fused capacity 4 | 64.576 s | 0.577x | 22,697 MiB | 10,150,528 KiB |

Capacity two is 44.7% slower than sequential at 1M and 50.7% slower at 10M.
Capacity four is 160.4% and 73.3% slower, respectively. Even capacity one is
about 50% slower, showing that the fused generated path itself carries a large
cost before extra slots can help. Capacity two reduces the fused execution
window only slightly relative to fused capacity one; capacity four regresses.

Using the same external-wall aggregation as the earlier evidence, fused
capacity two is 84.6%/127.8% slower than isolated two-backend execution at
1M/10M, and 71.1%/97.5% slower than two lockstep non-blocking streams. Fused
capacity four is worse still. Memory reaches essentially the same draw-major
VRAM scale as isolated backends while RSS is higher at 10M/four slots.

## Nsight Systems

The four-draw capacity-two trace contains:

- 19,776 kernel intervals, exactly half the launch count of the corresponding
  four-draw complete-backend traces;
- `gridDim.y = 2` for every kernel;
- one CUDA context and stream (`context 1`, `stream 7`);
- 309.524 ms summed and union kernel time, with maximum simultaneous kernel
  count one, as expected for one fused phase launch at a time.

This proves that grid-y batching was exercised rather than silently falling
back. It reduces launch and transfer call counts, but not enough work: 390 D2H
API calls consume 506.850 ms versus 780 calls consuming 372.180 ms in the
independent-backend trace. The fused trace's two shared chunks correctly report
capacity two and first draws 0 and 2.

## Decision

Close this fused grid-y implementation as a production candidate. Together with
prior evidence, all three measured CUDA draw-concurrency mechanisms are
negative:

1. complete default-stream backends improve host pipelining but serialize GPU
   kernels and consume draw-major memory;
2. lockstep non-blocking streams prove some kernel overlap but are slower than
   independently scheduled complete backends; and
3. fused grid-y batching halves phase launch count but is substantially slower
   than sequential, isolated, and lockstep execution at both scales.

CUDA concurrent draws therefore remain default-off and do not advance to the
conditional `0004-run-cuda-draws-concurrently` PRD. CPU Gate 1 remains a
separate open track.

## Operational note

The remote payload completed, pushed branch
`evidence/hyperstack-20260729T122838Z`, transferred and verified checksums, and
destroyed both paid resources. The local collector nevertheless returned status
1 because its final optional `bench-results.md` display probe became the shell's
exit status in concurrency-only mode. That post-success reporting bug does not
affect measurements or teardown and is fixed in the following source commit by
ending the collector explicitly with status zero.
