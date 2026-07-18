# Precision strategy benchmark results

> This file is generated atomically by `cargo run --release`. Do not edit the
> embedded state by hand. Unavailable cells remain explicitly unanswered.

## Two-machine assembly

1. Run `cargo run --release` on the development Mac and keep this generated file.
2. Ensure that exact `RESULTS.md` is present in the NVIDIA checkout (commit it first, or copy it there) before running `cargo run --release --features cuda`.
3. Copy the NVIDIA-generated file back. The embedded state preserves both machine blocks and the merged matrix chooses portable rows from development and native-f64 rows from NVIDIA.

Rows can come from different workloads when adapter sizing differs. Treat each row with its source machine and that machine's workload metadata; do not compare unlike `(N, G)` values as if they were one run.

## Merged strategy × metric matrix

Accuracy cells compare the final benchmark tick against the scalar CPU `f64` oracle computed once for that machine's workload.

| Strategy | Source machine | Timing method | ms/tick total | ms/tick segmented reduce | ms/tick segmented argmin | rows/sec | Reduction rel-err (max / mean) | Winner mismatch % | Fired mismatches | Unexplained arithmetic-mirror differences | Order-sensitive groups |
|---|---|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| f32 | Development | synchronized wall-clock fallback | 10.711042 | 3.273896 | 4.984000 | 2427401667.709 | 1.714669e-7 / 3.216831e-8 | 0.000000 | unanswered | n/a | 276097 |
| double-single | Development | synchronized wall-clock fallback | 13.868958 | 3.201605 | 4.334770 | 1874690225.466 | 1.096998e-14 / 1.206441e-15 | 0.000000 | unanswered | n/a | 276097 |
| native f64 (wgpu) | NVIDIA | unanswered on this adapter: native_f64: unsupported; adapter=NVIDIA H100 PCIe; backend=Vulkan; reason=wgpu 0.20 native f64 is disabled on NVIDIA H100 PCIe after an observed NVVM compiler failure; use the CUDA native-f64 path; device=NVIDIA H100 PCIe; fp64:fp32=1:2; class=full-rate; evidence=documented NVIDIA datacenter-compute model lookup; full-rate-extrapolation=allowed | unanswered | unanswered | unanswered | unanswered | unanswered | unanswered | unanswered | unanswered | unanswered |
| native f64 (CUDA) | NVIDIA | CUDA event timestamps | 0.724880 | 0.139216 | 0.146432 | 35868005249.531 | 0.000000e0 / 0.000000e0 | 0.000000 | 0 | 0 | 276097 |

## Reduction determinism choice

| Strategy | Source machine | Choice |
|---|---|---|
| f32 | Development | deterministic fixed-order two-pass reduction, no atomics (Level A) |
| double-single | Development | deterministic fixed-order two-pass reduction, no atomics (Level A) |
| native f64 (wgpu) | NVIDIA | deterministic fixed-order two-pass reduction, no atomics (Level A) |
| native f64 (CUDA) | NVIDIA | deterministic fixed-order two-pass reduction, no atomics (Level A) |

## Per-strategy verdicts

### f32

**Source:** Development. Portable baseline. It is the cheapest arithmetic path; its reported reduction and winner errors are the reference that double-single must improve.

### double-single

**Source:** Development. Portable double-single passed the strict-math behavior probe and the PRD-0002 reduction/winner thresholds on this adapter.

### native f64 (wgpu)

**Source:** NVIDIA. unanswered on this adapter: native_f64: unsupported; adapter=NVIDIA H100 PCIe; backend=Vulkan; reason=wgpu 0.20 native f64 is disabled on NVIDIA H100 PCIe after an observed NVVM compiler failure; use the CUDA native-f64 path; device=NVIDIA H100 PCIe; fp64:fp32=1:2; class=full-rate; evidence=documented NVIDIA datacenter-compute model lookup; full-rate-extrapolation=allowed. No throughput or accuracy number was fabricated.

### native f64 (CUDA)

**Source:** NVIDIA. Native binary64 matched every oracle argmin winner and fired flag, with zero unexplained fixed-tree arithmetic-mirror differences. Device `NVIDIA H100 PCIe` is classified `full-rate` from CUDA cudaDevAttrSingleToDoublePrecisionPerfRatio; This is full-rate fp64 hardware, so full-rate extrapolation is allowed.


## Development machine

- generated: `unix-seconds:1784153520`
- adapter: `Apple M2 Pro`
- backend/device type: `Metal` / `IntegratedGpu`
- driver: `` (``)
- `SHADER_F64`: `false`
- exact GPU/fp64 model: `Apple M2 Pro`
- fp64 class and ratio: `rate-limited` / `unknown`
- fp64 evidence: conservative fallback for an unrecognized model
- full-rate extrapolation: `refused`
- portable strict math trustworthy: `true` (requested=true, backend-supported=true, FMA-contraction-observed=false, reassociation-observed=false, residuals-preserved=true)

### Workload

- requested `(N, G)`: `(26000000, 1300000)`
- actual `(N, G)`: `(26000000, 1300000)`
- downscale reason: none; the full requested workload fits the sizing limits
- contested-key selector: `entity_id % 10 == 5, keyed by employer`
- benchmark tick: `7`
- warmup/measured ticks: `10` / `100`
- `beta` / `dt`: `0.35` / `0.25`

### Local rows

- **f32:** answered
- **double-single:** answered
- **native f64 (wgpu):** unanswered on this adapter: native_f64: unsupported; adapter=Apple M2 Pro; backend=Metal; reason=native WGSL f64 is supported by wgpu only on Vulkan; device=Apple M2 Pro; fp64:fp32=unknown; class=rate-limited; evidence=conservative fallback for an unrecognized model; full-rate-extrapolation=refused
- **native f64 (CUDA):** unanswered on this adapter: cuda: feature-disabled

## NVIDIA machine

- generated: `unix-seconds:1784348272`
- adapter: `NVIDIA H100 PCIe`
- backend/device type: `Vulkan` / `DiscreteGpu`
- driver: `NVIDIA` (`570.195.03`)
- `SHADER_F64`: `true`
- exact GPU/fp64 model: `NVIDIA H100 PCIe`
- fp64 class and ratio: `full-rate` / `1:2`
- fp64 evidence: CUDA cudaDevAttrSingleToDoublePrecisionPerfRatio
- full-rate extrapolation: `allowed`
- portable strict math trustworthy: `false` (requested=false, backend-supported=false, FMA-contraction-observed=false, reassociation-observed=false, residuals-preserved=false)

### Workload

- requested `(N, G)`: `(26000000, 1300000)`
- actual `(N, G)`: `(26000000, 1300000)`
- downscale reason: none; the full requested workload fits the sizing limits
- contested-key selector: `entity_id % 10 == 5, keyed by employer`
- benchmark tick: `7`
- warmup/measured ticks: `10` / `100`
- `beta` / `dt`: `0.35` / `0.25`

### Accuracy regression guards

- **f32:** passed
- **double-single:** failed: double-single winner mismatch rate 0.00002 does not satisfy the f32 baseline 0.00002
- **native f64 (wgpu):** unavailable: native_f64: unsupported; adapter=NVIDIA H100 PCIe; backend=Vulkan; reason=wgpu 0.20 native f64 is disabled on NVIDIA H100 PCIe after an observed NVVM compiler failure; use the CUDA native-f64 path; device=NVIDIA H100 PCIe; fp64:fp32=1:2; class=full-rate; evidence=documented NVIDIA datacenter-compute model lookup; full-rate-extrapolation=allowed
- **native f64 (CUDA):** passed

### Infrastructure metadata

- `expected-gpu`: `H100`
- `full-rate-extrapolation`: `refused-until-runtime-verification`
- `generated-at-utc`: `2026-07-18T04:17:48Z`
- `hyperstack-environment`: `default-CANADA-1`
- `hyperstack-flavor`: `n3-H100x1`
- `hyperstack-image`: `Ubuntu Server 22.04 LTS R570 CUDA 12.8`
- `hyperstack-region`: `CANADA-1`
- `nvidia-device`: `NVIDIA H100 PCIe, 570.195.03, 00000000:00:07.0`
- `provider`: `hyperstack`
- `repository-commit`: `d6c545f63a89135d01addeea42b9fbe44fac897a`
- `requested-fp64-class`: `full-rate`
- `run-id`: `b902e6a3318f221a138e8f88df0aa9e4-run-2`

### Local rows

- **f32:** answered
- **double-single:** answered
- **native f64 (wgpu):** unanswered on this adapter: native_f64: unsupported; adapter=NVIDIA H100 PCIe; backend=Vulkan; reason=wgpu 0.20 native f64 is disabled on NVIDIA H100 PCIe after an observed NVVM compiler failure; use the CUDA native-f64 path; device=NVIDIA H100 PCIe; fp64:fp32=1:2; class=full-rate; evidence=documented NVIDIA datacenter-compute model lookup; full-rate-extrapolation=allowed
- **native f64 (CUDA):** answered
## Embedded merge state

<!-- sembla-precision-state-v1:begin -->
```json
{
  "version": 1,
  "machines": {
    "development": {
      "machine_key": "development",
      "generated_at": "unix-seconds:1784153520",
      "hardware": {
        "adapter_name": "Apple M2 Pro",
        "backend": "Metal",
        "device_type": "IntegratedGpu",
        "driver": "",
        "driver_info": "",
        "shader_f64": false,
        "fp64": {
          "gpu_model": "Apple M2 Pro",
          "class": "rate-limited",
          "fp32_to_fp64_ratio": null,
          "evidence": "conservative fallback for an unrecognized model",
          "full_rate_extrapolation": false
        },
        "strict_math": {
          "backend_supported": true,
          "requested": true,
          "fma_contraction_observed": false,
          "reassociation_observed": false,
          "residuals_preserved": true,
          "trustworthy": true
        }
      },
      "workload": {
        "requested_rows": 26000000,
        "requested_groups": 1300000,
        "actual_rows": 26000000,
        "actual_groups": 1300000,
        "downscale_reason": "none; the full requested workload fits the sizing limits",
        "contested_key_selector": "entity_id % 10 == 5, keyed by employer",
        "benchmark_tick": 7,
        "warmup_ticks": 10,
        "measured_ticks": 100,
        "beta": 0.35,
        "dt": 0.25
      },
      "strategies": [
        {
          "strategy": "f32",
          "reduction_choice": "deterministic fixed-order two-pass reduction, no atomics (Level A)",
          "verdict": "Portable baseline. It is the cheapest arithmetic path; its reported reduction and winner errors are the reference that double-single must improve.",
          "status": {
            "status": "answered",
            "timing": {
              "total_ms": 10.7110415,
              "reduce_ms": 3.273896,
              "argmin_ms": 4.9839995,
              "rows_per_second": 2427401667.708971,
              "warmup_ticks": 10,
              "measured_ticks": 100,
              "method": "synchronized-wall-clock-fallback"
            },
            "accuracy": {
              "reduction_max_relative_error": 1.7146693137426378e-7,
              "reduction_mean_relative_error": 3.2168312051139436e-8,
              "winner_mismatch_fraction": 0.0,
              "order_sensitive_groups": 276097
            }
          }
        },
        {
          "strategy": "double-single",
          "reduction_choice": "deterministic fixed-order two-pass reduction, no atomics (Level A)",
          "verdict": "Portable double-single passed the strict-math behavior probe and the PRD-0002 reduction/winner thresholds on this adapter.",
          "status": {
            "status": "answered",
            "timing": {
              "total_ms": 13.868958,
              "reduce_ms": 3.2016045,
              "argmin_ms": 4.334770499999999,
              "rows_per_second": 1874690225.4661095,
              "warmup_ticks": 10,
              "measured_ticks": 100,
              "method": "synchronized-wall-clock-fallback"
            },
            "accuracy": {
              "reduction_max_relative_error": 1.096997868101701e-14,
              "reduction_mean_relative_error": 1.2064412155246437e-15,
              "winner_mismatch_fraction": 0.0,
              "order_sensitive_groups": 276097
            }
          }
        },
        {
          "strategy": "native f64 (wgpu)",
          "reduction_choice": "deterministic fixed-order two-pass reduction, no atomics (Level A)",
          "verdict": "unanswered on this adapter: native_f64: unsupported; adapter=Apple M2 Pro; backend=Metal; reason=native WGSL f64 is supported by wgpu only on Vulkan; device=Apple M2 Pro; fp64:fp32=unknown; class=rate-limited; evidence=conservative fallback for an unrecognized model; full-rate-extrapolation=refused. No throughput or accuracy number was fabricated.",
          "status": {
            "status": "unanswered",
            "reason": "unanswered on this adapter: native_f64: unsupported; adapter=Apple M2 Pro; backend=Metal; reason=native WGSL f64 is supported by wgpu only on Vulkan; device=Apple M2 Pro; fp64:fp32=unknown; class=rate-limited; evidence=conservative fallback for an unrecognized model; full-rate-extrapolation=refused"
          }
        },
        {
          "strategy": "native f64 (CUDA)",
          "reduction_choice": "deterministic fixed-order two-pass reduction, no atomics (Level A)",
          "verdict": "unanswered on this adapter: cuda: feature-disabled. No throughput or accuracy number was fabricated.",
          "status": {
            "status": "unanswered",
            "reason": "unanswered on this adapter: cuda: feature-disabled"
          }
        }
      ],
      "infrastructure": {}
    },
    "nvidia": {
      "machine_key": "nvidia",
      "generated_at": "unix-seconds:1784348272",
      "hardware": {
        "adapter_name": "NVIDIA H100 PCIe",
        "backend": "Vulkan",
        "device_type": "DiscreteGpu",
        "driver": "NVIDIA",
        "driver_info": "570.195.03",
        "shader_f64": true,
        "fp64": {
          "gpu_model": "NVIDIA H100 PCIe",
          "class": "full-rate",
          "fp32_to_fp64_ratio": 2,
          "evidence": "CUDA cudaDevAttrSingleToDoublePrecisionPerfRatio",
          "full_rate_extrapolation": true
        },
        "strict_math": {
          "backend_supported": false,
          "requested": false,
          "fma_contraction_observed": false,
          "reassociation_observed": false,
          "residuals_preserved": false,
          "trustworthy": false
        }
      },
      "workload": {
        "requested_rows": 26000000,
        "requested_groups": 1300000,
        "actual_rows": 26000000,
        "actual_groups": 1300000,
        "downscale_reason": "none; the full requested workload fits the sizing limits",
        "contested_key_selector": "entity_id % 10 == 5, keyed by employer",
        "benchmark_tick": 7,
        "warmup_ticks": 10,
        "measured_ticks": 100,
        "beta": 0.35,
        "dt": 0.25
      },
      "strategies": [
        {
          "strategy": "f32",
          "reduction_choice": "deterministic fixed-order two-pass reduction, no atomics (Level A)",
          "verdict": "Portable baseline. It is the cheapest arithmetic path; its reported reduction, winner, and fired errors are the reference for candidate qualification.",
          "status": {
            "status": "answered",
            "timing": {
              "total_ms": 0.8180639999999999,
              "reduce_ms": 0.135152,
              "argmin_ms": 0.227248,
              "rows_per_second": 31782354436.816685,
              "warmup_ticks": 10,
              "measured_ticks": 100,
              "method": "gpu-timestamp-queries"
            },
            "accuracy": {
              "reduction_max_relative_error": 1.7146693137426378e-7,
              "reduction_mean_relative_error": 3.2168312051139436e-8,
              "winner_mismatch_fraction": 0.0,
              "order_sensitive_groups": 276097,
              "fired_mismatch_count": 1
            }
          }
        },
        {
          "strategy": "double-single",
          "reduction_choice": "deterministic fixed-order two-pass reduction, no atomics (Level A)",
          "verdict": "Portable double-single produced usable measurements but is unqualified: double-single fired-flag mismatches must be zero; f32 baseline=1, double-single=1.",
          "status": {
            "status": "answered",
            "timing": {
              "total_ms": 0.832112,
              "reduce_ms": 0.135232,
              "argmin_ms": 0.22844799999999998,
              "rows_per_second": 31245793835.44523,
              "warmup_ticks": 10,
              "measured_ticks": 100,
              "method": "gpu-timestamp-queries"
            },
            "accuracy": {
              "reduction_max_relative_error": 1.7146693137426378e-7,
              "reduction_mean_relative_error": 3.2168312051139436e-8,
              "winner_mismatch_fraction": 0.0,
              "order_sensitive_groups": 276097,
              "fired_mismatch_count": 1
            }
          }
        },
        {
          "strategy": "native f64 (wgpu)",
          "reduction_choice": "deterministic fixed-order two-pass reduction, no atomics (Level A)",
          "verdict": "unanswered on this adapter: native_f64: unsupported; adapter=NVIDIA H100 PCIe; backend=Vulkan; reason=wgpu 0.20 native f64 is disabled on NVIDIA H100 PCIe after an observed NVVM compiler failure; use the CUDA native-f64 path; device=NVIDIA H100 PCIe; fp64:fp32=1:2; class=full-rate; evidence=documented NVIDIA datacenter-compute model lookup; full-rate-extrapolation=allowed. No throughput or accuracy number was fabricated.",
          "status": {
            "status": "unanswered",
            "reason": "unanswered on this adapter: native_f64: unsupported; adapter=NVIDIA H100 PCIe; backend=Vulkan; reason=wgpu 0.20 native f64 is disabled on NVIDIA H100 PCIe after an observed NVVM compiler failure; use the CUDA native-f64 path; device=NVIDIA H100 PCIe; fp64:fp32=1:2; class=full-rate; evidence=documented NVIDIA datacenter-compute model lookup; full-rate-extrapolation=allowed"
          }
        },
        {
          "strategy": "native f64 (CUDA)",
          "reduction_choice": "deterministic fixed-order two-pass reduction, no atomics (Level A)",
          "verdict": "Native binary64 matched every oracle argmin winner and fired flag, with zero unexplained fixed-tree arithmetic-mirror differences. Device `NVIDIA H100 PCIe` is classified `full-rate` from CUDA cudaDevAttrSingleToDoublePrecisionPerfRatio; This is full-rate fp64 hardware, so full-rate extrapolation is allowed.",
          "status": {
            "status": "answered",
            "timing": {
              "total_ms": 0.7248800098896027,
              "reduce_ms": 0.13921599835157394,
              "argmin_ms": 0.14643199741840363,
              "rows_per_second": 35868005249.53051,
              "warmup_ticks": 10,
              "measured_ticks": 100,
              "method": "gpu-timestamp-queries"
            },
            "accuracy": {
              "reduction_max_relative_error": 0.0,
              "reduction_mean_relative_error": 0.0,
              "winner_mismatch_fraction": 0.0,
              "order_sensitive_groups": 276097,
              "fired_mismatch_count": 0,
              "unexplained_arithmetic_mirror_difference_count": 0
            }
          }
        }
      ],
      "guards": {
        "double-single": {
          "status": "failed",
          "reason": "double-single winner mismatch rate 0.00002 does not satisfy the f32 baseline 0.00002"
        },
        "f32": {
          "status": "passed"
        },
        "native f64 (CUDA)": {
          "status": "passed"
        },
        "native f64 (wgpu)": {
          "status": "unavailable",
          "reason": "native_f64: unsupported; adapter=NVIDIA H100 PCIe; backend=Vulkan; reason=wgpu 0.20 native f64 is disabled on NVIDIA H100 PCIe after an observed NVVM compiler failure; use the CUDA native-f64 path; device=NVIDIA H100 PCIe; fp64:fp32=1:2; class=full-rate; evidence=documented NVIDIA datacenter-compute model lookup; full-rate-extrapolation=allowed"
        }
      },
      "infrastructure": {
        "expected-gpu": "H100",
        "full-rate-extrapolation": "refused-until-runtime-verification",
        "generated-at-utc": "2026-07-18T04:17:48Z",
        "hyperstack-environment": "default-CANADA-1",
        "hyperstack-flavor": "n3-H100x1",
        "hyperstack-image": "Ubuntu Server 22.04 LTS R570 CUDA 12.8",
        "hyperstack-region": "CANADA-1",
        "nvidia-device": "NVIDIA H100 PCIe, 570.195.03, 00000000:00:07.0",
        "provider": "hyperstack",
        "repository-commit": "d6c545f63a89135d01addeea42b9fbe44fac897a",
        "requested-fp64-class": "full-rate",
        "run-id": "b902e6a3318f221a138e8f88df0aa9e4-run-2"
      }
    }
  }
}
```
<!-- sembla-precision-state-v1:end -->
