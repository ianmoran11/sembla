# CUDA kernel profile — 2026-07-26

Kernel-level profile of `demographic_slots` (no-grouped) on CUDA, taken to scope
PRD 0006 after PRD 0002 fixed the wrong kernels. Host: Hyperstack `n3-H100x1`
(H100 PCIe 80GB, AMD EPYC 9554), CUDA 12.8, commit `dbc665f`.

Method: `nsys profile --trace=cuda` over 2 ticks at each scale, then
`nsys stats --report cuda_gpu_kern_sum`. Absolute times are inflated by
profiling overhead; the **relative** distribution and the **scaling exponent**
are the findings.

## Distribution shifts with scale — the 500k ranking is misleading

| Kernel | 500k | 2M | 5M |
|---|---:|---:|---:|
| `sembla_resolve_conflicts` | 28.5% | 56.7% | **77.9%** |
| `sembla_check_candidate_errors` | 37.7% | 22.7% | 11.7% |
| `sembla_prepare_effects` | 33.7% | 20.6% | 10.3% |

Everything else is ~0.0%, including all four kernels PRD 0002 parallelised.

## `resolve_conflicts` is quadratic

| Rows | ns/instance | vs previous |
|---:|---:|---|
| 500,000 | 483,282,045 | — |
| 2,000,000 | 6,432,974,613 | 13.3x for 4x rows |
| 5,000,000 | 43,979,638,562 | 6.8x for 2.5x rows |

Fitted exponent over the full range: **1.96** — quadratic. Extrapolating to 10M
gives ~176 s/tick from this kernel alone, against 235.3 s/tick measured for the
whole run at 10M (`hyperstack-l4-attempt-20260726/`). It is the CUDA cost.

The other two scale **linearly** (4.0x and 4.1x for 4x rows). They are serial and
worth fixing, but they are not the scaling problem.

## Cause

`crates/sembla-cuda/src/codegen.rs`, `sembla_resolve_conflicts` contains:

```c
for (unsigned long long other_row = 0; other_row < row_counts[...]; ++other_row)
```

an inner scan over every row of the resource table, inside a kernel already
running one thread per row: n threads x n iterations.

Conflict resolution selects the argmin among candidates contesting the same
resource. `DESIGN.md` §4.2 names **segmented argmin** as a member of the closed
kernel fragment — the intended primitive. The CUDA backend implements it as a
naive nested scan instead. The CPU oracle completes 10M in 434.6s, so it does not
pay this cost.

## Consequence

PRD 0006 is scoped from this profile: the quadratic is the target, and the two
linear serial kernels are secondary. Profiling at 500k alone would have ranked
them in the opposite order — which is why the folder README now requires
profiling at the scale being fixed.
