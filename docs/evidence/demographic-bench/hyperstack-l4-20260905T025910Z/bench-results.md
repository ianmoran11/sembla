# Frozen demographic benchmark results

Repository commit: `1688e51ac107511908d762e389bb26f4850fd747`; session: `d1fe96c9-d89f-4691-bd1b-9ae2e2709f85`; host identity: `5b6188c18c9b648ccd423ec07734afc34343780aa03bc12b1fad32a19a227e61`.

| Measurement | Replicate 1 s | Replicate 2 s | Replicate 3 s | Median s | Min–max s |
|---|---:|---:|---:|---:|---:|
| CUDA no-grouped | 6.110 | 6.210 | 6.060 | 6.110 | 6.060–6.210 |
| CPU no-grouped | 48.840 | 49.920 | 50.340 | 49.920 | 48.840–50.340 |
| CPU full | 80.910 | 81.810 | 82.940 | 81.810 | 80.910–82.940 |
| CPU no-ageing | 47.040 | 47.030 | 48.700 | 47.040 | 47.030–48.700 |

§L4 ratio: **8.170×**; verdict: **MET**.

Ageing-share replicates: 0.418613, 0.425131, 0.412829; median **0.418613**; range 0.412829–0.425131. This strengthens the §K2 trigger evidence; §K2 is not decided here.
