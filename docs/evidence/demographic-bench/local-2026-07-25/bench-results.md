# Demographic benchmark results

Machine class: **Apple M2 Pro, 16 GiB, CPU-only moderate-memory local**; OS `Darwin 24.5.0`; architecture `arm64`; CPU `Apple M2 Pro`; RAM `16.0 GiB`. No hostname or workspace path is recorded.

Backend: `cpu`; seed: `9009`; ticks: `24`; present fraction: `0.8`; streams: `birth:600,overseas:250,internal:150`.

| Slots | State MiB | Synth s | Load s | Full s | No-ageing s | No-grouped s | ticks/s | Ageing share | Grouped share | Peak RSS MiB | Export s |
|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| 2,000,000 | 91.55 | 0.700 | 0.680 | 108.590 | 99.820 | 99.960 | 0.221 | 0.081 | 0.079 | 738.3 | 1.440 |
| 5,000,000 | 228.88 | 0.260 | 1.720 | 286.810 | 278.190 | 268.870 | 0.084 | 0.030 | 0.063 | 1535.1 | 3.780 |
| 10,000,000 | 457.76 | 0.610 | 3.530 | 593.050 | 524.430 | 520.180 | 0.040 | 0.116 | 0.123 | 1944.4 | 8.240 |

Cost shares are `(full − variant) / full` wall time. Negative values are retained rather than hidden because these are single local measurements.
