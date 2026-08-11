# Demographic spine and aggregate-first calibration PRDs

Status: approved for implementation on 2026-08-11. PRD 0001 records its
binding decisions before implementation begins.

This ordered PRD set implements the foundation described in
[`docs/design/demographic-spine-and-modular-calibration.md`](../design/demographic-spine-and-modular-calibration.md):

1. replace broad seventeen-dimensional demographic NPE with a declared
   aggregate-first calibration experiment;
2. build and validate a deterministic aggregate companion for the Australian
   population model;
3. add a checked Lean-only person-facet fusion layer that emits the unchanged
   executable IR;
4. expose the existing Australian population model as a facet-ready demographic
   base without modifying its frozen artifacts; and
5. prove extensibility with synthetic labour and justice stock-state models.

The user selected the **foundation** scope. This folder does not acquire real
labour or justice data and makes no scientific claim about either domain. The
numbered specifications are currently moved—not copied—into
[`docs/prds-run-queue/`](../prds-run-queue/) after the observation-builder
prerequisite so one unattended software run can execute the complete approved
sequence.

`README.md` is ignored by `/piprd run`. Every queued demographic PRD must read
this file first. These constraints remain binding while a PRD is queued; when a
numbered PRD conflicts with this README, this README wins.

## Preconditions

- The Lean-IR formalization track's PRD 0009, “observation builders and macro
  delegation,” must be accepted before this folder runs. It is currently the
  pending observation-builder item in `docs/prds-run-queue/`. Facet work must
  build on its single pure final-model assembly boundary, not race or duplicate
  it.
- Commit this track and the design document before starting the managed run.
  `/piprd` requires the Sembla working directory to be clean.
- The retained Australian-population evidence, annual targets, model exports,
  fixtures and calibration reports must all be present and passing their current
  checks before PRD 0001.

If the observation-builder prerequisite is absent or the working tree is not
clean, stop before `/piprd run`; do not weaken a PRD to work around the missing
precondition.

## Authority and scope

- [`DESIGN.md`](../../DESIGN.md), especially §§4.1–4.6 and §§5.1, 5.5, remains
  the semantic authority.
- [`DECISIONS.md`](../../DECISIONS.md) §§J and N bind composition, scheduling,
  the current Australian model and its retained calibration evidence. PRD 0001
  appends §O; it does not rewrite historical §N decisions or evidence.
- The [design proposal](../design/demographic-spine-and-modular-calibration.md)
  is the readable input. After PRD 0001 it points to §O as authority.
- The current `australian_population` model is an immutable scientific baseline.
  A facet adapter may import and project it; no PRD may edit its source modules,
  generated parameter-family tables, canonical JSON/plan bytes, state fixtures,
  goldens or retained evidence.
- The folder implements frontend static fusion, not V1 `Share`/`Identify`.
  Authored facets disappear into one ordinary primitive box before executable
  IR is emitted.

## The frontend/runtime boundary is frozen

This entire track emits the current `Sembla.IR.Model` and current
`sembla.executable-plan/v1`. It makes **no** change to:

- `frontend/Sembla/IR.lean`, `frontend/Sembla/Json.lean` or
  `frontend/Sembla/Plan*.lean`;
- `Sembla.Composition.SourceV1`, linker semantics, source maps or plan identity;
- any Rust crate, CPU executor, CUDA code generator/kernel or state-artifact
  schema;
- `OutputBuilder`, one-row aggregate wire payloads, Ref semantics, table
  cardinality or the one-global-`dt` scheduler contract; or
- any schema/version/hash/identity string already accepted.

A PRD that concludes it needs person-granular row-projecting outputs, dynamic
rows, cross-row mutation, generic enum-keyed joins, scheduled clocks,
heterogeneous schedulers or `Share`/`Identify` must record the measured blocker
and stop. It must not implement that feature in this folder.

## Retained demographic evidence is immutable

The 2026-08-07 NPE run is retained evidence of the old declared experiment:
three accepted yearly posteriors, six SBC failures, six inadmissible posteriors,
and O-D WAPE worsening from gravity-only 22.4% to 29.4% after accepted spatial
updates. No PRD may regenerate, edit, delete or relabel that evidence.

The replacement experiment writes new artifacts alongside it. It never
rewrites history to make the aggregate-first path appear to have been the old
plan.

## Aggregate calibration contract

The replacement retains §N5's migration hazard and the fifteen annual spatial
parameters. It changes how parameters are identified:

- for candidate `φ = (peak_months, k)`, re-profile the fifteen spatial
  parameters against all 56 annual O-D cells;
- fit one pooled positive `φ` over run years 2010–2024 from conditional
  state/direction/sex/age compositions;
- never force the incompatible 2020 O-D and margin totals into one count
  likelihood;
- report overdispersed/cluster-robust uncertainty rather than treating the
  inverse Poisson Hessian as complete uncertainty;
- use a deterministic aggregate companion to calculate expected stocks and
  flows under the actual monthly contest/lifecycle semantics; and
- use micro-runs for parity and residual diagnosis, not to relearn the spatial
  block.

The evidence gate may conclude that an aggregate summary is only approximate or
that an age parameter remains unidentified. That is an acceptable scientific
finding. The fallback is the positive gravity fit plus the declared age prior
centre, never a clipped or raw-coordinate NPE posterior.

## Person-facet contract

The first release is deliberately restricted:

- one existing model, one target box and one target person table;
- fixed table cardinality and same-row effects only;
- each facet owns a unique set of appended Real, Int or enum attributes; Ref
  ownership is deferred because lifecycle assignment would require additional
  claim/resource semantics;
- a facet may read explicitly declared base or earlier-facet person attributes,
  plus declared fields on input ports already owned by the base box, but may
  create no port and write only its owned attributes;
- facet parameters, attributes, transitions and views append in source order;
- generated runtime names are `<facet-id>_<local-name>` and must satisfy the
  existing runtime-name grammar;
- every facet-owned field has exactly one entry initializer, appended to each
  explicitly named base entry transition as one atomic effect suffix;
- entry initializer right-hand sides are literals or facet parameters only;
  they never pretend to read demographic values written by that same transition,
  because all effects read the tick-start row;
- facet transitions, ordinary views and pure-API-only grouped views are
  automatically restricted to `occupancy = present`; the thin PRD 0006 surface
  exposes ordinary views only;
- exit transitions do not write facet fields: the next entry resets every field
  before a new generation becomes active, avoiding conflicting accepted writes
  or domain events deferring demographic exits;
- this does not make domain and exit events mutually exclusive: both may be
  counted in one tau-leap tick while the exited row becomes inactive. Synthetic
  fixtures must expose that behavior, and a real domain model must perform
  timestep sensitivity rather than hide it;
- the base owns `occupancy`, `generation`, demographic attributes and lifecycle
  event selection; facets never write those fields;
- empty fusion with the original model name is exact raw-model identity; and
- every successful fusion is certified by the existing pure final model builder
  and `checkModel`, never by a second checker inside a macro.

The facet layer may retain trusted parser/token mapping, as the current DSL does,
but semantic construction and validation are pure Lean functions. No accepted
facet construct may be inert.

## Frozen names and new artifact strings

| Concern | Frozen value |
|---|---|
| Decision section | `DECISIONS.md` §O |
| Track folder | `docs/prds-demographic-spine/` |
| Profile-fit format | `sembla.abs-profile-fit/v1` |
| Expectation format | `sembla.abs-expectation/v1` |
| Selection format | `sembla.abs-aggregate-selection/v1` |
| Lifecycle evidence format | `sembla.demographic-spine-lifecycle/v1` |
| Profile parameter root | `data/abs/params/profiled/` |
| Selected aggregate parameter root | `data/abs/params/aggregate/` |
| Foundation evidence root | `docs/evidence/demographic-spine-foundation/` |
| Facet namespace | `Sembla.Frontend.Facets` |
| Facet check script | `scripts/check-demographic-spine.sh` |
| Facet guide | `docs/guides/person-facets.md` |
| Foundation model doc | `docs/models/demographic-spine-foundation.md` |
| Synthetic demographic model | `demographic_spine_foundation` |
| Synthetic labour model | `demographic_labour_foundation` |
| Synthetic labour/justice model | `demographic_labour_justice_foundation` |

No new serialized facet schema is introduced. Facets are Lean authoring values
and disappear before `IR.Model` serialization.

## Verification required by every PRD

From the repository root, every implementation and review must run the checks
applicable to its files, plus the full repository gate:

```bash
./scripts/check.sh
git diff --check
python3 scripts/check-prd-allowlist.py <this PRD at its current path>
```

PRDs touching `data/abs/` must also run:

```bash
python3 -B -m unittest discover -s data/abs/tests -p 'test_*.py'
bash scripts/check-abs-data.sh
```

PRDs touching the Lean frontend or models must also run:

```bash
(cd frontend && lake build)
bash frontend/scripts/test-negative.sh
bash frontend/scripts/check-parity.sh
bash frontend/scripts/check-proofs.sh
```

PRDs 0007–0010 must run `bash scripts/check-demographic-spine.sh`. No PRD may
refresh a frozen fixture to obtain parity.

## Run order

| PRD | Purpose |
|---|---|
| 0001 | Record §O, approve the replacement experiment and frontend-only boundary |
| 0002 | Fit the pooled age profile while re-profiling the annual spatial block |
| 0003 | Implement the deterministic monthly demographic expectation runner |
| 0004 | Measure aggregate/micro parity and select the new calibration fallback path |
| 0005 | Add the pure checked person-facet fusion compiler |
| 0006 | Add the thin Lean facet declaration surface and lifecycle lowering |
| 0007 | Adapt the frozen Australian model as a facet base and land a runnable spine fixture |
| 0008 | Generate fixed-region feedback and a synthetic labour facet pilot |
| 0009 | Add a synthetic justice stock-state facet and explicit labour coupling |
| 0010 | Land integrated evidence, maintained docs and the deferred-semantics verdict |

Every later PRD depends on all earlier PRDs unless its text explicitly narrows
the dependency. Do not reorder or combine PRDs. In particular, do not start
facet syntax before the pure compiler, and do not add labour or justice state
before lifecycle reset tests pass.

## Global non-goals

- No real labour, justice, health, household or linked-person data acquisition.
- No calibrated or causal labour/justice claim; all domain pilots are synthetic
  conformance models.
- No production full-scale composed run or paid-hardware benchmark.
- No free O-D matrix or dyadic-affinity extension; that remains a separate
  declared model comparison.
- No modification of `calibration/npe` or its environment.
- No new Python, Lean or Rust dependency.
- No modification of existing parameter files under `data/abs/params/gravity/`
  or retained calibrated evidence.
- No row insertion, row deletion, arbitrary allocator, event stream, household,
  employer matching, court queue, sentence scheduler or one-to-many justice
  ontology.
- No public promise that the experimental facet syntax is a stable 1.0 API.
- No editing `.piprd/`, CI workflow semantics or unrelated active tracks.
