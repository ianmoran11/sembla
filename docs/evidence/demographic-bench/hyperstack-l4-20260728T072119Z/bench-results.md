# Frozen demographic benchmark results

Repository commit: `04ada45cb2c8e0b365bae4d4d3c1415b111ed7eb`; session: `142b6f40-127d-4533-8a7c-9edd3c860d90`; host identity: `f4a041f9e9568ce58c705d4eca5ec968880d8935edd4dff30c3e30748d23c206`.

| Measurement | Replicate 1 s | Replicate 2 s | Replicate 3 s | Median s | Min–max s |
|---|---:|---:|---:|---:|---:|
| CUDA no-grouped | 13.900 | 14.100 | 14.240 | 14.100 | 13.900–14.240 |
| CPU no-grouped | 49.070 | 49.040 | 50.070 | 49.070 | 49.040–50.070 |
| CPU full | 80.220 | 80.420 | 80.650 | 80.420 | 80.220–80.650 |
| CPU no-ageing | 47.830 | 47.340 | 47.360 | 47.360 | 47.340–47.830 |

§L4 ratio: **3.480×**; verdict: **MET**.

Ageing-share replicates: 0.403765, 0.411340, 0.412771; median **0.411340**; range 0.403765–0.412771. This strengthens the §K2 trigger evidence; §K2 is not decided here.
