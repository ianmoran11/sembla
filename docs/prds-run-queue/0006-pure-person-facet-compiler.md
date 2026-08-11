# PRD 0005: Pure checked person-facet fusion compiler

## Dependencies

PRDs 0001–0004 accepted. The Lean-IR formalization track's PRD 0009 observation
builder is an external prerequisite and must already be accepted. Read the
binding [demographic-spine README](../prds-demographic-spine/README.md) first.

## Context

The current executable IR already supports one table with many attributes and
many same-row transitions. What it does not support is shared mutable tables
across composition leaves or person-granular wire projection. The foundation
therefore keeps modularity at authoring time: pure Lean combines facets into one
ordinary base model before serialization.

This PRD owns the semantic compiler only. Syntax, source-token diagnostics and
models come later. It must reuse the accepted pure final-model assembly/checking
boundary from the Lean-IR formalization track rather than constructing a second
`IR.Model` checker beside it.

## Goal

Add a syntax-independent, pure and tested `Sembla.Frontend.Facets` compiler that
qualifies facet-local names, checks ownership/lifecycle contracts, fuses ordered
facet declarations into one existing person table, and returns an ordinary
current `IR.Model` certified by the existing checker.

## Specification

### 1. Module and public values

Create:

- `frontend/Sembla/Frontend/Facets/Core.lean`;
- `frontend/Sembla/Frontend/Facets/CoreTests.lean`; and
- `frontend/Sembla/Frontend/Facets.lean` as the import surface.

Expose public structures equivalent to:

```text
FacetTransitionSpec
FacetViewSpec
FacetGroupedViewSpec
PersonFacetSpec
FacetBaseSpec
PersonFusionSpec
PersonFusionResult
FacetErrorCategory / FacetErrorPath / FacetError
```

Exact internal field names may follow established builder conventions, but the
public information is fixed:

`PersonFacetSpec` carries a facet ID, local parameters, local owned Real/Int/
enum attributes, explicit readable non-local person attributes, explicit
readable existing base input-port fields, local transitions, ordinary/grouped
views and entry-reset effects. It has no Ref-owned attribute,
tables, input/output ports, wires, summaries, exit effects or scheduler settings.

`FacetBaseSpec` carries the complete already-valid base model, target box/table,
`occupancy` attribute and present variant, `generation` attribute, and a
nonempty exact list of entry transition names. `PersonFusionSpec` adds the output model name and
source-ordered facets.
`PersonFusionResult` retains the raw model, the accepted checked-model
certificate or exact public equivalent from the final builder, and an ownership/
qualification manifest for tests and diagnostics.

### 2. Frozen qualification

Facet IDs and local declaration names use the existing runtime slug grammar.
Qualify every facet-owned runtime declaration as:

```text
<facet-id>_<local-name>
```

This applies to parameters, attributes, transitions, ordinary views and grouped
views. Variants, base names and explicitly declared readable qualified
attributes are unchanged.

Recursively qualify references through every supported expression, aggregate,
filter, effect and claim with explicit row context:

- outside `Expr.input`, a local parameter name resolves to the facet-qualified
  parameter and a local attribute name resolves to the facet-qualified person
  attribute;
- every non-owned base or earlier-facet person attribute must appear by its
  exact runtime name in `readAttrs` and remains exact after validation;
- base-model parameter reads are rejected; a facet must declare its own local
  parameter instead;
- inside an `Expr.input port aggregate`, `selfAttr` names belong to that input
  row schema, are never facet-qualified, and every `(port, field)` used must be
  declared in `readInputs`; and
- unknown or ambiguous names fail before final model checking with a structured
  facet path.

Do not qualify arbitrary strings by textual replacement. Traverse constructors
and preserve exact scientific encodings, enum variants, list order and source
spelling metadata retained by the spec.

### 3. Ownership and dependency rules

Validate before fusion:

- facet IDs are unique;
- generated global parameter/attribute/transition/view names do not collide
  with the base or another facet;
- owned attribute types are Real, Int or enum; Ref ownership fails explicitly in
  this foundation contract;
- every ordinary/entry effect writes only an attribute owned by its facet;
- every person-row read is the facet's own attribute or an explicitly declared
  valid base/earlier-facet attribute; base-model parameter reads are invalid;
- every input read names an existing base input port/field and is explicitly
  declared by the facet; facets cannot create or alter the port;
- dependencies point only to earlier facets, making source order the explicit
  acyclic dependency order;
- no facet writes `occupancy`, `generation` or any base attribute;
- the base entry-transition list is nonempty before any facet is compiled;
- every owned attribute has exactly one entry assignment;
- each entry right-hand side is exactly a matching literal or direct local facet
  parameter reference—no self/base attribute, input, aggregate, arithmetic or
  Boolean expression—because all effects read tick-start state;
- duplicate, missing or disallowed entry assignments fail;
- every named entry transition resolves in the target box and targets the
  person table;
- the named occupancy attribute is the declared enum with the supplied present
  variant; and
- the named generation attribute exists on the target table and is Int.

The base remains the owner of generation increment, exit and presence/event
choice. The compiler checks entry-reset coverage; it does not add an implicit
generation write or any exit write. Vacant domain bytes are inactive and must be
overwritten by the next entry before the row is visible as a new generation.

### 4. Exact fusion order

Given an accepted base and facets in source order:

1. retain every base parameter, box, table, transition, port, output, view,
   grouped view, wire and summary exactly;
2. append qualified facet parameters after all base parameters, facet order then
   local order;
3. append qualified owned attributes to the target person table in the same
   order;
4. append each facet's entry effects to every named base entry transition,
   preserving base effects first and facet/effect order after them;
5. leave every base exit transition exactly unchanged;
6. append qualified ordinary facet transitions after all base transitions,
   conjoining `occupancy = present` to every generated guard;
7. append ordinary/grouped facet views after the base views, conjoining the same
   presence predicate to every generated filter; and
8. change only `Model.name` to the explicitly supplied output name.

Effect suffix order is a serialization/commit contract only: every right-hand
side reads the pre-tick row. The entry-value restriction in §3 prevents a facet
from treating textual suffix order as read-after-write sequencing.

All other boxes are byte-structurally unchanged. Empty fusion with output name
equal to the base name must return exact raw-model equality, not merely an
observational projection.

### 5. Single checker/builder boundary

Inspect and use the accepted Lean-IR formalization track PRD 0009 final pure
model builder under `Sembla.Frontend.Builders`; do not predict or duplicate its
API. The facet
compiler may perform its facet-specific ownership/qualification checks, then
must pass the complete candidate through that authoritative assembly/checkModel
boundary.

Preserve exact underlying builder/checker errors inside a facet error wrapper
with path prefixing; do not translate them into a second semantic category set.
A successful result exposes the same acceptance and exact-erasure evidence as
the final builder.

Do not edit the accepted builder or `Sembla.Semantics` modules to make this PRD
pass. If the public final builder is insufficient, stop and amend this PRD.

### 6. Theorem obligations

Land named public theorem/lemma declarations, with exact names chosen to match
module style, establishing statements equivalent to:

| Obligation | Required statement |
|---|---|
| Empty identity | successful empty same-name fusion returns the base raw model exactly |
| Base preservation | projection removing qualified facet additions restores every base field/effect in order |
| Qualification fidelity | generated declaration/reference names equal the frozen prefix rule |
| Ownership safety | every added ordinary/entry effect targets its facet's qualified owned attribute |
| Entry coverage | successful fusion gives one literal/direct-facet-parameter assignment per owned attribute on every named entry transition |
| Vacancy isolation | every generated facet transition and view filter implies the declared present predicate, while base exits are unchanged |
| Append order | parameters, attributes, entry effects, transitions and views retain the prescribed source order |
| Dependency order | every accepted non-owned person read is declared and any cross-facet target belongs to an earlier facet; base-model parameters are unreadable |
| Final acceptance/erasure | successful fusion is accepted by the authoritative final checker and erases to the returned raw model |
| Failure characterization | failure is a facet structural/ownership error or an exact wrapped final-builder error |

No theorem may contain `sorry`, `admit`, `axiom`, `native_decide`, `unsafe`,
`implemented_by` or an opaque semantic proposition. Register new declarations
with the existing proof-hygiene audit.

### 7. Fixture matrix

Imported Lean tests must cover:

- empty exact identity over a multi-box model with ports/wires/observations;
- two facets with parameters, Real/Int/enum attributes, transitions and views;
- entry-reset effect order over multiple entry transitions;
- automatic present guards on transitions and ordinary/grouped views;
- unchanged base exit transitions and stale vacant values excluded by views;
- a justice facet explicitly reading an earlier qualified labour attribute;
- a labour facet explicitly reading numeric fields from an existing base input,
  with input-aggregate `selfAttr` names left unqualified;
- all collision classes, including rejected Ref ownership;
- missing/duplicate entry initialization and rejected entry reads/arithmetic;
- facet attempts to write a base or other-facet attribute;
- undeclared base/earlier-facet, later-facet and cyclic person-read dependencies,
  forbidden base-parameter reads, plus undeclared/missing input port fields;
- missing/wrong-type occupancy or generation attributes and unknown present
  variant;
- an empty base entry list plus missing, duplicate or wrong-table entry
  transitions;
- recursive qualification in nested arithmetic/Boolean/aggregate expressions,
  effects and claims;
- exact final checker rejection retained under its facet path; and
- exact raw/JSON equality against a manually constructed fused model.

Use one-defect negative values. Computed fixtures do not replace the theorem
matrix.

### 8. Documentation

Update `frontend/README.md` and `docs/design/lean-ir-coverage.md` with the exact
boundary:

- facet structural fusion is pure Lean and checked;
- parser/token mapping does not exist yet;
- facets emit one primitive current model and are not composition leaves;
- no `Share`/`Identify`, person-granular wire or runtime behavior has been added;
  and
- the final accepted builder/checker remains authoritative.

## Allowed files

- `frontend/Sembla/Frontend/Facets/Core.lean` (new)
- `frontend/Sembla/Frontend/Facets/CoreTests.lean` (new)
- `frontend/Sembla/Frontend/Facets.lean` (new)
- `frontend/Sembla.lean` (imports/audit registration only)
- `frontend/README.md`
- `docs/design/lean-ir-coverage.md`
- implementation notes/artifacts created by the managed run

## Non-goals

- No macro/syntax, source tokens, model migration or runtime fixture.
- No inputs, outputs, wires, summaries, new tables or table sharing in a facet.
- No arbitrary merge conflict resolution or facet reordering.
- No edit to accepted builder/semantics modules, IR/JSON/plan/composition/Rust
  code, dependencies or frozen artifacts.
- No proof that the fused model has the same scientific behavior as its base;
  ordinary facets intentionally add behavior.

## Acceptance criteria

1. All frontend, proof-hygiene, full repository, allowlist and diff checks from
   the README pass.
2. The pure API represents every frozen field, performs constructor-aware
   qualification and emits only current raw IR.
3. Ownership, nonempty entry targets, literal/direct-facet-parameter entry
   reset, present-row isolation, declared base/earlier-facet/input reads,
   forbidden base-parameter reads, occupancy/generation targets and dependency
   order are enforced with structured errors.
4. Fusion preserves the base exactly, appends every addition in the frozen
   order and gives exact empty-fusion identity.
5. The authoritative Lean-IR formalization PRD 0009 final builder/checker is the
   sole complete-model acceptance boundary; no duplicate checker or edited
   accepted builder exists.
6. Every theorem obligation passes the automated audit without forbidden proof
   constructs.
7. The complete positive/negative fixture matrix and manual exact-model twin
   pass.
8. The frozen IR, JSON/plan/composition modules, every Rust crate and every
   pre-existing canonical artifact named by the track contract are unchanged.
