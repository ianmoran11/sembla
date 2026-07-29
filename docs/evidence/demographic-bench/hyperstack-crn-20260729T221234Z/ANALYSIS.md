# CRN correctness arm: free-running non-blocking CUDA streams

## Verdict

**PASS.** The free-running non-blocking-stream design is byte-exact under CRN
noise at every measured arm. Together with the independent-noise timing matrix
in [`hyperstack-freestream-20260729T152534Z`](../hyperstack-freestream-20260729T152534Z/ANALYSIS.md),
this **discharges CUDA Gate 1** for the design: independent noise supplied the
timing case, CRN and independent noise have now both supplied correctness
cases.

Repository commit: `8ace75c83755b7bc767ef07c6e7eef05d3c2036b`.
Hardware: one NVIDIA H100 PCIe. Workers 1/2/4, CRN noise, **one repetition**
per scale — this is a correctness arm, not timing evidence. The schedule
control and Nsight trace were intentionally skipped; both were established by
the independent-noise arm.

## Results

| scale | comparisons | all byte-equal | negative control rejected |
|---:|---:|---:|---:|
| 1M | 3 (workers 1/2/4 vs reference) | **yes** | yes |
| 10M | 3 | **yes** | yes |

Every comparison covers the complete output tree: manifests, summaries,
grouped sidecars, exported pairs, and `final_state_sha256`. The deliberate
perturbation changed one grouped output and was rejected at both scales. The
final local `SHA256SUMS` verifies all 883 evidence files.

Single-repetition walls are context only (not timing evidence): 1M
6.40/3.76/3.50 s and 10M 36.56/23.12/20.77 s for workers 1/2/4 — consistent
with the three-repetition independent-noise medians (1M 5.155/4.070/4.058 s;
10M 37.078/23.207/21.247 s).

## Gate status

The free-running non-blocking-stream design now satisfies every Gate-1
requirement measured to date:

- draw `k` alone equals draw `k` after and alongside other draws (both noise
  modes, both scales);
- parameter sampling and replica seeds remain pure functions of `k` (CRN
  shares the seed across lanes; byte parity proves no lane-order leakage);
- complete file sets and bytes equal the sequential arm, including grouped
  sidecars and `final_state_sha256`;
- perturbed comparisons fail;
- setup, execution, publication, and resource use are separately visible;
- whole-sweep medians improve 1.598x/1.745x at 10M with real two-stream
  kernel overlap proven by Nsight.

Drafting `0004-run-cuda-draws-concurrently` is now justified under the
conditional PRD sequence, subject to its binding contract: bounded admission
with explicit capacity failure, isolated per-lane mutable state, deterministic
`k`-derived seeds, ascending-`k` publication, truthful timing, and
default-off.

## Operational note

The remote payload completed, pushed branch
`evidence/hyperstack-20260729T221606Z`, transferred and verified checksums, and
destroyed both paid resources with zero provider orphans. The collector
returned 0.
