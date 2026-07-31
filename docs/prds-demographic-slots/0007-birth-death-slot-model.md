# PRD 0007: The aggregate birth/death slot model

## Context

Read `docs/prds-demographic-slots/README.md` first; DECISIONS §K1 (slot
architecture, `(slot, generation)` identity), §K8, §K10 (interpretation
caveats) bind. Every framework piece now exists: generic state loading
(0002), chained runs (0003), arithmetic effects and Int params (0004),
contests (0005), grouped observations (0006). This PRD assembles them into
the first demographic model — births and deaths only — at test scale, with
the demographic-balance invariants as its acceptance tests. Overseas and
internal migration arrive in PRD 0008 **by extending this model**; the
goldens this PRD creates are folder-internal artifacts that PRD 0008 is
explicitly permitted to regenerate (unlike pre-existing frozen goldens).

The slot state machine being implemented (from the use case, as amended):

```text
vacant,  event=none, entry_stream=birth ── birth_activate ──> present, event=birth
present, event=none ───────────────────── die_*  ──────────> vacant,  event=death
*,       event≠none ───────────────────── clear_event ─────> same occupancy, event=none
```

Entries and exits require `event = none`; marker clearing fires only when
`event ≠ none`. Guards are disjoint, so no transition pair double-writes
`event`. The consequence — a one-tick lockout during which a new entrant
cannot die — is the documented §K10 trade-off, and this PRD **measures**
it rather than hiding it.

## Goal

A canonical `demographic_slots` model authored entirely in the surface
DSL, loaded from a checked-in state artifact, running deterministically
with grouped observations, whose acceptance tests are the demographic
accounting identities, slot-reuse generation semantics, and the lockout
measurement.

## Specification

### 1. Model — `frontend/Sembla/Composition/…` no — a dedicated module

New `frontend/Sembla/Models/DemographicSlots.lean` (follow wherever
canonical models live — read `frontend/Sembla/Models.lean` and mirror its
registration pattern), `sembla_model demographicSlots
(name := "demographic_slots") (dt := 1.0)`, box `demographic`:

- **Tables.**
  - `PersonSlot (rows := 5_000)`: `occupancy : {vacant, present}`,
    `event : {none_, birth, death, overseas_arrival, overseas_departure,
    internal_arrival, internal_departure}` (spell the first variant to
    avoid any `none` keyword clash — check the surface's enum-variant
    rules first and pick the working spelling; the full seven variants are
    declared now so PRD 0008 extends without a schema change),
    `sex : {male, female}`, `age_months : Int`, `event_age_months : Int`,
    `generation : Int`, `entry_stream : {birth_slot, overseas_slot,
    internal_slot}`, `entry_age_months : Int`, `area : Area`,
    `slot_resource : SlotResource`.
  - `Area (rows := 4)`: `area_key : Int`.
  - `SlotResource (rows := 5_000)` (empty schema or a single placeholder
    attr if empty tables are unsupported — check and record).
- **Parameters.** `param birth_rate : ℝ := …`, three age-banded mortality
  params (`mortality_young`, `mortality_adult`, `mortality_old`), all with
  priors so sweep works later. Values: plausible monthly hazards at test
  scale (e.g. birth_rate sized so ~1% of birth slots activate per tick) —
  the science is not the point; the invariants are.
- **Transitions** (general form, all guards written to be pairwise
  disjoint on any shared written attribute):
  - `age_monthly on PersonSlot`: guard `occupancy = present`, hazard
    `1e300`, `set age_months := age_months + 1` (PRD 0004).
  - `clear_event on PersonSlot`: guard `event ≠ none_`, hazard `1e300`,
    `set event := none_`.
  - `birth_activate on PersonSlot`: guard `occupancy = vacant ∧ event =
    none_ ∧ entry_stream = birth_slot`, hazard `birth_rate`, effects:
    `set occupancy := present`, `set event := birth`,
    `set age_months := 0`, `set event_age_months := 0`,
    `set generation := generation + 1`. Sex and area are preclassified
    per vacant slot (§K10 aggregate caveat: this hazard is per eligible
    vacant slot, not a fertility rate).
  - `die_young` / `die_adult` / `die_old on PersonSlot`: guards
    `occupancy = present ∧ event = none_` plus disjoint age bands
    (`age_months < 240`; `240 ≤ … < 780` written with the accepted
    comparison forms; `age_months > 779`), hazards the respective params,
    each with `contest slot_resource by race_time` (PRD 0005 — the claim
    matters from PRD 0008 on, and is correct now), effects:
    `set event_age_months := age_months` (old-snapshot read = tick-start
    age, the §K10-adjacent classification-error guard),
    `set occupancy := vacant`, `set event := death`.
- **Views.** From the use case's compile-checked list, abridged to what
  the tests consume: `population`, `invalid_age` (count where present ∧
  `age_months < 0`), `males`, `females`, a reduced age-band set
  (`age_00_04`, `age_20_24`, `age_65_69`, `age_85_plus` — four bands
  suffice at test scale; the full 18 arrive with the real model),
  `births_this_tick` (`event = birth`), `deaths_this_tick`
  (`event = death`), `locked_out` (`occupancy = present ∧ event ≠ none_`),
  `vacant_birth_slots` (`occupancy = vacant ∧ entry_stream = birth_slot ∧
  event = none_`), `max_generation` (`max … using generation`).
- **Grouped views** (flag-gated): `population_cells := count PersonSlot by
  sex, area, band age_months 60 where occupancy = present` and
  `deaths_cells := count PersonSlot by sex, band event_age_months 60 where
  event = death`.
- **Summaries.** `final_population`, `maximum_invalid_age_count := max …
  invalid_age`, `births_total := sum … births_this_tick`, `deaths_total`,
  `final_max_generation`, `locked_out_total := sum … locked_out`.

### 2. Initial state fixture

`fixtures/state/demographic_slots.state` via the regen-test pattern
(0002): deterministic, clearly-labelled non-scientific synthesis — 4,000
present slots (age uniform-ish over 0–1080 months, sex alternating, area
round-robin, `generation = 1`, `event = none_`), 1,000 vacant birth slots
(`generation = 0`, `age_months = 0`, preclassified sex/area), `Area` rows
with `area_key = 0..3`, `slot_resource[i] = i`. All synthesized with
plain deterministic arithmetic in the regen test (no RNG needed).

### 3. Registration and goldens

Register the model with the exporter; append a parity section for its
canonical export under `fixtures/demographic/` (not `examples/` — that
directory stays frozen); also export a `direct_stable` plan fixture (used
by PRD 0008's calibration work). Run goldens: 24 ticks, fixed seed,
`--enable grouped-observations`, `--population
fixtures/state/demographic_slots.state` — CSV, grouped CSVs, manifest
(normalized), hashes, all checked in under `fixtures/demographic/goldens/`
with a re-run comparison test.

### 4. Invariant tests (the acceptance core)

A CLI test parses the golden run's CSVs and asserts over **every tick**:

1. **Stock-flow identity:** `population(t) = population(t−1) +
   births_this_tick(t) − deaths_this_tick(t)`.
2. **Marker accounting:** `births_total = Σₜ births_this_tick` and
   likewise deaths (summaries cross-check the per-tick columns).
3. **No invalid ages:** `maximum_invalid_age_count = 0`.
4. **Single exit:** with only death as an exit this reduces to
   deaths ≤ present — assert it anyway; the competing form matures in
   PRD 0008.
5. **Grouped/scalar consistency:** Σ of `population_cells` counts at each
   tick equals `population(t)`; Σ of `deaths_cells` counts equals
   `deaths_this_tick(t)`.
6. **Slot reuse:** `final_max_generation ≥ 2` at the chosen
   seed/rates (tune birth_rate/mortality so reuse provably occurs within
   24 ticks; the test pins that the reused slot is a *new* person —
   generation strictly increased).
7. **Lockout measurement:** `locked_out(t)` equals
   `births_this_tick(t)` for this model (entrants are the only present
   rows with markers), and `locked_out_total` is reported. Add a model
   doc-comment quantifying the trade-off (an entrant is death-ineligible
   for exactly one tick) and citing §K10.
8. **Determinism:** the full run reproduces bitwise, twice.

### 5. Chained-window smoke

One test chains two 12-tick windows via `--export-state`/state input with
different mortality θ between windows (PRD 0003 machinery on the real
model), asserting the chain-link hash equality and window-2 determinism.

### 6. Documentation

New `docs/models/demographic.md`: the slot architecture as implemented,
the state machine diagram, the lockout contract with its measured
magnitude at the golden seed, the §K10 caveats verbatim, and how to run
the model (flags, state artifact, grouped outputs). Link from
`docs/guides/state-format.md`.

## Allowed files

- `frontend/Sembla/Models/DemographicSlots.lean` (new), registration
  sites (`Models.lean`/`Main.lean`/`Sembla.lean`),
  `frontend/scripts/check-parity.sh` (append only)
- `fixtures/state/**`, `fixtures/demographic/**` (new)
- `crates/sembla-cli/tests/**` (invariant + chained tests, regen test)
- `docs/models/demographic.md` (new), `docs/guides/state-format.md` (link only)
- implementation notes/artifacts created by the managed run

## Non-goals

- Migration flows, emigration, saturation scenarios (PRD 0008).
- Scientific calibration, real ABS-derived rates, or the full 18 age-band
  view set.
- Mother-linked births, categorical draws, or any §K9 deferred construct
  — the preclassified-slot design exists to avoid them.
- New framework features of any kind; if the model cannot be expressed,
  the earlier PRD that should have enabled it is where the fix belongs
  (stop and say so rather than working around).

## Acceptance criteria

1. Full check battery + negative suite + parity pass; the model's
   canonical export and plan fixture byte-reproduce via the appended
   parity section.
2. All eight invariant tests pass against the golden run; the goldens
   reproduce bitwise on re-execution.
3. The state fixture regenerates byte-identically from its regen test;
   `examples/**` and all pre-existing goldens untouched.
4. Slot reuse with strict generation increase is pinned; the lockout is
   measured and documented with §K10 cited.
5. The chained-window smoke passes with the manifest chain link asserted.
6. `docs/models/demographic.md` exists with the state machine, lockout
   contract, and run instructions; `git diff --check` passes.
