# Calibrated Australian population evidence — 2026-08-07

This is the compact scientific and execution evidence for PRD 0008's complete
2010–2024 forward walk (stocks through 30 June 2025). It is not PRD 0009's
rolling-origin validation and makes no full-scale claim.

## Headline result

- Held-out single-year stock mean annual WAPE improved from
  **9.008%** to **7.701%**
  (1.307% points;
  14.5%
  relative reduction).
- Fitted interstate O–D mean annual WAPE improved from
  **72.727%** to
  **29.416%**. The final value is worse than
  the previously measured gravity-only mean (~22.4%) because accepted NPE
  posteriors updated the spatial parameters in 2010, 2012 and 2019; this is
  reported, not tuned away.
- Fitted five-year stock WAPE improved from
  **7.053%** to
  **5.558%**.
- Terminal national population is **27,765,300** versus
  **27,606,008**: +159,292
  (+0.577%), compared with the baseline's
  +174,492 (+0.632%).
  Mean absolute terminal state error is **1.748%**.

## Identification finding

Only 3 of 15 yearly posteriors were accepted:
`2010, 2012, 2019`. SBC failed in
`2011, 2013, 2015, 2016, 2017, 2023`. SBC passed but the unconstrained flow put a
posterior median outside the strictly positive parameter domain in
`2014, 2018, 2020, 2021, 2022, 2024`; those years retained all gravity
values. This is strong evidence that the offline gravity fit supplied most of
the useful correction and that raw-coordinate NPE is not reliably identified
for every year. A future log-space NPE experiment requires a new declaration;
it was not substituted mid-run.

The first 2014 attempt exposed the missing domain invariant: SBC passed but the
`interstate_base` median was about `-0.0003639`, producing zero interstate moves
and a correctly refused score denominator. Its exact compact report, traceback
and raw-file hash inventory are retained under `execution/`. The corrected
wholesale fallback rule is DECISIONS §N20.

## Integrity and capacity

- 15/15 parameter files, run files, score reports and exported states match
  their recorded raw SHA-256 values.
- Every domain-separated state link, semantic seed, resolved 377-value θ,
  capacity report and score was independently recomputed. All non-metric score
  content is exact; x86_64/arm64 aggregate metric roundoff was at most
  `6.37e-12`.
- All fifteen θ̂ reruns pass DECISIONS §N21's strict point-predictive gate:
  fitted-role MAE and RMSE both beat the same-year frozen PRD 0007 baseline.
  The gate was added during final review, has no tuned tolerance, excludes
  held-out cells, and changes no recorded scientific output.
- Terminal state raw SHA-256:
  `4aa2f243e5a37ce9c10ab6b9f757d287cd2e3dfb5d23af16b578c3bd84d158ee`.
- Terminal state-artifact digest:
  `9757b0b3ca992a32f85c982a0b1c4a3402f25de5195860c18341d47294ad4ab6`.
- Minimum vacancies were 4,621 birth and 7,418 overseas
  slots. Maximum deferred/fired ratio was
  0.556% in
  2016.
- The exact Australian plan/state/θ CPU–CUDA gate returned `verdict=equal`
  (3.583 CPU versus
  7.730 CUDA
  ticks/s).
- The 2020 score still records the material 27,626 margin-vintage conflict; raw
  margins remain validation-only.

## Files

- `held-out-comparison.json` — PRD comparison using held-out targets only.
- `performance-summary.json` — annual and terminal baseline/calibrated metrics.
- `posterior-summary.json` — all 17 parameter summaries, contraction, SBC,
  admissibility and θ̂ decisions for all years.
- `parameter-verification.json` — all 15 full 377-value files, proving the 360
  fixed parameters remain equal to their annual gravity-centred constants.
- `point-predictive-check.json` — same-year fitted MAE/RMSE dominance over the
  frozen baseline, with both criteria and values recorded for every year.
- `chain-verification.json` — hashes, state links, seeds, capacity and score
  recomputation.
- `execution.json` and `execution/` — H100/CUDA/build/differential provenance and
  the preserved failed-attempt record.
- `validation.json`, `review.md` and `review-result.json` — final check battery
  and independent approval with zero blocker/high/medium/low findings.
- `diagnostics/` — exact per-year NPE diagnostics.
- `residuals/` — exact per-year calibrated score reports.
- `data/abs/params/calibrated/` — final full 377-value annual parameter files.
- `../gravity/fit-report.json` in the parameter tree remains the complete O–D
  residual/deviance evidence; the 2020 conflict is not reconciled.

The 8.9 GB raw remote draw tree was deliberately not committed. The local
retrieval retained the 30 checksum-verified NPE pair files, 15 posterior models,
15 boundary states and compact execution logs before VM destruction. Hyperstack
VM 967013 and its SSH rule were destroyed, provider reconciliation found zero
orphans, and all ephemeral session/deploy credentials were removed.
