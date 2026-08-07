# Australian population model PRDs

Ordered PRD set building an ABS-calibrated microsimulation of the Australian
population from 2010 to 2025: agents carrying age, sex and state of residence,
**genuinely moving between states**, with the parameters governing those moves
calibrated so simulated stocks and flows are consistent with published ABS data.

Scoped in [`docs/design/australian-population-model.md`](../design/australian-population-model.md),
which this folder implements. Run from the Sembla repository with:

```text
/piprd run docs/prds-australian-population
```

`README.md` is ignored by `/piprd run`. Every numbered PRD must read this file
first; the constraints below are binding. When a PRD conflicts with this README,
this README wins.

## Authority and scope

- `DESIGN.md` (especially §4.2 kernel fragment, §4.6 observation-as-sink, §5.1
  conflict argmin, §5.4 manifest rules, §5.5 default-off flags, §10.5
  population generation is external), `DECISIONS.md` (§§E3, E7, E8, G5, I1–I6,
  J, K, M) and the demographic-slots folder's frozen contracts all bind.
- The demographic-slots track (PRDs 0001–0008, landed) is the **substrate**:
  `sembla.state/v1`, `--export-state`, chained annual runs, the
  `grouped-observations` flag, `contest … by race_time`, arithmetic set
  effects and Int params all exist and are reused unchanged.
- `docs/design/australian-population-model.md` and
  `docs/australian-population-use-case.md` are **input, not authority**. Where
  this README deviates, the deviation is a recorded decision (§N), not drift.
- Frozen and untouchable: `examples/**`, all CSV/hash goldens, plan/source
  schemas and version strings, `SEMBLA_POP` and every SIR legacy path,
  `sembla.state/v1` and its hash domain, the negative-suite expectations,
  Philox layout, conflict argmin semantics, CUDA numeric contracts, and the
  entire `demographic_slots` model with its fixtures and goldens.

### This folder adds no new surface syntax and no IR change

Every construct the model needs already exists: enum attributes and guards,
arithmetic set effects, `contest … by race_time`, grouped views with band keys,
summaries, and `LogNormal` priors. **A PRD that finds itself needing new syntax,
a new `Expr` variant, or a new feature flag has misread this README** — stop and
raise it rather than extending the language.

## The two accepted foreclosures

Both follow from making `area` an enum, and both must be stated in
documentation rather than discovered by a reader later:

1. **No population-dependent hazards keyed on area.** `freq (pred) over <ref>`
   and IR aggregates (`Agg { on: AggJoin }`) join only on declared **Ref** keys.
   With `area` as an enum no hazard can reference "the population of my state".
   Rates are exogenous — correct for a cohort-component model, but it rules out
   crowding/agglomeration feedback, and fertility stays an aggregate
   birth-slot activation rather than a rate on resident women aged 15–49
   (§K10's caveat carries over unchanged).
2. **No transcendental functions.** The expression language is Add/Sub/Mul/Div
   only. Age profiles are piecewise-constant via guards, or rational functions.

## Why `area` is an enum (the load-bearing design fact)

The expression language has no row-literal: the only Ref-valued expression is
`SelfAttr` reading an existing Ref column, so a Ref destination can never be
*chosen*. Extending the IR would not help either — expressions are deterministic
and first-order, so the randomness that selects a destination has to come from
hazard firing times.

Therefore movement is: `area` is an enum; one transition per ordered
origin→destination pair; every move, death and emigration declares
`contest slot_resource by race_time`. DESIGN §5.1's argmin over sampled firing
times then guarantees **exactly one event per person per tick** and draws the
destination by competing exponential clocks — correct competing-risks
multinomial choice, with each destination's probability proportional to its
hazard. Losers defer and are counted per contested resource, so saturation stays
a visible diagnostic.

This caps geography at 8 states (56 pairs). GCCSA (1,190), SA4 (11,342) and SA2
(6.1M) are out of reach and are not attempted.

## Frozen names and version strings

| Concern | String |
|---|---|
| Canonical model | `australian_population` (box `demographic`) |
| Model module | `frontend/Sembla/Models/AustralianPopulation.lean` |
| Geography enum | `area : {nsw, vic, qld, sa, wa, tas, nt, act}` |
| Origin marker | `prev_area : {none_, nsw, vic, qld, sa, wa, tas, nt, act}` |
| Data pipeline root | `data/abs/` (Python, standard library only) |
| Raw download cache | `data/abs/cache/` (gitignored, checksummed) |
| Normalised extracts | `data/abs/extracts/*.csv` (committed) |
| Targets artifact | `sembla.targets/v1` (canonical JSON) |
| State artifacts | `sembla.state/v1` (unchanged, written from Python) |
| Scale identifiers | `full`, `tenth`, `hundredth` |
| Chain driver | `scripts/run-australian-population.sh` |
| Model documentation | `docs/models/australian-population.md` |
| Evidence root | `docs/evidence/australian-population/` |

## Frozen model shape

Set by PRD 0004 and extended only as later PRDs state:

- `PersonSlot`: `occupancy {vacant, present}`,
  `event {none_, birth, death, overseas_arrival, overseas_departure,
  interstate_move}`, `sex {male, female}`, `age_months : Int`,
  `event_age_months : Int`, `generation : Int`,
  `entry_stream {birth_slot, overseas_slot, retired_slot}`,
  `entry_age_months : Int`, `area` (8 variants), `prev_area` (9 variants),
  `slot_resource : SlotResource`.
- `SlotResource`: empty schema, one exclusive row per person slot.
- There is **no `Area` table** and **no `internal_slot` entry stream**. Internal
  moves no longer consume slots, which is what makes full scale fit in memory.
  `retired_slot` is never activated: initially present rows use it, and death or
  emigration writes it before vacating a row. Birth and overseas slots are thus
  single-use, preserving their build-time ABS composition.

`prev_area` is the frozen mechanism for observing origin→destination flows
(`count PersonSlot by prev_area, area where event = interstate_move`). Encoding
the origin into the `event` enum was considered and rejected: it saves one
column but overloads `event` and makes every flow view harder to read.

## Parameterisation is deliberately low-dimensional

A free 56-cell O–D matrix per year would not calibrate. Frozen structure:

```text
hazard(o → d) = interstate_base · push_o · pull_d · ageProfile(age_months)
ageProfile(a) = 1 / (1 + k · (a − peak) · (a − peak))
```

with `push_nsw ≡ 1` and `pull_nsw ≡ 1` for identifiability. That is 7 push +
7 pull + base + peak + k = 17 parameters for the entire matrix. Mortality band
constants come from annual ABS age-specific death rates (with life-table
snapshots used only for validation); mortality and fertility stay **fixed**
during inference, so they cost nothing in θ dimension. Total θ is 17.

## Calibration is a per-year forward walk

Given the state artifact at 30 June *t*, fit θ_t against year-*t* flows and
*t+1* stocks, export the final state, advance. This matches the chained-run
architecture, keeps each NPE problem at 17 parameters, and absorbs
year-specific shocks such as the 2020–21 border closure.

It is a filtering-style procedure, **not** a joint posterior over the whole
path, and errors compound across years. Every PRD that reports fit must also
report rolling-origin validation, and must keep fitted controls separate from
held-out evidence.

## Identifiability: stocks alone are not enough

Annual ERP stocks by state × age × sex pin down only the **net** effect of the
four components; in- and out-migration are not separable from stocks. Published
flow series (births, deaths, overseas arrivals/departures, interstate
arrivals/departures) are therefore mandatory calibration targets, not optional
extras. A PRD that calibrates against stocks only has failed.

## The Python pipeline is quarantined and standard-library only

`data/abs/` follows `calibration/npe`'s quarantine rule (§G5): it never imports
a Sembla crate, Rust library, model parser or runtime API. Unlike the NPE
pipeline it takes **no third-party dependencies at all** — `urllib`, `csv`,
`json`, `zipfile`, `hashlib` are sufficient, and this deliberately avoids
creating a second hash-locked requirements contract. A PRD that wants pandas has
misread this README.

Raw ABS downloads are cached and SHA-256 pinned; normalised extracts are small
(a few hundred KB) and committed, so every downstream build is offline and
byte-reproducible. Re-downloading must be an explicit opt-in flag, never a
build-time side effect.

## Data vintages as at 2026-08-05

| Series | Vintage | Role |
|---|---|---|
| National, state and territory population | Quarterly to Dec 2025 | Stock control; interstate arrivals/departures by state |
| Regional population by age and sex | 30 June 2024 (2025 edition due 27 Aug 2026) | Sub-state age × sex cross-check |
| Overseas Migration | 2024–25 | Overseas arrival/departure targets by age, sex, state |
| Births / Deaths, Australia | Annual | Fertility and mortality targets and diagnostics |

The window is the fifteen run years 2010 to 2024, carrying stocks from
30 June 2010 through 30 June 2025 — the last 30 June with published state-level
ERP by single year of age and sex. Later years are projection. Sub-state data
ends at 30 June 2024, which does not bind because N1 puts sub-state geography
out of scope. Published components will not reconcile exactly to ERP because
ERP is itself modelled, confidentialised, constrained and revised — PRDs must
report the discrepancy, never silently force it.

## Required checks for every PRD

```bash
./scripts/check.sh
cd frontend && lake build
bash frontend/scripts/check-parity.sh
git diff --check
```

PRDs touching the model (0004, 0005, 0006) must also run
`bash frontend/scripts/test-negative.sh`. PRDs adding documentation must run
`python3 scripts/check-markdown-links.py`. Any diff to a frozen artifact is a
failed PRD.

## Run order

| PRD | Why it is here |
|---|---|
| 0001 | Freeze every decision into `DECISIONS.md` §N before code exists |
| 0002 | ABS acquisition and normalisation — nothing downstream is testable without real extracts |
| 0004A | Export the enum-area model schema needed to type state artifacts |
| 0003 | 2010 initial state at three scales; needs 0002 and 0004A |
| 0004B | Complete movement invariants and goldens against the 0003 fixture |
| 0005 | ABS-derived fertility and mortality replacing placeholder rates |
| 0006 | Observation set and the `sembla.targets/v1` contract |
| 0007 | Chained annual driver, 2010→2025, per-year θ |
| 0008 | Calibration harness — direct rates, then per-year NPE |
| 0009 | Validation, rolling-origin backtest, and the non-claims |
| 0010 | Full-scale 1:1 run and benchmark evidence |

PRD 0004 is deliberately executed in two phases to break the artifact/model
cycle: schema and standalone export first, PRD 0003 artifacts second, then PRD
0004 invariants, registration and goldens. Scale-specific state companions
change the two table `size_hint` values and omit only feature-gated grouped
views so the existing public `sembla validate` command accepts them. Canonical
execution uses the adjacent feature-bearing plan; tests prove identical schema,
parameters, transitions and scalar views.

## Global non-goals

- No new surface syntax, no new `Expr` variant, no new feature flag, no IR
  change, no CUDA kernel change, no linker or plan-schema change.
- No row-literal, Ref reassignment, categorical draw, cross-row write,
  mother-linked birth, or household ref — every §K deferral stands with its
  trigger unchanged.
- No sub-state geographic dynamics. SA2/SA4 are not modelled and are not
  carried as attributes in this folder.
- No household, income, labour-force, education, visa or country-of-birth
  attributes (schema is age, sex, state).
- No endogenous fertility — foreclosed above, not deferred.
- No joint-path posterior; per-year forward walk only.
- No Census synthesis or reweighting.
- No new Rust, Lean or Python dependencies.
- No edits to `.piprd/`, CI workflow semantics, `examples/**`, the
  `demographic_slots` model, or any pre-existing golden.
