# Demographic benchmark results

Machine class: **Apple M2 Pro, 16 GiB, CPU-only moderate-memory local**; OS `Darwin 24.5.0`; architecture `arm64`; CPU `Apple M2 Pro`; RAM `16.0 GiB`. No hostname or workspace path is recorded.

Backend: `cpu`; seed: `9009`; ticks: `24`; present fraction: `0.8`; streams: `birth:600,overseas:250,internal:150`.

| Slots | State MiB | Synth s | Load s | Full s | No-ageing s | No-grouped s | ticks/s | Ageing share | Grouped share | Peak RSS MiB | Export s |
|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| 10,000 | 0.46 | 0.170 | 0.000 | 0.510 | 0.480 | 0.460 | 47.059 | 0.059 | 0.098 | 22.9 | 0.010 |
| 100,000 | 4.58 | 0.000 | 0.030 | 5.360 | 4.870 | 4.820 | 4.478 | 0.091 | 0.101 | 129.1 | 0.070 |
| 1,000,000 | 45.78 | 0.050 | 0.350 | 54.680 | 51.500 | 56.770 | 0.439 | 0.058 | -0.038 | 551.8 | 0.720 |

Cost shares are `(full − variant) / full` wall time. Negative values are retained rather than hidden because these are single local measurements.
