# PRD 0009: Validation, rolling-origin backtest, and the non-claims

## Context

Read `docs/prds-australian-population/README.md` first. `DECISIONS.md` §N11
(errors compound; rolling-origin validation is mandatory) and §N13 (reporting
discipline and the non-claims) bind, as does the required validation report in
`docs/australian-population-use-case.md`.

PRD 0008 produced a calibrated chain and reported its fit. Fit to fitted
controls is reconstruction, not validation. This PRD is where the model is asked
whether it predicts anything it was not shown, and where the limits are written
down in a form a reader cannot miss.

The per-year forward walk is a filtering-style procedure. Its characteristic
failure is that each year looks fine while the fifteen-year path drifts. Only a
rolling-origin backtest exposes that.

## Goal

A validation report that separates fitted from held-out evidence, quantifies
error growth with projection horizon across several origins, tests the
Census-boundary case explicitly, and states the model's non-claims plainly
enough that they survive being quoted out of context.

## Specification

### 1. Rolling-origin backtest — `data/abs/backtest.py`

For each origin year in at least {2013, 2016, 2019, 2021}: take the calibrated
chain's state at that origin, freeze θ at its fitted value, and project forward
without any further fitting for horizons of 1, 2, 3 and 5 years. Score each
projection against held-out targets.

Report error as a function of horizon, per target family and per state. The
headline quantity is **error growth with horizon**, not error at horizon one —
a model that is excellent one year out and useless five years out is a specific,
reportable finding.

Include the 2019 and 2021 origins deliberately: projecting across the 2020–21
border closure with pre-COVID parameters should fail badly, and a backtest that
does *not* show that failure indicates the harness is leaking future information
into the projection. Treat an implausibly good COVID-era projection as a bug to
investigate before it is reported.

### 2. Census-boundary test

ERP is rebased at each Census, so 2011→2016 and 2016→2021 projections cross a
discontinuity in the target series itself. Run both, and report the simulated
path against **both** vintages where PRD 0002 obtained them: as-published-at-the-
time and latest-rebased. Where only one vintage was obtained, say so and scope
the claim accordingly.

Attribute the error honestly between model error and rebasing, and state that
the two cannot be fully separated from published data alone.

### 3. The validation report — `docs/evidence/australian-population/validation-<date>/`

Implement the use-case note's required report. At minimum:

- signed and absolute cell error; MAE, RMSE and maximum error;
- total absolute error halved, or percentage classification error;
- relative error reported only above a documented minimum denominator, because
  relative error on a cell of 3 people is noise;
- a distributional divergence with documented smoothing for zero cells;
- residual detail by state and error by population size;
- logical and referential-integrity failures;
- structural versus sampling zeros distinguished;
- held-out results reported separately from fitted, with the split taken from
  the targets artifact rather than re-derived;
- uncertainty intervals across replicate seeds and, where PRD 0008 produced
  them, across posterior draws.

Correlation may appear but never as the headline: large states can carry a high
correlation while NT and ACT are poor, and the report must show that case
explicitly if it occurs.

### 4. Scale-effect check

The model is calibrated at `hundredth` scale (§N8). Quantify what that costs:
run the same calibrated θ at `hundredth` and `tenth` scale over a common window
and compare held-out error. Report whether conclusions are scale-stable. If they
are not, that materially qualifies PRD 0008's results and must be stated there
as well as here.

### 5. The non-claims — `docs/models/australian-population.md`

A prominent section, in plain language, stating that this model:

- has **no sub-state validity** — agents move only between states, and no finer
  geography is modelled or carried;
- says nothing about households, income, employment, education, disability,
  visa status or country of birth;
- has **no endogenous fertility** and no crowding or agglomeration feedback,
  both foreclosed by §N3a;
- produces entry rates that are pool-relative artefacts of the fixed-slot
  architecture, not behavioural rates comparable across years (PRD 0005 §2);
- reproduces ERP, which is itself a modelled, confidentialised, constrained and
  revised estimate — agreement with ERP is not agreement with truth;
- is calibrated by a per-year forward walk whose errors compound, with the
  measured compounding from §1 quoted directly.

Write it so each bullet stands alone when quoted. A reader who reads only this
section must not come away over-confident.

### 6. Summary for the design document

Append a results section to `docs/design/australian-population-model.md`
recording what was achieved against what was scoped, including any risk from its
risk table that materialised.

## Allowed files

- `data/abs/backtest.py`, `data/abs/report.py`, `data/abs/tests/**` (new)
- `docs/evidence/australian-population/**` (new)
- `docs/models/australian-population.md` (non-claims section)
- `docs/design/australian-population-model.md` (results section)
- `scripts/check-abs-data.sh` (extend)
- implementation notes/artifacts created by the managed run

## Non-goals

- No refitting, retuning or parameter adjustment in response to a validation
  result — that would convert held-out evidence into fitted evidence.
- No model, IR, runtime or CLI change.
- No full-scale run — PRD 0010.
- No new claim beyond what the evidence supports.

## Acceptance criteria

1. Full check battery passes; `git diff --check` passes.
2. Rolling-origin backtests run from at least four origins at horizons 1, 2, 3
   and 5, and error growth with horizon is reported per family and per state.
3. The 2019 and 2021 origins show the expected COVID-era projection failure; an
   implausibly good result is investigated and explained rather than reported.
4. Both Census-boundary projections run, reported against every ERP vintage
   obtained, with the rebasing caveat stated.
5. The validation report contains every element in §3, with held-out results
   separated from fitted using the targets artifact's own split.
6. The scale-effect check is reported, and any scale instability is reflected
   back into PRD 0008's documentation.
7. The non-claims section exists, is quotable bullet by bullet, and includes the
   measured error compounding.
8. No parameter value changed anywhere in the repository as a result of running
   this PRD.
