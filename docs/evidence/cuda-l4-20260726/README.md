# §L4 gate measurement and post-fix profile — 2026-07-26

Host: Hyperstack `n3-H100x1-spot` (H100 PCIe 80GB, AMD EPYC 9554), CUDA 12.8,
commit `206900c` (contains PRD 0006 segmented argmin and PRD 0007). One shared
10M state artifact, one release binary, three replicates per backend.

## §L4 verdict: NOT MET

| | median | replicates | spread |
|---|---:|---|---|
| CUDA | **171.2 s** | 171.2 / 169.4 / 178.4 | 5.3% |
| CPU | **438.7 s** | 433.5 / 438.7 / 445.6 | 2.8% |

**Ratio 2.56x.** The gate requires 3x. A well-measured miss: no replicate
approached the bar.

## PRD 0006 nonetheless succeeded

Pre-fix CUDA was `5647.9s` (`hyperstack-l4-attempt-20260726/`). This is
**33.0x faster**, and the output hashes are **byte-identical** to the pre-fix
run — `results_sha256=9d770ee3...`, `final_state_sha256=e2011191...`. The
segmented argmin selects exactly the same winners as the quadratic scan it
replaced, which is the property §E3 required.

`resolve_conflicts` is no longer quadratic: 47,152 ns at 500k, 185,377 ns at 2M
(3.93x for 4x rows), 460,628 ns at 5M (2.48x for 2.5x rows) — linear.

## The bottleneck is no longer the GPU

At 5M rows over 2 ticks:

| | time |
|---|---:|
| wall clock | 10,620 ms |
| **all GPU kernels combined** | **9.1 ms** |
| host-side CUDA API | 402 ms |
| unaccounted (host CPU) | ~10,200 ms |

**GPU kernels are 0.09% of wall time.** Of the CUDA API time,
`cuMemcpyDtoHAsync` is 93.8% (377 ms over 12 calls) — device-to-host readback,
each forcing a synchronisation. The remaining ~96% of wall time is host-side
work outside CUDA entirely.

The post-fix kernel ranking is flat — no kernel above 11.5%, top nine summing to
63% — which is what "no remaining pathology" looks like.

**Consequences.** Further CUDA kernel optimisation is not worth doing: making
every kernel instant would save under 0.1%. PRD 0007's parallelisation, though
correct, buys nothing measurable. The 2.56x ceiling exists because both backends
are dominated by the same host-side path. The next profiling target is the host,
and that is free to investigate locally with a CPU profiler.

## Ageing share — replicated, for §K2

`(full - no-ageing) / full` over three replicates: **12.2%**, 11.6%, 13.3%,
median **12.2%**. All three exceed the §K2 10% threshold, as did earlier single
readings of 11.6% (M2 Pro) and 12.2% (EPYC). This is the first properly
replicated measurement. **Recorded, not decided** — §K2 has its own trigger
process.
