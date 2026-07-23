# PRD 0008: Migration flows, competing exits, and the calibration fixture

## Context

Read `docs/prds-demographic-slots/README.md` first; DECISIONS §K1, §K9
(paired migration deferred — residuals are *reported*), and §K10 bind.
PRD 0007's model covers births and deaths; this PRD extends the **same
model** to the full aggregate flow set — overseas arrivals/departures and
internal arrivals/departures — which makes the competing-exit contest
real (three exits race per slot), makes capacity/saturation observable,
and produces the folder's calibration-ready fixture exercised through
`sweep`, `--export-pairs`, and CRN `compare`.

Folder-internal goldens from PRD 0007 may be regenerated here (they are
this folder's artifacts, not frozen contracts); every regeneration must
be called out in the implementation notes with the reason.

## Specification

### 1. Model extension (`demographic_slots`, same module)

- **State fixture.** Extend the regen-generated
  `fixtures/state/demographic_slots.state`: of the 1,000 vacant slots,
  reclassify deterministically into 600 `birth_slot`, 250
  `overseas_slot` (with plausible preclassified `entry_age_months`
  spread, sex, area), 150 `internal_slot` (likewise). Present slots
  unchanged.
- **New parameters** (with priors): `overseas_arrival_rate`,
  `emigration_rate`, `internal_departure_rate`, `internal_arrival_rate`.
- **New transitions.**
  - `overseas_arrive on PersonSlot`: guard `occupancy = vacant ∧ event =
    none_ ∧ entry_stream = overseas_slot`, hazard
    `overseas_arrival_rate`, effects: `set occupancy := present`,
    `set event := overseas_arrival`,
    `set age_months := entry_age_months`,
    `set event_age_months := entry_age_months`,
    `set generation := generation + 1`.
  - `internal_arrive on PersonSlot`: same shape over `internal_slot`
    with `event := internal_arrival` (aggregate design: the arrival is
    an independent activation, not a moved person — §K9).
  - `emigrate on PersonSlot`: guard `occupancy = present ∧ event =
    none_`, hazard `emigration_rate`,
    `contest slot_resource by race_time`, effects:
    `set event_age_months := age_months`, `set occupancy := vacant`,
    `set event := overseas_departure`.
  - `internal_depart on PersonSlot`: same shape, hazard
    `internal_departure_rate`, `event := internal_departure`.
  - The three `die_*` transitions, `emigrate`, and `internal_depart` now
    genuinely compete: every exit claims `slot_resource` by race time,
    so at most one exit wins per slot per tick.
- **New views:** `overseas_arrivals_this_tick`,
  `overseas_departures_this_tick`, `internal_arrivals_this_tick`,
  `internal_departures_this_tick`, `vacant_overseas_slots`,
  `vacant_internal_slots`; grouped view `vacancy_cells := count
  PersonSlot by entry_stream, area where occupancy = vacant ∧ event =
  none_` (the stratified-capacity diagnostic from §K1).
- **New summaries:** totals for all four flows;
  `minimum_vacant_birth_slots := min … vacant_birth_slots` (the vacancy
  margin — check `min` view/summary support and use the supported
  spelling).

### 2. Invariant tests (extending PRD 0007's suite)

Over every tick of the new golden run:

1. **Full stock-flow identity:** `population(t) = population(t−1) +
   births − deaths + overseas_arrivals − overseas_departures +
   internal_arrivals − internal_departures` (all `_this_tick(t)`).
2. **Single exit under real competition:** per tick,
   `deaths + overseas_departures + internal_departures ≤
   population(t−1)`, and cumulative exits + current present + never-
   activated vacants account for every slot exactly (derive the closed
   count from views; no slot is lost or double-counted).
3. **Internal balance residual is reported, not reconciled:** compute
   `internal_arrivals_this_tick − internal_departures_this_tick` from
   the CSV; assert it is **nonzero at some tick** at the golden seed
   (proving the aggregate design's honesty — §K9/§K10), and that the
   documentation states the residual interpretation. No code may
   "correct" it.
4. **Arrival ages:** grouped `population_cells` gains mass in bands
   matching `entry_age_months` strata after arrivals (spot-assert one
   band delta at one tick rather than a full distribution test).
5. **Grouped/scalar consistency and determinism** as in PRD 0007.

### 3. Saturation scenario (capacity failure is visible)

A dedicated test with a high `overseas_arrival_rate` θ override: within
the run, `vacant_overseas_slots` reaches 0 and stays there while the
other streams continue. Assert the vacancy view shows the exhaustion and
the run completes deterministically — then assert the documentation
(§K1: "a run that saturates a slot stratum is not calibrated evidence")
is present in `docs/demographic-model.md`'s calibration section. If the
runtime's deferred-loser saturation counter surfaces here, assert its
value too (read the plumbing first).

### 4. Calibration fixture (the folder's payoff)

All against the `direct_stable` plan export of the extended model
(regenerate the PRD 0007 plan fixture; note it):

1. **Sweep.** `sembla sweep <plan> --theta-file <grid> --noise
   independent --enable grouped-observations …` over a small θ grid
   (~6 vectors varying mortality_adult, birth_rate, emigration_rate);
   golden the sweep manifest and one draw's outputs; assert plan tuples
   and `enabled_features` appear per the integration track's rules.
2. **Pairs export.** `--export-pairs` on that sweep; golden the `(θ, x)`
   CSV — `x` is the declared summaries, now including flow totals; this
   is the NPE-shaped artifact for the demographic model.
3. **CRN contrast.** `sembla compare <plan> --params-a low_migration.json
   --params-b high_migration.json --seed …` (integration-track compare):
   assert the paired-counterfactual property concretely — flows driven
   by unchanged rates stay **identical** between arms until the changed
   migration rates perturb shared state (pin the first diverging tick
   and which columns diverge first at the golden seed; death draws for
   the same untouched person-slots are identical by §J4
   content-addressed identity — say so in the test comment).
4. **Chained windows.** Re-run PRD 0007's two-window chain on the full
   model with a migration-rate change between windows.

### 5. Documentation

Extend `docs/demographic-model.md`: the full flow set, competing-exit
semantics, the residual-reporting contract (§K9 verbatim), stratified
capacity and the saturation rule (§K1), the calibration workflow
(sweep → pairs → external fitting → chained windows), and the explicit
statement of what this aggregate model does **not** claim (no person
identity across internal moves, no mother links — with the §K9 triggers
for when those become design work).

## Allowed files

- `frontend/Sembla/Models/DemographicSlots.lean`, registration sites,
  `frontend/scripts/check-parity.sh` (append/adjust this folder's
  section only)
- `fixtures/state/**`, `fixtures/demographic/**` (regeneration noted)
- `crates/sembla-cli/tests/**` (+ θ-grid/params fixture files per test
  conventions)
- `docs/demographic-model.md`
- implementation notes/artifacts created by the managed run

## Non-goals

- Paired/quota migration, identity-preserving moves, categorical draws,
  OD matrices (§K9 — the residual test exists to make their absence
  visible, not to fix it).
- New framework features; NPE/Python execution; ABS data ingestion.
- Realistic rate values — magnitudes need only make the tests decisive.

## Acceptance criteria

1. Full check battery + negative suite + parity pass; all regenerated
   folder-internal goldens are listed with reasons in the implementation
   notes; nothing pre-existing changed.
2. The five invariant groups pass, including the full six-flow
   stock-flow identity and the closed slot accounting.
3. The residual test proves nonzero unreconciled internal imbalance at
   the golden seed, and the docs carry the reporting contract.
4. The saturation scenario shows visible, deterministic stratum
   exhaustion.
5. Sweep, pairs-export, CRN-compare, and chained-window fixtures all
   pass with goldens; the CRN test pins the first divergence and its
   §J4 rationale.
6. `docs/demographic-model.md` covers flows, capacity, calibration, and
   the non-claims; `git diff --check` passes.
