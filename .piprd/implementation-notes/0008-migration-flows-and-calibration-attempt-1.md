# PRD 0008 implementation notes — attempt 1 revision

## Bounded compare bridge

The first implementation pass stopped because PRD 0006 froze grouped-view
`compare` and the existing compare formatter required SIR-only columns. The
revision instruction explicitly adopted the advisor's narrowly bounded bridge
as the implementation plan. Accordingly this revision changes
`crates/sembla-cli/src/main.rs`, `manifest.rs`, and focused CLI tests only to:

- accept explicit repeatable `--enable` values on `compare`;
- validate and execute both CPU arms with the runtime feature set;
- provide a generic same-model parameter-contrast CSV over the model's scalar
  and firing columns when the legacy SIR columns are absent;
- preserve the legacy SIR formatter and its exact checked golden bytes;
- record the existing `enabled_features` and direct-stable plan identity tuple
  in compare manifests;
- leave grouped CUDA execution and grouped `diff-backends` rejected.

Plan identity never implicitly enables a runtime feature. The demographic
compare command passes `--enable grouped-observations` explicitly. No manifest
schema or dependency was added.

## Model and state extension

`frontend/Sembla/Models/DemographicSlots.lean` now contains overseas/internal
arrivals, emigration/internal departure, their four parameters with priors,
flow/capacity views, `vacancy_cells`, flow summaries, and the minimum birth-slot
margin. Every death transition and both departure transitions claim the same
per-row `slot_resource`; one age band plus two departure types therefore race.
Internal arrival remains an independent activation and no residual correction
was added.

The deterministic state generator now retains the 4,000 present row values and
classifies the 1,000 vacancies as 600 birth, 250 overseas, and 150 internal
slots. Migration slots receive deterministic non-scientific entry-age spreads,
including a small plausible high-age stratum used by the grouped arrival-age
assertion. Sex, area, generation, and exclusive resources remain deterministic.
The Lean table declaration and artifact table order are both canonical
`Area`, `PersonSlot`, `SlotResource`, allowing the same strict state artifact to
load through both the canonical model and its direct-stable plan.

## Regenerated folder-internal artifacts

The following PRD 0007 folder-internal artifacts were explicitly regenerated
because the model gained parameters, transitions, views, summaries, canonical
table order, and migration behavior:

- `fixtures/state/demographic_slots.state`;
- `fixtures/demographic/demographic_slots.json`;
- `fixtures/demographic/demographic_slots.plan.json`;
- `fixtures/demographic/goldens/run.csv`;
- `fixtures/demographic/goldens/run.csv.summaries.csv`;
- `fixtures/demographic/goldens/run.grouped.population_cells.csv`;
- `fixtures/demographic/goldens/run.grouped.deaths_cells.csv`;
- `fixtures/demographic/goldens/run.manifest.normalized.json`;
- `fixtures/demographic/goldens/run.hashes.txt`.

`fixtures/demographic/goldens/run.grouped.vacancy_cells.csv` is new because
PRD 0008 adds the stratified capacity diagnostic.

New calibration inputs and goldens under `fixtures/demographic/calibration/`
cover a six-vector independent-noise theta sweep, exported `(theta, summary)`
pairs, a generic CRN compare, and both chained windows. The compare arms change
arrival rates only, so tick 0 pins arrival-driven divergence while unchanged
birth/death/departure draws remain identical under DECISIONS §J4.

## Acceptance evidence

The golden seed 7007 over 24 ticks produces all six flows, real deferred exit
competition, a nonzero internal residual, no invalid ages, generation 3, and a
minimum clear birth margin of 583. Tests assert:

- full six-flow stock accounting at every tick;
- logical cumulative-entry/exit closure and a 5,000-row physical partition;
- summary/marker equality, grouped population/death/vacancy totals, and
  bitwise reproduction twice;
- a tick-0 high-age grouped arrival cell;
- visible unreconciled internal residuals;
- entry-marker lockout across all three entry streams;
- deterministic isolated overseas-stratum exhaustion with other entries
  continuing and the surfaced deferred counter pinned to zero;
- direct-stable sweep plan tuples/features, pairs columns, CRN first divergence,
  and chained state-link hashes with golden outputs and manifests.

Documentation records the flow semantics, §K9 trigger, §K10 residual contract,
capacity rule, calibration workflow, measured values, and explicit non-claims.

## Frozen contracts

No dependencies were added. `Cargo.lock`, `DESIGN.md`, `examples/**`,
pre-existing non-demographic CSV/hash goldens, composition schemas, legacy SIR
compare output, Philox/conflict semantics, and CUDA numeric behavior remain
unchanged.

## Validation

Passed:

- `cargo test --locked -p sembla-cli --test demographic_slots` (14 passed,
  four explicit regeneration tests ignored);
- focused `compare`, `grouped_observations`, `run_manifest`, and `sweep` test
  targets;
- all four explicit demographic regeneration tests, followed by SHA-256
  comparison of all 37 state/demographic fixture files (byte-identical);
- `frontend/scripts/test-negative.sh` and `frontend/scripts/check-parity.sh`;
- `./scripts/check.sh`, including formatting, Clippy, workspace tests, Lean,
  proof hygiene, parity, documentation, dependency, and lock checks;
- `git diff --check`.
