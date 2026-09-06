# Frozen demographic benchmark results

Repository commit: `f84bb05b4d7d3c9fb4d6ce829570569ab42ac408`; session: `67c7c288-9dca-4eef-84ea-12fd58f3d687`; host identity: `06f61a7784d93f148a2e0452d477fdee0c7daa9ab4564b449b5241b011420461`.

| Measurement | Replicate 1 s | Replicate 2 s | Replicate 3 s | Median s | Min–max s |
|---|---:|---:|---:|---:|---:|
| CUDA no-grouped | 6.200 | 6.280 | 6.220 | 6.220 | 6.200–6.280 |
| CPU no-grouped | 50.120 | 49.640 | 50.990 | 50.120 | 49.640–50.990 |
| CPU full | 83.770 | 81.150 | 82.270 | 82.270 | 81.150–83.770 |
| CPU no-ageing | 48.030 | 48.010 | 48.040 | 48.030 | 48.010–48.040 |

§L4 ratio: **8.058×**; verdict: **MET**.

Ageing-share replicates: 0.426644, 0.408380, 0.416069; median **0.416069**; range 0.408380–0.426644. This strengthens the §K2 trigger evidence; §K2 is not decided here.
