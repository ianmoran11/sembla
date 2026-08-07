# PRD 0005: ABS-derived fertility, mortality and entry rates

## Context

Read `docs/prds-australian-population/README.md` first. `DECISIONS.md` §N6
(direct rates, held fixed during inference), §N7 (monthly ticks, per-year θ) and
§N3b (no transcendentals) bind. PRD 0004's model exists with placeholder rates;
this PRD replaces them with rates derived from PRD 0002's extracts.

§N6 now selects the annual ABS age-specific death-rate series rather than
relabelling overlapping three-year life-table `qx` snapshots as annual data.
The source already uses the model's five-year bands. Life tables remain an
independent validation of mortality level and shape.

## Goal

`data/abs/rates.py` derives per-year parameter files from ABS annual mortality
rates, births, deaths and overseas migration; life-table snapshots validate the
mortality conversion independently. The Lean model carries ABS-derived 2010
defaults with priors centred on them, and an uncalibrated 2010 run reproduces
published deaths and births closely enough to demonstrate that the direct-rate
half of §N6 works before any inference is attempted.

## Specification

### 1. Rate conversion — `data/abs/rates.py`

For run year *y*, use calendar registration-year *y* for births and mortality
rates. This is a named alignment convention, not a claim that calendar
registrations equal the financial-year occurrence component. Keep both series
in `rates.md` and report their difference; never scale the direct rates to make
natural increase close.

- **Mortality.** ABS publishes the central age-specific death rate as deaths
  per 1,000 population per year. Convert it to the model's monthly constant
  hazard with `h = rate_per_1000 / 1000 / 12`. The rates are already published
  by state, sex, year and the exact non-overlapping five-year bands used by the
  model, so do not re-average them. Two cells are explicitly absent because ABS
  publishes zero exposure: NT female 100+ in 2010 and 2011. For those two cells
  only, use the same-year Australian female 100+ rate from
  `mortality_rates_national_age_sex.csv`, mark the parameter provenance as
  `national_zero_exposure_fallback`, and test that no other fallback occurs.
- **Fertility.** Monthly birth-slot activation rate for state *s* in year *y*:
  published births divided by twelve, divided by the *expected vacant birth
  slots in state s during year y*. That denominator changes as the pool depletes,
  so the rate is genuinely year-specific and must be derived per year against the
  projected vacancy path, not once.
- **Overseas.** Arrival rate per state as NOM arrivals over expected vacant
  overseas slots; emigration rate per state as NOM departures over present
  population. Both per year.

### 2. The fixed-pool wart, documented not hidden

A constant activation rate against a depleting vacant pool yields a declining
entry flow. This is an artefact of the fixed-pool architecture (§K1), not
demography. Two consequences must be written into
`docs/models/australian-population.md`:

- entry rates are re-derived every year, and are therefore **not** comparable
  across years as behavioural quantities — they are pool-relative activation
  rates;
- if the vacancy margin ever approaches zero the entry flow is suppressed
  regardless of θ, which is why PRD 0008's saturation diagnostic is a
  correctness check and not a nicety.

### 3. Per-year parameter files

Emit `data/abs/params/<year>.json` in the format `sembla run --params` and
`sembla sweep --params` already accept (read the CLI and an existing params
fixture first; do not invent a shape). One file per run year 2010–2024, canonical
JSON, deterministic key order. Each file carries every parameter the model
declares, so a run is fully specified by `(model, state artifact, params file,
seed, ticks)`.

Emit alongside it `data/abs/params/priors.json` recording, for each parameter,
whether it is **fixed** (mortality bands, fertility, entry rates — §N6) or
**free** (the seventeen migration parameters — §N5). If the two NSW reference
factors are declared parameters rather than literals, classify them as
`fixed_normalization`, not free. PRD 0008 reads this to build its θ vector;
nothing else decides what is free.

Within the free set, record how each parameter is identified: `interstate_base`,
`push_o` and `pull_d` from published interstate margins, and `peak` and `k` only
through the age structure of simulated stocks (§N5). PRD 0008's offline fit uses
the first group and must leave the second alone, so the distinction has to be
machine-readable rather than described in prose.

### 4. Model defaults and priors

Update `AustralianPopulation.lean`'s parameter defaults to the 2010 values, with
`LogNormal` priors centred on each default and a documented sigma. The model
remains runnable with no params file, and that default run is the 2010 year.
Parameter values stay symbolic in the IR (§4.2) — nothing is inlined.

### 5. Uncalibrated fidelity check — the acceptance core

A one-year run (12 ticks) from the 2010 state artifact at `hundredth` scale with
`params/2010.json` and no calibration must reproduce published 2010 outcomes:

- national deaths within a stated tolerance of published deaths;
- deaths by state and by five-year age band, reported with signed error;
- national births within a stated tolerance;
- overseas arrivals and departures by state, reported.

Choose the tolerances from the observed Monte Carlo spread across several seeds
at `hundredth` scale, and record both the tolerance and the spread. A tolerance
picked to make the test pass, rather than from the measured spread, is a failed
PRD. If the check fails materially, report it and stop — do not tune the rates
to fit, because that would silently convert direct estimation into calibration
and destroy §N6's separation.

### 6. Rates report — `data/abs/extracts/rates.md`

The conversion formulas, the two-cell mortality fallback, the per-year
fertility denominator derivation, the fixed/free parameter split, the fidelity-check
results with tolerances and measured spread, and the pool-relative caveat from
§2.

## Allowed files

- `data/abs/rates.py`, `data/abs/params/**`, `data/abs/tests/**` (new)
- `data/abs/extracts/rates.md` (new)
- `frontend/Sembla/Models/AustralianPopulation.lean` (parameter defaults and
  priors only — no structural change)
- `fixtures/australian-population/**` (regenerated goldens, each listed with a
  reason in the implementation notes)
- `fixtures/state/australian_population_2010_hundredth.state.model.json`
  (parameter/prior-only companion regeneration; the `.state` binary must remain
  byte-identical)
- `crates/sembla-cli/tests/**` (fidelity check)
- `docs/models/australian-population.md` (extend), `scripts/check-abs-data.sh`
  (extend)
- implementation notes/artifacts created by the managed run

## Non-goals

- No calibration, sweep, posterior or targets work — PRDs 0006 and 0008.
- No structural model change: no new transition, table, view or attribute.
- No tuning of ABS-derived rates to improve the fidelity check.
- No transcendental function anywhere in the model or IR.
- No third-party Python dependency.

## Acceptance criteria

1. Full check battery, negative suite, parity and `scripts/check-abs-data.sh`
   pass; `git diff --check` passes.
2. Per-year parameter files exist for run years 2010–2024 in the shape the CLI already
   accepts, are canonical, and regenerate byte identically.
3. `priors.json` classifies every declared parameter as fixed or free, and the
   free set is exactly the seventeen migration parameters of §N5.
4. Mortality hazards equal the published rate divided by 12,000, proven by a
   hand-computed fixture. Exactly the two documented NT female 100+ cells use
   the same-year national fallback, and no missing rate is treated as zero.
5. The uncalibrated 2010 fidelity check passes with tolerances derived from
   measured multi-seed spread, and both are recorded.
6. `rates.md` documents the pool-relative entry-rate caveat, and
   `docs/models/australian-population.md` carries the fixed-pool wart from §2.
7. All regenerated goldens are listed with reasons; nothing pre-existing changed.

## Implementation evidence

- `data/abs/rates.py` emits all fifteen flat 377-parameter maps, the exhaustive
  360-fixed/17-free `priors.json` registry, and `extracts/rates.md` using only
  the Python standard library and committed extracts. Two consecutive runs are
  byte-identical under `scripts/check-abs-data.sh`.
- Entry hazards use the user-selected exact exponential convention
  `-log1p(-E / V) / 12` against the full-scale projected vacancy path. The final
  projected vacancies equal the independently frozen ten-percent headroom:
  455,278 birth slots and 746,288 overseas slots.
- Every one of the 5,040 annual state×sex×band mortality mappings is test-checked
  against its published rate divided by 12,000. Exactly NT female 100+ in 2010
  and 2011 use the same-year Australian rates, 433.9 and 450.0 per 1,000.
- The Lean model's 2010 defaults equal `params/2010.json` exactly. Positive
  direct defaults use median-centred LogNormal priors with spread 0.5; the seven
  published 2010 zero mortality cells remain exact zero and use the selected
  centred Normal exception. All direct transition hazards remain symbolic
  `Param` expressions.
- The implementation workflow wrote
  `params/fidelity-2010-predeclaration.json` before its first simulation. Its
  exact pilot seeds 1001--1010, held-out seed 2001, and
  `ceil(3 × sample SD)` rule have SHA-256
  `87f442ea1b97b90f718e0bf4205497aad03c2d24de0aa7470a09e160f391e18b`.
  The declaration and results remain in the same uncommitted implementation
  series, so Git history alone does not independently timestamp their order;
  the implementation-session workflow transcript is the temporal evidence.
  Those pilots measured full-scale-equivalent sample standard deviations of
  5,787.496 births and 2,633.565 deaths. The frozen tolerances are 17,363 and
  7,901. Held-out seed 2001
  produced 305,000 births (error +1,701 against 303,299) and 146,500 deaths
  (error +3,049 against 143,451), passing both gates without rate adjustment.
  State, age-band and overseas-flow signed errors are committed in
  `params/fidelity-2010.json` and rendered in `rates.md`.
- Replacing placeholder defaults regenerated the model, plan, validation-safe
  state companion, and every file listed in the golden README. The initial
  `.state` bytes and hashes remain unchanged; the new 24-tick hashes are
  `a2180de…` (results), `4dc8c375…` (final state), and `1719b285…`
  (observations).

### Regeneration inventory

Every regenerated pre-existing artifact and its reason is explicit:

- `fixtures/australian-population/australian_population.hundredth.json` — 360
  placeholder direct defaults/priors replaced by the ABS-derived 2010 values;
  schema, observations and transitions unchanged.
- `fixtures/australian-population/australian_population.hundredth.plan.json` —
  the same parameter/default metadata update in the executable plan; all rule
  expressions remain symbolic and structurally unchanged.
- `fixtures/state/australian_population_2010_hundredth.state.model.json` — the
  validation-safe companion received the same parameter/default metadata while
  retaining its empty feature-gated grouped-view list. The adjacent `.state`
  binary remains byte-identical at SHA-256
  `1d3f85db8fd93c66118df15622c70eac4fd6dfc1adcc72c9142b5949146eff5f`.
- `fixtures/australian-population/goldens/run.csv`,
  `run.csv.summaries.csv`, `run.hashes.txt`,
  `run.grouped.births_cells.csv`, `run.grouped.deaths_cells.csv`,
  `run.grouped.interstate_flows.csv`,
  `run.grouped.overseas_arrival_cells.csv`,
  `run.grouped.overseas_departure_cells.csv`,
  `run.grouped.population_cells.csv`, and `run.grouped.vacancy_cells.csv` —
  regenerated because the deterministic seed-8305 default run now uses the
  ABS-derived 2010 direct rates. The seed, ticks, schema, transitions and state
  artifact are unchanged.
