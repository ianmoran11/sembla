# PRD 0007: Australian facet-base adapter and runnable spine fixture

## Dependencies

PRDs 0001–0006 accepted. Read the binding [demographic-spine README](../prds-demographic-spine/README.md) first. The frozen
`australian_population` source, exports, fixtures and evidence are immutable.

## Context

The facet compiler needs two kinds of evidence before domain pilots:

1. an adapter proving that the complete Australian model can be named as a
   facet base without changing one raw byte under empty fusion; and
2. a small runnable fixed-pool model where entry, exit, generation and facet
   reset behavior can be inspected exhaustively without creating a new
   scientific Australian-population artifact.

The existing Australian entrant slots remain single-use under §N14. This PRD
does not change that scientific lifecycle. Slot reuse is demonstrated only in
the synthetic foundation fixture.

## Goal

Expose facet-base metadata for the frozen Australian model, land a separate
small `demographic_spine_foundation` model and state fixture, and permanently
check atomic facet initialization/reset and `(row ordinal, generation)` identity
through current CPU execution.

## Specification

### 1. Frozen Australian adapter

Create `frontend/Sembla/Models/AustralianPopulationSpine.lean` defining a public
`australianPopulationFacetBase` (or house-style equivalent) with:

- base model `Sembla.Models.australianPopulation`;
- target box `demographic`;
- target table `person_slot` using the actual exported runtime spelling;
- occupancy attribute/variant `occupancy = present`;
- generation attribute `generation`; and
- all eight `birth_<area>` and eight `overseas_arrive_<area>` transition names
  as entry transitions in existing order.

Derive the entry list from the same finite area authority used by the model or
from checked structural projections; do not maintain a hand-copied list.

Required guards:

- every named transition resolves uniquely and targets `PersonSlot`;
- the occupancy/present and generation declarations have the required types;
- no death, emigration, move, ageing or event-clear transition is classified as
  entry;
- empty same-name fusion returns structural and `IR.toJson` equality with the
  frozen public model; and
- all existing Australian model/plan/table hashes and validation guards remain
  unchanged.

This adapter is metadata and an exact identity test. It does not export a new
full Australian state or claim that current single-use slots are reusable.

### 2. Synthetic demographic spine model

Create `frontend/Sembla/Models/DemographicSpineFoundation.lean` with public
runtime model name `demographic_spine_foundation`. It is a small conformance
model, not an Australian estimate.

Frozen shape:

- one `population` box;
- `PersonSlot`, 64 rows, with `occupancy {vacant,present}`, `generation : Int`,
  `age_months : Int`, `sex {female,male}`, `area {nsw,vic}`, an event marker and
  `slot_resource : SlotResource`;
- `SlotResource`, 64 empty rows with one-to-one refs;
- reusable `enter`, `exit`, `move_nsw_vic`, `move_vic_nsw`, `age_monthly` and
  `clear_event` transitions using existing guards/effects/contests only;
- deterministic-test parameters with ordinary positive defaults; and
- scalar/grouped views sufficient to inspect occupancy, generation, area,
  event and lifecycle validity.

The base owns occupancy, generation increment, age/area and event choice.
Entry increments generation exactly once. Exit vacates the row. There is no
row creation/deletion.

Add one synthetic `probe` facet through the public PRD 0006 surface:

```text
probe_episode : Int
probe_status : {clear,marked}
```

with complete entry initializers and one ordinary transition that marks a
present row. The compiler automatically guards the transition and facet views
by `occupancy = present`. The emitted model remains named
`demographic_spine_foundation`; the direct unfused base may be private/test-only.

### 3. Deterministic state and exported artifacts

Add a standard-library script
`scripts/build-demographic-spine-fixtures.py` using the existing
`sembla.state/v1` writer to produce:

- `fixtures/demographic-spine/demographic_spine_foundation.model.json`;
- `fixtures/demographic-spine/demographic_spine_foundation.plan.json`;
- `fixtures/state/demographic_spine_foundation.state`; and
- a canonical build report with schema/model/plan/state hashes and row/cell
  counts.

Initial state must contain a declared mix of vacant/present rows, both sexes and
areas, generation zero for never-used slots and nonzero generations for a
small reuse test population. Every Ref is one-to-one. The script writes to
explicit override paths in tests and performs no network access.

Add a `sembla-export` alias only if required for deterministic model/plan
regeneration. Do not add the foundation model to an existing frozen canonical
catalog whose exact list is a compatibility fixture; use an append-only
foundation registry or explicit alias.

### 4. Runtime lifecycle evidence

Create `scripts/check-demographic-spine.sh`. At this PRD it must:

1. regenerate model, plan and state into a temporary directory and `cmp` every
   committed artifact;
2. validate model/plan/state with existing CLI commands;
3. run twice at a fixed seed and compare complete output/final-state hashes;
4. run targeted forced-rate fixtures that establish entry, mark, exit and
   re-entry in separate ticks, plus one declared same-tick mark/exit cofire;
5. inspect every row/generation boundary and prove:
   - entry increments generation once;
   - probe fields receive entry values atomically;
   - exit leaves the row vacant and facet transitions/views ignore its inactive
     probe bytes;
   - the same-tick cofire counts both events without a conflicting write, then
     exposes no active probe state;
   - re-entry overwrites probe fields and cannot inherit the prior generation;
   - moves preserve row ordinal and generation; and
   - no invalid occupancy/generation/resource combination occurs;
6. compare CPU execution with the existing CUDA backend only when CUDA is
   available, treating local unavailability as skipped hardware evidence rather
   than a pass claim.

Use generated state/readback evidence or existing state export; do not infer
row behavior only from aggregate CSV.

Retain the result under
`docs/evidence/demographic-spine-foundation/demographic/`:

- `README.md` with scope and reproduction commands;
- `lifecycle-result.json` with format
  `sembla.demographic-spine-lifecycle/v1`, exact model/plan/state/parameter/
  output hashes, every targeted row boundary before/after, generation, event
  firings, vacant-visibility checks and the declared cofire result;
- `structure.json` with adapter/fusion inventory and frozen Australian hash
  comparison; and
- `SHA256SUMS` over the retained directory files.

The report is regenerated from the fixed fixture by
`scripts/check-demographic-spine.sh` into a temporary path and compared exactly.

### 5. Lean tests and documentation

Add imported tests for:

- exact Australian empty-fusion identity and entry-family completeness;
- exact synthetic macro/direct-spec twin;
- generated attribute/transition/effect/view order;
- `checkModel` acceptance and direct-stable plan success;
- one-writer ownership, complete probe entry reset and automatic present
  guards; and
- foundation model names/counts pinned independently of serialization.

Create `docs/models/demographic-spine-foundation.md` documenting the adapter,
synthetic model, identity/lifecycle evidence, single-box lowering and explicit
non-claims. Link it from the model/documentation indexes. State prominently that
it is a conformance foundation, not a population estimate.

## Allowed files

- `frontend/Sembla/Models/AustralianPopulationSpine.lean` (new)
- `frontend/Sembla/Models/DemographicSpineFoundation.lean` (new)
- `frontend/Sembla/Models/DemographicSpineFoundationTests.lean` (new)
- `frontend/Sembla/Models.lean` (imports/exports only)
- `frontend/Sembla.lean` (test import only)
- `frontend/Main.lean` (foundation export alias only if required)
- `scripts/build-demographic-spine-fixtures.py` (new)
- `scripts/check-demographic-spine.sh` (new)
- `scripts/tests/test_build_demographic_spine_fixtures.py` (new)
- `fixtures/demographic-spine/**` (new)
- `fixtures/state/demographic_spine_foundation.state` (new)
- `docs/evidence/demographic-spine-foundation/demographic/**` (new)
- `docs/models/demographic-spine-foundation.md` (new)
- `docs/README.md` (model link only if its current structure calls for one)
- `docs/design/README.md` (link only if needed)
- implementation notes/artifacts created by the managed run

## Non-goals

- No edit to any file under
  `frontend/Sembla/Models/AustralianPopulation/`, the public
  `AustralianPopulation.lean`, its data tables or existing fixtures/evidence.
- No full Australian fused state/model artifact and no change from single-use
  Australian entry slots.
- No labour/justice state, regional feedback, real data or calibration.
- No new runtime feature, state schema, canonical catalog rewrite or paid
  hardware requirement.

## Acceptance criteria

1. All frontend, fixture-script, foundation-script, full repository, allowlist
   and diff checks from the README pass.
2. The Australian adapter derives the complete entry family, validates
   occupancy/present and generation, and empty fusion is exact raw/JSON identity;
   every frozen Australian hash/artifact is unchanged.
3. The 64-row foundation model uses only current IR, passes `checkModel`, exports
   a valid direct-stable plan and has byte-reproducible model/plan/state/build
   artifacts.
4. Direct-spec and surface models are exact twins with pinned declaration,
   entry-effect and automatic-presence-guard order.
5. The canonical lifecycle/structure evidence is hash-complete and targeted
   runtime evidence proves atomic entry reset, inactive vacant-state exclusion,
   declared same-tick domain/exit cofire without write conflict, re-entry
   generation isolation, move identity and one-to-one resources by inspecting
   rows, not only aggregates.
6. `scripts/check-demographic-spine.sh` regenerates with `cmp`, validates and
   runs twice deterministically; optional CUDA status is reported honestly.
7. Documentation calls the model synthetic/conformance-only and distinguishes
   the frozen single-use Australian model from the reusable fixture.
8. No executable IR, plan/schema, composition, Rust, dependency or existing
   canonical artifact changes.
