# PRD 0008: The calibration harness — direct fit, then per-year NPE

## Context

Read `docs/prds-australian-population/README.md` first. `DECISIONS.md` §N5
(gravity parameterisation), §N6 (fertility and mortality fixed), §N11 (per-year
forward walk; stocks alone insufficient) and §G5 (NPE is a quarantined external
workflow) bind.

PRD 0007 produced an uncalibrated baseline chain whose drift is the problem to
solve. The seventeen free migration parameters from §N5 are the only things
being fitted; everything else is fixed by §N6.

PRD 0002 supplies all 56 published interstate origin→destination cells for each
run year. With `push_nsw ≡ 1` and `pull_nsw ≡ 1`, §N5's spatial gravity model has
fifteen identifiable parameters and the annual O-D fit has 41 residual degrees
of freedom. The separable gravity assumption can therefore be tested rather
than merely imposed.

The matrix is all-ages, so it still cannot identify the age profile (`peak`,
`k`): their aggregate origin effect can be absorbed by the push factors. §N5
therefore fits the spatial parameters offline from O-D cells and fits `peak` and
`k` by NPE against simulated stock age structure **and within-state age-sex
interstate-flow compositions** from `NIM_FY`. Raw age-sex counts remain in the
artifact, but composition removes the incompatible total scale in 2020. The
separate headline margins are validation-only. Identification can still be weak
and must be measured rather than assumed.

## Goal

Migration parameters are fitted per year — the spatial parameters by an offline
gravity fit to published O-D cells, the age profile and remaining uncertainty
by NPE — producing a calibrated 2010–2025 chain that measurably beats PRD 0007's
baseline on held-out targets, with gravity residuals and every parameter's
identification status reported.

## Specification

### 1. Offline gravity fit — `data/abs/gravity_fit.py`

Fit `push_o`, `pull_d` and `interstate_base` to the 56 published O-D counts by
Poisson/log-linear estimation with `push_nsw ≡ 1` and `pull_nsw ≡ 1`, standard
library only. Use origin person-exposure from the ERP age structure as an
offset, evaluated with `peak` and `k` held at their prior centres. An IRLS loop
over the fifteen identifiable spatial parameters needs no external solver.

Emit fitted values per year into the parameter files' free slots and a fit
report with observed, expected, signed residual and deviance contribution for
every O-D cell; report total deviance against 41 residual degrees of freedom.
Also compare fitted O-D margins with the separate margin workbooks, but do not
alter cells or fit both incompatible products in 2020.

`peak` and `k` are **not** in this fit; all-age O-D cells carry no separable age
information (§N5). Hold them at their prior centres here and leave them to §3.
Measure how much of PRD 0007's baseline drift this removes before writing any
NPE code, and record it.

### 2. θ vector and draws — `data/abs/theta.py`

Reads `priors.json` (PRD 0005) and builds the θ vector as exactly the free
parameters. Generates draw files in the shape `sembla sweep --theta-file`
already accepts, holding every fixed parameter at its per-year value. Draws come
from priors centred on §1's fitted values, so the simulator is exercised in the
region the data already indicates rather than across the whole prior.

Never let the sweep sample priors implicitly for this model — the fixed/free
split lives in `priors.json` and nowhere else.

### 3. Per-year NPE

For each year, using existing commands and the existing quarantined pipeline:

- `sembla sweep --population <y>.state --theta-file <draws> --ticks 12
  --export-pairs pairs.csv --enable grouped-observations`, with the noise mode
  and draw-worker count recorded;
- PRD 0006's scorer reduces each draw's output to the ordered summary vector,
  using the reduction §6 of that PRD recommended; the fitting vector must carry
  the normalized interstate age-sex composition targets that identify `peak`
  and `k`, while raw count margins stay reporting evidence;
- `calibration/npe`'s trainer consumes the θ/x pairs and produces a posterior;
- the point estimate (record whether posterior mean or median, and why) becomes
  θ̂_y, and the year is re-run with it to export the state for *y+1*.

Do not modify `calibration/npe`. If an adapter is needed, add it under
`data/abs/` and keep the quarantine intact: nothing in `calibration/npe` may
learn about Australian geography.

### 4. Honest unavailability

If the pinned NPE environment cannot be installed, write a diagnostics file with
`status: "unanswered"` and `pass: false` and the dependency reason, then skip the
inference stage — exactly the convention `calibration/npe` already uses. The
offline gravity fit still runs and is still reported. An unanswered environment
is never evidence of a pass.

### 5. Diagnostics that can fail

Per year, and all of them able to fail the PRD rather than merely being printed:

- **Simulation-based calibration** rank uniformity on the free parameters, using
  the existing `sbc.py` where it applies;
- **Posterior predictive check** against fitted targets;
- **Prior/posterior contraction** per parameter — a parameter whose posterior
  equals its prior is not identified by the data and must be named as such
  rather than reported as fitted. `peak` and `k` are the expected candidates,
  since §N5 identifies them only through simulated age structure; report their
  contraction explicitly every year rather than leaving it to be inferred from
  a table;
- **Saturation and vacancy** checks from PRD 0007 §5, since a saturated run is
  not calibration evidence (§K1);
- **O-D gravity fit** deviance and 41-degree-of-freedom residual check from §1,
  with the separate 2020 margin-vintage conflict propagated as validation-only.

### 6. The calibrated chain and its comparison

Produce the calibrated 2010→2025 chain and compare it to PRD 0007's baseline on
**held-out** targets only. Commit evidence under
`docs/evidence/australian-population/calibrated-<date>/`: per-year θ̂, posterior
summaries, contraction per parameter, the held-out comparison against baseline,
and the residual O–D detail.

State plainly that every year is O-D-fitted, report gravity residuals, and name
parameters that failed to contract. If the calibrated chain does not beat the
baseline on held-out
targets, report that result — it is a finding about identifiability under §N11's
forward walk, not a failure to be tuned away.

### 7. Documentation — `docs/guides/australian-population-calibration.md`

The two-stage design and why the offline fit comes first, the fixed/free split,
the θ-file contract, the per-year loop, every diagnostic and its failure
condition, the O-D/margin vintage caveat, and the compounding-error warning from
§N11 with the rolling-origin pointer to PRD 0009.

## Allowed files

- `data/abs/gravity_fit.py`, `data/abs/theta.py`, `data/abs/calibrate.py`,
  `data/abs/tests/**` (new)
- `scripts/calibrate-australian-population.sh` (new)
- `docs/guides/australian-population-calibration.md` (new),
  `docs/models/australian-population.md` (link only)
- `docs/evidence/australian-population/**` (new)
- `data/abs/params/**` (fitted values written into free slots)
- implementation notes/artifacts created by the managed run

## Non-goals

- No modification of `calibration/npe`, its lock, or its quarantine.
- No fitting of fertility, mortality or entry rates (§N6).
- No joint posterior over the whole 2010–2025 path (§N11).
- No model, IR, runtime or CLI change; the harness composes existing commands.
- No third-party Python dependency in `data/abs/`.
- No tuning of a diagnostic threshold to obtain a pass.

## Acceptance criteria

1. Full check battery passes; `git diff --check` passes.
2. The offline gravity fit uses all 56 published O-D cells, reports cell
   residuals and deviance against 41 residual degrees of freedom, preserves the
   separate 2020 margin conflict, and measures its standalone reduction of
   baseline drift before any NPE work.
3. θ is exactly the seventeen free parameters from `priors.json`; no fixed
   parameter varies in any sweep, proven by test.
4. The per-year NPE loop runs run years 2010–2024 and produces posteriors, or records an
   honest `unanswered` status with its dependency reason.
5. Every diagnostic in §5 runs and can fail; parameters that fail to contract
   are named, not reported as fitted. `peak` and `k` are fitted against both
   stock age structure and normalized interstate age-sex compositions.
6. Every year is flagged as O-D-fitted, the gravity form is evaluated from its
   residuals, and the incompatible 2020 margin vintage remains validation-only.
7. The calibrated chain is compared to the baseline on held-out targets only,
   and the result is reported whichever way it comes out.
8. `calibration/npe` is byte-unchanged and contains no Australian-specific name.
