# Frozen demographic benchmark results

Repository commit: `00389a7876804c34ed2f57af68d39e13151d30ca`; session: `ab2f4ed5-845d-4d41-b1a5-a6daaf10e316`; host identity: `75fadf4c994661c1453577a3d18ec7936fad38a4b9fbd66dbbe797c1de8c6117`.

| Measurement | Replicate 1 s | Replicate 2 s | Replicate 3 s | Median s | Min–max s |
|---|---:|---:|---:|---:|---:|
| CUDA no-grouped | 26.070 | 26.370 | 27.570 | 26.370 | 26.070–27.570 |
| CPU no-grouped | 50.700 | 50.480 | 50.040 | 50.480 | 50.040–50.700 |
| CPU full | 82.540 | 81.830 | 82.500 | 82.500 | 81.830–82.540 |
| CPU no-ageing | 48.870 | 48.930 | 50.170 | 48.930 | 48.870–50.170 |

§L4 ratio: **1.914×**; verdict: **NOT MET**.

Ageing-share replicates: 0.407923, 0.402053, 0.391879; median **0.402053**; range 0.391879–0.407923. This strengthens the §K2 trigger evidence; §K2 is not decided here.
