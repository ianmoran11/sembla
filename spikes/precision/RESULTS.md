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

| Strategy | Source machine | Timing method | ms/tick total | ms/tick segmented reduce | ms/tick segmented argmin | rows/sec | Reduction rel-err (max / mean) | Winner mismatch % | Order-sensitive groups |
|---|---|---|---:|---:|---:|---:|---:|---:|---:|
| f32 | Development | synchronized wall-clock fallback | 10.711042 | 3.273896 | 4.984000 | 2427401667.709 | 1.714669e-7 / 3.216831e-8 | 0.000000 | 276097 |
| double-single | Development | synchronized wall-clock fallback | 13.868958 | 3.201605 | 4.334770 | 1874690225.466 | 1.096998e-14 / 1.206441e-15 | 0.000000 | 276097 |
| native f64 (wgpu) | Development | unanswered on this adapter: native_f64: unsupported; adapter=Apple M2 Pro; backend=Metal; reason=native WGSL f64 is supported by wgpu only on Vulkan; device=Apple M2 Pro; fp64:fp32=unknown; class=rate-limited; evidence=conservative fallback for an unrecognized model; full-rate-extrapolation=refused | unanswered | unanswered | unanswered | unanswered | unanswered | unanswered | unanswered |
| native f64 (CUDA) | Development | unanswered on this adapter: cuda: feature-disabled | unanswered | unanswered | unanswered | unanswered | unanswered | unanswered | unanswered |

## Reduction determinism choice

| Strategy | Source machine | Choice |
|---|---|---|
| f32 | Development | deterministic fixed-order two-pass reduction, no atomics (Level A) |
| double-single | Development | deterministic fixed-order two-pass reduction, no atomics (Level A) |
| native f64 (wgpu) | Development | deterministic fixed-order two-pass reduction, no atomics (Level A) |
| native f64 (CUDA) | Development | deterministic fixed-order two-pass reduction, no atomics (Level A) |

## Per-strategy verdicts

### f32

**Source:** Development. Portable baseline. It is the cheapest arithmetic path; its reported reduction and winner errors are the reference that double-single must improve.

### double-single

**Source:** Development. Portable double-single passed the strict-math behavior probe and the PRD-0002 reduction/winner thresholds on this adapter.

### native f64 (wgpu)

**Source:** Development. unanswered on this adapter: native_f64: unsupported; adapter=Apple M2 Pro; backend=Metal; reason=native WGSL f64 is supported by wgpu only on Vulkan; device=Apple M2 Pro; fp64:fp32=unknown; class=rate-limited; evidence=conservative fallback for an unrecognized model; full-rate-extrapolation=refused. No throughput or accuracy number was fabricated.

### native f64 (CUDA)

**Source:** Development. unanswered on this adapter: cuda: feature-disabled. No throughput or accuracy number was fabricated.


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

Not yet measured; its rows remain unanswered in the merged matrix.
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
              "reduction_mean_relative_error": 1.2064412155246435e-15,
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
    }
  }
}
```
<!-- sembla-precision-state-v1:end -->
