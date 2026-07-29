# Free-running non-blocking CUDA stream sweep analysis

## Verdict

The free-running non-blocking-stream spike is **correct and positive on the
independent-noise timing case** — the first CUDA draw-concurrency design to
beat both prior stream designs. It combines the independent scheduler's host
pipelining with real kernel overlap, without the tick-barrier gaps that made
lockstep execution slower.

Repository commit: `b6e42cef6e1e00c740d538ae0163f8d50922b0ca`.
Hardware: one NVIDIA H100 PCIe. Each scale used 20 independent-noise draws,
24 ticks, workers 1/2/4, and three repetitions. Every candidate was compared
with the workers-1 reference from the same repetition.

This fills the last untested scheduling/stream quadrant: the prior arms
covered default-stream independent scheduling and lockstep non-blocking
streams, but never independent scheduling *on* non-blocking streams.

## Correctness

- All 18 candidate/reference complete-tree comparisons (9 per scale) are
  byte-equal, covering manifests, summaries, grouped outputs, exported pairs,
  and final-state hashes.
- Both deliberate negative controls changed one grouped output and were
  rejected.
- The schedule control ran two lanes with a forced draw-zero delay:
  `execution_mode = "cuda-free-nonblocking-streams"` was asserted, completion
  order inverted (draw 1 finished before draw 0), and the output tree still
  matched the reference byte-for-byte with ascending-`k` publication.
- The final local `SHA256SUMS` verifies all 2,319 evidence files.

## Whole-sweep results

Times are medians of three external wall-clock measurements.

| population | arm | median wall | vs workers-1 | peak GPU memory | peak RSS |
|---:|---|---:|---:|---:|---:|
| 1M | workers 1 (default stream) | 5.155 s | 1.000x | 1,033 MiB | 452,276 KiB |
| 1M | free streams 2 | 4.070 s | 1.266x | 1,613 MiB | 689,344 KiB |
| 1M | free streams 4 | 4.058 s | 1.270x | 2,733 MiB | 1,112,024 KiB |
| 10M | workers 1 (default stream) | 37.078 s | 1.000x | 6,025 MiB | 2,551,996 KiB |
| 10M | free streams 2 | 23.207 s | 1.598x | 11,597 MiB | 5,229,676 KiB |
| 10M | free streams 4 | 21.247 s | 1.745x | 22,701 MiB | 9,161,224 KiB |

### Against the prior measured designs (same aggregation)

| scale | arm | isolated default-stream | lockstep streams | **free streams** |
|---:|---|---:|---:|---:|
| 1M | 2 workers | 4.044 s | 4.362 s | **4.070 s** |
| 1M | 4 workers | 4.051 s | 4.347 s | **4.058 s** |
| 10M | 2 workers | 24.663 s | 28.448 s | **23.207 s** |
| 10M | 4 workers | 22.196 s | 26.380 s | **21.247 s** |

Free streams is the fastest measured CUDA design at 10M — 6.3%/4.5% faster
than isolated default-stream backends at two/four workers and 22.6%/24.2%
faster than lockstep — and is effectively tied with isolated default-stream
backends at 1M (within 0.6%), as expected: at that scale the win was always
host-side pipelining, which both designs share. The quoted prior-arm speedups
come from those analyses' internal paired-median aggregation while this arm's
figures are external ratio-of-medians; on a common basis the 10M figures are
isolated 1.485x/1.656x, lockstep 1.294x/1.396x, and free streams
1.598x/1.745x, so no conclusion changes.

## Nsight Systems

The four-draw two-lane trace contains:

- 39,552 kernel intervals split evenly (19,776 each) across two **non-default
  streams** (context 1, streams 13 and 14) — unlike the isolated default-stream
  trace where every kernel serialized on legacy stream 7;
- 362.242 ms summed kernel time, 332.773 ms union, **29.470 ms at concurrency
  ≥2 (8.86% of union)**, maximum simultaneous kernels 2;
- no tick-barrier structure: overlap arises naturally from independent lane
  progress rather than synchronized phase alignment.

Comparison: isolated default-stream backends had 0 ms overlap; lockstep had
35.939 ms (11.25% of union) but paid +48.1 ms of summed kernel time and ~250 ms
of additional inter-kernel gaps relative to the isolated trace. Free streams'
summed kernel time (362.242 ms) is slightly *higher* than lockstep's
(355.407 ms), and its total inter-kernel gap (~377 ms) sits between lockstep's
(~417 ms) and isolated's (~169 ms) — so the win is not reduced GPU work. The
wall-time advantage comes from the scheduler dynamically filling those gaps
with useful work from the other lane instead of forcing convoy alignment at
tick boundaries.

## Decision

Free-running non-blocking streams is the **conditionally positive** CUDA
Gate-1 design: the independent-noise timing case passes with exact output
parity, real kernel overlap, and the best measured whole-sweep times. Before a
numbered PRD (`0004-run-cuda-draws-concurrently`) may be drafted, the gate
still requires:

1. **CRN hardware correctness coverage** — this matrix, like the lockstep
   arm, used independent noise only. The gate states independent noise is the
   timing case while CRN and independent noise are both correctness cases.
2. Memory-capacity acknowledgement: VRAM remains draw-major (22,701 MiB at
   10M/four lanes), so a production design needs explicit capacity admission
   and clear failure, not an `auto` worker count.

The complete-backend/default-stream, complete-backend/lockstep-stream, and
fused grid-y mechanisms remain negative and closed; this arm supersedes them
as the CUDA concurrency candidate. CPU Gate 1 remains separate and open.

## Operational note

The remote payload completed, pushed branch
`evidence/hyperstack-20260729T153207Z`, transferred and verified checksums, and
destroyed both paid resources with zero provider orphans. The collector's
post-success exit-status bug (fixed in `1e1083e`) did not recur: the driver
returned 0.
