# Frozen demographic benchmark results

Repository commit: `4458a9de2f7477b34d0bb5b7585685185e65ea63`; session: `1f180015-9224-4f8f-9e0f-c7e946aee473`; host identity: `04a71206c109b5f75095ad7ef0739f6aa8512953480c30b8b96e082800c4a268`.

| Measurement | Replicate 1 s | Replicate 2 s | Replicate 3 s | Median s | Min–max s |
|---|---:|---:|---:|---:|---:|
| CUDA no-grouped | 6.280 | 6.320 | 6.370 | 6.320 | 6.280–6.370 |
| CPU no-grouped | 52.860 | 49.940 | 49.930 | 49.940 | 49.930–52.860 |
| CPU full | 82.990 | 82.810 | 81.280 | 82.810 | 81.280–82.990 |
| CPU no-ageing | 47.590 | 47.400 | 47.410 | 47.410 | 47.400–47.590 |

§L4 ratio: **7.902×**; verdict: **MET**.

Ageing-share replicates: 0.426557, 0.427605, 0.416708; median **0.426557**; range 0.416708–0.427605. This strengthens the §K2 trigger evidence; §K2 is not decided here.
