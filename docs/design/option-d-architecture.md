# Option D architecture: Lean-linked composition source and canonical flat execution

**Status:** Proposed architecture detail, 2026-07-21. This document selects and
specifies the recommended shape of Option D for review. It does not implement
composition syntax, change the runtime, or supersede a decision record.

**Scope:** The serialized composition-source representation, canonical Lean 4
linker, flat executable plan, semantics and proof boundary, stable identities,
artifacts, compatibility, validation, migration, rollout, and acceptance tests.

**Authority:** This document refines
[`composition-options.md`](composition-options.md), especially Option D and the
recommended hybrid pipeline. It is constrained by [`DESIGN.md`](../../DESIGN.md)
and [`DECISIONS.md`](../../DECISIONS.md). Once accepted, its normative choices
should be recorded in `DECISIONS.md` and implemented through focused PRDs.

**Implementation status:** The current Lean frontend elaborates directly to the
flat `Sembla.IR.Model`, serializes it, and hands it to Rust for validation and
execution. `CompositionSourceV1`, `ExecutablePlanV1`, and the linker described
below do not yet exist.

The words **MUST**, **MUST NOT**, **SHOULD**, and **MAY** describe the proposed
contract. They become normative only after the architecture is accepted.

---

## 1. Decision summary

Option D separates reusable composition from execution without creating two
runtime semantics:

1. Human and machine frontends produce a typed, serialized, frontend-neutral
   `CompositionSourceV1`.
2. One canonical Lean 4 linker validates and deterministically lowers that
   source into a versioned, flat `ExecutablePlanV1`.
3. Lean defines independent denotational meanings for the composition source
   and flat plan and states a preservation theorem for the linker.
4. Only the flat executable plan crosses the Rust execution boundary.
5. The plan carries stable semantic identities and an embedded link to its
   source, identity map, and source map.
6. The source and plan are both retained in the logical artifact bundle.
7. Direct flat IR remains a compatibility and machine-writer path, but it must
   enter the same canonical-plan validation path and is not a second semantics.
8. Composition topology is static for this architecture. Dynamic rows,
   relations, and references inside a leaf ACSet remain supported; dynamic
   creation or deletion of component instances is a separate future design.

The central architectural rule is:

> **One serialized composition language, one canonical linker, one flat runtime
> contract, and one executable semantics.**

### 1.1 Decisions recommended here

| Question | Recommended answer |
|---|---|
| Is the source graph serialized from day one? | Yes; Lean-only authoring is acceptable initially, Lean-only serialized records are not. |
| Who produces the canonical flat plan? | A pure, deterministic Lean 4 linker. |
| What does Rust consume? | Only a versioned canonical flat plan. |
| Where is semantic reasoning performed? | Lean, over independent source and plan denotations. |
| Does runtime hierarchy execute recursively? | No; hierarchy survives through source and provenance. |
| Are source and plan both archived? | Yes. |
| Is ACSet compatibility required? | A lossless mapping is desirable; a Julia dependency is not required. |
| Are ordinary wires, exposure, sharing, and atomic families one operation? | No; they are semantically distinct constructors. |
| Is direct flat IR coequal with composition source? | No; it is an alternate producer of the canonical plan with weaker source-level guarantees. |

---

## 2. Goals, non-goals, and compatibility envelope

### 2.1 Goals

The architecture MUST support:

- reusable primitive and composite component definitions;
- multiple independently initialized instances of one definition;
- machine product with disjoint tagged ownership and interfaces;
- typed one-tick-delayed wires;
- zero-delay exposure, hiding, and renaming;
- static nesting that can be flattened without changing behavior;
- stable definition, instance, transition, port, wire, mailbox, and draw-site
  identities;
- deterministic lowering to one canonical flat executable plan;
- frontend-neutral source serialization;
- hierarchy-aware diagnostics and widgets through source maps;
- explicit source, plan, linker, identity, and hash versions;
- reproducible execution of old plans under their original interpretation; and
- a formal statement that source-to-plan linking preserves the selected
  observations.

### 2.2 Initial non-goals

The first implementation MUST NOT require:

- a recursively executable runtime IR;
- dynamic creation or deletion of component instances during a run;
- implicit same-name port merging;
- variable sharing or structured-cospan gluing disguised as an ordinary wire;
- zero-delay combinational wiring;
- arbitrary schema coercion or implicit port variance;
- synchronized cross-component transition families;
- semantic invariants requiring prospective global commit checks;
- heterogeneous scheduler domains; or
- proof verification of the Rust interpreter or CPU/CUDA implementation.

The source schema MAY reserve versioned extension points for later features, but
reserved fields MUST NOT acquire semantics without a schema and linker version
change.

### 2.3 Compatibility envelope

The following remain valid inputs during migration:

- current Lean `sembla_model` programs;
- current direct Lean `Sembla.IR.Model` constructors;
- current unversioned flat JSON fixtures; and
- future non-Lean producers of `CompositionSourceV1`.

Compatibility does not mean reinterpretation. A legacy artifact retains its
legacy positional-ID and ordering semantics. It is not silently upgraded to the
new stable-identity contract.

---

## 3. Current baseline and required change

Today the Lean deep IR is flat:

```lean
structure Box where
  name : String
  tables : List Table
  transitions : List Transition
  inputs : List PortDecl
  outputs : List OutputDecl
  views : List ViewDecl

structure Wire where
  source : WireEndpoint
  target : WireEndpoint

structure Model where
  name : String
  dt : Scientific
  params : List ParamDecl
  boxes : List Box
  wires : List Wire
  summaries : List SummaryDecl
```

The current flow is:

```text
Lean `sembla_model`
        ↓ elaboration
Sembla.IR.Model
        ↓ Sembla.IR.toJson
unversioned flat JSON
        ↓ Rust parse + validate
ValidatedModel
        ↓
CPU execution / differential backend execution
```

This division is already valuable:

- Lean elaborates, renders structure widgets, states specification-level
  properties, and serializes models.
- Rust rejects malformed whole models and executes accepted plans.
- The Rust backend does not depend on Lean.

Option D inserts a first-class composition source and a reasoned-about lowering
pass while retaining that boundary:

```text
Lean composition syntax ───────┐
                               │
non-Lean source producer ──────┼──> CompositionSourceV1
                               │              │
machine-generated source ─────┘              │
                                              ▼
                                  canonical Lean 4 linker
                                              │
                                              ▼
                                     ExecutablePlanV1
                                              │
                                    Rust parse + validate
                                              │
                                              ▼
                                         CPU / CUDA
```

The current `Model` shape can be the first plan core, but the executable
artifact requires a versioned envelope and additional stable identity,
mailbox, scheduler-domain, and provenance fields. Calling the current
unversioned JSON “canonical” does not make equivalent composition graphs
canonical: current arrays preserve declaration order.

---

## 4. Architecture and trust boundaries

### 4.1 Build-time and runtime responsibilities

| Layer | Responsibility | Trust status |
|---|---|---|
| Surface frontend | Parse author syntax, perform frontend-only typing, emit `CompositionSourceV1` | Untrusted producer; output is validated by the linker |
| `CompositionSourceV1` | Frontend-neutral serialized composition contract | Public versioned contract |
| Lean linker | Validate, resolve, flatten, assign identities, canonicalize, emit plan and maps | Canonical compiler pass; specified in Lean |
| Lean source semantics | Meaning of hierarchy, product, exposure, and delayed wiring | Formal ground truth for source composition |
| Lean plan semantics | Meaning of the flat executable plan | Formal ground truth at the execution boundary |
| Rust validator | Defense-in-depth validation of serialized plans | Trusted implementation boundary |
| CPU interpreter | Executable semantics oracle | Trusted, differentially tested |
| CUDA/other backend | Optimized execution | Trusted, differentially tested against CPU |

### 4.2 Canonical linker ownership

The canonical flat plan SHOULD be produced by Lean 4 at build/export time.
The linker MUST be expressible as a pure function over serialized source data:

```lean
opaque linkV1 :
  CompositionSourceV1 →
  Except (List LinkErrorV1) LinkResultV1
```

It MUST NOT depend on:

- filesystem traversal order;
- process-global state;
- wall-clock time;
- random numbers;
- network services;
- source declaration order except where the source semantics explicitly says
  order is meaningful; or
- Lean elaborator accidents not represented in `CompositionSourceV1`.

The linker can be shipped as a Lean-built standalone executable. A non-Lean
frontend therefore need not embed Lean; it emits neutral source JSON and invokes
the same canonical linker.

### 4.3 Proof boundary

Lean reasoning stops at the executable-plan boundary, matching the existing
project trust model. A theorem about `linkV1` establishes a property of the Lean
source and flat denotations. It does not verify that Rust or CUDA implements
that denotation.

The runtime boundary remains:

```text
proved/stated Lean source-to-plan refinement
                         │
                         ▼
              serialized plan boundary
                         │
                         ▼
          trusted Rust/CPU/CUDA implementation
```

Rust validation and CPU/CUDA differential testing are mandatory even after a
Lean preservation proof exists.

---

## 5. Versions, canonical bytes, and hashes

### 5.1 Independently versioned concerns

The artifact MUST name these versions independently:

| Concern | Example identifier | Why separate |
|---|---|---|
| Composition source schema | `sembla.composition-source/v1` | Source fields and constructor meanings evolve independently |
| Primitive/plan schema | `sembla.executable-plan/v1` | Runtime fields and validation evolve independently |
| Linker semantics | `sembla.linker/v1` | Same source schema can have a bug-fixed or changed lowering algorithm |
| Identity scheme | `sembla.identity/stable-v1` | RNG and report identity are long-lived scientific contracts |
| Legacy identity scheme | `sembla.identity/legacy-positional-v1` | Old artifacts retain old dense-ID meaning |
| Canonical encoding | `sembla.canonical-json/v1` | Byte ordering and numeric/string encoding affect hashes |
| Source-map schema | `sembla.source-map/v1` | Diagnostics can evolve without changing execution semantics |
| Hash algorithm/domain | e.g. `blake3:sembla-plan-core-v1` | Domain separation prevents accidental cross-object equality |

The exact strings above are illustrative until accepted in a decision record.
Unknown required versions MUST be rejected. They MUST NOT be interpreted by
best effort.

### 5.2 Canonical encoding

Each serialized contract MUST define:

- field presence and optional-field rules;
- object-key order or a canonical object encoding;
- array order;
- exact decimal and floating-point encoding;
- enum spelling;
- Unicode normalization policy;
- stable-ID byte and text form;
- unknown-field behavior; and
- whether human source locations participate in canonical bytes.

Executable arrays SHOULD be sorted by stable semantic identity unless their
order is itself semantically meaningful. Display order belongs in explicit
non-semantic presentation metadata.

### 5.3 Hash separation

Every persisted hash is an all-or-nothing record, not an opaque digest string:

```json
{
  "algorithm": "blake3-256",
  "domain": "sembla.plan-core/v1",
  "digest": "..."
}
```

`algorithm` names the cryptographic algorithm and output convention. `domain`
names the object class and canonical-byte version. Both travel with the digest
in artifacts and run manifests.

At minimum, the bundle distinguishes:

1. **Source artifact hash** — exact canonical `CompositionSourceV1` bytes,
   domain `sembla.source-artifact/v1`.
2. **Plan semantic hash** — executable core, including identities, identity
   maps, mailboxes, scheduler information, and every execution-affecting field,
   domain `sembla.plan-core/v1`.
3. **Plan envelope hash** — executable core plus linked provenance and maps,
   domain `sembla.plan-envelope/v1`.
4. **Whole-bundle integrity hash** — optional root over the manifest payload and
   named file-hash records, domain `sembla.bundle-root/v1`.

A normalized source-semantic hash MAY be added later, but it MUST NOT be claimed
until alpha-renaming, reassociation, and source equivalence are specified.

The plan semantic hash MUST exclude its own hash record. Prefer placing hashes
in the bundle manifest. If a plan embeds an envelope hash, canonical envelope
bytes MUST omit that field while hashing.

Source maps used only for diagnostics MAY be excluded from the plan semantic
hash, but stable IDs and runtime identity mappings MUST be included because they
affect draws, reports, and reproducibility.

A bundle root MUST NOT hash a manifest containing its own digest. If
`bundle_integrity` is stored in `bundle-manifest.json`, its input is defined as:

1. canonical manifest bytes with the `bundle_integrity` field omitted; followed
   by
2. the complete `{path, algorithm, domain, digest}` records for named files in
   lexicographic path order.

The manifest file itself is not a second named-file input. An implementation MAY
instead store the bundle-root record outside the manifest.

---

## 6. Logical artifact bundle

A linked Option D artifact produces a logical bundle, not necessarily a new
archive format:

```text
composition-source.json   canonical CompositionSourceV1
executable-plan.json      independently runnable ExecutablePlanV1
link-report.json          optional warnings/statistics; never semantic
bundle-manifest.json      versions, hashes, linker, feature set, relationships
```

A direct-stable or legacy bundle omits `composition-source.json`, the link
report, and the source/linker tuple while retaining the executable plan, plan
hash records, identity scheme, enabled features, and manifest.

The plan MUST remain runnable if copied away from the source. The source MUST
remain inspectable and relinkable if copied away from the plan. The relationship
between them must survive ordinary artifact movement.

An illustrative manifest is:

```json
{
  "bundle_schema": "sembla.bundle/v1",
  "source": {
    "schema": "sembla.composition-source/v1",
    "path": "composition-source.json",
    "hash": {
      "algorithm": "blake3-256",
      "domain": "sembla.source-artifact/v1",
      "digest": "..."
    }
  },
  "linker": {
    "semantics": "sembla.linker/v1",
    "implementation": "lean4",
    "build": "git:..."
  },
  "plan": {
    "schema": "sembla.executable-plan/v1",
    "identity_scheme": "sembla.identity/stable-v1",
    "enabled_features": [],
    "path": "executable-plan.json",
    "semantic_hash": {
      "algorithm": "blake3-256",
      "domain": "sembla.plan-core/v1",
      "digest": "..."
    },
    "envelope_hash": {
      "algorithm": "blake3-256",
      "domain": "sembla.plan-envelope/v1",
      "digest": "..."
    }
  },
  "source_map_schema": "sembla.source-map/v1",
  "canonical_encoding": "sembla.canonical-json/v1",
  "bundle_integrity": {
    "algorithm": "blake3-256",
    "domain": "sembla.bundle-root/v1",
    "digest": "..."
  }
}
```

Every run manifest MUST record the complete plan-semantic-hash record—algorithm,
domain/version, and digest—plus the identity scheme, mandatory sorted
`enabledFeatures`, scheduler-domain plan, backend, numeric contract, and
determinism level.

For `origin = linked`, the source-hash record and linker semantic version form an
all-present tuple and MUST also be recorded. For `directStable` and `legacy`,
that tuple is wholly absent; it is never fabricated. A run manifest MUST NOT rely
on finding an adjacent sidecar later.

---

## 7. `CompositionSourceV1` data model

### 7.1 Representation choice

`CompositionSourceV1` is a typed graph of definitions and declarations. Surface
constructors such as `A ⊗ B` elaborate into that graph. The serialized graph is
the public source contract; a frontend-specific syntax AST is not.

The graph MUST preserve:

- reusable component-definition boundaries;
- instance ownership and definition references;
- stable source identities;
- parameter requirements and bindings;
- declared boundary ports and schemas;
- delayed wires;
- exposure, hiding, and renaming;
- scheduler requirements/domains;
- constraints and feature gates; and
- source anchors for diagnostics, as non-semantic metadata.

### 7.2 Illustrative Lean types

These types communicate structure; they do not freeze exact implementation
syntax or stable-ID encoding. `Table`, `Transition`, `PortDecl`, and related
names denote the published frontend-neutral primitive IR records, not a
requirement that non-Lean producers construct Lean values.

For a primitive definition, `ComponentDefinition.ports` is the authoritative
component boundary and MUST correspond one-to-one with
`PrimitiveBody.inputs/outputs`. For a composite definition, its ports are the
outer boundary resolved through exposures and closures. The linker rejects any
mismatch.

```lean
opaque StableId : Type
opaque OccurrenceId : Type
opaque SchemaId : Type
opaque SourceSpan : Type

inductive PortDirection where
  | input
  | output

structure ComponentPort where
  id : StableId
  displayName : String
  direction : PortDirection
  schema : SchemaId

structure PrimitiveBody where
  tables : List Table
  transitions : List Transition
  inputs : List PortDecl
  outputs : List OutputDecl
  views : List ViewDecl

structure InstanceDecl where
  id : StableId                 -- declaration ID within the owning definition
  displayName : String
  definition : StableId
  parameterBindings : List ParameterBinding
  initialization : InitializationRef
  correlationKey : Option StableId
  sourceSpan : Option SourceSpan

structure WireDecl where
  id : StableId                 -- declaration ID within the owning definition
  sourceInstance : StableId
  sourcePort : StableId
  targetInstance : StableId
  targetPort : StableId
  delayTicks : Nat             -- V1 requires exactly 1
  sourceSpan : Option SourceSpan

structure ExposureDecl where
  id : StableId
  innerInstance : StableId
  innerPort : StableId
  outerPort : StableId
  sourceSpan : Option SourceSpan

structure CompositeBody where
  instances : List InstanceDecl
  wires : List WireDecl
  exposures : List ExposureDecl
  hiddenPorts : List StableId
  visibleRenames : List VisibleRename
  constraints : List ConstraintDecl

inductive ComponentBody where
  | primitive (body : PrimitiveBody)
  | composite (body : CompositeBody)

structure ComponentDefinition where
  id : StableId
  displayName : String
  parameters : List ParameterRequirement
  ports : List ComponentPort
  schedulerRequirement : SchedulerRequirement
  body : ComponentBody
  sourceSpan : Option SourceSpan

structure CompositionSourceV1 where
  schemaVersion : String
  modelId : StableId
  displayName : String
  outerDt : Scientific
  parameters : List ParamDecl
  definitions : List ComponentDefinition
  rootDefinition : StableId
  requiredFeatures : List String
  summaries : List SourceSummaryDecl
```

### 7.3 Semantic IDs versus names and paths

A source record has distinct concepts:

| Concept | Purpose | May change under display rename? |
|---|---|---|
| Stable declaration ID | Reusable source definitions and local declarations | No |
| Instance occurrence ID | One expanded use of a declaration chain | No |
| Display name | Human UI/reporting | Yes |
| Qualified source path | Diagnostics/navigation | Yes |
| Runtime ordinal | Compact array indexing | Yes |
| Runtime RNG word | Current Philox compatibility | Only through a versioned identity mapping |

Declaration and occurrence IDs MUST NOT be delimiter-concatenated display names
or traversal positions. Source spans MUST NOT be required to interpret source
semantics.

### 7.4 Instance-occurrence identity

Declaration IDs inside a reusable composite are not globally unique runtime
occurrences. If `TwoRegions` instantiates `EpidemicPolicy` twice, the inner
`population` declaration appears twice. The linker therefore constructs a
stable occurrence identity from the complete semantic instance chain:

```text
root-occurrence = H("sembla.occurrence/v1", model-id)
child-occurrence = H(
  "sembla.occurrence/v1",
  parent-occurrence,
  child-InstanceDecl.id
)
```

Repeated application encodes the full chain from the root. AST-only tensor
parentheses and other non-semantic grouping nodes do not contribute a chain
element. Moving an instance across a named composite boundary changes its
occurrence identity unless an explicit versioned migration maps the old and new
occurrences; it is not automatically an identity-preserving refactor.

Every expanded leaf, transition, port endpoint, wire, mailbox, scheduler-domain
membership, and draw site uses occurrence IDs rather than unqualified local
instance declaration IDs. A wire occurrence is derived from the owning
composite occurrence, local `WireDecl.id`, and resolved endpoint occurrences.

The source and plan test corpus MUST include two instances of one reusable
composite and verify that all nested leaf, transition, wire, mailbox, and draw
identities are distinct while each projection remains noninterfering.

### 7.5 Product representation

A surface expression:

```lean
sembla_component Independent := Population ⊗ Policy
```

elaborates into a composite definition with two instances and disjoint tagged
boundary ports. Product introduces no wire and no shared mutable state.

The source map MAY retain the original tensor expression and syntax anchor, but
the serialized semantic graph does not require every frontend to share Lean's
expression syntax.

### 7.6 ACSet compatibility

The source graph SHOULD admit a lossless mapping to an ACSet-like schema:

```text
DefinitionPort.owner       -> Definition
Instance.parent            -> Definition
Instance.definition        -> Definition
InstancePort.owner         -> Instance
InstancePort.declaration   -> DefinitionPort
Wire.owner                 -> Definition
Wire.source                -> InstancePort
Wire.target                -> InstancePort
Exposure.owner             -> Definition
Exposure.inner             -> InstancePort
Exposure.outer             -> DefinitionPort
```

This is an interoperability and tooling property, not a commitment to store the
source in Julia or to use categorical product as machine product.

---

## 8. Composition constructs and lowering contract

| Source construct | Source meaning | Flat lowering | Runtime consequence |
|---|---|---|---|
| Primitive definition | One reusable leaf behavior | One leaf template | None until instantiated |
| Instance | Independent owned use of a definition | Stable leaf instance(s) | State and local transitions |
| Tensor/product | Parallel state and tagged interfaces | Concatenated stable-ID-sorted leaves | No connection or shared state |
| Wire | Directional table channel with one-tick delay | One explicit mailbox and delivery edge | Adds semantic state and delay |
| Exposure | Child port aliases composite boundary | Source-map/boundary alias | No mailbox and no delay |
| Hide | Remove a direct child boundary port from public interface | Visibility/source-map entry | No state deletion |
| Rename | Change visible label only | Presentation/source-map entry | No semantic-identity change |
| Explicit adapter | Leaf component that transforms a schema | Ordinary instantiated leaf and wires | Behavior declared by adapter |
| Constraint | Restriction/assertion according to declared class | Validation obligation or coordinator | Never implicit repair |
| Synchronized family | One atomic multi-owner event | Deferred beyond first release | Requires explicit plan support |
| `Share`/`Identify` | Boundary/variable identification | Deferred separate constructor | Never an ordinary delayed wire |

### 8.1 Static topology

Definitions, instances, wires, exposures, and scheduler domains are fixed before
execution. A leaf may still contain dynamic ACSet rows and relations, including
birth, death, and reference changes, under the leaf's execution semantics.
Dynamic component topology requires a new source and plan version.

### 8.2 Visibility

Child ports are private outside their immediate composite unless exposed. A
parent may wire, expose, hide, or rename a direct child's boundary port. It MUST
NOT reach through that child to an unexposed descendant.

### 8.3 Delay discipline

`CompositionSourceV1` distinguishes:

- ordinary wire: exactly one tick of delay;
- exposure/rename: zero-delay structural relation;
- adapter: behavior and delay determined by an explicit leaf component;
- future sharing/identification: separate zero-delay structural semantics; and
- future synchronized family: one atomic event, not message transport.

These meanings cannot be changed without a source schema and linker semantic
version change.

---

## 9. Worked source example

All syntax in this section is proposed and is not accepted by the current
parser.

### 9.1 Proposed Lean authoring surface

```lean
sembla_component Population where
  input restriction_modifier where
    restriction : ℝ
  output infection_count from Person where
    infected : Int := count where health = I
  system Person ...
  infect on Person : health: S →[ ... ] I
  recover on Person : health: I →[ ... ] R

sembla_component Policy where
  input infection_count where
    infected : Int
  output restriction_modifier from Controller where
    restriction : ℝ := sum (restriction)
  system Controller ...
  transition restrict on Controller where ...
  transition reopen on Controller where ...

sembla_component EpidemicPolicy where
  instance population := Population
  instance policy := Policy

  wire population.infection_count -> policy.infection_count
  wire policy.restriction_modifier -> population.restriction_modifier

  expose population.infection_count as infection_count
  expose policy.restriction_modifier as restriction_modifier
```

### 9.2 Illustrative serialized source records

```json
{
  "schema": "sembla.composition-source/v1",
  "model_id": "model:epidemic-policy-v1",
  "display_name": "EpidemicPolicy",
  "outer_dt": "0.25",
  "root_definition": "def:epidemic-policy-v1",
  "required_features": [],
  "definitions": [
    {
      "id": "def:epidemic-policy-v1",
      "kind": "composite",
      "instances": [
        {
          "id": "inst:population-v1",
          "display_name": "population",
          "definition": "def:population-v1"
        },
        {
          "id": "inst:policy-v1",
          "display_name": "policy",
          "definition": "def:policy-v1"
        }
      ],
      "wires": [
        {
          "id": "wire:population-count-to-policy-v1",
          "from": ["inst:population-v1", "port:infection-count-v1"],
          "to": ["inst:policy-v1", "port:infection-count-input-v1"],
          "delay_ticks": 1
        },
        {
          "id": "wire:policy-restriction-to-population-v1",
          "from": ["inst:policy-v1", "port:restriction-output-v1"],
          "to": ["inst:population-v1", "port:restriction-input-v1"],
          "delay_ticks": 1
        }
      ],
      "exposures": [
        {
          "id": "expose:infection-count-v1",
          "inner": ["inst:population-v1", "port:infection-count-v1"],
          "outer": "port:epidemic-infection-count-v1"
        }
      ]
    }
  ]
}
```

The JSON omits primitive bodies and some fields for readability. IDs and wire
format are illustrative.

---

## 10. Canonical Lean 4 linker

### 10.1 Result type

```lean
structure LinkerDescriptor where
  sourceSchema : String
  planSchema : String
  linkerSemantics : String
  identityScheme : String
  canonicalEncoding : String
  sourceMapSchema : String

structure LinkResultV1 where
  plan : ExecutablePlanV1
  report : LinkReportV1

opaque linkV1 :
  CompositionSourceV1 →
  Except (List LinkErrorV1) LinkResultV1
```

The plan contains the authoritative identity map and, for a linked origin, the
linker descriptor and source map. `report` contains warnings and statistics
only. It MUST NOT duplicate authoritative maps or contain information required
to interpret or execute the plan.

### 10.2 Link stages

The linker runs these deterministic stages:

1. **Envelope/version validation**
   - Reject unknown required schema, identity, or feature versions.
   - Validate and canonicalize the sorted source `requiredFeatures` set.
   - Check canonical-decoding constraints before semantic processing.
2. **Definition collection**
   - Index definitions and ports by stable ID.
   - Reject duplicate IDs even when display names differ.
3. **Dependency analysis**
   - Resolve definition references.
   - Reject missing definitions and recursive definition cycles.
4. **Parameter resolution**
   - Resolve model parameters, requirements, explicit bindings, and defaults.
   - Reject ambiguous or unsatisfied bindings.
5. **Instance expansion**
   - Expand the root definition to stable leaf instances.
   - Preserve definition and instance identities independently.
6. **Interface and visibility resolution**
   - Resolve direct-child ports, exposure, hiding, and renaming.
   - Reject access through unexposed descendants.
7. **Schema and wiring validation**
   - Require exact ordered schemas in V1.
   - Reject hidden required inputs, multiple drivers, and illegal fan-in.
   - Create one mailbox for each ordinary wire.
8. **Scheduler and feature validation**
   - Preserve leaf scheduler-domain declarations.
   - Reject unsupported scheduler-spanning families or gated constructs.
9. **Semantic identity construction**
   - Construct occurrence IDs from the full semantic instance-declaration chain.
   - Construct stable transition, family, port, wire, mailbox, observation, and
     draw-site identities from those occurrences.
   - Map identities to compact runtime ordinals and, if necessary, versioned
     current-width RNG words. Any collision in a finite mapping is a link error,
     not a reason to reassign an existing identity.
10. **Flattening**
    - Emit flat leaf boxes, transitions, ports, outputs, wires, mailboxes,
      claims, summaries, and scheduler domains.
    - Exposure and renaming emit mappings, not mailboxes.
11. **Canonical ordering**
    - Sort executable collections by stable identity unless order is semantic.
    - Reject duplicate semantic identities or content-derived identity
      collisions deterministically; do not probe into an order-dependent spare
      assignment.
12. **Plan and provenance construction**
    - Emit `ExecutablePlanV1` containing the identity map and complete linked
      provenance tuple, plus a non-semantic deterministic link report.
13. **Final plan validation**
    - Apply the Lean plan validator before serialization.
    - A linker success result always contains a valid plan.

### 10.3 Deterministic failure

Link failure is atomic: no partial executable plan is emitted. The linker SHOULD
collect independent errors when safe, but MUST stop stages that would otherwise
invent identities or dereference invalid structure.

Errors are sorted deterministically by stable source identity, error code, and
related identity—not by hash-map or traversal order.

---

## 11. Link errors and diagnostics

### 11.1 Error shape

```lean
inductive LinkErrorCodeV1 where
  | unknownVersion
  | duplicateStableId
  | missingDefinition
  | recursiveDefinition
  | missingPort
  | inaccessibleDescendantPort
  | directionMismatch
  | schemaMismatch
  | hiddenRequiredInput
  | multipleDrivers
  | ambiguousParameterBinding
  | unsupportedFeature
  | schedulerBoundaryViolation
  | identityCollision
  | reservedRuntimeIdentity
  | invalidConstraint

structure LinkErrorV1 where
  code : LinkErrorCodeV1
  message : String
  primary : StableId
  sourceSpan : Option SourceSpan
  related : List StableId
  sourcePath : Option String
```

Machine error codes are stable within the linker semantic version. Human
messages may improve without becoming control-flow APIs.

### 11.2 Source maps

A source map is not restricted to one-to-one mappings:

- one component definition may map to many instantiated leaf objects;
- one product expression may map to several instances and tagged ports;
- one exposure may map to no executable state and one boundary alias;
- one primitive transition maps to one transition per instance;
- a future family may map to one event plus several legs and claims; and
- normalization may map multiple equivalent source declarations to one plan
  object only when the source semantics permits it.

The map MUST be bidirectionally queryable for diagnostics:

```text
source declaration -> zero/one/many plan objects
plan object         -> one/many source declarations and instance context
```

Source spans and display paths are non-semantic. Stable source IDs are semantic.

---

## 12. `ExecutablePlanV1`: the flat runtime contract

### 12.1 Shape

The plan remains flat but makes execution-relevant identity and mailbox state
explicit:

```lean
structure PlanCoreV1 where
  modelId : StableId
  dt : Scientific
  params : List ParamDecl
  leafInstances : List PlannedLeaf
  transitions : List PlannedTransition
  ports : List PlannedPort
  wires : List PlannedWire
  mailboxes : List PlannedMailbox
  schedulerDomains : List PlannedSchedulerDomain
  summaries : List PlannedSummary
  enabledFeatures : List String
  runtimeIdentityTable : RuntimeIdentityTable

structure HashRecordV1 where
  algorithm : String
  domain : String
  digest : String

inductive PlanOrigin where
  | linked
  | directStable
  | legacy

structure LinkedProvenanceV1 where
  sourceArtifactHash : HashRecordV1
  linker : LinkerDescriptor
  sourceMap : SourceMapV1

structure ExecutablePlanV1 where
  schemaVersion : String
  identityScheme : String
  origin : PlanOrigin
  core : PlanCoreV1
  identityMap : IdentityMapV1
  linkedProvenance : Option LinkedProvenanceV1
```

The exact decomposition is illustrative. The contract is that all fields needed
to execute and reproduce the plan are inside the plan envelope or executable
core. No runtime behavior depends on loading `composition-source.json`.

### 12.2 Plan invariants

A valid plan MUST satisfy:

- every runtime ordinal maps to exactly one stable semantic identity;
- stable identities are unique in their declared namespace;
- compact ordinals are not reused as semantic or RNG identities;
- reserved RNG namespaces remain excluded;
- every ordinary wire owns exactly one mailbox;
- mailbox identity includes a stable wire ID and both endpoints;
- every input has at most one driver unless an explicit merge leaf exists;
- every port endpoint exists and has the required direction and exact schema;
- exposure and rename records allocate no mailbox;
- every leaf belongs to exactly one scheduler domain;
- every transition reads and writes only permitted owned resources;
- `enabledFeatures` is mandatory, recognized, duplicate-free, and sorted for
  linked, stable-direct, and normalized legacy plans; normalized legacy plans
  use their historically correct baseline set, currently empty;
- canonical list order follows the declared ordering algorithm;
- `identityMap` and `runtimeIdentityTable` are present for linked, stable-direct,
  and normalized legacy plans; and
- `linkedProvenance` is present as one complete tuple for `origin = linked` and
  absent for `directStable` and `legacy`; and
- a linked plan's `linker.sourceMapSchema` identifies the embedded source-map
  representation so the plan remains interpretable when moved without its
  bundle manifest.

### 12.3 No second mutable hierarchy

The source hierarchy is not copied into mutable runtime state. The plan may
carry hierarchy metadata and source paths for diagnostics, but execution is over
flat leaf instances, mailboxes, claims, and scheduler domains.

---

## 13. Identity, RNG, and mailbox contract

### 13.1 Semantic identity tuple

A stochastic declaration derives from persisted source identities:

```text
component-definition-id
× instance-occurrence-id
× local-transition-or-family-id
× event/entity-key
× draw-site-id
```

For example:

```text
<Population-v1, occ:7f3c...91a2, infect-v1, person-1042, race-draw>
```

A UI may display the same occurrence as `north/epidemic/population`, but that
qualified path is non-semantic and may change under renaming.

Display renaming, exposure, hiding, flattening, and reassociation that preserves
the semantic instance-declaration chain MUST NOT change that tuple. Instantiating
the same composite twice produces different occurrence chains and therefore
different tuples.

### 13.2 Runtime identity mapping

The current runtime uses a dense `u32` rule word in the Philox coordinate. The
new plan must choose one versioned strategy:

- widen or restructure the RNG coordinate so the versioned semantic identity is
  represented without finite-space reassignment; or
- deterministically map stable semantic identities into the current word with
  reserved-namespace exclusion and reject the plan if any two accepted
  identities collide.

The exact mapping remains open, but it MUST be:

- a pure function of each semantic identity and versioned configuration;
- insertion-invariant for unrelated siblings in every accepted collision-free
  identity set;
- reproducible by a clean build with no external registry;
- collision checked, with collisions reported as deterministic link errors
  rather than resolved by reassigning existing words; and
- preserved in the executable plan and run manifest.

### 13.3 Mailbox identity

Each ordinary wire has one mailbox. Its stable identity derives from:

```text
wire-occurrence-id
× source-instance-occurrence-id
× source-port-id
× target-instance-occurrence-id
× target-port-id
```

Including the target disambiguates fan-out. Exposure, hiding, and renaming have
no mailbox identity because they create no mailbox.

### 13.4 Legacy interpretation

Current positional rule IDs remain valid only under an explicitly tagged legacy
identity scheme. Legacy plans can remain reproducible without satisfying:

- sibling insertion invariance;
- product noninterference at draw-coordinate level; or
- stable canonical plan ordering.

No migration rewrites an archived legacy plan and then claims it is the same
artifact.

---

## 14. Independent denotations and linker preservation

### 14.1 Why two meanings are necessary

Defining source meaning as `denotePlan (link source)` would make preservation
true by construction and would not independently check flattening. Instead:

- source semantics interprets definitions, instances, product, hierarchy,
  exposure, and delayed wires directly; and
- plan semantics interprets flat leaf instances and explicit mailboxes.

Both may share the primitive leaf transition semantics and table/channel
algebra, but source hierarchy and flattening remain independently modeled.

### 14.2 Observation contract

```lean
structure CompositionObservation where
  leafState : StableId → Tick → CanonicalTableState
  mailboxState : StableId → Tick → CanonicalTableState
  externalOutputs : StableId → Tick → CanonicalTable
  fired : Tick → List StableId
  drawCoordinates : Tick → List DrawCoordinate
  observations : StableId → Tick → Scalar
```

The final equality or equivalence relation must explicitly state whether it
includes each field. Hashes are consequences of canonical artifacts, not a
substitute for defining observations.

### 14.3 Core preservation theorem

```lean
opaque denoteSource :
  CompositionSourceV1 → Inputs → CompositionObservation

opaque denotePlan :
  ExecutablePlanV1 → Inputs → CompositionObservation

opaque linkV1 :
  CompositionSourceV1 → Except (List LinkErrorV1) LinkResultV1

theorem linkV1_produces_valid_plan
    (src : CompositionSourceV1)
    (result : LinkResultV1)
    (h : linkV1 src = .ok result) :
    ValidPlanV1 result.plan := by
  sorry

theorem linkV1_preserves
    (src : CompositionSourceV1)
    (result : LinkResultV1)
    (h : linkV1 src = .ok result) :
    denoteSource src = denotePlan result.plan := by
  sorry
```

The theorem statements and observation type are required before source syntax is
considered stable. Proofs may be deferred consistently with the project's
current policy, but executable preservation tests gate rollout until proofs are
complete.

### 14.4 Additional obligations

The Lean specification SHOULD state:

- product identity, associativity, and selected symmetry quotient;
- unwired product projection/noninterference;
- exposure and rename add zero delay;
- every ordinary wire adds exactly one tick;
- nesting and flattening preserve mailbox state and traces;
- local transitions lift without changing their meaning;
- disjoint accepted effects commute;
- stable identities survive allowed refactors;
- canonical linking is deterministic;
- canonical source and plan encoders round-trip; and
- link errors are deterministic under irrelevant source ordering.

---

## 15. Direct flat IR policy

Direct flat IR must not bypass the canonical plan contract:

```text
CompositionSourceV1 -> Lean linker ----------------┐
                                                   ├-> CanonicalPlan validation -> execution
Direct flat input -> parse + normalize + validate -┘
```

### 15.1 Stable direct plans

A machine writer MAY emit a versioned stable-identity flat plan. It MUST satisfy
the same:

- plan schema;
- identity uniqueness and reserved-namespace rules;
- mailbox and endpoint rules;
- canonical ordering;
- hash algorithm; and
- Rust validation contract

as a source-linked plan.

Its linked-provenance tuple—source artifact hash, canonical linker descriptor,
and source map—is wholly absent, not partially populated or fabricated. Its
`identityMap` and `runtimeIdentityTable` remain mandatory because they are plan
semantics, not source provenance. A direct plan cannot claim source-level
hierarchy, flattening, or refactoring proofs.

### 15.2 Legacy direct plans

Current unversioned flat JSON remains accepted through an explicit legacy parser
branch and receives `legacy-positional-v1` interpretation. Missing version fields
MUST NOT be guessed as stable compositional identity.

### 15.3 Twin fixtures

At least one `CompositionSourceV1` fixture and one hand-authored stable flat
fixture MUST normalize to the same plan semantic hash. This checks that direct
flat input is an alternate producer, not an alternate executable semantics.

---

## 16. Frontend neutrality

### 16.1 Neutral source records

The source schema is published independently of Lean syntax. Its specification
must define fields, ordering, stable IDs, schemas, constructor meanings, and
error conditions without referring to Lean elaborator objects.

`CompositionSourceV1` MUST NOT require:

- Lean proof terms;
- Lean declaration names;
- Lean source spans;
- Lean-generated declaration order;
- Lean-specific name mangling; or
- a type-theoretic value not represented in the serialized schema.

Frontend-only conveniences such as dimensional typing MAY be discharged before
source serialization. Obligations required of every producer belong in the
linker.

### 16.2 Non-Lean producer conformance

A non-Lean producer fixture MUST link to byte-identical canonical plan core
bytes as the equivalent Lean-produced source fixture.

A future Rust or other linker MAY be implemented for convenience, but it is
conforming only when it matches the canonical Lean linker for its declared
semantic version. It is not co-authoritative.

### 16.3 Lean toolchain failure hedge

The neutral source and flat plan preserve the existing frontend-swap hedge:

- previously linked plans execute without Lean;
- neutral source can be inspected and migrated independently;
- another frontend can emit source records; and
- an alternative linker can be built against the published semantics if Lean is
  eventually retired.

---

## 17. Rust and runtime boundary

Rust initially receives only `ExecutablePlanV1`.

The Rust side MUST:

1. reject unknown required plan and identity versions;
2. parse canonical and non-canonical input bytes into the same semantic model
   only if the format permits non-canonical transport;
3. validate all plan invariants independently;
4. preserve stable IDs and identity mappings in reports and manifests;
5. execute explicit flat mailboxes and scheduler domains; and
6. reject source-only constructs that somehow cross the plan boundary.

The Rust validator is defense in depth, not a second linker. It should not infer
exposure, flatten hierarchy, assign source identities, or repair source errors.

The CPU interpreter remains the executable oracle. Every optimized backend is
differentially checked against it for the selected determinism contract.

### 17.1 Proposed command flow

```sh
# Lean-authored source
cd frontend
lake exe sembla-source-export EpidemicPolicy /tmp/epidemic-source.json
lake exe sembla-link /tmp/epidemic-source.json \
  --plan /tmp/epidemic-plan.json \
  --manifest /tmp/epidemic-bundle.json
cd ..

# Rust consumes only the plan
cargo run -p sembla-cli -- validate /tmp/epidemic-plan.json
cargo run -p sembla-cli -- run /tmp/epidemic-plan.json --seed 55 --ticks 200
```

Command names are illustrative.

---

## 18. Validation ownership

| Check | Frontend | Lean linker | Rust validator |
|---|---:|---:|---:|
| Human syntax and local elaboration | primary | no | no |
| Frontend-only units/dimensions | primary | serialized result only | no |
| Source schema/version | optional early | authoritative | no source input |
| Duplicate stable IDs | optional early | authoritative | plan IDs rechecked |
| Definition cycles | optional early | authoritative | not applicable after flattening |
| Parameter binding | optional early | authoritative | resolved vector rechecked |
| Port visibility and exposure | optional early | authoritative | final endpoints only |
| Exact wire schema | optional early | authoritative | defense in depth |
| Driver/fan-in rules | optional early | authoritative | defense in depth |
| Stable identity/RNG mapping | no | authoritative | defense in depth |
| Canonical ordering | no | authoritative | can verify |
| Runtime ownership/claims | no | constructs plan | authoritative defense |
| Backend numeric behavior | no | no | runtime/differential tests |

No layer silently repairs a model. Validation either accepts the exact meaning
or returns deterministic errors.

---

## 19. Testing strategy

### 19.1 Lean source and linker tests

- serialize/parse round trips for every source constructor;
- required-feature canonicalization and unsupported-feature rejection;
- duplicate/missing/cyclic definition errors;
- parameter-binding ambiguity;
- exact schema and direction mismatch;
- visibility and hidden-input errors;
- fan-in without an explicit merge leaf;
- stable identity collisions and reserved namespaces;
- deterministic error ordering;
- product identity and associativity after linking;
- alpha-renaming and permitted reassociation stability;
- two instances of one composite having distinct nested occurrence, wire,
  mailbox, transition, and draw identities;
- source-map cardinality and reverse lookup; and
- literal canonical plan twins.

### 19.2 Formal specification checks

- source and plan denotations typecheck independently;
- preservation theorem statements typecheck;
- source meaning is not defined by invoking the linker;
- plan validity follows from successful linking; and
- serialization/canonicalization properties are stated.

### 19.3 Golden artifact tests

For every canonical fixture:

1. emit canonical source bytes;
2. link using the canonical linker;
3. compare exact plan bytes with the checked-in fixture;
4. validate using Rust;
5. execute twice at fixed seed;
6. compare output bytes, state hashes, output hashes, mailboxes, fired stable
   identities, and draw coordinates; and
7. reject any unreviewed fixture regeneration.

### 19.4 Non-Lean conformance

At least one source fixture is produced without Lean syntax and must link to the
same canonical plan core as its Lean twin.

### 19.5 Runtime and backend tests

| Fixture | Required result |
|---|---|
| `IndependentEpidemicPolicy` | Population projection matches standalone Population, including draws under stable identity |
| `EpidemicPolicy` | Count arrives after one tick; returned restriction completes a two-tick loop |
| `RegionalResponse` | Nested source and flat plan have identical states, mailboxes, draws, outputs, and observations |
| Two renamed/reassociated regions | Selected canonical plan and draw quotient is unchanged |
| Fan-out fixture | Distinct target endpoints receive distinct mailbox identities |
| Direct-flat/source twin | Same plan semantic hash and runtime trace |
| Legacy positional fixture | Old output remains byte-identical under legacy interpretation |
| CPU/CUDA composition corpus | Differential agreement for plan, mailbox, conflict, and draw semantics |

Property-based tests SHOULD generate small definitions, instances, products,
wirings, nestings, renames, and reassociations, then compare source and plan
observations.

---

## 20. Migration strategy

### 20.1 Migration principles

- Old artifacts are never silently reinterpreted.
- New stable identity begins under an explicit version.
- The first linked plan embeds source identity and maps even before all source
  constructors are public.
- Source and plan schemas evolve through explicit migrations.
- A migration produces a new artifact with a new hash and records its parent;
  it does not mutate historical identity.

### 20.2 Input migration matrix

| Existing input | Initial treatment | New guarantees |
|---|---|---|
| Current `sembla_model` | Continue direct flat export; optionally emit trivial `CompositionSourceV1` through new frontend path | Legacy path unchanged until explicitly migrated |
| Current direct Lean IR | Serialize as legacy direct plan | No source-level hierarchy proof |
| Current flat JSON | Parse with `legacy-positional-v1` | Existing reproducibility only |
| New Lean composition syntax | Emit `CompositionSourceV1`, invoke Lean linker | Full stable identity and provenance |
| Non-Lean composition producer | Emit same neutral source schema | Same linker and plan contract |
| Stable direct machine plan | Normalize/validate as direct `ExecutablePlanV1` | Plan-level guarantees, no source guarantees |

### 20.3 Linker-version retention

Reproducibility can be preserved by retaining the exact linked plan. Relinking
old source under a new linker is a new derivation unless the new linker declares
byte-compatible semantics for that source/linker version.

The bundle records the original linker semantic version and implementation
build. CI SHOULD retain executable access to supported historical linker
semantics or retain enough golden plans that relinking is unnecessary for run
reproduction.

---

## 21. Rollout phases

### Phase 0 — decisions and semantic specification

Deliver:

- selected observation quotient;
- independent source and plan denotation skeletons;
- preservation theorem statement;
- stable identity and mailbox identity specification;
- source, plan, linker, map, encoding, and hash version plan;
- direct-flat compatibility policy;
- scheduler-preserving refactor scope; and
- entity-ID namespacing decision.

**Exit criterion:** no syntax implementation begins while these remain implicit.

### Phase 1 — artifact and identity foundation

Deliver:

- versioned executable-plan envelope;
- stable IDs and a mandatory identity map for every plan origin;
- plan-schema support for the optional all-or-nothing linked-provenance tuple;
- legacy positional parser branch;
- canonical ordering and hash functions;
- Lean and Rust plan validators; and
- artifact/manifest fields from the first linked plan.

No reusable composition syntax is required yet.

**Exit criterion:** current flat models can pass through the new envelope without
behavior changes, and legacy fixtures remain byte-compatible under legacy mode.

### Phase 2 — `CompositionSourceV1`, primitive definitions, and product

Deliver:

- neutral serialized source schema;
- canonical Lean linker executable;
- reusable primitive definitions and instances;
- basic composite definitions sufficient to represent product and repeated
  composite instantiation;
- occurrence IDs for repeated composite instantiation;
- disjoint tagged product;
- source/plan maps and hierarchy-aware widgets; and
- product identity, associativity, and noninterference tests.

**Exit criterion:** `IndependentEpidemicPolicy` links to two unwired current-shaped
leaf instances and projects to standalone behavior; two instances of one
composite have disjoint nested occurrence and draw identities.

### Phase 3 — ordinary wires and mailboxes

Deliver:

- exact-schema directional source wires;
- endpoint ownership, direction, visibility, and single-driver validation;
- one explicit mailbox per ordinary wire;
- empty tick-zero mailbox initialization;
- fan-out-safe wire and mailbox occurrence identities;
- exactly one tick of delivery delay; and
- delayed feedback trace tests.

**Exit criterion:** in `EpidemicPolicy`, the population count reaches policy
after one tick and the resulting restriction returns to population after the
second tick, with two distinct stable mailboxes.

### Phase 4 — general nesting and interface control

Deliver:

- arbitrarily nested composite definitions beyond the basic Phase 2 product
  container;
- expose, hide, and rename;
- visibility validation;
- flattening equivalence tests; and
- source-aware diagnostics.

**Exit criterion:** `RegionalResponse` links without adding a mailbox or tick.

### Phase 5 — explicit adapters and merges

Deliver:

- exact schemas remain default;
- explicit project/reorder/rename adapters;
- deterministic commutative fan-in leaves; and
- schema/data-migration hooks where justified.

**Exit criterion:** no implicit port coercion or multi-driver semantics exists.

### Phase 6 — synchronized families and global claims

Begin only after event matching, join compilation, clock ownership, atomic
commit, reporting, identity, and scheduler scope are frozen.

**Exit criterion:** every family event has one stable identity, explicit
participants, one clock, combined claims, and all-or-none commit.

### Phase 7 — assertions and constrained products

Observational assertions MAY land earlier. Semantic invariants require
initialization, local preservation, family preservation, and concurrent-commit
preservation semantics.

**Exit criterion:** invariant failure is deterministic rejection/error, never
rollback, repair, or silent filtering.

### Phase 8 — heterogeneous schedulers

Deliver:

- leaf scheduler declarations;
- common outer boundaries;
- stable internal draw/event coordinates;
- scheduler-spanning family gating; and
- full domain plan in the run manifest.

**Exit criterion:** no domain observes another domain's internal substeps.

---

## 22. Implementation map

The file names below are illustrative and should be finalized in PRDs.

### Lean

```text
frontend/Sembla/Composition/Source.lean       CompositionSourceV1 types
frontend/Sembla/Composition/Json.lean         canonical source encoding
frontend/Sembla/Composition/Semantics.lean    source denotation
frontend/Sembla/Composition/Link.lean         canonical linker
frontend/Sembla/Composition/Errors.lean       stable error codes
frontend/Sembla/Composition/SourceMap.lean    source/identity maps
frontend/Sembla/Plan.lean                     versioned plan envelope
frontend/Sembla/PlanSemantics.lean            flat plan denotation
frontend/Sembla/PlanJson.lean                 canonical plan encoding
frontend/Sembla/LinkMain.lean                 standalone linker executable
```

Existing `frontend/Sembla/IR.lean` primitive types SHOULD be reused where they
already express leaf semantics. Reuse must not confuse component definitions
with concrete flat instances.

### Rust

```text
crates/sembla-ir/src/plan.rs                  plan envelope and version dispatch
crates/sembla-ir/src/identity.rs              stable/runtime identity validation
crates/sembla-ir/src/validate.rs              canonical plan validation
crates/sembla-cli/src/main.rs                 plan validate/run and bundle commands
crates/sembla-runtime/...                     explicit mailbox/domain consumption
```

Rust does not initially need a `CompositionSourceV1` linker implementation.

### Tests and fixtures

```text
frontend/Sembla/CompositionTests.lean
frontend/Sembla/CompositionProofs.lean
frontend/fixtures/composition-source/*.json
examples/plans/*.json
crates/sembla-ir/tests/plan_validation.rs
crates/sembla-runtime/tests/linked_composition.rs
```

---

## 23. One-way doors and mitigations

| One-way door | Failure if chosen casually | Required mitigation |
|---|---|---|
| Semantic authority | Source laws and plan behavior drift | Independent denotations and explicit preservation theorem |
| Stable identity/RNG scheme | Scientific traces and CRN pairing change | Versioned scheme, persisted map, legacy interpretation |
| Public source schema | Every producer and archive depends on early field choices | Neutral V1 schema, explicit migrations, no Lean accidents |
| Canonical ordering/hashes | Equivalent models unexpectedly get new identities | Version canonicalization independently; define hash coverage |
| Artifact retention | Source cannot reproduce old plan or plan loses author intent | Archive both source and exact plan with linker descriptor |
| Primitive meanings | Delayed wires later conflated with sharing or atomic events | Separate constructors from V1 |
| Direct flat escape hatch | Two runtime semantics and bypassed invariants | One canonical-plan validation/execution entry point |
| Static topology | Later dynamic component creation requires redesign | State the boundary now; keep dynamic ACSet state inside leaves |
| Frontend neutrality | Lean becomes impossible to replace | Publish neutral records and non-Lean conformance fixture |

Option D itself is not the one-way door. Its purpose is to put these one-way
contracts behind explicit versions and artifacts.

---

## 24. Rejected or deferred alternatives

### 24.1 Surface-only Lean lowering as the final architecture

Rejected because hierarchy and composition semantics disappear before the
serialized boundary, other frontends cannot share the source language, and
source-level laws become Lean elaborator properties rather than artifact
properties.

A narrow prototype is acceptable only if its output is private/experimental and
the plan already carries the future identity/provenance envelope.

### 24.2 Recursive executable hierarchy

Rejected as the default because it forces recursion into validation, state
addressing, schedulers, CPU, CUDA, hashing, reporting, and every fixture. It also
creates a risk that hierarchical and flat executions become different
semantics.

Runtime-visible hierarchy can be reconsidered if source maps and flat-plan
metadata prove insufficient.

### 24.3 Free composition AST as the executable contract

Rejected because unrestricted categorical generality can outrun concrete model
needs and every runtime/backend would need an interpreter. Algebraic syntax is
valuable at the source layer; execution remains one canonical flat plan.

### 24.4 Rust-only canonical linker

Rejected because source-to-flat translation would sit outside the project's Lean
semantic ground truth. Rust may later implement a conforming linker, but Lean
remains canonical for the declared linker semantic version.

### 24.5 Lean-only serialized source

Rejected because it breaks the frontend-swap hedge. Lean may be the first and
best authoring frontend without making Lean declarations the interchange format.

### 24.6 Sidecar-only provenance

Rejected because copied or archived plans lose their source relationship. The
plan envelope embeds the source hash and required identity relationship; the
bundle retains complete source and maps.

### 24.7 One undifferentiated model hash

Rejected because source bytes, normalized source meaning, executable semantics,
and complete envelope integrity are different claims.

---

## 25. Explicitly open questions

The architecture fixes the pipeline but leaves these choices for decisions or
prototypes:

1. What exact stable-ID and occurrence-ID encoding is used?
2. Is the runtime RNG coordinate widened, or are stable IDs mapped to the
   current `u32` word with collisions rejected?
3. What exact observations define `denoteSource ≈ denotePlan`?
4. Is product symmetry byte-visible canonical equality or equality under a
   documented interface isomorphism?
5. Can instances share parameters by explicit binding, or only receive copied
   values?
6. May component definitions declare `dt`, or only scheduler requirements under
   a root outer interval?
7. Which source-map fields are excluded from the plan semantic hash?
8. Is the native source storage itself an ACSet, or only losslessly mappable to
   one?
9. Which invariant classes are statically proved, runtime asserted, coordinated,
   or rejected?
10. When, if ever, are `Share`/`Identify` and synchronized families added?
11. Is a non-Lean reference linker useful as an independent test oracle?
12. How long are historical linker semantic versions supported for relinking?

Synchronized families, semantic invariants, variable sharing, and heterogeneous
schedulers are not required for the first composition release.

---

## 26. Architecture and implementation acceptance

### 26.1 Architecture-approval and planning gate

Option D is ready for implementation planning when all of the following are
settled in this document or an accepted decision record:

1. The source → canonical Lean linker → flat plan → Rust pipeline and trust
   boundary are approved.
2. The neutral source schema and flat plan contracts are specified
   independently of Lean surface syntax.
3. Source and plan semantic authority, the independent-denotation requirement,
   the observation quotient, and preservation theorem shape are agreed.
4. Stable declaration IDs, instance-occurrence IDs, wire/mailbox IDs, runtime
   ordinals, and legacy identity interpretation are distinguished.
5. The RNG strategy is selected: widened coordinate or collision-rejecting
   finite mapping.
6. Source, plan, linker, identity, map, canonicalization, and hash versions and
   hash-record domains are assigned.
7. The artifact-bundle, bundle-root, direct-flat, and migration policies are
   unambiguous.
8. Link stages, deterministic error categories, canonical ordering, and rollout
   phase ownership are specified.
9. Remaining open questions are explicitly assigned to later phases and do not
   block the first implementation slice.
10. Accepted choices are recorded in `DECISIONS.md` before implementation.

This gate requires an implementable specification, not completed linker,
runtime, fixture, or backend code.

### 26.2 Phase and final implementation acceptance

The responsible rollout phase is complete only when its applicable criteria
below pass; the complete Option D implementation satisfies all of them:

1. The neutral source schema parses and round-trips independently of Lean
   syntax.
2. The canonical linker signature, stages, deterministic error model, and
   canonical ordering are implemented.
3. Source and plan denotations exist independently in Lean.
4. The linker-validity and preservation theorem statements typecheck; proof
   status is explicit.
5. Every runtime ordinal maps through a mandatory identity map, including stable
   direct and normalized legacy plans.
6. Repeated instances of one composite receive distinct nested occurrence,
   transition, wire, mailbox, and draw identities.
7. Mailbox identity includes the wire occurrence and both endpoint occurrences.
8. Linked provenance is all present for linked plans and absent for direct or
   legacy plans; execution identity maps remain present in every plan.
9. The plan is independently runnable and contains every execution-relevant
   identity and relationship.
10. Every plan origin and run manifest carries the canonical enabled-feature
    set; only linked origins carry the all-present source/linker tuple.
11. Complete `{algorithm, domain, digest}` hash records and all relevant versions
    appear in bundle and run manifests.
12. Unknown versions, hash domains, collisions, and unsupported features reject
    deterministically.
13. Direct flat IR enters the same canonical-plan validation/execution path and
    carries an explicit identity interpretation.
14. Legacy artifacts retain byte-compatible legacy behavior.
15. A non-Lean source fixture and Lean source fixture link to the same canonical
    plan core.
16. Product, repeated-composite occurrence, wire delay, fan-out mailbox,
    exposure, nesting, identity, source-map, and noninterference fixtures pass.
17. Rust validates every linked golden plan.
18. CPU and CUDA differential tests compare mailboxes, draws, conflicts, states,
    outputs, and observations—not only final hashes.
19. The artifact bundle can be moved without losing the source-plan relationship.
20. No current syntax or runtime feature is claimed merely because it is
    specified here.

---

## 27. Recommended next decisions

The architecture should be accepted or revised through a focused decision with
this order:

1. Approve the source → canonical Lean linker → flat plan → Rust pipeline.
2. Approve independent source and plan denotations and the preservation
   obligation.
3. Approve frontend-neutral serialized source from the first public release.
4. Approve the direct-flat compatibility policy.
5. Freeze version and hash separation.
6. Select the stable identity/RNG migration design.
7. Approve Phase 0 and Phase 1 before implementing composition syntax.

This ordering prevents attractive surface syntax from becoming a compatibility
contract before the semantic identities, artifacts, and linker boundary are
ready.
