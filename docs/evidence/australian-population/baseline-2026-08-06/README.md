# Australian population: uncalibrated 2010–2025 baseline

This is the PRD 0007 baseline, not a calibrated forecast. The seventeen
free migration entries in every annual parameter file remain their shared
pre-calibration defaults; PRD 0008 is responsible for fitting them. Only
the fixed ABS-derived fertility, mortality and overseas rates vary annually.

## Reproduction and chain identity

- Scale: `hundredth`; replica index: `0`.
- Run years: 2010–2024 (boundaries 2010-06-30 to 2025-06-30).
- Sembla: `sembla 0.3.0`, raw SHA-256 `48dc4fa8db73d62e11f1e54f7817ac97299aa639994fe2b7972bba9b4570bbd0`.
- Chain report SHA-256: `29fe3755a14f909c7437d01fb2e667d6525473533f4ffb917c348a4ab0075c24`.
- All 226 generated chain files reproduced byte for byte.
- Run year 2017 reproduced byte for byte in isolation (15 files).
- Every input/output state tuple, raw state byte hash, model/plan identity,
  annual parameter bytes, scalar/summary/grouped output and score report is
  checked by `scripts/verify-population-chain.sh`.

```bash
cargo build --release --locked
scripts/run-australian-population.sh --scale hundredth \
  --start-year 2010 --end-year 2024 \
  --params-dir data/abs/params --targets-dir data/abs/targets \
  --out /tmp/australian-baseline --backend cpu \
  --enable grouped-observations
scripts/verify-population-chain.sh --out /tmp/australian-baseline
scripts/run-australian-population.sh --scale hundredth \
  --start-year 2010 --end-year 2024 \
  --params-dir data/abs/params --targets-dir data/abs/targets \
  --out /tmp/australian-baseline-repeat --backend cpu \
  --enable grouped-observations
scripts/run-australian-population.sh --scale hundredth \
  --start-year 2017 --end-year 2017 \
  --initial-state /tmp/australian-baseline/2017.state \
  --params-dir data/abs/params --targets-dir data/abs/targets \
  --out /tmp/australian-baseline-2017 --backend cpu \
  --enable grouped-observations
python3 data/abs/chain.py evidence \
  --chain /tmp/australian-baseline \
  --reproduction-chain /tmp/australian-baseline-repeat \
  --middle-chain /tmp/australian-baseline-2017 --middle-run-year 2017 \
  --out docs/evidence/australian-population/baseline-2026-08-06
```

The driver refuses a non-empty output directory. This is deliberate: a
subset is a new annual run with an explicit input state, not checkpoint
resume. Seeds are SHA-256-derived from model identity, scale, run year,
annual parameter-file digest and replica index; list position and output
path never enter the coordinate.

## Eight-state ERP drift

| boundary | target | simulated | signed error | relative error |
|---:|---:|---:|---:|---:|
| 2011 | 22,336,907 | 22,376,000 | +39,093 | +0.175% |
| 2012 | 22,730,432 | 22,782,300 | +51,868 | +0.228% |
| 2013 | 23,125,167 | 23,169,400 | +44,233 | +0.191% |
| 2014 | 23,472,790 | 23,520,500 | +47,710 | +0.203% |
| 2015 | 23,813,144 | 23,828,800 | +15,656 | +0.066% |
| 2016 | 24,186,299 | 24,178,200 | -8,099 | -0.033% |
| 2017 | 24,587,917 | 24,616,300 | +28,383 | +0.115% |
| 2018 | 24,958,533 | 25,006,200 | +47,667 | +0.191% |
| 2019 | 25,330,050 | 25,414,900 | +84,850 | +0.335% |
| 2020 | 25,644,445 | 25,754,800 | +110,355 | +0.430% |
| 2021 | 25,680,562 | 25,830,900 | +150,338 | +0.585% |
| 2022 | 26,013,800 | 26,183,000 | +169,200 | +0.650% |
| 2023 | 26,654,952 | 26,832,000 | +177,048 | +0.664% |
| 2024 | 27,189,291 | 27,372,400 | +183,109 | +0.673% |
| 2025 | 27,606,008 | 27,780,500 | +174,492 | +0.632% |

The terminal 30 June 2025 eight-state population is 27,780,500 against 27,606,008: +0.632% drift. The largest absolute national relative drift is +0.673% at the 2024 boundary.

### Final 30 June 2025 population by state

| state | target | simulated | signed error | relative error |
|---|---:|---:|---:|---:|
| nsw | 8,590,113 | 8,600,900 | +10,787 | +0.126% |
| vic | 7,069,856 | 6,809,800 | -260,056 | -3.678% |
| qld | 5,669,915 | 5,296,900 | -373,015 | -6.579% |
| sa | 1,901,615 | 2,035,300 | +133,685 | +7.030% |
| wa | 3,046,209 | 3,033,700 | -12,509 | -0.411% |
| tas | 577,770 | 793,500 | +215,730 | +37.338% |
| nt | 265,895 | 530,500 | +264,605 | +99.515% |
| act | 484,635 | 679,900 | +195,265 | +40.291% |

## Annual component errors

WAPE is total absolute cell error divided by the published family total.
The interstate result is expected to be poor before PRD 0008 because
`interstate_base`, push/pull factors, `peak_months` and `k` are not fitted.

| run year | births WAPE | deaths WAPE | arrivals WAPE | departures WAPE | O-D WAPE |
|---:|---:|---:|---:|---:|---:|
| 2010 | 2.631% | 4.298% | 3.439% | 4.363% | 73.150% |
| 2011 | 2.946% | 3.904% | 3.546% | 3.964% | 74.107% |
| 2012 | 5.643% | 6.628% | 2.217% | 3.559% | 71.323% |
| 2013 | 4.114% | 3.863% | 2.636% | 4.724% | 72.480% |
| 2014 | 2.658% | 2.453% | 3.779% | 4.093% | 72.392% |
| 2015 | 3.694% | 10.307% | 3.650% | 5.338% | 74.364% |
| 2016 | 3.706% | 11.571% | 2.714% | 5.206% | 74.416% |
| 2017 | 4.837% | 7.574% | 2.379% | 3.831% | 74.682% |
| 2018 | 2.579% | 8.740% | 2.240% | 4.928% | 73.988% |
| 2019 | 3.815% | 8.844% | 1.632% | 3.925% | 72.005% |
| 2020 | 4.391% | 7.018% | 4.720% | 8.510% | 72.076% |
| 2021 | 2.328% | 9.380% | 1.208% | 3.737% | 71.005% |
| 2022 | 3.251% | 8.026% | 1.528% | 7.109% | 72.653% |
| 2023 | 2.921% | 11.625% | 2.583% | 4.161% | 70.253% |
| 2024 | 4.712% | 9.769% | 3.093% | 5.590% | 72.006% |

## Capacity and interpretation

- Minimum vacant birth slots: 4,606.
- Minimum vacant overseas slots: 7,425.
- Maximum deferred/fired ratio: 0.652174% in run year 2020.
- No year crossed the strict 10% saturation threshold or reached a zero
  vacancy margin; either condition makes the driver fail.

Birth/death registration years, financial-year migration and 30 June ERP
stocks do not form an exact accounting identity. Other Territories remain
outside the eight-state model. Residuals are therefore evidence, not values
to be silently reconciled. Detailed per-year reports are under `residuals/`.

## Chained versus continuous runs

A chained 12+12 pair is intentionally not bitwise equal to one continuous
24-tick run because the second annual window restarts tick coordinates and
has its own semantic seed and theta. This is asserted by
`crates/sembla-cli/tests/chained_runs.rs`; annual state artifacts are chain
links, not hidden checkpoints.
