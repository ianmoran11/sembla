# Frozen demographic benchmark results

Repository commit: `ca235b07ec765d71cea2c0bbb4fcf3efcf67a63d`; session: `9368da83-26b4-48c0-9e73-25f72ee1fe50`; host identity: `4f3de9961a490eefc98494e598cdce76cf5742a3b51f0bb639a28246003f3712`.

| Measurement | Replicate 1 s | Replicate 2 s | Replicate 3 s | Median s | Min–max s |
|---|---:|---:|---:|---:|---:|
| CUDA no-grouped | 14.800 | 14.910 | 15.100 | 14.910 | 14.800–15.100 |
| CPU no-grouped | 49.820 | 50.730 | 49.830 | 49.830 | 49.820–50.730 |
| CPU full | 82.500 | 81.930 | 82.070 | 82.070 | 81.930–82.500 |
| CPU no-ageing | 48.250 | 48.940 | 49.640 | 48.940 | 48.250–49.640 |

§L4 ratio: **3.342×**; verdict: **MET**.

Ageing-share replicates: 0.415152, 0.402661, 0.395150; median **0.402661**; range 0.395150–0.415152. This strengthens the §K2 trigger evidence; §K2 is not decided here.
