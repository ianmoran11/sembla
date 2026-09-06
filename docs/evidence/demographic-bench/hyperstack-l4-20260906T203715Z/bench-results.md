# Frozen demographic benchmark results

Repository commit: `107a686cebbcc4b002aa90b34cbdfb4448405860`; session: `0ecce7c9-c6b8-4b85-a9ed-226eb43423be`; host identity: `b017ceec4e3718ce601a0f7bd43b5f063d97f35df95c3e44f38794ef4bec1175`.

| Measurement | Replicate 1 s | Replicate 2 s | Replicate 3 s | Median s | Min–max s |
|---|---:|---:|---:|---:|---:|
| CUDA no-grouped | 4.670 | 4.670 | 4.690 | 4.670 | 4.670–4.690 |
| CPU no-grouped | 29.520 | 30.190 | 28.770 | 29.520 | 28.770–30.190 |
| CPU full | 36.670 | 37.510 | 38.600 | 37.510 | 36.670–38.600 |
| CPU no-ageing | 20.210 | 20.400 | 20.470 | 20.400 | 20.210–20.470 |

§L4 ratio: **6.321×**; verdict: **MET**.

Ageing-share replicates: 0.448868, 0.456145, 0.469689; median **0.456145**; range 0.448868–0.469689. This strengthens the §K2 trigger evidence; §K2 is not decided here.
