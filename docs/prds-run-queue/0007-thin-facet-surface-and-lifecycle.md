# PRD 0006: Thin Lean facet surface and lifecycle lowering

## Dependencies

PRD 0005 accepted and the binding [demographic-spine README](../prds-demographic-spine/README.md) applies. The pure facet compiler is the
only semantic/fusion path. This PRD adds trusted parsing and positioned
diagnostics around it; it does not add another builder.

## Context

Direct `PersonFacetSpec` construction proves the static-fusion semantics but is
too verbose for driver models. The surface must make ownership, readable
attributes and lifecycle resets visible. It must also avoid pretending that a
facet is a composition leaf or runtime object.

The existing command frontend already owns scalar type/expression syntax,
parameter priors, transition blocks and view syntax. Reuse its accepted parser
categories and pure builder delegation wherever possible. Do not quote a second
complete `sembla_model` and lose source positions.

## Goal

Add an experimental `sembla_person_facet` declaration and
`sembla_fused_person_model` command that lower through PRD 0005, preserve exact
source order and positioned diagnostics, and emit an ordinary current `Model`
constant with atomic entry reset and automatic present-row isolation.

## Specification

### 1. Module and exact syntax

Implement in a new `frontend/Sembla/FacetDSL.lean` unless a minimal shared
parser extraction from `DSL.lean` is required. Import it from `Sembla.lean`.

Accept this declaration family:

```lean
sembla_person_facet LabourFacet (id := "labour") where
  param job_find_rate : ℝ := 0.08 ~ LogNormal (-2.5257286443082556) 0.2

  read age_months
  read area
  -- Existing base input fields use an explicit separate declaration:
  -- read input regional_labour_signal field nsw_modifier

  owns employment : {inactive, unemployed, employed} where
    entry inactive

  owns earnings : ℝ where
    entry 0.0

  transition find_work where
    guard employment = unemployed
    hazard job_find_rate
    set employment := employed

  view employed := count where employment = employed
```

and this fusion command:

```lean
sembla_fused_person_model DemographicLabour
    (base := demographicBase)
    (name := "demographic_labour_foundation")
    (box := "population")
    (table := "person")
    (occupancy := "occupancy")
    (present := "present")
    (generation := "generation")
    (entries := ["enter"])
    (facets := [LabourFacet])
```

`sembla_person_facet` defines an ordinary namespace-respecting
`PersonFacetSpec` constant. `sembla_fused_person_model` defines an ordinary
`Sembla.IR.Model` constant by constructing `FacetBaseSpec`/`PersonFusionSpec`
and calling the sole pure fusion compiler.

Exact syntax rules:

- `(id := "...")` is mandatory and uses the runtime slug grammar;
- declarations may be interleaved, with relative source order retained inside
  params, reads, owned attributes, transitions and views;
- `param` reuses current Real/Int default/prior syntax and diagnostics;
- one `read <runtime-identifier>` declaration is required for every non-owned
  person attribute used by a facet expression; generated earlier-facet names
  such as `labour_employment` are written explicitly;
- `read input <port> field <field>` declares read-only use of one field on an
  input port already present in the base target box; it never creates a port;
- `owns` accepts current Real, Int and enum attribute types and requires exactly
  one `entry` value; that value is a matching literal or direct local facet
  parameter only. Ref ownership and entry expressions reading attributes,
  inputs, aggregates or arithmetic are rejected explicitly;
- transition target is the fused person table implicitly; `guard`, `hazard` and
  one or more `set` lines follow current command-transition syntax;
- a facet transition has no contests in this foundation syntax. The pure API
  may retain checked raw contests for future machine authors, but accepting
  inert surface contest syntax is forbidden;
- ordinary `count|sum|min|max` views use current expression/reducer rules with
  the person table implicit; and
- no facet-created input port/output/wire/summary/grouped-view syntax is
  accepted in this PRD; `read input` only authorizes an existing base port.

The fusion header arguments are mandatory. Lists preserve exact order; the
facet list may be empty and the entry list must be nonempty. The compiler
conjoins the declared occupancy/present predicate to every facet transition and
view, and emits no facet effect on base exit transitions.

### 2. Name resolution and source tokens

Facet-local parameters/attributes are authored unqualified and lower to the
PRD 0005 local representation. Explicit reads retain their runtime spelling.
Within an ordinary facet expression resolve in this order only when unique:

1. local owned attribute;
2. local parameter;
3. explicit person `read` name.

Within `inputSum`/input-aggregate syntax, resolve field identifiers only through
an exact `read input <port> field <field>` declaration and leave them
unqualified for PRD 0005's input-row context.

A spelling present in more than one class is an ambiguity error at the use.
Unknown names point at the source identifier and prescribe `owns`, `param` or
`read`; never infer a base field or permit a base-model parameter by probing the
base model during facet declaration.

The fusion command resolves the base/facet Lean constants, then PRD 0005 checks
that every explicit read, occupancy/generation field and entry transition name
exists. Preserve tokens for facet ID, declarations, expressions/effects/views
and every fusion header/list item so structured compiler paths map back to exact
source locations.

### 3. Thin adapter boundary

The macro implementation may:

- parse syntax;
- collect declarations/tokens;
- call accepted scalar/type/expression surface-lowering helpers;
- construct `PersonFacetSpec` and `PersonFusionSpec` values;
- map exact structured errors to tokens; and
- splice the successful result as one constant.

It may not:

- assemble `IR.Model`, append declarations/effects or qualify names itself;
- rerun ownership, lifecycle, schema or final model checks;
- sort or repair declarations;
- construct a composition source/plan; or
- silently drop an accepted declaration.

If existing scalar parser logic is trapped inside `DSL.lean`, extract the
minimum dependency-free parser/lowering API with exact existing behavior and
parity tests. Do not copy it.

### 4. Positive twins

Add imported tests containing:

1. the exact Labour example above fused to a tiny valid demographic base;
2. a second Justice facet reading the generated `labour_employment` attribute;
3. a two-facet fusion with multiple entry transitions and unchanged base exits;
4. a facet with Real/Int/enum owned attributes and prior/priorless params;
5. a facet reading declared numeric fields from an existing base input port;
6. count/sum/min/max views; and
7. a hand-constructed `PersonFacetSpec`/`PersonFusionSpec` twin.

Assert facet value equality where source tokens are erased, exact fused raw
model equality, exact `IR.toJson`, declaration/effect order and widget data where
applicable. Empty-facet fusion through the command must equal the base exactly
when the output name is unchanged.

### 5. Positioned negative suite

Add complete negative files and exact expected lines for at least:

- missing/invalid/duplicate facet ID;
- duplicate local param/owned attr/transition/view;
- missing or duplicate `entry` values and rejected Ref ownership;
- unknown and ambiguous expression names;
- undeclared explicit person read, attempted base-model parameter read or
  undeclared input-port field use, missing base input port/field and wrong input
  field type;
- wrong guard/hazard/effect/lifecycle/view types;
- entry initializer using a demographic/self/input/aggregate/arithmetic
  expression rather than a literal or direct local facet parameter;
- facet effect attempting a base/other-facet write;
- missing/wrong base constant, box, table, occupancy/present or generation
  attribute;
- missing, duplicate or wrong-table entry transition names;
- duplicate/colliding generated runtime names;
- later-facet dependency and reversed facet order;
- empty entry list;
- attempted facet-created input/output/wire/summary/grouped-view/contest
  declarations (distinct from `read input`);
  and
- a final `checkModel` failure whose exact nested path is retained.

The complete-error harness rule applies: expected lines plus any unexpected
additional error fail. Diagnostics point to authored facet/fusion tokens, never
generated IR.

### 6. Documentation and status

Create `docs/guides/person-facets.md` documenting:

- facets as experimental compile-time authoring values;
- exact syntax and generated naming;
- attribute ownership, earlier-facet reads and explicit existing-input reads;
- literal/direct-facet-parameter entry values and tick-start effect semantics;
- atomic entry reset, automatic present-row guards, inactive vacant bytes and
  generation identity;
- the one-primitive-box output;
- why this is not `Share`/`Identify`;
- absence of person-granular wires, dynamic rows and multiple schedulers; and
- direct `PersonFacetSpec` construction as the machine-writer path.

Update `frontend/README.md` and coverage docs from PRD 0005 to name the trusted
parser/token boundary. Do not present the syntax as a stable 1.0 contract.

## Allowed files

- `frontend/Sembla/FacetDSL.lean` (new)
- `frontend/Sembla/FacetDSLTests.lean` (new)
- `frontend/Sembla/DSL.lean` (minimal parser-helper extraction only if required)
- `frontend/Sembla/CommandFrontendTests.lean` (parity only if shared helpers move)
- `frontend/Sembla/Positive/**` (new facet fixtures only)
- `frontend/Sembla/Negative/**` (new facet fixtures only)
- `frontend/scripts/test-negative.sh`
- `frontend/Sembla.lean` (imports only)
- `frontend/README.md`
- `docs/design/lean-ir-coverage.md`
- `docs/guides/person-facets.md` (new)
- implementation notes/artifacts created by the managed run

## Non-goals

- No edit to PRD 0005's semantic rules except a separately reviewed bug fix
  required by a failing exact twin; scope expansion requires stopping.
- No canonical model migration, Australian adapter, runtime state or domain
  pilot.
- No facet-created output/input port, grouped-view, summary or contest syntax;
  read-only declarations for existing base input fields are the sole input
  surface.
- No formatter, autocomplete, widget redesign or public stability guarantee.
- No IR/JSON/plan/composition/Rust/dependency change.

## Acceptance criteria

1. All frontend, negative, parity, proof-hygiene, full repository, allowlist and
   diff checks from the README pass.
2. Both commands accept exactly the frozen syntax and define ordinary Lean
   constants through the sole PRD 0005 compiler.
3. Code inspection finds no macro-side model assembly, qualification, ownership,
   lifecycle or final-check logic and no copied scalar parser/type checker.
4. Positive direct-value twins are structurally and JSON exact, retain every
   list/effect order and cover earlier-facet/input reads, literal/parameter
   entry values, multiple entry transitions, automatic present guards and
   unchanged base exits.
5. Every listed negative has one exact positioned diagnostic and unsupported
   declarations are rejected rather than accepted inertly.
6. Empty command fusion is exact identity; nonempty fusion emits only current IR
   and passes the current final checker.
7. Documentation clearly labels facets experimental and compile-time, and names
   every absent runtime/composition capability.
8. Existing syntax, diagnostics, canonical exports, fixtures and every Rust file
   are unchanged.
