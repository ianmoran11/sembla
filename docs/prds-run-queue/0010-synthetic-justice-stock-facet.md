# PRD 0009: Synthetic justice stock-state facet and explicit coupling

## Dependencies

PRDs 0001–0008 accepted. Read the binding [demographic-spine README](../prds-demographic-spine/README.md) first and the maintained
`docs/justice-event-schema-and-aggregate-statistics.md` plus
`docs/justice-event-schema-implementation-sequence.md`. Their distinction
between the normalized evidence ontology and the executable stock-state MVP
binds.

## Context

The current runtime can express same-row stock-state transitions but not
arbitrary matter/event row creation, cross-row writes, scheduled hearings or
one-to-many justice histories. The foundation pilot must stay on the expressible
side of that boundary.

This PRD also tests the facet dependency rule: justice may explicitly read an
earlier labour attribute, but labour does not read or write justice state. A
coupling parameter then has a precise owner and a CRN counterfactual can prove
that changing it leaves demographic/labour transition streams untouched.

## Goal

Add a synthetic justice stock-state facet after the labour facet, demonstrate
an explicit read-only labour→justice coupling and lifecycle reset, and retain
evidence that the model does not require or pretend to provide deferred event,
row-allocation or scheduler semantics.

## Specification

### 1. Justice facet

Create `frontend/Sembla/Models/DemographicLabourJusticeFoundation.lean` with
public runtime model name `demographic_labour_justice_foundation`. Fuse facets
in this exact order:

```text
labour
justice
```

The justice facet owns at least:

```text
justice_status : {none_, remand, sentenced}
offence_group : {none_, property, violent}
justice_event : {none_, admission, sentencing, release}
```

and positive parameters for admission, remand→sentence and release, plus a
positive `unemployment_multiplier`. It explicitly reads:

```text
age_months
sex
area
labour_employment
```

Required same-row behavior:

- eligible rows in `justice_status = none_` may enter remand;
- admission hazard uses only declared demographic fields, the explicit
  `labour_employment` read and justice-owned parameters;
- unemployment modifies admission through a named guarded arithmetic factor;
- remand may become sentenced;
- sentenced rows may be released to `none_`;
- a one-tick justice event marker records admission, sentencing or release and
  prevents a new justice event until it is cleared; and
- every entry atomically resets all justice fields to their declared sentinel
  values, while all justice transitions/views are restricted to present rows.

Exit leaves justice bytes inactive on the vacant row. They are excluded from
facet behavior and overwritten by the next entry before the new generation is
observable. No generic exit cleanup writes a justice field.

Every effect writes a justice-owned attribute. No justice transition writes
occupancy, generation, age, sex, area, labour state or probe state. There is no
feedback from justice to labour in this foundation model.

### 2. Stock observations only

Add ordered ordinary views sufficient to report the small fixture's current
stock cells by:

- justice status;
- sex and age band;
- area;
- labour employment status; and
- offence group.

One-tick event markers may count admissions, sentencing and releases if they are
same-row bounded state. Do not add matter, charge, hearing, sentence-order,
custody-episode or event-stream tables. Do not call the current-state projection
an event-history model.

Any synthetic expected counts are conformance fixtures. They do not use or
imitate published ABS prisoner totals.

### 3. Structural and dependency tests

Imported Lean tests must establish:

- direct-spec and facet-surface exact twins;
- generated justice names, params, attrs, transitions, entry-reset effects and
  views in source order after labour declarations;
- `labour_employment` resolves only because it is an explicit earlier-facet
  read;
- reversing facet order or deleting the read declaration fails at the authored
  dependency token;
- all justice effects write only justice attributes;
- labour/base model projections are unchanged except appended justice state and
  declared entry-effect suffixes;
- entry reset covers every justice attribute on every base entry;
- every justice transition/view has the generated present-row predicate and
  every base exit remains unchanged;
- no table other than the existing person table is added by the justice facet;
  and
- current `checkModel` and direct-stable plan validation pass.

Add one-defect negatives for a justice write to labour state, an undeclared
cross-facet read, reversed dependency, missing entry reset and an attempted
unsupported justice matter/event-table declaration.

### 4. Fixtures and row-level runtime evidence

Extend the standard fixture builder and check script for:

- `fixtures/demographic-spine/demographic_labour_justice_foundation.model.json`;
- its direct-stable plan and build report; and
- `fixtures/state/demographic_labour_justice_foundation.state`.

The synthetic state must include eligible/ineligible ages, both sexes/areas,
each labour status and each initial justice stock state in declared small
counts.

Runtime checks must:

1. regenerate and `cmp` model/plan/state artifacts;
2. run twice with exact output/final-state hashes;
3. force admission→sentence→release and exit→re-entry paths, inspecting row and
   generation after every boundary;
4. prove vacant justice bytes are ignored and entry overwrites every field before
   a new generation can be observed;
5. compare `unemployment_multiplier = 1` with a larger declared value using
   CRN;
6. assert all demographic and labour transition firing streams, state columns
   and draw identities are bitwise equal between those two arms;
7. assert the first permitted difference is a justice admission firing/state;
   and
8. retain the ordered stock-cell views and event-marker evidence.

If changing the coupling parameter perturbs labour/demographic behavior, the
model has violated the declared one-way dependency and the PRD fails.

### 5. Evidence and documentation

Create `docs/evidence/demographic-spine-foundation/justice/` with exact hashes,
structural/dependency manifests, lifecycle result, CRN coupling comparison and
reproduction commands.

Extend the facet guide and foundation model doc. Cross-link—but do not rewrite—the
justice schema/sequence documents. Documentation must state:

- this is a synthetic current-state projection;
- `offence_group` is a tiny conformance enum, not ANZSOC evidence;
- no Indigenous-status or other empirical subgroup is synthesized;
- no court/prison capacity, matter concurrency, sentence schedule or recidivism
  history is represented;
- the unemployment multiplier is a test parameter, not an estimated or causal
  effect; and
- real justice work requires a dedicated evidence/initial-population track.

## Allowed files

- `frontend/Sembla/Models/DemographicLabourJusticeFoundation.lean` (new)
- `frontend/Sembla/Models/DemographicLabourJusticeFoundationTests.lean` (new)
- `frontend/Sembla/Models.lean` (import/export only)
- `frontend/Sembla.lean` (test import only)
- `frontend/Main.lean` (foundation export alias only if required)
- `frontend/Sembla/Positive/**` (new justice facet fixtures only)
- `frontend/Sembla/Negative/**` (new justice facet fixtures only)
- `frontend/scripts/test-negative.sh`
- `scripts/build-demographic-spine-fixtures.py`
- `scripts/check-demographic-spine.sh`
- `scripts/tests/test_build_demographic_spine_fixtures.py`
- `fixtures/demographic-spine/**` (new justice artifacts only)
- `fixtures/state/demographic_labour_justice_foundation.state` (new)
- `docs/evidence/demographic-spine-foundation/justice/**` (new)
- `docs/guides/person-facets.md`
- `docs/models/demographic-spine-foundation.md`
- `docs/justice-event-schema-and-aggregate-statistics.md` (link only if needed)
- `docs/justice-event-schema-implementation-sequence.md` (link only if needed)
- implementation notes/artifacts created by the managed run

## Non-goals

- No real prisoner, court, corrective-services, ANZSOC, labour or linked data.
- No empirical target ledger, initial synthetic Australian prisoner population,
  fitting, validation or causal interpretation.
- No matter/charge/event/episode rows, dynamic allocation, cross-row effect,
  append-only stream, court queue, capacity or scheduled clock.
- No justice→labour/demography feedback and no new composition/runtime feature.
- No change to Australian-population scientific artifacts.

## Acceptance criteria

1. All frontend, negative, foundation-script, full repository, allowlist and
   diff checks from the README pass.
2. The justice facet is an exact direct/surface twin, follows labour in source
   order, reads only declared base/earlier state and writes only owned state.
3. Current stock/event-marker ordinary views are complete for the declared
   conformance cells, and no sparse/event ontology table is introduced.
4. Entry reset, present-row guards and row-level runtime evidence prove
   admission/sentence/release plus inactive exit/re-entry generation isolation.
5. CRN coupling evidence proves a changed unemployment multiplier affects only
   justice admission/state and leaves demographic/labour draws, firings and
   state bitwise equal.
6. Reversed/undeclared dependencies, cross-owner writes, incomplete reset and
   unsupported event-table syntax fail with exact diagnostics.
7. Evidence is hash-complete and documentation makes every synthetic/non-causal
   limitation explicit.
8. Existing IR/plan/composition/Rust/dependencies and Australian/canonical
   artifacts remain unchanged.
