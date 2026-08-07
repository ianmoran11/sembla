# Aggregate demographic slot model

`demographic_slots` is Sembla's test-scale aggregate birth, death, overseas-
migration, and internal-migration model. It is an executable accounting and
calibration fixture, not a calibrated population projection. Its Lean source is
`frontend/Sembla/Models/DemographicSlots.lean`; canonical model and
direct-stable plan exports live under `fixtures/demographic/`.

## Slot architecture and initial state

`PersonSlot` has 5,000 fixed rows. A logical person is `(slot, generation)`,
not the slot index alone. Every activation increments `generation`, so reuse
creates a new logical person without allocating a row. `slot_resource[i] = i`
gives each row one exclusive resource for competing exits. Sex and area are
preclassified slot properties.

The deterministic, non-scientific state fixture starts with 4,000 present rows
and 1,000 vacant rows. Vacancies are stratified into 600 birth, 250 overseas,
and 150 internal slots. Overseas and internal slots carry deterministic
preclassified entry ages; sex alternates and area is round-robin. The four
`Area` rows contain keys 0–3. A Rust regeneration test constructs the artifact
without RNG.

## State machine and flows

```text
vacant birth slot, event=none_    -- birth_activate  --> present, event=birth
vacant overseas slot, event=none_ -- overseas_arrive --> present, event=overseas_arrival
vacant internal slot, event=none_ -- internal_arrive --> present, event=internal_arrival

present, event=none_ -- die_young|die_adult|die_old --> vacant, event=death
present, event=none_ -- emigrate                  --> vacant, event=overseas_departure
present, event=none_ -- internal_depart           --> vacant, event=internal_departure

*, event!=none_ -- clear_event --> same occupancy, event=none_
```

The three age-banded death rules, emigration, and internal departure all claim
the row's `slot_resource` by `race_time`. Exactly one death band is eligible,
so death, emigration, and internal departure genuinely race and at most one
exit commits for a slot in a tick. Death/departure event ages read
the tick-start age. Overseas and internal activations use the slot's
preclassified `entry_age_months`.

Internal arrival is an independent vacant-slot activation, not the relocation
of the person represented by an internal departure. In particular:

> National internal-migration balance holds only in expectation; the residual is always reported and never silently reconciled.

This is the DECISIONS §K10 reporting contract. The golden run deliberately has
nonzero per-tick residuals. DECISIONS §K9 retains the trigger verbatim:

> paired migration events/quotas (trigger: reported balance residual
> unacceptable → Option D Phase 6)

No correction, pairing, or quota is applied by this aggregate model.

## Marker lockout

An entrant is exit-ineligible for exactly one tick: its entry marker is visible
in the committed entry tick, and `clear_event` removes it on the next tick
while exit guards still read the old marker. `locked_out` therefore equals the
sum of births, overseas arrivals, and internal arrivals at every tick. At seed
7007 over 24 ticks the model reports 448 births, 90 overseas arrivals, 57
internal arrivals, and **595 locked-out row-ticks**.

The birth hazard remains a rate per eligible vacant birth slot, not a fertility
hazard, and must not be interpreted as one without an explicit scaling
derivation.

## Stratified capacity and saturation

`vacant_birth_slots`, `vacant_overseas_slots`, and `vacant_internal_slots`
expose the remaining clear capacity of each stream. The grouped
`vacancy_cells` output further stratifies vacancies by entry stream and area.
This makes capacity failure visible rather than silently reallocating slots.

The isolated saturation test sets overseas activation extremely high, disables
exits, and leaves birth and internal activation running. Overseas vacancy
reaches zero and remains zero while the other streams continue. Under §K1,
**a run that saturates a slot stratum is not calibrated evidence**. Calibration
must reject or redesign a capacity allocation that exhausts a stratum in its
intended operating regime.

## Golden accounting evidence

At seed 7007 over 24 monthly ticks, the engineering fixture reports final
population 3,751, 448 births, 435 deaths, 90 overseas arrivals, 191 overseas
departures, 57 internal arrivals, and 218 internal departures. The minimum
clear birth capacity is 583 slots and the maximum generation is 3. These values
are deterministic test evidence, not demographic estimates.

Tests enforce both accounting interpretations:

```text
initial population + cumulative entries
  = current population + cumulative exits

current population + clear vacancies + current marked exits
  = 5,000 physical slots
```

Grouped population, death, and vacancy totals cross-check their scalar views.

## Running the model

Grouped observations are explicit and CPU-only:

```sh
cargo run --locked -p sembla-cli -- run \
  fixtures/demographic/demographic_slots.json \
  --population fixtures/state/demographic_slots.state \
  --seed 7007 --ticks 24 --out demographic.csv \
  --enable grouped-observations
```

The run writes scalar and summary CSVs plus:

- `demographic.grouped.population_cells.csv` by sex, area, and five-year age;
- `demographic.grouped.deaths_cells.csv` by sex and five-year event age;
- `demographic.grouped.vacancy_cells.csv` by entry stream and area;
- a manifest recording the state, enabled feature, and grouped hashes.

## Calibration workflow

The checked-in direct-stable plan is exercised as follows:

1. Run an independent-noise sweep over a complete theta grid.
2. Export `(theta, summaries)` pairs for an external fitting system.
3. Fit externally; this repository does not run NPE or claim fitted values.
4. Use explicit-feature CRN comparison to inspect a low/high migration
   counterfactual.
5. Apply selected parameters in chained windows connected by exported-state
   hashes.

```sh
cargo run --locked -p sembla-cli -- sweep \
  fixtures/demographic/demographic_slots.plan.json \
  --population fixtures/state/demographic_slots.state \
  --seed 7505 --ticks 12 \
  --theta-file fixtures/demographic/calibration/theta-grid.json \
  --noise independent --out demographic-sweep \
  --export-pairs demographic-pairs.csv \
  --enable grouped-observations

cargo run --locked -p sembla-cli -- compare \
  fixtures/demographic/demographic_slots.plan.json \
  --population fixtures/state/demographic_slots.state \
  --seed 7404 --ticks 12 \
  --params-a fixtures/demographic/calibration/low-migration.json \
  --params-b fixtures/demographic/calibration/high-migration.json \
  --out demographic-compare.csv \
  --enable grouped-observations
```

The CRN fixture changes arrival rates only. At tick 0 the arrival columns and
their resulting stock/capacity columns diverge; among flow columns only
overseas/internal arrivals differ, while unchanged birth, death, and departure
flows remain identical. Under DECISIONS §J4 the same untouched slot/rule
coordinates receive identical draws; later aggregate counts may differ after
arrival commits have changed shared state.

## Explicit non-claims

This aggregate model does not claim:

- person identity continuity between internal departure and arrival;
- paired or quota-balanced migration;
- origin–destination matrices or household relocation;
- mother-linked births or fertility interpretation of activation hazards;
- ABS-derived, fitted, or forecast-quality parameter values.

Per §K9, an unacceptable reported balance residual triggers Option D Phase 6
paired migration design. Evidence that mother or household identity is
scientifically required triggers a design-options note first; it does not
justify silently adding cross-row or mother-linked semantics here.

The manual scale measurements and the §K2 `Expr::Tick` recommendation are in
[the demographic benchmark report](../performance/demographic-benchmark.md).
For the individual-agent design that preserves identity and closes interstate
flows exactly, see [Australian population](australian-population.md).
