# Frozen demographic benchmark results

Repository commit: `ca235b07ec765d71cea2c0bbb4fcf3efcf67a63d`; session: `dafcb8ac-0e25-4199-b69e-e97f120ee9cc`; host identity: `4d61dccf2aa78c49d6f4b8d202cee0c6c4029397b7ad682001e6581cff8b8b61`.

| Measurement | Replicate 1 s | Replicate 2 s | Replicate 3 s | Median s | Min–max s |
|---|---:|---:|---:|---:|---:|
| CUDA no-grouped | 14.600 | 14.850 | 14.630 | 14.630 | 14.600–14.850 |
| CPU no-grouped | 50.570 | 49.300 | 50.250 | 50.250 | 49.300–50.570 |
| CPU full | 82.170 | 81.790 | 81.960 | 81.960 | 81.790–82.170 |
| CPU no-ageing | 49.570 | 48.470 | 48.640 | 48.640 | 48.470–49.570 |

§L4 ratio: **3.435×**; verdict: **MET**.

Ageing-share replicates: 0.396738, 0.407385, 0.406540; median **0.406540**; range 0.396738–0.407385. This strengthens the §K2 trigger evidence; §K2 is not decided here.
