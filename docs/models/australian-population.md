# Australian population

`australian_population` is a monthly fixed-pool microsimulation of the eight
Australian states and territories. It carries individual age, sex and state
from 30 June 2010 and represents interstate migration as a write to the same
row's enum-valued `area`, preserving identity across movement.

The implementation is in
[`frontend/Sembla/Models/AustralianPopulation.lean`](../../frontend/Sembla/Models/AustralianPopulation.lean).
The calibrated fifteen-year chain and validation evidence are delivered by the
remaining [Australian population PRDs](../prds-australian-population/README.md).

## State and lifecycle

`PersonSlot` has occupancy, event, sex, age in months, event age, generation,
entry stream, entry age, current area, previous area and an exclusive
`slot_resource` reference. `SlotResource` has one empty-schema row per person.
There is no `Area` table.

Initial residents have `entry_stream = retired_slot`. Birth and overseas slots
are pre-classified once from ABS composition, activate at most once, and become
`retired_slot` on death or emigration. Consequently the pool arithmetic and
entrant composition remain fixed: annual parameter files change activation
hazards, not the state, sex or age mix of eligible rows.

Identity is `(row ordinal, generation)`. Interstate movement changes `area`,
sets `prev_area`, and leaves generation and age untouched; the independent
monthly ageing transition advances age on that tick.

## Generated transitions

The schema and observations use the existing `sembla_model` command. Ordinary
Lean list functions generate the repetitive raw IR and the authoritative
post-splice checker validates the result:

- 56 directed `move_<origin>_<destination>` transitions;
- 336 mortality transitions: eight states × 21 five-year age bands through
  100+ × two sexes;
- eight each for births, overseas arrivals and emigration;
- monthly ageing and event clearing.

Moves, deaths and emigration contest the same row's `slot_resource` by
`race_time`. Competing exponential clocks therefore choose at most one event
and one migration destination per person per tick.

## Parameterisation

There are 17 free migration parameters: `interstate_base`, seven non-NSW push
factors, seven non-NSW pull factors, `peak_months` and `k`. NSW push and pull are
literal 1.0 factors and are absent from the parameter vector. The rational age
profile uses only supported arithmetic:

```text
1 / (1 + k * (age_months - peak_months) * (age_months - peak_months))
```

Per-state birth, overseas-arrival and emigration parameters and 336
state×band×sex mortality parameters bring the declared total to 377. The model's
no-params defaults are the ABS-derived 2010 values, and
[`data/abs/params/2010.json`](../../data/abs/params/2010.json) carries the same
complete map. Files through `2024.json` provide each subsequent run year's full
parameter environment. All hazards are monthly.

The 360 direct parameters remain fixed during inference; only the 17 migration
parameters are free. Positive direct defaults use median-centred LogNormal
priors with spread 0.5. The seven mortality cells whose published 2010 rate is
exactly `0.0` retain a zero default and use the documented centred Normal
exception with spread `0.05 / 12000`, because a LogNormal cannot be centred at
zero. [`priors.json`](../../data/abs/params/priors.json) is the sole
machine-readable fixed/free and identification registry. The derivation,
source hashes, two NT female 100+ fallbacks, period-life-table comparison and
uncalibrated fidelity evidence are in
[`rates.md`](../../data/abs/extracts/rates.md).

### Fixed-pool entry wart

Birth and overseas-arrival hazards are re-derived every year against the
projected vacancies in their single-use pools. For start-of-year vacancies `V`
and published entries `E`, the monthly hazard is
`-log1p(-E / V) / 12`; the logarithm is evaluated offline and only the resulting
parameter enters the IR. These are pool-relative activation rates, **not
comparable across years as behavioural quantities**.

If the vacancy margin approaches zero, entry flow is suppressed regardless of
theta. PRD 0008's saturation diagnostic is therefore a correctness check, not
a nicety. Ten-percent headroom reduces that risk but does not remove the fixed
pool's architectural limit.

## Observations

Scalar observations cover population, all five event flows, lockout, invalid
age, eligible vacancies and maximum generation. Grouped observations cover
population by area, sex and five-year age band; interstate origin-destination
flows; entry and exit cells; and vacancies by entry stream and area.

Three additive sink observations support the versioned target ledger without
changing execution: single-year stocks by 12-month age band, deaths by
60-month event-age band, and interstate moves by origin, destination, sex and
60-month event-age band. Their mapping, scale-up rule, structural/spatial
holdouts and scoring contract are documented in the
[targets and scoring guide](../guides/targets.md).

`prev_area` is cleared on the tick after a move, so
`count PersonSlot by prev_area, area where event = interstate_move` is the
unambiguous O-D flow observation.

## Initial artifacts

The deterministic builder and full arithmetic are documented in
[`data/abs/extracts/initial-state-2010.md`](../../data/abs/extracts/initial-state-2010.md).
The committed one-in-a-hundred files are:

- `fixtures/australian-population/australian_population.hundredth.json`;
- `fixtures/australian-population/australian_population.hundredth.plan.json`;
- `fixtures/state/australian_population_2010_hundredth.state` and its paired
  `.model.json`.

The paired state model omits only feature-gated grouped views so the existing
public `sembla validate` command accepts it. It retains exactly the canonical
schema, parameters, transitions and scalar views. Scientific runs use the
feature-bearing plan, which restores every grouped observation.

Run the current test-scale model with:

```bash
target/release/sembla run \
  fixtures/australian-population/australian_population.hundredth.plan.json \
  --population fixtures/state/australian_population_2010_hundredth.state \
  --seed 17 --ticks 12 --enable grouped-observations --out results.csv
```

For the scientific 2010–2025 walk, use the deterministic annual driver and
verification workflow in the
[Australian population runs guide](../guides/australian-population-runs.md).

## Invariant golden

The committed [24-tick invariant golden](../../fixtures/australian-population/goldens/README.md)
checks all eight national/state, movement, lifecycle, sentinel, generation and
replay groups. Its in-memory trajectory test inspects every slot at all 24 tick
boundaries and matches the committed final-state hash; targeted forced tests
also exhaust movement and activation/clearing/exit/retirement mechanisms.

This model's national interstate inflow-minus-outflow residual is exactly zero
on every tick. The test explicitly contrasts that result with the unchanged
aggregate `demographic_slots` §K9 golden, where internal arrivals and departures
have an accepted nonzero imbalance because origin identity is not represented.
That aggregate foreclosure does not apply here.

## Accepted foreclosures

The following two consequences are copied from `DECISIONS.md` §N3 and are not
deferred features:

> Making `area` an enum permanently forecloses two capabilities, and both are
> recorded as consequences rather than as deferrals with triggers. First, no
> hazard may reference the population of its own state, because
> `freq (pred) over <ref>` and `Agg { on: AggJoin }` join only on declared Ref
> keys. Second, the expression language offers Add, Sub, Mul and Div only, so no
> transcendental function is available to any hazard.

The resulting rates are exogenous. There is no crowding or agglomeration
feedback, and fertility is aggregate birth-slot activation rather than a rate
applied to resident women of childbearing age.

## Provisional tick cost

On an Apple M2 Pro using the debug CPU binary, five ten-tick one-in-a-hundred
runs with grouped observations and the 418-transition state-specific mortality
model had wall times of 4.383, 4.135, 4.071, 3.864 and 3.817 seconds. The median
was 4.071 seconds, or 0.407 seconds per tick for 352,460 rows. The committed
24-tick replay test checks all scalar and grouped bytes deterministically. This
is a local implementation baseline, not the full-scale GPU benchmark required
by PRD 0010.

## Non-claims

The model has no sub-state validity, does not identify individual fertility,
and is not evidence of causal migration mechanisms. Matching state-age-sex
stocks and published flows is reconstruction of those controls; held-out and
rolling-origin evidence must be reported separately.
