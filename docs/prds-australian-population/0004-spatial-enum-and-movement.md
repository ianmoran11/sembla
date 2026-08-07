# PRD 0004: Enum geography and genuine interstate movement

## Context

Read `docs/prds-australian-population/README.md` first; its frozen model shape,
the enum-`area` rationale and the parameterisation are binding, as is
`DECISIONS.md` §§N1–N5. The landed `demographic_slots` model is the structural
template and must not be modified.

This is the PRD the folder exists for. Everything else calibrates, feeds or
validates the model built here. The distinguishing property against the landed
track is identity continuity: a person who moves from NSW to QLD is *the same
agent*, keeping their age and their `generation`, where `demographic_slots`
vacates a slot and activates a different pre-classified one (§K9).

Rates in this PRD are plausible placeholders. The science arrives in PRD 0005;
what is being accepted here is the mechanism and its invariants.

## Goal

`australian_population` exists as a canonical Lean model with an enum `area`,
56 origin→destination move transitions resolved by racing clocks, and an
invariant suite proving that people genuinely move, that interstate flows
conserve exactly, and that no slot is ever lost or double-counted.

## Specification

### 1. The model — `frontend/Sembla/Models/AustralianPopulation.lean`

Follow `frontend/Sembla/Models/DemographicSlots.lean` and mirror its
registration pattern (read `Models.lean` / `Sembla.lean` first). Execute this
PRD in two phases: 0004A defines the model and standalone schema export needed
by PRD 0003; 0004B performs registration, invariants and goldens against the
committed `hundredth` artifact.

`sembla_model australianPopulation (name := "australian_population")
(dt := 1.0)`, box `demographic`, with the README's frozen tables: `PersonSlot`
(including `area` over eight variants, `prev_area` over nine and the
single-use `retired_slot` stream), `SlotResource` with an empty schema, and
**no `Area` table**.

Transitions, all guards written to be pairwise disjoint on any shared written
attribute except where a contest resolves them:

- `age_monthly`: guard `occupancy = present`, hazard `1e300`,
  `set age_months := age_months + 1`.
- `clear_event`: guard `event ≠ none_`, hazard `1e300`,
  `set event := none_`, `set prev_area := none_`.
- **56 `move_<origin>_<destination>`**: guard
  `occupancy = present ∧ event = none_ ∧ area = <origin>`, hazard per §N5,
  `contest slot_resource by race_time`, effects `set prev_area := <origin>`,
  `set area := <destination>`, `set event := interstate_move`,
  `set event_age_months := age_months`. Note what is deliberately *absent*:
  `generation` and `age_months` are untouched, because the person persists.
- **8 states × 21 bands × 2 sexes** `die_<state>_<band>_<sex>` (five-year
  bands to `100+`): guard `occupancy = present ∧ event = none_` plus the
  disjoint state, band and sex, `contest slot_resource by race_time`, set event
  age, write `entry_stream := retired_slot`, vacate and emit death. The 336
  transitions are required because PRD 0005 forbids averaging the published
  state-specific rates and the expression language has no conditional lookup.
  This adds compute but no per-row column; PRD 0010 must measure the cost.
- **8 `birth_<state>`**: guard `occupancy = vacant ∧ event = none_ ∧
  entry_stream = birth_slot ∧ area = <state>`, effects `set occupancy :=
  present`, `set event := birth`, `set age_months := 0`,
  `set event_age_months := 0`, `set generation := generation + 1`.
- **8 `overseas_arrive_<state>`**: same shape over `overseas_slot`, with
  `set age_months := entry_age_months` and `event := overseas_arrival`.
- **8 `emigrate_<state>`**: guard `occupancy = present ∧ event = none_ ∧
  area = <state>`, `contest slot_resource by race_time`,
  `set event_age_months := age_months`, `set entry_stream := retired_slot`,
  `set occupancy := vacant`, `set event := overseas_departure`.

Per-state births, arrivals and departures are required because hazards cannot
reference their own state's population (§N3a); state variation has to live in
separate transitions.

### 2. Generating the 56 move transitions

Writing 56 near-identical transitions by hand is unacceptable and so is
inventing macro machinery. Prefer a Lean helper that expands to the transition
syntax within the existing `sembla_model` command. **Read the DSL's command
syntax first** and establish whether it admits splicing of generated transition
syntax; the surface is a command macro, so this is not guaranteed.

If splicing is not supported, fall back to a generated-and-checked-in Lean
source with a regeneration test that byte-compares the committed file against
fresh generator output — the same discipline as the state fixtures. Pick the
honest option, record which was used and why, and do not extend the DSL to make
the nicer option work; that would breach the folder's no-new-syntax rule.

### 3. Parameters

Per §N5: `interstate_base`, `push_<state>` for seven states with
`push_nsw ≡ 1`, `pull_<state>` for seven states with `pull_nsw ≡ 1`,
`peak_months`, `k`, plus placeholder
`birth_rate_<state>`, `mortality_<state>_<band>_<sex>`,
`overseas_arrival_<state>` and
`emigration_<state>`. All carry `LogNormal` priors so PRD 0008's sweep works.
The age profile is written with the available operators only:

```text
1 / (1 + k · (age_months − peak_months) · (age_months − peak_months))
```

Express `push_nsw ≡ 1` and `pull_nsw ≡ 1` either as literals in NSW-origin
and NSW-destination hazards or as parameters fixed at 1.0 and excluded from θ.
Record the choice; both reference factors must be fixed to remove both scale
invariances.

### 4. Views, grouped views and summaries

Scalar views: `population`, `births_this_tick`, `deaths_this_tick`,
`overseas_arrivals_this_tick`, `overseas_departures_this_tick`,
`interstate_moves_this_tick`, `locked_out`, `invalid_age`,
`vacant_birth_slots`, `vacant_overseas_slots`, `max_generation`.

Grouped views: `population_cells := count PersonSlot by area, sex,
band age_months 60 where occupancy = present`;
`interstate_flows := count PersonSlot by prev_area, area where
event = interstate_move`; `births_cells`, `deaths_cells`,
`overseas_arrival_cells`, `overseas_departure_cells` by `area` (and `sex` where
PRD 0006 needs it); `vacancy_cells := count PersonSlot by entry_stream, area
where occupancy = vacant ∧ event = none_`.

Summaries: national totals for all five flows, `final_population`,
`minimum_vacant_birth_slots`, `minimum_vacant_overseas_slots`,
`maximum_invalid_age_count`, `locked_out_total`, `final_max_generation`.

### 5. Invariant tests — the acceptance core

Over every tick of a golden run at `hundredth` scale from PRD 0003's fixture:

1. **Interstate flows conserve exactly.** Total in equals total out every tick,
   and equals `interstate_moves_this_tick`. This must be *exactly* zero
   residual — unlike §K9's aggregate design, whose nonzero internal imbalance
   PRD 0008 of the demographic folder was required to prove. Assert the
   contrast explicitly; it is the proof that N1 was achieved.
2. **No self-moves.** `interstate_flows` has no `(s, s)` cell, ever.
3. **Identity continuity.** A moving person keeps their `generation` and their
   `age_months` advances by exactly one that tick — assert by tracking slots
   across a tick boundary, not by aggregate.
4. **National stock-flow identity.** `population(t) = population(t−1) + births
   − deaths + overseas_arrivals − overseas_departures`; interstate moves cancel
   nationally and must not appear in this identity.
5. **Per-state stock-flow identity.** For each state, the same identity plus
   interstate in minus out, derived from `population_cells` and
   `interstate_flows`.
6. **One event per person per tick.** `deaths + overseas_departures +
   interstate_moves ≤ population(t−1)`, and no slot carries two events.
7. **Closed slot accounting.** Present, eligible never-activated vacant, and
   retired slots account for every row exactly; `generation` changes from 0 to
   1 only when a pre-classified entrant activates, and never on an interstate
   move; `invalid_age` is always 0.
8. **Determinism.** Goldens reproduce bitwise on re-execution.

### 6. Registration, goldens, and cost

Register the model wherever `demographic_slots` is registered, append to
`frontend/scripts/check-parity.sh`, and commit the canonical export plus the
direct-stable plan fixture. Record a one-line tick-cost measurement at
`hundredth` scale in the implementation notes: the retained state-specific
mortality design carries 418 transitions against `demographic_slots`' 12, and
PRD 0010 needs a baseline.

### 7. Documentation — `docs/models/australian-population.md`

The state machine, the enum-`area` rationale and both foreclosures verbatim from
§N3, the competing-clocks destination mechanism, the transition inventory, the
invariant contract including the exact-conservation contrast with §K9, and how
to run the model. Link from `docs/models/README.md` and
`docs/models/demographic.md`.

## Allowed files

- `frontend/Sembla/Models/AustralianPopulation.lean` (new), registration sites
  (`Models.lean` / `Sembla.lean` / `Main.lean` as the existing pattern requires)
- `frontend/scripts/check-parity.sh` (append only)
- `fixtures/australian-population/**` (new), `fixtures/state/**` (new entries only)
- `crates/sembla-cli/tests/**` (invariant and determinism tests)
- `docs/models/australian-population.md` (new), `docs/models/README.md`,
  `docs/models/demographic.md` (links only)
- implementation notes/artifacts created by the managed run

## Non-goals

- No new surface syntax, `Expr` variant, feature flag, or DSL extension.
- No change to `demographic_slots`, its fixtures, or any pre-existing golden.
- No ABS-derived rates — placeholders only; PRD 0005 owns the science.
- No calibration, sweep, or targets work.
- No sub-state geography, and no `mortality_sex_factor` column.

## Acceptance criteria

1. Full check battery, negative suite and parity pass; the canonical export and
   plan fixture byte-reproduce.
2. All eight invariant groups pass against the golden run, which reproduces
   bitwise.
3. Interstate conservation holds with **exactly** zero residual every tick, and
   the test asserts the contrast with §K9's aggregate design.
4. Identity continuity is proven per-slot across a tick boundary: `generation`
   unchanged and age advanced by one on a move.
5. The 56 move transitions are generated, not hand-copied; the chosen mechanism
   and its rationale are recorded, and no DSL change was made to enable it.
6. `docs/models/australian-population.md` carries both §N3 foreclosures verbatim
   and the tick-cost baseline; `git diff --check` passes.

## Implementation evidence

- The committed seed-8305, 24-tick golden and its stable hashes are documented
  in `fixtures/australian-population/goldens/README.md`.
- `all_eight_invariant_groups_hold_over_every_golden_tick` checks national and
  eight-state identities, exact interstate closure, event accounting, pool
  closure, age sentinels and bounded generation at every golden tick.
- `per_slot_golden_trajectory_preserves_identity_and_retirement_every_tick`
  executes the same plan rule identities and seed one tick at a time, inspects
  every slot at all 24 boundaries, cross-checks golden event counts, and matches
  the final-state hash. It observes entrants through activation → clearing →
  exit → permanent retirement and every move through age/generation continuity.
- The isolated-movement and forced-lifecycle tests additionally exhaust those
  mechanisms under controlled parameters, including every entrant slot.
- The §K9 contrast is a direct assertion against the unchanged aggregate
  demographic golden: it has a nonzero accepted internal-flow residual while
  every Australian tick closes to zero.
- The 418-transition local baseline is five ten-tick debug CPU runs with median
  4.071 seconds (0.407 seconds/tick) at 352,460 rows on an Apple M2 Pro.
