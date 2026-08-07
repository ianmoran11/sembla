# PRD 0006: The observation set and the `sembla.targets/v1` contract

## Context

Read `docs/prds-australian-population/README.md` first. `DECISIONS.md` §N11
(stocks alone are insufficient), §N12 (targets are a versioned artifact) and
§N13 (fitted and held-out must be distinguishable) bind.

PRD 0004 emits views, grouped views and summaries. PRD 0002 emits ABS extracts.
Nothing yet connects a model observation to the published number it is supposed
to match, and doing that connection ad hoc inside the calibration harness is how
a fitted cell ends up quoted as validation evidence. This PRD makes the mapping
an artifact with a hash.

Observation remains a sink (§4.6): targets are read *from* run output and never
influence execution.

## Goal

`sembla.targets/v1` exists as a versioned, hashed artifact mapping every
calibration target to a named model observation, an ABS source series with its
vintage, and a fitted-or-held-out role; a scorer turns a run's output plus a
targets file into a residual report.

## Specification

### 1. The artifact — `sembla.targets/v1`

Canonical JSON, SHA-256 over exact file bytes with hash domain
`sembla.targets/v1`, mirroring the state-artifact hashing convention. Contents:

- a header: format string, model name, geography enum version, scale, the
  covered period, and the `sources.json` digest it was built from;
- an ordered list of targets, each carrying:
  - `observation`: the model observation name, its kind (`view`, `grouped_view`
    or `summary`), and for grouped views the exact key tuple in the same
    rendering the grouped CSV uses (enum variant names, band indices);
  - `tick` or `period` identifying when it is read;
  - `value`, and where the source publishes one, an uncertainty;
  - `source`: ABS series `id`, reference period and release date from
    `sources.json`;
  - `role`: `fitted` or `heldout`;
  - `aggregation`: how the model quantity is reduced before comparison.

A target naming an observation the model does not declare is a hard error, not a
skipped row. Validate the artifact against the exported model JSON.

### 2. Builder — `data/abs/targets.py`

Builds one artifact per calibration year from the committed extracts. Frozen
target set per year:

- **Stocks** at the year boundary: `population_cells` by state × sex × five-year
  band.
- **Flows** over the year, all mandatory under §N11: births by state, deaths by
  state and age band, overseas arrivals and departures by state,
  and interstate flows from `interstate_flows`.
- PRD 0002 established that the quarterly `ABS_DEM_QIM` dataflow supplies all
  56 interstate origin→destination cells for **every** run year. Those cells are
  spatial fitting targets. `interstate_state_age_sex.csv` also supplies raw
  arrival/departure margins by age and sex. Preserve those counts, and derive
  within-state/direction age-sex compositions with their denominators recorded;
  the compositions are fitting targets for `peak` and `k`. The separately
  published all-age state margins remain validation-only: their material 2020
  conflict with O-D totals must not create an impossible duplicate count target.

### 3. Scale reconciliation

The model runs at `hundredth` or `tenth` while ABS publishes full-scale counts.
Freeze the direction: **model output is scaled up** to full scale for
comparison, using the exact integer scale factor, and the resulting
discretisation floor is recorded per target. Scaling targets down instead is
rejected because it would silently re-round published values that PRD 0003
already rounded once.

### 4. The held-out split

Declared in the artifact, not in the harness. At minimum:

- every target for a year strictly after the current rolling origin is
  `heldout`;
- **single-year-of-age stocks are always `heldout`**, because the model is
  fitted on five-year bands — this is a genuine structural hold-out that tests
  whether the age distribution inside a band is right, and it costs nothing to
  reserve;
- a named subset of states is reserved unfitted in at least one artifact variant
  so spatial transfer can be assessed.

A scorer that reads a `heldout` target while in fitting mode must refuse to run.

### 5. Scorer — `data/abs/score.py`

Reads a run's scalar CSV, grouped CSVs and manifest, plus a targets artifact,
and emits a residual report: per-target signed and absolute error, MAE/RMSE and
maximum error by target family, error by population size, and per-state residual
detail. Correlation may be reported but never alone (§N13).

Also emits the ordered **summary vector** that PRD 0008 consumes as NPE's `x`,
with its ordering fixed by the artifact's target order so it is stable across
runs.

### 6. Reducing the observation vector — resolve the open question

The full annual target set exceeds 1,000 numbers once O-D cells and interstate
age-sex compositions are included, which is wide for NPE. This PRD settles the
reduction with evidence rather than assertion. Implement at least two choices
behind a name recorded in the artifact header:

- `full`: every target cell;
- `reduced`: state totals, national five-year age profile, O-D and flow moments,
  and age-sex composition moments that retain direct sensitivity to `peak` and
  `k`.

Measure both on prior-predictive draws — dimensionality, correlation structure,
and whether `reduced` preserves sensitivity to each of the seventeen free
migration parameters. Record the comparison and the recommendation in the
targets guide. If `reduced` loses sensitivity to any free parameter, say so and
recommend `full`; do not choose on convenience.

### 7. Documentation — `docs/guides/targets.md`

The artifact schema and hash domain, the frozen target set, the scale-up rule,
the held-out policy and why single-year-of-age is reserved, the scorer's report
contract, and the §6 reduction comparison. Link from
`docs/models/australian-population.md` and `docs/guides/abs-data.md`.

### 8. Binding implementation contract

The following user-selected clarifications resolve ambiguities in §§1--6 and
supersede any conflicting implication above:

- Commit fifteen hundredth-scale run-year ledgers, `2010.json` through
  `2024.json`. Ledger `y` contains run-year-`y` flows and end-boundary ERP at
  30 June `y+1`, so the 2024 ledger contains the terminal 2025 stock without
  inventing 2025 flows. Initial 2010 ERP remains state-initialisation evidence,
  not a post-run target.
- Each ordinary ledger has 2,797 ordered entries: 1,096 raw fitted controls,
  1,616 always-held-out single-year stock cells, and 85 fitted derived entries
  used only by the reduced projection. It carries two named ordered fitted
  projections over that one ledger: `full` has 1,096 components and `reduced`
  has 165. The selected reduced recipe is eight state stock totals, 21 national
  five-year stock totals, eight births, eight state death totals, sixteen
  overseas state flows, all 56 O-D cells, and 48 interstate composition moments
  (female share plus first and second moments of the published 16-band age
  ordinal for each state and direction).
- Add exactly three sink-only grouped observations:
  `population_single_year_cells` by area, sex and 12-month age band;
  `deaths_state_age_cells` by area, sex and 60-month event-age band; and
  `interstate_age_sex_flows` by previous area, area, sex and 60-month event-age
  band. No schema, transition, runtime, IR or existing observation changes.
- One additional `2010.spatial_holdout_nt.json` ledger reserves NT. Every raw or
  derived cell involving NT is held out; aggregate training-state targets
  explicitly exclude NT so no validation value leaks into fitting. Ordinary
  ledgers use only the structural single-age holdout.
- A mixed-role ledger is usable in fitting mode: the scorer selects only the
  requested fitted projection in artifact order and hard-fails if the caller
  explicitly requests any held-out target ID. Rolling-origin future-year
  reclassification is a later multi-ledger validation operation; it does not
  turn the required `y+1` calibration stock into a holdout here.
- The geography contract is `australian_states_and_territories/v1`, ordered
  `nsw,vic,qld,sa,wa,tas,nt,act`. The committed calibration scale is
  `hundredth` with exact scale-up factor and count lattice quantum 100. Each
  count target records the nearest attainable absolute error; ratio/moment
  targets record that their floor is denominator-dependent rather than
  inventing a fixed count floor.
- Target digests are
  `SHA-256(b"sembla.targets/v1\\0" + exact_canonical_file_bytes)` and live in
  `targets/index.json`; an artifact never self-hashes. The header records raw
  SHA-256 digests of exact `sources.json` and exported model bytes.
- Exact grouped-key selectors are ordered like the grouped CSV. Equality,
  wildcard and open-tail (`>=`) selectors are explicit so ABS 100+ and 75+
  remain honest. Interstate composition values and moments retain exact integer
  numerators and denominators; decimal renderings are derived only. Published
  `not_stated` deaths and incompatible all-age interstate margins are
  diagnostics, not manufactured age cells or duplicate fitted controls.
- No extract supplies formal statistical uncertainty, so no uncertainty is
  invented. Source rounding resolution and simulation discretisation are
  separate fields.
- A predeclared, bounded 2010 hundredth-scale diagnostic ensemble over only the
  seventeen free migration parameters is permitted in this PRD. It measures
  full/reduced dimension, effective correlation rank and per-parameter
  effect-to-noise sensitivity. It performs no optimization, calibration, NPE,
  posterior construction or parameter selection. Recommend `full` if the
  predeclared reduced sensitivity gate fails for any free parameter.

## Allowed files

- `data/abs/targets.py`, `data/abs/score.py`, `data/abs/targets/**`,
  `data/abs/tests/**` (new)
- `docs/guides/targets.md` (new), `docs/models/australian-population.md`,
  `docs/guides/abs-data.md` (links only)
- `scripts/check-abs-data.sh` (extend)
- `frontend/Sembla/Models/AustralianPopulation.lean` (only the three additive
  grouped views frozen in §8)
- `fixtures/australian-population/**`,
  `crates/sembla-cli/tests/australian_population.rs` (regenerated observation
  fixtures and exact output inventory; every changed path listed with reason)
- `scripts/measure-target-sensitivity.py` (bounded diagnostic ensemble only)
- implementation notes/artifacts created by the managed run

## Non-goals

- No calibration, optimization, NPE, posterior or parameter selection — PRD
  0008. The bounded diagnostic ensemble explicitly allowed by §8 is sensitivity
  measurement, not inference.
- No change to grouped-observation semantics, the CSV format, or the runtime.
- No feedback from targets into execution; observation stays a sink.
- No adjustment of published origin→destination cells to force separately
  revised margins to agree.
- No new Rust or Lean dependency; no third-party Python dependency.

## Acceptance criteria

1. Full check battery plus `scripts/check-abs-data.sh` passes;
   `git diff --check` passes.
2. Fifteen run-year target ledgers exist for 2010–2024, cover end-boundary
   stocks through 2025 without inventing a 2025 flow, are canonical, regenerate
   byte identically, and hash under the `sembla.targets/v1` domain.
3. Every artifact validates against the exported model JSON; a target naming an
   unknown observation fails loudly, proven by test.
4. Flow targets are present for every year (§N11), including all 56 observed
   interstate O–D cells and normalized age-sex compositions. Raw margins remain
   reporting/validation evidence, and their 2020 total conflict with the O–D
   dataflow is explicitly flagged.
5. Single-year-of-age stocks are `heldout` in every artifact; fitting mode
   consumes only an ordered fitted projection and refuses any explicitly
   requested held-out target ID, proven by test.
6. The scale-up reconciliation is exact and its discretisation floor is recorded
   per target.
7. The `full` versus `reduced` comparison is measured on prior-predictive draws,
   recorded in `docs/guides/targets.md`, and carries a recommendation with
   per-parameter sensitivity evidence.

## Implementation evidence

- `data/abs/targets.py` writes fifteen standard 2010--2024 ledgers, the 2010 NT
  holdout, a checked `targets/execution.json` IR/plan identity contract, and
  `targets/index.json`. Each standard ledger has 2,797 entries,
  exactly 1,616 structural holdouts, and fitted dimensions 1,096 (`full`) and
  165 (`reduced`). Domain-separated and raw hashes, byte counts, role counts and
  dimensions are independently rechecked by the offline test suite.
- `data/abs/score.py` validates exact model bytes, complete artifact fields,
  grouped declarations, ordered selectors, run manifests, IR/plan identity,
  twelve ticks, scale, features, scalar/summary hashes, exact grouped-output
  inventory, grouped-file hashes, typed keys and duplicate rows/entries.
  Hand-computed tests cover count scale-up, sparse zeros, ratios,
  weighted moments, signed/absolute residuals, MAE/RMSE/maximum, family/role/
  state/size splits, stable vector order and hard refusal of explicit heldouts
  in fitting mode.
- Every ledger retains all 56 O-D controls and 512 normalized interstate
  age-sex compositions. The 2020 diagnostic records the unchanged 27,626-person
  worst margin disagreement; `not_stated` deaths remain diagnostics rather than
  synthetic age cells.
- The model adds only `population_single_year_cells`,
  `deaths_state_age_cells` and `interstate_age_sex_flows`. Parameters, schema,
  scalar observations, summaries and all 418 transitions remain unchanged.
- The exact sensitivity predeclaration has SHA-256
  `759b3f77887335a79865bdde48d3fecb56627f34b2acfc1cada5424956bbdadb`.
  Its final 108-run evidence has SHA-256
  `9cbb5389eb7182e7d64d6b16a9bb5c5179bdf13b9d7a8353204b894b010d3403`
  and byte-reproduces from the warm cache. It retains replayable vectors and
  exact executable, source, scorer, IR and plan provenance. (PRD 0008
  re-executed the frozen ensemble with the advanced release binary
  `dc533aec372cb6b3dff07d788b1a823a42dcdd978c745b7dbd946fd0f4a6ea8a`; all run
  hashes, vectors, analyses and the recommendation are byte-identical to the
  first measurement, whose binary
  `48dc4fa8db73d62e11f1e54f7817ac97299aa639994fe2b7972bba9b4570bbd0` remains
  this PRD's recorded provenance and produced evidence hash
  `da07a240d4dabde850756f9a953451518f9aeb24e5c5e79cabc97b519a68da11`.) Only
  `interstate_base` passed the
  reduced effect-to-noise gate; all other sixteen free parameters failed, so
  the predeclared rule recommends `full`. The guide explicitly reports that the
  full vector itself also showed weak signal at hundredth scale.

### Regeneration inventory

- `fixtures/australian-population/australian_population.hundredth.json` and
  `australian_population.hundredth.plan.json` — regenerated solely for the
  three additive grouped observations.
- `fixtures/australian-population/goldens/run.grouped.population_single_year_cells.csv`,
  `run.grouped.deaths_state_age_cells.csv`, and
  `run.grouped.interstate_age_sex_flows.csv` — new observation goldens.
- `crates/sembla-cli/tests/australian_population.rs` and the golden README —
  updated to inventory and replay all ten grouped files.

Every pre-existing scalar, summary and grouped golden remained byte-identical;
`run.hashes.txt`, final-state hash, state artifact, parameters, transitions and
validation-safe state companion were unchanged.

### Validation

- `bash scripts/check.sh` passed the full documentation, Rust, Lean proof-hygiene,
  negative-elaboration, export/parity, lock and dependency checks. The Australian
  Rust suite passed 13 tests with the intentional full/tenth regeneration test
  ignored.
- `bash scripts/check-abs-data.sh` passed after the final provenance hardening:
  140 tests, offline cache verification, byte-identical extract/rate/target/
  parameter/report regeneration and byte-identical hundredth state regeneration.
- A real twelve-tick run scored successfully under both 1,096-component `full`
  and 165-component `reduced` projections. The final sensitivity evidence was
  reproduced byte-identically from its complete warm cache after the clean
  108-run measurement.
- The final independent review reported **PASS** with no blocker, high or medium
  findings. `git diff --check`, Markdown links and formatting passed, and no
  files were staged.
