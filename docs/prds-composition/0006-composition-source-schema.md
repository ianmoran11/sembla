# PRD 0006: `CompositionSourceV1` types, canonical serialization, and fixtures

## Context

Read `docs/prds-composition/README.md` first; its constraints bind. The plan
side of the pipeline exists (PRDs 0003–0005). This PRD creates the **source**
side: the typed, serialized `CompositionSourceV1` graph that the canonical
linker (PRDs 0007–0009) consumes. No linking happens here — only types,
canonical encoding, parsing with deterministic errors, and the frozen fixture
corpus every later PRD links against.

Design rules restated from the README and DECISIONS §J:

- The serialized graph is the public contract; Lean surface syntax (PRD 0011)
  is just one producer.
- Stable IDs are slugs with kind prefixes (`def:population`,
  `inst:north`, `port:infection_count`, `wire:count_to_policy`).
- Source arrays preserve **author order** (unlike plan arrays); canonical
  bytes still use sorted object keys, compact form.
- `required_features` must be `[]`; unknown fields and unknown versions are
  deterministic rejections.

## Goal

Lean types, canonical writer, and parser for `CompositionSourceV1`; the
complete fixture corpus defined as Lean values, exported to checked-in
canonical JSON via `sembla-export --source`; byte-exact round-trip and
deterministic parse-error tests.

## Specification

### 1. Types — `frontend/Sembla/Composition/Source.lean`

```lean
namespace Sembla.Composition

structure StableId where  -- kind-prefixed, e.g. "def:population"
  raw : String
  deriving Repr, BEq, Ord

structure PortDeclV1 where
  id : StableId                  -- "port:" ++ name of the body input/output
  displayName : String
  direction : PortDirection      -- | input | output
  schema : List IR.OutputField   -- reuse the existing (name, type) field shape
                                 -- used by ports today; match IR exactly

structure ParameterBinding where
  requirement : String           -- component-local parameter name
  parameter : String             -- model-level parameter name

structure InstanceDeclV1 where
  id : StableId                  -- "inst:…"
  displayName : String
  definition : StableId          -- "def:…"
  parameterBindings : List ParameterBinding

structure WireDeclV1 where
  id : StableId                  -- "wire:…"
  sourceInstance : StableId
  sourcePort : StableId
  targetInstance : StableId
  targetPort : StableId
  delayTicks : Nat               -- must be 1 in V1 (checked at parse)

structure ExposureDeclV1 where
  id : StableId                  -- "expose:…"
  innerInstance : StableId
  innerPort : StableId
  outerPort : StableId           -- new boundary port id "port:…"

structure HiddenPortV1 where
  instance_ : StableId
  port : StableId

structure PrimitiveBodyV1 where  -- exactly the existing leaf shape
  tables : List IR.Table
  transitions : List IR.Transition
  inputs : List IR.PortDecl
  outputs : List IR.OutputDecl
  views : List IR.ViewDecl

structure CompositeBodyV1 where
  instances : List InstanceDeclV1
  wires : List WireDeclV1
  exposures : List ExposureDeclV1
  hiddenPorts : List HiddenPortV1

inductive ComponentBodyV1 where
  | primitive (body : PrimitiveBodyV1)
  | composite (body : CompositeBodyV1)

structure ComponentDefinitionV1 where
  id : StableId                  -- "def:…"
  displayName : String
  parameterRequirements : List String
  ports : List PortDeclV1
  body : ComponentBodyV1

structure SourceSummaryV1 where
  name : String
  reduce : IR.SummaryReduce
  instancePath : List StableId   -- chain of inst ids from the root
  view : String

structure CompositionSourceV1 where
  schemaVersion : String         -- "sembla.composition-source/v1"
  modelId : StableId             -- "model:…"
  displayName : String
  outerDt : IR.Scientific
  parameters : List IR.ParamDecl -- the model-level θ declarations
  definitions : List ComponentDefinitionV1
  rootDefinition : StableId
  requiredFeatures : List String -- must be []
  summaries : List SourceSummaryV1
```

There is deliberately no `rename` construct, no `correlationKey`, no
`schedulerRequirement`, no `sourceSpan` in V1 (DECISIONS §J9/§J12; spans may
arrive with surface syntax later without entering canonical bytes).

### 2. Structural well-formedness at parse/construction time

Provide `def wellFormed : CompositionSourceV1 → Except String Unit` checking
purely local rules (cross-definition resolution belongs to the linker):

- every `StableId` has the right kind prefix for its position and a slug
  payload (`[a-z][a-z0-9_]*`; for occurrence-free source IDs no `/` allowed);
- `schemaVersion` equals the frozen string; `requiredFeatures == []`;
- `delayTicks == 1` on every wire;
- IDs unique within their scope: definition ids globally; instance/wire/
  exposure ids within one composite; port ids within one definition;
- for a primitive definition: `ports` correspond one-to-one with the body's
  `inputs ++ outputs` — port id must be `"port:" ++ decl.name`, direction
  matching, schema equal to the body declaration's schema;
- for a composite: `hiddenPorts`/`exposures`/`wires` reference instance ids
  declared in the same composite (port existence is linker work).

### 3. Canonical encoding and parsing — `frontend/Sembla/Composition/Json.lean`

- **Writer:** encode to the `CJson` layer from PRD 0005 and render. Field
  names in snake_case: `schema_version`, `model_id`, `display_name`,
  `outer_dt`, `parameters`, `definitions`, `root_definition`,
  `required_features`, `summaries`; definition fields `id`, `display_name`,
  `parameter_requirements`, `ports`, `kind` + flattened body fields
  (`kind: "primitive"` with `tables/transitions/inputs/outputs/views`;
  `kind: "composite"` with `instances/wires/exposures/hidden_ports`); wire
  fields `id`, `source_instance`, `source_port`, `target_instance`,
  `target_port`, `delay_ticks`; exposure fields `id`, `inner_instance`,
  `inner_port`, `outer_port`; hidden-port fields `instance`, `port`.
  Primitive-body lists reuse the exact JSON shapes the plan writer
  (PRD 0005) uses for the same IR types. Empty lists are emitted (not
  omitted) — presence is uniform for required fields.
- **Parser:** `def parse (bytes : String) : Except String CompositionSourceV1`
  using `Lean.Json.parse` then explicit field decoding. Every error message
  must name the JSON path and the rule violated (e.g.
  `definitions[2].wires[0].delay_ticks: V1 requires exactly 1`). Unknown
  fields anywhere are errors (`unknown field 'foo' at definitions[0]`).
  Unknown `schema_version` is the exact deterministic error
  `unknown schema_version '<v>'; supported: sembla.composition-source/v1`.
  Any non-empty `required_features` → error naming the first feature. After
  decoding, run `wellFormed` and surface its error. Parsing accepts
  non-canonical *formatting* (whitespace/key order) so humans can author
  sources; canonical bytes are defined by the writer.

### 4. Fixture corpus — `frontend/Sembla/Composition/Fixtures.lean`

Define the corpus as Lean values, using primitive bodies **adapted from the
two boxes of `examples/sir_policy.json`** with `rows` scaled down (Person
1000, Employer 50, Controller 1) and the parameter names those bodies
reference declared as `parameterRequirements`. The frozen ids from the folder
README are mandatory. Corpus (one `CompositionSourceV1` value each):

| Fixture (source file name) | Root | Contents |
|---|---|---|
| `solo_population` | `def:solo_population` | composite: one instance `inst:population` of `def:population`; no wires/exposures |
| `independent_epidemic_policy` | `def:independent_epidemic_policy` | two instances, no wires (the product) |
| `two_independent_regions` | `def:two_independent_regions` | `inst:north`, `inst:south`, both of `def:independent_epidemic_policy` |
| `epidemic_policy` | `def:epidemic_policy` | two instances + `wire:count_to_policy`, `wire:restriction_to_population` |
| `two_regions` | `def:two_regions` | `inst:north`, `inst:south` of `def:epidemic_policy` (which, for this fixture family, also exposes `port:infection_count` from `inst:population`) |
| `regional_response` | `def:regional_response` | `inst:epidemic` of the exposing `def:epidemic_policy`; exposes `epidemic`'s boundary `port:infection_count` as `port:regional_infection_count`; hides its boundary `port:restriction_modifier` (exposed by `def:epidemic_policy` for this family) |

Each fixture embeds every definition it references (fixtures are closed
files). Model-level `parameters` carry the θ declarations (β, γ, thresholds…)
with the same values as the scaled `sir_policy` bodies; instances bind
requirements to the same-named model parameters explicitly. Every fixture has
`outer_dt := 0.25` and at least `two_regions` has one summary (e.g.
`peak_i := max` over `[inst:north, inst:population]` view `I`) so summaries
are exercised. Where `def:epidemic_policy` needs exposures
(`two_regions`/`regional_response` rows), define an `epidemic_policy_exposed`
definition value reused by both fixtures rather than mutating the plain one —
the plain `epidemic_policy` fixture file must stay exposure-free for PRD 0008.

### 5. Export mode, checked-in fixtures, and tests

- Extend `frontend/Main.lean`: `sembla-export --source <fixture-name>
  <out.json>` for each corpus entry (names = table rows above).
- Check in `fixtures/composition-source/<name>.source.json` — the canonical
  bytes. Append a delimited section to `frontend/scripts/check-parity.sh`
  exporting each and `cmp`-ing (append only).
- Tests in `frontend/Sembla/Composition/SourceTests.lean` (imported from
  `frontend/Sembla.lean`):
  - round-trip: for every fixture, `parse (render (encode f))` succeeds and
    re-renders to identical bytes (`#guard` on string equality);
  - reformat-tolerance: parse a hand-pretty-printed variant of
    `epidemic_policy` (small inline string) and `#guard` its canonical
    re-render equals the fixture bytes;
  - negative parses, each pinned to its message: bad slug (`def:North`),
    wrong kind prefix (`inst:` where `def:` expected), `delay_ticks: 2`,
    duplicate definition id, duplicate instance id, non-empty
    `required_features`, unknown top-level field, unknown definition field,
    unknown `schema_version`, primitive port/body mismatch (port with no
    matching output; schema mismatch between port and output).

## Allowed files

- `frontend/Sembla/Composition/Source.lean`, `Json.lean`, `Fixtures.lean`,
  `SourceTests.lean` (new)
- `frontend/Main.lean`, `frontend/Sembla.lean`
- `frontend/scripts/check-parity.sh` (append a delimited source section only)
- `fixtures/composition-source/**` (new)
- implementation notes/artifacts created by the managed run

## Non-goals

- Linking, occurrence identities, plans from sources, or source maps
  (PRD 0007+).
- Rust changes of any kind — Rust never reads composition sources.
- Surface syntax (`sembla_component`), renames, scheduler requirements,
  correlation keys, source spans, or any deferred construct from
  DECISIONS §J12.
- Editing `examples/**` or the plain-`epidemic_policy` fixture to add
  exposures.

## Acceptance criteria

1. `cd frontend && lake build` passes with all new modules imported; every
   round-trip and negative-parse `#guard` holds.
2. All six checked-in `fixtures/composition-source/*.source.json` files are
   byte-reproduced by `sembla-export --source` via the appended parity
   section.
3. The corpus uses exactly the frozen ids from the folder README (spot-check
   `def:population`, `inst:north`, `wire:count_to_policy`,
   `port:regional_infection_count` appear in the fixture bytes).
4. `parse` accepts reformatted JSON but canonical bytes are unique: proven by
   the reformat-tolerance guard.
5. Each listed negative case fails with its specific pinned message
   (path-bearing, deterministic).
6. `./scripts/check.sh`, full parity script, and `git diff --check` pass; no
   Rust diff outside `Cargo.lock` (which should also be unchanged).
