# Demographic benchmark results

Machine class: **Hyperstack NVIDIA H100 PCIe, 177 GiB host RAM, backend cpu**; OS `Linux 6.8.0-90-generic`; architecture `x86_64`; CPU `AMD EPYC 9554 64-Core Processor`; RAM `177.1 GiB`. No hostname or workspace path is recorded.

Backend: `cpu`; seed: `9009`; ticks: `24`; present fraction: `0.8`; streams: `birth:600,overseas:250,internal:150`.

| Slots | State MiB | Synth s | Load s | Full s | No-ageing s | No-grouped s | ticks/s | Ageing share | Grouped share | Peak RSS MiB | Export s |
|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| 10,000,000 | 457.76 | 0.880 | 2.870 | 481.630 | 423.050 | 438.970 | 0.050 | 0.122 | 0.089 | 2674.4 | 5.890 |

Cost shares are `(full − variant) / full` wall time. Negative values are retained rather than hidden because these are single local measurements.
