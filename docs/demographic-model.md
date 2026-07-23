# Aggregate demographic slot model

`demographic_slots` is Sembla's test-scale birth/death demographic model. It is
an executable accounting fixture, not a calibrated population projection. The
canonical Lean source is `frontend/Sembla/Models/DemographicSlots.lean`; its
canonical model and direct-stable plan exports live under
`fixtures/demographic/`.

## Slot architecture

`PersonSlot` has 5,000 fixed rows. A logical person is identified by
`(slot, generation)`, not by the slot index alone. Activating a vacant slot
increments `generation`; therefore reuse creates a new logical person without
allocating a row. `slot_resource[i] = i` gives each row an exclusive contest
resource for exits. Sex and area are preclassified properties of slots.

The checked-in `fixtures/state/demographic_slots.state` is deliberately
non-scientific deterministic synthesis: 4,000 initially present rows have
alternating sex, round-robin area, arithmetic ages over 0–1080 months, and
generation 1; 1,000 initially vacant birth slots have generation 0. The four
`Area` rows contain keys 0–3. A Rust regeneration test constructs these bytes
without RNG.

## State machine

```text
vacant, event=none_, entry_stream=birth_slot
    -- birth_activate --> present, event=birth, generation=generation+1

present, event=none_
    -- die_young|die_adult|die_old --> vacant, event=death

*, event!=none_
    -- clear_event --> same occupancy, event=none_
```

Birth and death guards require `event = none_`; marker clearing requires the
opposite. The three death guards use disjoint age bands and claim the row's
`slot_resource` by `race_time`. Age, clearing, entry, and exit effects all read
the tick-start snapshot. `event_age_months` records that old-snapshot age for
death classification.

## Marker lockout and interpretation

A new entrant is death-ineligible for exactly one tick: its `birth` marker is
visible in the committed birth tick, and `clear_event` removes it on the next
tick while death guards still read the old marker. This is the measured
DECISIONS §K10 trade-off, not hidden runtime behavior. At the golden seed 7007,
24 ticks produce **548 births and 548 locked-out row-ticks**
(`locked_out_total = 548`), so the equality is checked at every tick as well as
in the summary.

DECISIONS §K10's interpretation caveats are binding, verbatim:

> The birth-activation hazard is a rate per eligible vacant slot, not a
> fertility hazard, and must not be interpreted as one without an explicit
> scaling derivation. The one-tick event-marker lockout—new entrants are
> ineligible for events while their marker persists—is a documented and
> measured model trade-off counted by PRD 0007, not a framework bug. National
> internal-migration balance holds only in expectation; the residual is always
> reported and never silently reconciled.

The initial state, parameter defaults, and golden results are engineering test
fixtures. They are not ABS-derived rates, estimates, or forecasts.

## Running the model

Grouped observations are default-off and CPU-only, so execution must explicitly
enable them:

```sh
cargo run --locked -p sembla-cli -- run \
  fixtures/demographic/demographic_slots.json \
  --population fixtures/state/demographic_slots.state \
  --seed 7007 --ticks 24 --out demographic.csv \
  --enable grouped-observations
```

The run writes:

- `demographic.csv`, including scalar population, flow, age, generation, and
  lockout views;
- `demographic.csv.summaries.csv`, including total births/deaths and final
  population/generation;
- `demographic.grouped.population_cells.csv`, keyed by sex, area, and five-year
  age band;
- `demographic.grouped.deaths_cells.csv`, keyed by sex and five-year event-age
  band;
- `demographic.csv.manifest.json`, recording the enabled feature and hashes for
  both grouped outputs.

The checked-in 24-tick artifacts under `fixtures/demographic/goldens/` reproduce
bitwise. The integration test also chains two 12-tick windows through
`--export-state`, changes mortality parameters for the second window, verifies
the manifest state-link hash, and repeats window two bit-for-bit.
