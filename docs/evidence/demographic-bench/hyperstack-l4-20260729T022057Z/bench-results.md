# Frozen demographic benchmark results

Repository commit: `5616dbe56cddb26e6a6541bead3572639827a8c2`; session: `86c3c927-92be-43ee-9c52-d1946c87d57f`; host identity: `5b6188c18c9b648ccd423ec07734afc34343780aa03bc12b1fad32a19a227e61`.

| Measurement | Replicate 1 s | Replicate 2 s | Replicate 3 s | Median s | Min–max s |
|---|---:|---:|---:|---:|---:|
| CUDA no-grouped | 6.550 | 6.480 | 6.500 | 6.500 | 6.480–6.550 |
| CPU no-grouped | 50.830 | 51.310 | 50.750 | 50.830 | 50.750–51.310 |
| CPU full | 81.140 | 81.870 | 81.890 | 81.870 | 81.140–81.890 |
| CPU no-ageing | 48.070 | 48.570 | 48.290 | 48.290 | 48.070–48.570 |

§L4 ratio: **7.820×**; verdict: **MET**.

Ageing-share replicates: 0.407567, 0.406742, 0.410307; median **0.407567**; range 0.406742–0.410307. This strengthens the §K2 trigger evidence; §K2 is not decided here.
