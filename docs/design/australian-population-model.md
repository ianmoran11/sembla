# Australian population model: scope and approach

Status: accepted; implemented by [`docs/prds-australian-population/`](../prds-australian-population/README.md)
Authority: `DECISIONS.md` §N. This document is a readable companion to that
section, not a competing specification; where they differ, §N wins.
Date: 2026-08-05
Input: [`docs/australian-population-use-case.md`](../australian-population-use-case.md) (read-only)
Builds on: [`docs/prds-demographic-slots/`](../prds-demographic-slots/README.md) (PRDs 0001–0008 landed)

A microsimulation of the Australian population from 2010 to 2025, with agents
carrying age and state of residence, moving between states over time, and with
the parameters governing those movements calibrated so that simulated flows and
stocks are consistent with published ABS data.

## Confirmed decisions

| # | Decision | Consequence |
|---|---|---|
| D1 | Geography is the 8 states/territories, with **genuine individual movement** | `area` becomes an enum; 56 origin→destination transitions |
| D2 | **Calibrate at 1:100** (220,287 present agents; 352,460 slots), **validate at 1:1** (35,245,914 slots) | Per-capita hazards are scale-invariant; small-cell noise in NT/ACT needs care |
| D3 | **Monthly ticks, chained annual runs, 2010–2025** | 12 ticks × 15 chained runs; per-year θ absorbs the COVID-era migration collapse |
| D4 | **Hybrid calibration**: fertility and mortality direct from ABS, NPE for migration structure and residual scaling | Keeps θ small and identifiable |
| D5 | **Minimal schema**: age, sex, state | Every calibration target is a published ERP cell |
| D6 | **In-repo Python pipeline**, mirroring `calibration/npe` conventions | Checksummed downloads, committed derived extracts, offline-reproducible builds |

Recorded as `DECISIONS.md` §§N1–N19, which is the binding authority for all ten
PRDs. D1 is N1, D2 is N8, D3 is N7, D4 is N6 and N11, D5 is N9, and D6 is N10.

## What already exists

The demographic track has landed PRDs 0001–0008, so this is an extension rather
than a new build:

- `frontend/Sembla/Models/DemographicSlots.lean` — a working fixed-pool slot
  model with monthly ageing, births, three-band mortality, overseas and internal
  migration, grouped views, and summaries.
- `sembla.state/v1` artifacts with `--export-state`, hash-linked through run
  manifests. Chained annual runs are the accepted architecture; there is no
  checkpoint subsystem and none is wanted (standing-no #6).
- `grouped-observations` runs on both CPU and CUDA, so cell-level calibration
  targets are not CPU-bound.
- The NPE reference pipeline consumes θ/summary pairs via
  `sembla sweep --export-pairs`.
- Common random numbers across parameter vectors, for paired counterfactuals.

## The core design problem: making agents move

Today `area : Area` is a **Ref that is never written**. `internal_arrive`
vacates a slot and activates a *different* pre-classified slot, so no agent
moves — the arrival is a new agent with a preclassified age (§K9).

A genuine move cannot be expressed by assigning the Ref. The expression language
has no row-literal: the only Ref-valued expression is `SelfAttr` reading an
existing Ref column, so a destination can never be *chosen*. Extending the IR
with a row-literal would not help either, because expressions are deterministic
and first-order — the only randomness is in hazard firing times.

**The route that works, with no framework changes:**

- `area` becomes an enum `{nsw, vic, qld, sa, wa, tas, nt, act}`. Enum writes
  need no resource claim (only Ref writes do — `validate.rs:748`).
- One transition per ordered origin→destination pair, guard `area = <origin>`,
  effect `set area := <destination>`.
- Every move, death and emigration declares `contest slot_resource by race_time`.
  Conflicts resolve by argmin over sampled firing times with a lexicographic
  tie-break (DESIGN §5.1), so **exactly one event happens per person per tick**
  and the destination is drawn by competing exponential clocks — which is
  precisely correct competing-risks multinomial choice, with each destination's
  probability proportional to its hazard.

Losers defer to the next tick and are counted per contested resource, so
saturation is a visible diagnostic rather than a silent bias.

### Why this caps geography at state level

Guards are evaluated per transition per row, so transition count is the binding
cost. Origin→destination pairs grow as A²:

| Geography | Areas | O–D transitions | Verdict |
|---|---:|---:|---|
| States/territories | 8 | 56 | Chosen |
| GCCSA | ~35 | 1,190 | Doubtful |
| SA4 | 107 | 11,342 | Infeasible |
| SA2 | 2,473 | 6.1M | Impossible |

An origin-independent variant (depart to a `moving` state, then 8 arrival
transitions) costs only ~9 transitions and would scale further, at the cost of
assuming destination choice is independent of origin. Held in reserve.

### What this design forecloses

Two limitations follow directly and must be stated rather than discovered later:

1. **No population-dependent hazards keyed on area.** `freq (pred) over <ref>`
   and IR aggregates join only on declared **Ref** keys. Once `area` is an enum,
   no hazard can reference "the population of my state". Rates are therefore
   exogenous — standard for a cohort-component model, but it rules out
   endogenous crowding or agglomeration feedback, and it means fertility remains
   an aggregate birth-slot activation rather than a rate applied to resident
   women aged 15–49 (the §K10 caveat stands unchanged).
2. **No transcendental functions.** The expression language is Add/Sub/Mul/Div
   only — there is no `exp`, `log` or `pow`. Age profiles must be either
   piecewise-constant via guards, or rational functions.

## Model shape

Tables collapse relative to the current model, because internal moves no longer
consume slots:

- `PersonSlot`: `occupancy {vacant, present}`, `event`, `sex {male, female}`,
  `age_months : Int`, `event_age_months : Int`, `generation : Int`,
  `entry_stream {birth_slot, overseas_slot, retired_slot}`,
  `entry_age_months : Int`, `area` (enum, 8 variants), `prev_area` (9 variants),
  `slot_resource : SlotResource`. Initially present and exited rows are retired;
  pre-classified birth and overseas rows are single-use.
- `SlotResource`: empty schema, one exclusive row per slot.
- The `Area` table is dropped; `internal_slot` disappears from `entry_stream`.

Origin→destination flows are observed through the accepted `prev_area` marker
(9 variants including `none_`) grouped with `area`. Encoding origin into the
event enum was rejected because it overloads event meaning and complicates every
flow view.

### Parameterisation

Seventeen free migration parameters, kept low-dimensional deliberately — a free
56-cell O–D matrix per year would not calibrate. ABS-derived mortality,
fertility and entry rates remain fixed during inference.

- **Mortality**: five-year age bands × state × sex, hazards fixed from annual
  ABS age-specific death rates. Overlapping three-year life-table `qx`
  snapshots validate the level and age shape but are not relabelled as annual
  inputs. Band constants remain symbolic model parameters held fixed during
  inference, so they cost nothing in θ dimension.
- **Fertility**: per-state birth-slot activation rates with ABS-derived defaults.
- **Migration**: hazard for o→d is
  `interstate_base · push_o · pull_d · ageProfile(age_months)`, with
  `ageProfile = 1 / (1 + k·(age_months − peak)²)`. Fixing both
  `push_nsw = 1` and `pull_nsw = 1` removes the two multiplicative scale
  invariances, giving 7 push + 7 pull + base + peak + k = 17 parameters for the
  entire 56-cell matrix.
- **Overseas**: arrival rates by state and an emigration rate, ~10 parameters.

## Calibration design

### Identifiability

Annual ERP stocks by state × age × sex pin down only the **net** effect of the
four components. In- and out-migration cannot be separated from stocks alone.
Published flow series must therefore be calibration targets alongside stocks:
births, deaths, overseas arrivals and departures, all 56 interstate O-D cells,
and normalized state-age-sex interstate-flow compositions.

### Per-year forward walk

Calibrate **one year at a time**, walking forward: given the state artifact at
30 June *t*, fit θ_t to the year-*t* flows and the *t+1* stocks, export the
final state, and proceed. This matches the chained-run architecture exactly,
keeps each NPE problem at 17 parameters, and handles year-specific shocks such
as the 2020–21 border closure naturally.

It is a filtering-style procedure, not a joint posterior over the whole path,
and errors compound across years. Rolling-origin validation is therefore not
optional.

### Targets per year

Stocks (8 states × 2 sexes × 21 age bands = 336 cells) plus births, deaths,
overseas flows, 56 interstate O-D cells and 512 interstate age-sex composition
cells exceed 1,000 numbers per year. This is a large NPE observation vector; a
reduction may use moments, but it must retain sensitivity to all seventeen free
parameters and preserve the age-composition evidence for `peak` and `k`.

### Reporting

Fitted controls and held-out evidence must be reported separately, per the
use-case doc's validation report: signed and absolute cell error, MAE/RMSE and
maximum error, error by population size, residual maps, and uncertainty
intervals across replicates. Correlation alone is not acceptable — large states
can carry a high correlation while NT and ACT are poor.

## Data

All series are annual and, at state level, published by single year of age and
sex — a better joint than the SA2 route, which is only five-year bands.

| Series | Vintage as at 2026-08-05 | Role |
|---|---|---|
| National, state and territory population | Quarterly to **Dec 2025**; June 2026 due 17 Dec 2026 | Primary stock control; interstate arrivals/departures by state |
| Regional population by age and sex | **30 June 2024**; 2025 edition due **27 Aug 2026** | Sub-state age × sex detail, initialisation cross-check |
| Overseas Migration | **2024–25**, released Dec 2025 | Overseas arrival/departure targets by age, sex, state |
| Births, Australia / Deaths, Australia | Annual | Fertility and mortality targets and diagnostics |

The fifteen run years 2010 to 2024 carry stocks through 30 June 2025, which is
the last published state-level ERP by single year of age and sex; 2025–26 is
projection. Sub-state data ends at 30 June 2024 but does not bind, because D1
puts sub-state geography out of scope. Published components will not reconcile exactly to ERP,
because ERP is itself modelled, confidentialised, constrained and revised.

## Scale arithmetic

The slot pool must cover peak population plus every entry over the window.
Genuine movement means internal moves no longer burn slots, saving roughly 6M.

- Full scale: 22.03M present + 5.01M birth slots + 8.21M overseas slots =
  **35.25M slots** including 10% headroom per entry stream. At ~64–80 B/slot
  double-buffered this is ~4.5–5.6 GB steady state and fits a 24 GB L4.
- Calibration scale (1:100): **352,460 slots** — cheap enough for repeated NPE
  simulations.

## Principal risks

| Risk | Mitigation |
|---|---|
| 418 transitions × 180 ticks × full pool is too slow | Calibrate at 1:100; benchmark before committing to 1:1; origin-independent variant held in reserve |
| Observation vector too wide for NPE | Spike margin/moment reduction and embeddings before the harness PRD |
| Per-year calibration compounds error | Rolling-origin validation and Census-anchored backtest |
| Aggregate births are not a fertility model | Recorded as a known limitation; endogenous fertility is foreclosed by the enum choice |
| ABS revisions and rebasing change targets underneath the model | Pin vintages by checksum; retain as-published and latest-rebased series separately |

## Proposed PRD folder

`docs/prds-australian-population/`, run with `/piprd run`:

| PRD | Scope |
|---|---|
| 0001 | Decision record and folder README — freeze D1–D6, the movement design, and both foreclosures; append DECISIONS.md §L |
| 0002 | ABS acquisition and normalisation — checksummed downloads, canonical tidy extracts, offline fixture path |
| 0003 | 2010 initial-state builder — deterministic `sembla.state/v1` at 1:1, 1:10, 1:100; exact agreement with published June-2010 cells |
| 0004 | Enum `area` and genuine interstate movement — Lean-generated 56 transitions, competing-clock destination choice, slot-accounting invariants |
| 0005 | ABS-derived fertility and mortality — annual banded death rates by state and sex, life-table validation, fixed parameters |
| 0006 | Observation set and targets contract — grouped views for stocks and O–D flows, annual summaries, targets file format |
| 0007 | Chained annual run driver — 2010→2025, per-year θ, manifest chain verification, determinism |
| 0008 | Calibration harness — direct-rate initialisation, per-year NPE, priors, diagnostics |
| 0009 | Validation and reporting — fitted vs held-out, rolling-origin backtest, explicit non-claims |
| 0010 | Full-scale run and benchmark evidence — 1:1 on GPU, memory and throughput measurement |

## Non-claims

A model that matches published state × age × sex controls is not thereby
realistic on households, income, employment, or any omitted relationship. It
carries no sub-state validity: agents move only between states, so any finer
geography is a static label that will drift. Exact fit to fitted controls is
reconstruction, not validation.
