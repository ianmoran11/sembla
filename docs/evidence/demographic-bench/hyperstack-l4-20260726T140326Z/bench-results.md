# Frozen demographic benchmark results

Repository commit: `917d9309a1a77d465fed3a3133b5f5552244f6db`; session: `b997af3c-431a-48fb-ac08-292b0d57ac0d`; host identity: `ddec7a88e0f41a9bd27aa38b2c88138cbb60493f91a1e869bcfb38e32d093a97`.

| Measurement | Replicate 1 s | Replicate 2 s | Replicate 3 s | Median s | Min–max s |
|---|---:|---:|---:|---:|---:|
| CUDA no-grouped | 31.670 | 33.610 | 31.820 | 31.820 | 31.670–33.610 |
| CPU no-grouped | 134.090 | 133.860 | 133.850 | 133.860 | 133.850–134.090 |
| CPU full | 166.310 | 166.540 | 166.770 | 166.540 | 166.310–166.770 |
| CPU no-ageing | 112.950 | 111.910 | 111.790 | 111.910 | 111.790–112.950 |

§L4 ratio: **4.207×**; verdict: **MET**.

Ageing-share replicates: 0.320847, 0.328029, 0.329676; median **0.328029**; range 0.320847–0.329676. This strengthens the §K2 trigger evidence; §K2 is not decided here.
