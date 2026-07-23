# Demographic slot-model PRDs

Ordered PRD set implementing the framework extensions and the aggregate
demographic model scoped in the Australian-population use case, as assessed
and amended in the decision record (PRD 0001 → DECISIONS.md §K). The driving
goal: a fixed-pool **person-slot model** of ageing, births, deaths, overseas
and internal migration, calibratable against area × age × sex stocks and
flows — built on the *minimum* framework extensions, with the full
individual-linkage semantics explicitly deferred. Run from the Sembla
repository with:

```text
/piprd run docs/prds-demographic-slots
```

`README.md` is ignored by `/piprd run`. Every numbered PRD must read this
file first; the constraints below are binding. When a PRD conflicts with
this README, this README wins.

## Authority and scope

- `DESIGN.md` (especially §4.2 kernel fragment, §4.6 observation-as-sink,
  §5.4 manifest rules, §5.5 default-off flags), `DECISIONS.md` (§§E3, E7,
  E8, I1–I6, J), and both prior PRD folders' frozen contracts bind.
- The use-case note is **input, not authority**: where this README deviates
  from it (chained runs instead of checkpoints, loaded rate columns instead
  of rate tables, deferred pairing/event-sink), the deviation is a recorded
  decision (§K), not drift.
- Frozen and untouchable: `examples/**`, all CSV/hash goldens, plan/source
  schemas and version strings from the composition track (one exception:
  the known-feature set, below), the `SEMBLA_POP` binary format and every
  SIR legacy path, the negative-suite expectations, Philox layout, conflict
  argmin semantics, and CUDA numeric contracts.
- **Deferred with named triggers** (record in §K; reject, don't half-build):
  `Expr::Tick`/derived age (trigger: PRD 0009 measures ageing-write cost as
  material); categorical draws, cross-row writes, mother-linked births,
  vacant-slot claiming, non-exclusive `Ref` reassignment, household refs
  (trigger: aggregate model shows identity linkage is scientifically
  required → design-options note first); paired migration events/quotas
  (trigger: reported balance residual unacceptable → Option D Phase 6);
  keyed contest orderings (v0.5); event-stream sinks; sub-annual rate
  tables; CUDA support for grouped observations (follow-up folder).

## Frozen names and version strings

| Concern | String |
|---|---|
| State artifact format | `sembla.state/v1` |
| State artifact hash domain | `sembla.state-artifact/v1` (SHA-256 over exact file bytes) |
| State artifact magic | ASCII `SEMBLA_STATE` (12 bytes) |
| Feature flag (the first ever) | `grouped-observations` |
| CLI flag mechanism | `--enable <feature>` (repeatable) |
| Grouped output file | `<out-stem>.grouped.<view>.csv` beside the run output |
| Canonical model | `demographic_slots` (box `demographic`) |
| Benchmark script | `scripts/bench-demographic.sh` |

## Frozen state-artifact format (`sembla.state/v1`)

One binary file, loadable wherever `--population` accepts a path (dispatch
on magic bytes; `SEMBLA_POP` files keep their exact legacy path):

1. **Magic:** the 12 bytes `SEMBLA_STATE`.
2. **Header length:** little-endian `u32`.
3. **Header:** canonical JSON (`sembla.canonical-json/v1` rules — sorted
   keys, compact, no trailing newline) with exactly:
   `{"schema_version": "sembla.state/v1", "tables": [...]}` where each
   table entry is `{"box": ..., "table": ..., "row_count": N, "columns":
   [{"name": ..., "type": "real"|"int"|"enum"|"ref", "variant_count": N?,
   "ref_target": {"box":...,"table":...}?}]}` — `variant_count` present iff
   enum, `ref_target` present iff ref (all-present-or-absent per type).
   Tables and columns appear in **model declaration order**.
4. **Column blobs:** immediately after the header, in header order, raw
   little-endian arrays matching the runtime's `ColumnData` exactly
   (`crates/sembla-runtime/src/state.rs:11-15`): `real` = `f64`, `int` =
   `i64`, `enum` = `u16` variant indices in the model's declaration order,
   `ref` = `u32` row indices. No padding, no alignment, no trailing bytes.

Load-time validation (deterministic errors, never repair): magic/version;
header/table/column **exact bijection** with the validated model's boxes,
tables, and attrs (name and type); `row_count` **must equal** the model's
declared `rows` for every table — this is the PRD that makes `rows :=` an
enforced contract instead of a size hint, for artifact-loaded runs; enum
values `< variant_count` and `variant_count` equal to the model's; ref
values `< ref_target.row_count`; file length exactly consistent with the
header. The artifact carries **no** execution metadata (no tick, seed, or
model hash inside the file) — manifests own that.

## Chained runs, not checkpoints (the reframe)

There is no checkpoint/restart subsystem (standing-no #6 stands). Instead:

- `sembla run … --export-state <path>` writes the final committed state as
  a `sembla.state/v1` artifact.
- The run manifest gains two optional all-present-or-absent tuples:
  `initial_state: {format, hash: {algorithm, domain, digest}}` (when the
  population input was a state artifact) and `exported_state: {format,
  hash}` (when `--export-state` was used). Legacy manifests are
  byte-unchanged (absent fields).
- An annual calibration window is **one run**: its own seed, θ, ticks, and
  manifest, chained to the next by the state artifact whose hash appears in
  both manifests. Chaining is explicitly **not** bitwise-equivalent to one
  continuous run (tick coordinates restart) — this is documented and
  test-asserted, never hidden.

## The `grouped-observations` flag (first §5.5 flag)

- A runtime option (`--enable grouped-observations`), never a Cargo
  feature. Enabled flags are recorded sorted in the run manifest
  (`enabled_features`, absent when empty — legacy manifests unchanged).
- A model containing grouped views is **rejected** with a diagnostic naming
  the flag unless it is enabled; when enabled, the construct has full
  elaboration, validation, and runtime meaning. No inert syntax.
- The IR carries grouped views as a new **optional** `grouped_views` field
  on `Box` (serde default + skip-if-empty: legacy model JSON bytes are
  unchanged). A `direct_stable` plan export of such a model lists
  `"grouped-observations"` in `enabled_features`; the plan validator's
  known-feature set grows from `{}` to `{"grouped-observations"}` — this
  is the **one sanctioned revision** to the composition track's
  "exactly `[]`" rule, recorded in §K. Unknown features still reject.
  Composition **sources** may not contain grouped views in this folder
  (parser rejects deterministically; linker support is a follow-up).
- Observation stays a sink (DESIGN §4.6): the mechanical test is that two
  models differing only in grouped views produce bitwise-identical state
  hashes, `results.csv`, and fired traces.
- V1 is CPU-backend only: `--backend cuda` with grouped views present is a
  deterministic rejection naming the limitation.

## Frozen surface syntax added by this folder

```lean
-- PRD 0004: arithmetic set effects (Int/Real attrs; enum/Ref unchanged)
set age_months := age_months + 1
set generation := generation + 1

-- PRD 0004: Int parameters (no priors on Int; positioned diagnostic)
param start_calendar_month : Int := 24_000

-- PRD 0005: contest declarations (race_time only; keyed orderings v0.5)
transition die on PersonSlot where
  guard occupancy = present
  hazard mortality_hazard
  contest slot_resource by race_time
  set occupancy := vacant
  set event := death

-- PRD 0006: grouped views (count only; band only on Int attrs)
grouped view population_cells :=
  count PersonSlot by sex, area, band age_months 60 where occupancy = present
```

Grouped output is long-format CSV, header
`tick,<key1>,…,<keyN>,count`, one row per **non-empty** group, sorted by
`(tick, key tuple)`; enum keys render as variant names, ref keys as row
indices, band keys as `floor(value / width)` band indices.

## Required checks for every PRD

```bash
./scripts/check.sh
cd frontend && lake build
bash frontend/scripts/check-parity.sh
git diff --check
```

PRDs touching surface syntax (0004, 0005, 0006, 0007, 0008) must also run
`bash frontend/scripts/test-negative.sh`. Any diff to a frozen artifact is
a failed PRD.

## Run order

1. `0001-decision-record.md` — DECISIONS §K, roadmap amendments.
2. `0002-state-format-and-loader.md` — `sembla.state/v1` + generic loader.
3. `0003-state-export-and-chained-runs.md` — `--export-state`, manifest
   tuples, chained-run tests.
4. `0004-surface-arithmetic-and-int-params.md` — arithmetic `set`, Int
   params.
5. `0005-contest-syntax.md` — `contest … by race_time`.
6. `0006-grouped-observations.md` — the flag, IR construct, runtime,
   grouped CSV.
7. `0007-birth-death-slot-model.md` — the aggregate model, balance
   invariants, lockout accounting.
8. `0008-migration-flows-and-calibration.md` — overseas/internal flows,
   residual reporting, sweep/CRN fixture.
9. `0009-benchmark-50m.md` — scale measurement; produces the `Expr::Tick`
   trigger data.

Later PRDs depend on every earlier PRD. Do not combine, reorder, or
implement a later PRD's constructs early — each earlier PRD's validator or
elaborator must keep rejecting them deterministically.

## Global non-goals

- No dynamic row allocation, stream-compaction birth/death, or
  `(tick, parent, slot)` entity IDs — the slot pool exists to avoid them;
  the roadmap's flagged birth/death design remains deferred with its
  trigger unchanged.
- No initial-population *generation* beyond deterministic test/benchmark
  synthesis clearly labelled non-scientific (DESIGN §10.5 stands).
- No new Rust or Lean dependencies; no CUDA kernel changes.
- No changes to composition source/plan schemas beyond the single
  known-feature addition; no linker changes.
- No NPE/Python-side work; calibration fixtures stop at `sweep`,
  `--export-pairs`, and `compare`.
- No editing `.piprd/`, CI workflow semantics, or external vault copies
  (the use-case note is read-only input).
