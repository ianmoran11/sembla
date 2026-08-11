# PRD 0008: Fixed-region dispatch and synthetic labour facet

## Dependencies

PRDs 0001–0007 accepted. Read the binding [demographic-spine README](../prds-demographic-spine/README.md) first. This is a synthetic
conformance pilot; it uses no Australian labour evidence and supports no labour
market estimate.

## Context

A writable enum residence cannot participate in the current Ref-keyed aggregate
join, but a small fixed geography can be compiled into guarded cases. Existing
input ports already carry one aggregate row and existing expressions can read a
named numeric field. The missing piece is authoring-time generation of one
transition per enum variant, not a new runtime lookup.

The labour pilot is the first evidence that a facet can own person state, read
demographic state and a regional input, survive entry/exit/re-entry, and preserve
base demographic behavior under an unwired/no-effect signal.

## Goal

Add a pure finite-enum dispatch helper and a two-region synthetic labour facet,
then retain runtime/CRN evidence that regional signals affect only the generated
labour transition family after the ordinary wire delay.

## Specification

### 1. Pure finite-region dispatch

Create `frontend/Sembla/Frontend/Facets/Regional.lean` and imported tests.
Expose a syntax-independent helper equivalent to `FiniteRegionDispatchSpec`
that receives:

- a facet transition local-name stem;
- a declared readable enum attribute;
- an ordered, complete variant list;
- an existing base input-port name;
- exactly one numeric input field per variant in the same order;
- a common Boolean guard and positive Real base hazard expression; and
- ordered effects writing only facet-owned attributes.

Emit one `FacetTransitionSpec` per variant, in variant order:

```text
name   = <stem>_<variant>
guard  = common_guard AND enum_attr = variant
hazard = common_hazard × inputSum(port, matching_field)
effects = common effects
```

Use the current `Expr.input` aggregate form and current transition/effect
constructors. Validate:

- enum/field lengths agree and both are nonempty;
- variants and fields are unique;
- every variant belongs to the target base enum at fusion time;
- every field exists in the named base input and is Real or Int as current
  multiplication/coercion rules permit;
- generated names do not collide; and
- the source variant order is the emitted transition order.

The helper must work for all eight Australian state spellings in a direct test.
It is static unrolling, not an enum lookup expression. Add fidelity/order/error
lemmas consistent with PRD 0005 proof hygiene.

### 2. Synthetic labour facet

Create `frontend/Sembla/Models/DemographicLabourFoundation.lean`, applying a
public `labour` facet to a demographic foundation base extended with an existing
`regional_labour_signal` input and a one-row `labour_policy` box/output wired to
it.

Public runtime model name: `demographic_labour_foundation`.

The labour facet owns at least:

```text
employment : {inactive, unemployed, employed}
earnings_band : Int
```

and positive parameters for entering/leaving employment. It declares person
reads for `age_months` and `area`, plus one explicit existing-input read for each
regional signal field consumed by the generated transitions. Required behavior:

- every entry atomically resets both owned fields;
- every labour transition/view is automatically restricted to present rows;
- working-age inactive rows may enter unemployment;
- unemployed rows find work through the generated per-area dispatch family;
- employed rows below the retirement threshold may lose work;
- a deterministic/test transition changes earnings band while employed;
- retirement applies only at/above that threshold and returns a row to inactive,
  making every pair of transitions that writes `employment` syntactically
  guard-disjoint; and
- no transition writes occupancy, generation, age, sex, area or any probe/base
  field.

Use two regions (`nsw`, `vic`) in the runnable fixture but directly test the
eight-state helper. Facets contribute only ordered ordinary views. After fusion,
the shared model builder from the prerequisite PRD may attach identical existing
model-layer grouped observations to both direct and surface twins; no grouped
facet-view syntax is implied or added by this PRD.

The policy box emits one aggregate row with one positive modifier per region.
It does not receive person rows. An unwired/neutral parameterization has
modifier one in both regions.

### 3. Exact structural and lifecycle tests

Imported Lean tests must establish:

- direct-spec and facet-surface values/models are exact twins, including every
  explicit base-input field read;
- two generated find-work transitions appear in region order and the eight-state
  unit fixture emits eight;
- every generated hazard reads the matching field and guard variant;
- every pair of labour transitions writing the same attribute has a pinned
  syntactically disjoint guard in this model;
- labour owns every labour write and no base write;
- entry-reset effects occur atomically after base effects in all entry
  transitions;
- every labour transition/view carries the generated present-row predicate;
- adding the labour facet preserves all base transition names, order, guards,
  hazards and claims, with only declared entry-effect suffixes changed;
- base exit transitions remain exact and vacant rows are excluded from labour
  observations despite retaining inactive old labour bytes;
- an empty/neutral labour comparison retains base demographic firings under CRN;
  and
- complete `checkModel`/plan validation succeeds with current schemas.

### 4. Fixtures and runtime evidence

Extend the fixture builder to generate model, plan and state artifacts under
`fixtures/demographic-spine/` for `demographic_labour_foundation`. The state is
small and synthetic, with both areas, sexes, working-age/status cells and
vacancies represented.

Extend `scripts/check-demographic-spine.sh` to:

1. regenerate and `cmp` the labour artifacts;
2. validate and run twice deterministically;
3. force entry→unemployed→employed→exit→re-entry paths, proving the vacant row
   is inactive/ignored and re-entry overwrites every labour field before the new
   generation is observed;
4. compare neutral versus region-skewed policy parameters with CRN;
5. assert no difference before the declared mailbox delay can reach the
   population;
6. assert the first difference is in the expected region's labour transition/
   state, with demographic transition draws/firings unchanged where guards are
   unchanged; and
7. retain scalar output, externally attached model-layer grouped output and
   final-state hashes.

Do not use aggregate equality alone to prove person lifecycle behavior.

### 5. Evidence and documentation

Add `docs/evidence/demographic-spine-foundation/labour/` containing the exact
fixture hashes, structural manifest, lifecycle result, CRN comparison and
reproduction commands. It may use fully synthetic target values only to prove
observation plumbing; label them `conformance_expected`, never fitted/held-out
empirical targets.

Extend the person-facet guide and foundation model doc with:

- finite enum dispatch lowering;
- the one-row regional signal and wire delay;
- labour-owned versus demographic-owned fields;
- the absence of employer/vacancy matching, households and real earnings; and
- why the pilot does not identify a labour parameter or validate labour science.

## Allowed files

- `frontend/Sembla/Frontend/Facets/Regional.lean` (new)
- `frontend/Sembla/Frontend/Facets/RegionalTests.lean` (new)
- `frontend/Sembla/Frontend/Facets.lean` (import only)
- `frontend/Sembla/Models/DemographicLabourFoundation.lean` (new)
- `frontend/Sembla/Models/DemographicLabourFoundationTests.lean` (new)
- `frontend/Sembla/Models.lean` (import/export only)
- `frontend/Sembla.lean` (test imports only)
- `frontend/Main.lean` (foundation export alias only if required)
- `scripts/build-demographic-spine-fixtures.py`
- `scripts/check-demographic-spine.sh`
- `scripts/tests/test_build_demographic_spine_fixtures.py`
- `fixtures/demographic-spine/**` (new labour artifacts only)
- `fixtures/state/demographic_labour_foundation.state` (new)
- `docs/evidence/demographic-spine-foundation/labour/**` (new)
- `docs/guides/person-facets.md`
- `docs/models/demographic-spine-foundation.md`
- implementation notes/artifacts created by the managed run

## Non-goals

- No ABS Labour Force, Census, HILDA, PLIDA, employer, wage or vacancy data.
- No real target ledger, calibration, inference or policy conclusion.
- No generic enum-keyed lookup/join, person-granular output, new input/output
  builder or IR/runtime change.
- No employer table matching, capacity, household, occupation or industry.
- No change to Australian population parameters, artifacts or evidence.

## Acceptance criteria

1. All frontend, foundation-script, full repository, allowlist and diff checks
   from the README pass.
2. The pure helper emits complete ordered guarded families through current IR,
   passes its theorem/error matrix and emits exactly eight transitions for the
   Australian state test.
3. The two-region labour model is an exact direct/surface twin, passes current
   checking/plan validation and preserves every undeclared base field/rule.
4. Regenerated model/plan/state fixtures are byte-identical and runtime tests
   prove atomic entry/re-entry reset, present-row guards and inactive vacant
   state by row and generation.
5. CRN evidence pins the mailbox delay, expected regional labour effect and
   noninterference of demographic behavior outside declared guard changes.
6. Evidence is hash-complete and every synthetic expectation is labelled
   conformance-only.
7. Documentation names all missing labour mechanisms/data and makes no empirical
   or causal claim.
8. Existing model/IR/plan/composition/Rust/dependency/canonical artifacts remain
   unchanged.
