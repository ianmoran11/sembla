# Australian population invariant golden

This fixture is a deterministic, uncalibrated **24-month invariant run**, not a
claim of multi-year demographic fidelity. It uses:

- the 1:100 state artifact (352,460 physical rows);
- the feature-bearing executable plan with ABS-derived 2010 direct-rate defaults;
- seed `8305`; and
- 24 monthly ticks.

Generate it with:

```bash
cargo run -p sembla-cli -- run \
  fixtures/australian-population/australian_population.hundredth.plan.json \
  --population fixtures/state/australian_population_2010_hundredth.state \
  --seed 8305 --ticks 24 --enable grouped-observations \
  --out run.csv --export-state final.state
cargo run -p sembla-cli -- state-hash final.state
```

The raw run manifest is deliberately not committed because it contains absolute
paths and a wall-clock timestamp. `run.hashes.txt` retains the stable runtime
hashes:

```text
Results hash:      a2180de2747f491659fbdca5df2c692291ef58fcf4db901986c46b5294865cda
Final state hash:  4dc8c3759afa8aaf529687fe9fe9023f69d60e6188356b98b2f88e5f5b98a7ff
Observation hash:  1719b28589e3861419b2191a3ea543842b95985c301b57548deccce1d1d8ee16
```

PRD 0005 regenerated the following files because the placeholder direct rates
were replaced by the ABS-derived 2010 defaults:

- `run.csv`, `run.csv.summaries.csv`, and `run.hashes.txt`;
- `run.grouped.births_cells.csv` and `run.grouped.deaths_cells.csv`;
- `run.grouped.interstate_flows.csv`;
- `run.grouped.overseas_arrival_cells.csv` and
  `run.grouped.overseas_departure_cells.csv`; and
- `run.grouped.population_cells.csv` and `run.grouped.vacancy_cells.csv`.

No schema, transition, state-artifact byte, seed or rule expression changed.

PRD 0006 regenerated the canonical model and plan solely to add the three
sink-only grouped observations required by the target ledger. It added:

- `run.grouped.population_single_year_cells.csv` for structural age holdouts;
- `run.grouped.deaths_state_age_cells.csv` for state × event-age deaths; and
- `run.grouped.interstate_age_sex_flows.csv` for direction/state age-sex
  compositions.

Every pre-existing scalar, summary and grouped CSV remained byte-identical, as
did `run.hashes.txt`, the final-state hash, all parameters, all 418 transitions,
and the state artifact. The three new files are additive observation evidence.

`twenty_four_tick_golden_reproduces_all_committed_outputs` reruns the model and
compares the scalar, summary and ten grouped CSV files byte-for-byte.
`per_slot_golden_trajectory_preserves_identity_and_retirement_every_tick` then
executes the same plan rule identities, seed and 24 tick coordinates one tick at
a time in memory. At every boundary it inspects every slot, cross-checks event
and population counts against the committed CSV, and finally matches the
committed final-state hash. Together with
`all_eight_invariant_groups_hold_over_every_golden_tick`, this checks:

1. birth and overseas entrant streams remain available and both are exercised;
2. population, current exits and eligible/retired vacancies partition the fixed
   physical pool exactly;
3. every moving slot keeps its generation, advances age exactly once, records a
   distinct origin/destination, and preserves occupancy and entry stream;
4. national and all eight state stock-flow identities close exactly;
5. exited slots write `retired_slot` and remain vacant at every later boundary;
   at least one golden entrant is observed through activation → event clearing
   → exit → permanent retirement, in addition to the forced lifecycle test;
6. no invalid/sentinel age is observed;
7. generation increments only on first activation, never exceeds one, and every
   final per-slot value equals the trajectory-derived value; and
8. replay is bitwise deterministic.

The §K9 contrast is executable rather than rhetorical: the same test asserts
that this individual-agent interstate ledger has zero national residual on
every tick, while the accepted aggregate `DemographicSlots` golden contains at
least one nonzero `internal_arrivals_this_tick - internal_departures_this_tick`
residual. The aggregate imbalance is an explicit foreclosure of that different
model; it is not accepted for the Australian individual-agent model.
