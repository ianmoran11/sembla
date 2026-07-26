# Frozen demographic benchmark results

Repository commit: `206900c25e5124039f95b1d64d0f3a60dbd8ed7d`; session: `e799b867-0655-4e7f-ae6e-690337c75e1f`; host identity: `f4a041f9e9568ce58c705d4eca5ec968880d8935edd4dff30c3e30748d23c206`.

| Measurement | Replicate 1 s | Replicate 2 s | Replicate 3 s | Median s | Min–max s |
|---|---:|---:|---:|---:|---:|
| CUDA no-grouped | 171.200 | 169.410 | 178.430 | 171.200 | 169.410–178.430 |
| CPU no-grouped | 433.500 | 438.710 | 445.550 | 438.710 | 433.500–445.550 |
| CPU full | 478.760 | 482.950 | 483.660 | 482.950 | 478.760–483.660 |
| CPU no-ageing | 420.490 | 427.010 | 419.320 | 420.490 | 419.320–427.010 |

§L4 ratio: **2.563×**; verdict: **NOT MET**.

Ageing-share replicates: 0.121710, 0.115830, 0.133027; median **0.121710**; range 0.115830–0.133027. This strengthens the §K2 trigger evidence; §K2 is not decided here.
