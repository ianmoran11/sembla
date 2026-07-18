# Verified full-rate H100 precision evidence — 2026-07-18

This directory is the durable, tracked copy of the three-run Hyperstack evidence
bundle collected by `infra-hyperstack/collect-runs.sh`. Artifact verification
passed before the paid VM was deleted.

## Identity

- Provider / region: Hyperstack / `CANADA-1`
- Flavor: `n3-H100x1`
- Image: Ubuntu Server 22.04 LTS R570 CUDA 12.8
- GPU: NVIDIA H100 PCIe
- Driver: `570.195.03`
- Runtime FP64 classification: full-rate, FP32:FP64 ratio `2:1`, reported by
  `cudaDevAttrSingleToDoublePrecisionPerfRatio`
- Repository commit: `d6c545f63a89135d01addeea42b9fbe44fac897a`
- Workload: 26,000,000 rows, 1,300,000 groups, tick 7, `beta = 0.35`,
  `dt = 0.25`, 10 warmups, 100 measured ticks
- Collection ID: `b902e6a3318f221a138e8f88df0aa9e4`

The three distinct run IDs are the collection ID suffixed with `run-1`, `run-2`,
and `run-3`. Their external logs bind each run ID and repository commit to the
result SHA-256 below.

## NVIDIA-local measurements

All timings use GPU timestamp queries. Values are read from each result file's
`machines.nvidia.strategies[*].status`, not from the rendered cross-machine
matrix.

| Run | Strategy | Total ms/tick | Rows/sec | Max reduction relative error | Winner mismatch fraction | Fired mismatches | Unexplained mirror differences |
|---|---|---:|---:|---:|---:|---:|---:|
| 1 | `f32` | 0.817456000 | 31,805,993,227.770 | `1.714669e-7` | 0 | 1 | n/a |
| 1 | double-single | 0.829600000 | 31,340,405,014.465 | `1.714669e-7` | 0 | 1 | n/a |
| 1 | native `f64` (CUDA) | 0.724384010 | 35,892,564,781.780 | 0 | 0 | 0 | 0 |
| 2 | `f32` | 0.818064000 | 31,782,354,436.817 | `1.714669e-7` | 0 | 1 | n/a |
| 2 | double-single | 0.832112000 | 31,245,793,835.445 | `1.714669e-7` | 0 | 1 | n/a |
| 2 | native `f64` (CUDA) | 0.724880010 | 35,868,005,249.531 | 0 | 0 | 0 | 0 |
| 3 | `f32` | 0.817936000 | 31,787,328,103.910 | `1.714669e-7` | 0 | 1 | n/a |
| 3 | double-single | 0.829472000 | 31,345,241,310.135 | `1.714669e-7` | 0 | 1 | n/a |
| 3 | native `f64` (CUDA) | 0.722815990 | 35,970,427,250.628 | 0 | 0 | 0 | 0 |

Three-run medians:

| Strategy | Median total ms/tick | Implied rows/sec | Throughput retained vs `f32` |
|---|---:|---:|---:|
| `f32` | 0.817936000 | 31,787,328,103.910 | 100.000% |
| double-single | 0.829600000 | 31,340,405,014.465 | 98.594% |
| native `f64` (CUDA) | 0.724384010 | 35,892,564,781.780 | 112.915% |

CUDA native `f64` used 88.562% of the `f32` time and was therefore 11.438%
lower latency on this workload.

## Guard outcomes and decision

Every run recorded the same outcomes:

- `f32`: passed its baseline guard, but its measured row had one fired-flag
  mismatch and does not implement the unchanged `f64` contract.
- double-single: failed its guard because the one-million-row winner mismatch
  rate (`0.00002`) did not improve on `f32` (`0.00002`). Its full-workload row
  also had one fired mismatch and the NVIDIA/Vulkan strict-arithmetic probe was
  not trustworthy. Strategy B does not qualify.
- native `f64` via wgpu: unavailable on this exact H100 path. NVIDIA's Vulkan
  compiler previously reported `NVVM compilation failed` for the wgpu 0.20
  shader and crashed during device teardown, so pipeline creation is safely
  gated for the observed adapter.
- native `f64` via CUDA: passed every guard, had zero reduction error, zero
  winner and fired mismatches, and zero unexplained fixed-tree mirror
  differences in all three runs. It exceeded the 75% performance floor.

Under ADR 0001's binding rule, **Strategy A qualifies and is selected**, with
**CUDA named as the production native backend**. Strategy B does not qualify;
Strategy C is unnecessary and its measured `f32` row would not satisfy the
required zero-fired-mismatch reduced contract.

This supports Level A reproducibility only for the same pinned binary and GPU
model with fixed-order kernels. Level B cross-hardware bitwise reproducibility
remains unproven.

## Result and log bindings

| Run | Start time (UTC) | Result SHA-256 | Result file | External log |
|---|---|---|---|---|
| 1 | `2026-07-18T04:17:27Z` | `68e0acd5a9aeb4c624693f3e81319f4e7a502331d27c5bc758b5a9d0439b7e69` | `RESULTS.run-1.md` | `run-1.log` |
| 2 | `2026-07-18T04:17:48Z` | `f6b18ebbbe9d5cedc54119e3126b6153a74d08d59f96a305d21ee103157a98fe` | `RESULTS.run-2.md` | `run-2.log` |
| 3 | `2026-07-18T04:18:06Z` | `e941ff74d1fffc377392233d3cc2dfec6d29fd6befd7989b3ffaab26ca94fb47` | `RESULTS.run-3.md` | `run-3.log` |

Other captured files preserve the selected profile, full `nvidia-smi -q`,
bootstrap transcript and diagnostics, exact commit, trusted host key, guest SSH
self-test key, and collection identity.

## Teardown

Hyperstack VM `938155` and its ingress rule were deleted after artifact
verification. Terraform subsequently reported an empty state and empty outputs.
The paid saved plan was deleted. Hyperstack bills both ACTIVE and SHUTOFF VMs,
so deletion—not guest shutdown—was the billing control.
