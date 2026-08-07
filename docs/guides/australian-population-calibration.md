# Calibrating the Australian population model

This guide describes the two-stage migration calibration harness from PRD
0008. Read the [run guide](australian-population-runs.md) first; it defines the
annual chain, semantic seeds and capacity gates that calibration builds on.

## Why two stages, and why the offline fit comes first

`DECISIONS.md` §N5 writes the monthly interstate hazard as

```
interstate_base × push[origin] × pull[destination] × ageProfile(age_months)
```

with `push_nsw ≡ pull_nsw ≡ 1` and
`ageProfile(a) = 1 / (1 + k·(a − peak_months)²)`. Seventeen parameters are free
(§N6 fixes everything else to ABS-derived annual values):

- `interstate_base`, seven `push_*`, seven `pull_*` — the spatial block; and
- `peak_months`, `k` — the age profile.

The published origin–destination table is all-ages. It identifies the fifteen
spatial parameters directly — with an age-weighted origin exposure offset the
expected table is an ordinary Poisson log-linear model — but it cannot identify
the age profile: any aggregate age effect at an origin is absorbed by that
origin's push factor. Fitting all seventeen by simulation alone would spend
expensive draws re-discovering what a closed-form fit gives exactly, and would
still leave `peak_months` and `k` weak. The harness therefore:

1. fits the spatial block offline, exactly, from published cells; then
2. fits the age profile — and measures the remaining uncertainty on all
   seventeen — by neural posterior estimation (NPE) against simulated
   summaries that do carry age information.

## Stage 1 — the offline gravity fit

```bash
python3 data/abs/gravity_fit.py
```

Standard library only. For each run year it:

- computes origin exposure from `erp_state_age_sex.csv` with the profile held
  at its prior centres (`peak_months = 360`, `k = 1e-5`), averaging over the
  birth-month spread used by `build_state.py` and summing over the run's twelve
  monthly ticks of ageing;
- fits `interstate_base`, the seven pushes and the seven pulls to all 56
  published O-D cells by Newton/IRLS Poisson maximum likelihood, with
  `push_nsw ≡ pull_nsw ≡ 1`, leaving exactly 41 residual degrees of freedom;
- writes `data/abs/params/gravity/<year>.json` — the full 377-parameter annual
  file with only the fifteen free slots changed — and
  `data/abs/params/gravity/fit-report.json`.

The fit report carries, per year and per cell: observed, expected, signed
residual, Pearson residual and deviance contribution; total deviance against
the 41 degrees of freedom; and the margin comparison against the separately
published `interstate_margins.csv`. Two structural facts are asserted by test:

- fitted O-D margins equal the published table margins exactly (a maximum
  likelihood property of the log-linear form), while the *separately published*
  margin workbook disagrees with the table in 2020 — arrivals differ by 27,626
  for NSW alone. That vintage conflict is validation-only: it is reported every
  time and never reconciled away;
- the separable push×pull form is *rejected* by its own residuals (deviance is
  hundreds per degree of freedom). The gravity model is the best separable fit,
  not a true model; the residual pattern (ACT↔NSW and NT↔SA badly
  under-predicted) is evidence about what a distance or affinity term would
  have to explain, and is reported as such.

Measured standalone effect (hundredth scale, full 2010–2025 chain against the
PRD 0007 baseline): mean annual O-D WAPE falls from 72.7% to 22.4%, and mean
absolute state-level error at the 2025 boundary falls from 24.4% to 1.6%.
National drift is essentially unchanged (+0.63% → +0.58%) because national
totals were never the problem.

## Stage 2 — θ, draws, and the θ-file contract

`data/abs/theta.py` reads `params/priors.json` — the sole fixed/free authority —
and builds θ as exactly the seventeen free parameters. Draw files are written
in the shape `sembla sweep --theta-file` already accepts: a JSON array whose
every object carries **all 377** parameters, with the 360 fixed rates held at
their per-year values. No sweep can therefore vary a rate the data has already
determined, and a test proves no fixed slot ever moves across draws.

Draws are log-normal, centred on the gravity-fitted values with the prior
spreads from `priors.json`, so the simulator is exercised in the region the
published table already indicates. Randomness is a counter-based SHA-256 stream
keyed by semantic coordinates (model identity, scale, run year, parameter-file
digest, draw count, purpose) — never list position, path or wall clock.

One adapter mapping is required: the pairs contract reserves the column name
`k` for the draw index, and one free parameter is itself named `k`. Every
parameter column in pairs artifacts is therefore prefixed uniformly —
`theta_k`, `theta_push_vic`, … — never special-cased.

## Stage 3 — the per-year NPE loop

For each run year `y`:

```bash
# draws centred on the gravity fit (all 377 slots, 17 varied)
python3 data/abs/theta.py --run-year <y> --draws <N> \
  --model-identity <ir-hash> --out theta-<y>.json --record theta-<y>.record.json

# simulate: independent noise is mandatory for NPE (DECISIONS.md §G5)
sembla sweep fixtures/australian-population/australian_population.hundredth.plan.json \
  --population <y>.state --seed <semantic> --theta-file theta-<y>.json \
  --ticks 12 --out sweep-<y> --backend cpu --noise independent \
  --draw-workers 8 --enable grouped-observations
```

`data/abs/calibrate.py` then reduces every draw to the ordered 126-dimensional
summary vector and writes contract-shaped pairs artifacts:

- 6 headline scalars (final population, births, deaths, interstate moves,
  overseas arrivals and departures);
- the 56 O-D counts (`fired_move_*` from the run CSV);
- the **normalized interstate age-sex composition** — 2 sexes × 16 ABS
  five-year bands, summing to one — which carries the age information the O-D
  table lacks; raw count margins stay reporting evidence because their
  2020-vintage total scale is incompatible;
- the **normalized stock age-sex composition** at the final tick, same shape.

Grouped CSVs emit band *indices* (`band age_months 60` produces 0, 1, 2, …),
not attribute values; the extraction maps index `b` to ABS band `min(b, 15)`,
and a regression test locks the convention after a pilot-era bug collapsed
every band into 0-4.

### Where the sweeps run

On CPU the sweep's draw-worker pool is unavailable (`--draw-workers > 1`
requires `--backend cuda`), so the CPU path shards the draw file and runs one
sweep *process* per worker with per-chunk semantic seeds. On CUDA a single
sweep runs the pool in-process. The CUDA backend is gated before any loop
runs: `sembla diff-backends … --enable grouped-observations` must report
`verdict=equal` for this exact model and state (measured 2026-08-07 on an
H100: 7.9 ticks/s CUDA against 3.6 on the host CPU, equality exact). The
15-year loop ran on a Hyperstack H100 under
`spikes/precision/infra-hyperstack/`'s paid-session discipline; CPU fallback
remains a first-class path (~6x slower wall clock).

The quarantined `calibration/npe` trainer consumes the pairs and produces a
posterior per year. The quarantine is byte-untouched: `calibrate.py`'s training
entry point runs under the calibration venv interpreter and only then imports
the trainer, injecting per-parameter recovery tolerances *at runtime* — the
reference trainer hard-codes tolerances for its two reference parameters and
raises on any other name. In-memory injection leaves every quarantined byte
unchanged and adds no Australian name to it; the injected values (draw standard
deviations) are informational and never a pass gate.

The point estimate follows a predeclared gate, never tuned per year:

- if the year's SBC rank-uniformity gate fails, the posterior is rejected
  wholesale and every parameter keeps its offline gravity value (the year is
  flagged `sbc_failed`);
- otherwise each parameter whose posterior contracts (posterior SD ÷ draw SD
  below 0.9) takes the **posterior median** — chosen over the mean because
  posteriors for weakly identified multiplicative parameters are skewed — and
  each parameter that does not contract is named unidentified and keeps its
  gravity value.

`peak_months` and `k` follow the same rule with the gravity file's prior
centres as their fallback. The year is then re-run with θ̂_y to export the
state for y+1, with `chain.py`'s exact command and semantic seed so the
calibrated chain is comparable link by link with the baseline.

## Diagnostics, and how each can fail

All of these run per year and any of them fails the calibration, not merely
prints:

- **Capacity and saturation** (PRD 0007 §5): every draw's run CSV is checked;
  a saturated or zero-vacancy draw is not calibration evidence and aborts the
  year.
- **Simulation-based calibration**: rank uniformity over held-out draws, using
  the quarantined `sbc.py` gate against `posterior.pt`.
- **Posterior predictive check**: the θ̂_y re-run is scored in evaluation mode;
  fitted targets must sit within the score report's own tolerances.
- **Prior/posterior contraction** per parameter: the posterior standard
  deviation divided by the draw standard deviation. A parameter whose ratio is
  ≥ 0.9 was not informed by the data and is **named as not identified** —
  reported as such, never reported as fitted. `peak_months` and `k` are the
  expected candidates and are reported explicitly every year.
- **Gravity fit**: deviance against 41 residual degrees of freedom, with the
  2020 margin conflict propagated as validation-only.

## Honest unavailability

If the pinned NPE environment cannot be installed, the year writes a
diagnostics file with `status: "unanswered"` and `pass: false` plus the
dependency reason, and the inference stage is skipped — the offline gravity fit
still runs and is still reported. An unanswered environment is never evidence
of a pass.

## The compounding warning

Calibration is a per-year forward walk (§N11), not a joint posterior over the
whole path: θ̂_y is fitted against the state the walk actually reached, so an
error absorbed in year y propagates into the starting point of year y+1. A
calibrated chain that fits every year locally can still drift globally, and
PRD 0009's rolling-origin validation exists to measure exactly that. Never
read a per-year pass as a whole-path guarantee.
