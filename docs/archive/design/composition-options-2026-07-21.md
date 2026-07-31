# Composition options: product, wiring, nesting, and constrained systems

> **Archived design exploration — 2026-07-21.** Option D was selected. Current architecture is documented in [`../../design/option-d-architecture.md`](../../design/option-d-architecture.md), with normative decisions in [`../../../DECISIONS.md`](../../../DECISIONS.md).

**Status:** Design options / discussion note, 2026-07-21. No option in this
note is implemented merely by being described here.
Option D was selected and recorded in DECISIONS.md §J;
option-d-architecture.md plus docs/prds-composition/README.md supersede this
note's open decisions.

**Scope:** Reusable systems/boxes, parallel product, wiring and feedback,
nesting and encapsulation, transition behavior under composition, constrained
products/invariants, stochastic identity, and heterogeneous scheduler
boundaries. This note does not propose a relational Cartesian product of table
rows.

**Authority:** This note develops the original commitments in
[`DESIGN.md`](../../../DESIGN.md) §1 and §4.4. After an option is selected, the
normative decision should be recorded in [`DECISIONS.md`](../../../DECISIONS.md)
and implemented through focused PRDs.

---

## 1. Executive summary

Sembla's design says that systems compose by **nesting, wiring, and product**,
and that moving a valid box boundary must not silently change meaning. The
current implementation realizes only a flat subset:

- a model owns an ordered list of boxes and model-level wires;
- a box owns tables, transitions, ports, and views, but no child boxes;
- unwired boxes advance independently at the same global tick;
- wires transport exact-schema tables with one-tick delay; and
- the runtime executes a flat plan.

There is currently no reusable component definition, product operator, nested
box, exposed-port map, invariant, or composition-stable stochastic identity.

The recommended direction is a **hybrid architecture**:

1. introduce a typed composition-source representation for reusable components,
   instances, tensor/product, wiring, nesting, exposure, hiding, and constraints;
2. normalize and validate it with one deterministic, versioned linker;
3. execute a canonical flat plan, initially using the existing Rust/CPU/CUDA
   runtime shape; and
4. retain a source map and identity map so diagnostics, widgets, manifests, and
   stochastic coordinates refer to stable component/instance declarations.

This obtains first-class composition without making recursive hierarchy a
second executable semantics.

Two semantic prerequisites should be settled before syntax implementation:

- stochastic identities must survive permitted composition refactors; and
- refactoring invariance must be scoped to transformations that preserve
  primitive scheduler domains and their numerical contracts.

### Example notation and running systems

Examples appear throughout this note, with their status repeated beside each
code block:

- **Current/implemented Sembla syntax** is accepted by today's flat
  `sembla_model` frontend. An abridged block may contain `...`; the linked
  source is the executable reference.
- **Proposed Sembla composition syntax (not implemented)** illustrates a
  possible future surface. Keywords such as `sembla_component`, `instance`,
  `expose`, `hide`, `restrict`, and `family` are not accepted today.
- **Proposed composition-source IR** and **semantic pseudocode** describe the
  intended graph, linker, or operational behavior rather than a parser.
- **Conceptual ACSet/Catlab-style notation** explains a mathematical
  correspondence and is not a frozen Catlab or Sembla API.

The primary running example is the existing workplace SIR population plus a
policy controller. Its complete current source is
[`Step05_PolicyFeedback.lean`](../../../frontend/Sembla/Tutorial/Step05_PolicyFeedback.lean).

**Current/implemented Sembla syntax (abridged):**

```lean
sembla_model policyFeedbackSIR
    (name := "tutorial_05_policy_feedback_sir")
    (dt := 0.25) where
  box population where
    input restriction_modifier where
      restriction : ℝ
    output infection_count from Person where
      infected : Int := count where health = I
    ...

  box policy where
    input infection_count where
      infected : Int
    output restriction_modifier from Controller where
      restriction : ℝ := sum (restriction)
    ...

  wire population infection_count -> policy infection_count
  wire policy restriction_modifier -> population restriction_modifier
```

**Proposed Sembla composition syntax (not implemented):**

```lean
sembla_component EpidemicPolicy where
  instance population := Population
  instance policy := Policy

  wire population.infection_count -> policy.infection_count
  wire policy.restriction_modifier -> population.restriction_modifier

  expose population.infection_count as infection_count
  expose policy.restriction_modifier as restriction_modifier
```

`EpidemicPolicy` will be reused for product, wiring, feedback, nesting,
identity, lowering, and test examples. A compatible `Hospital` component extends
it into `CareNetwork` where synchronized events, capacity constraints, and
heterogeneous scheduler domains need a genuinely multi-owner example.

> [!kimi] kimi: Review summary (2026-07-21)
> Reviewed against the current tree. The load-bearing claims check out:
> `validate` in `crates/sembla-ir/src/validate.rs` assigns dense
> declaration-order `u32` rule IDs, and `crates/sembla-runtime/src/rng.rs`
> keys Philox counters as `[tick, rule_id, entity_id, draw_idx]` with
> `u32::MAX - 1` / `u32::MAX` reserved — so §3.1's identity warning and §10's
> migration obligation are accurate, not hypothetical. The hybrid (Option D)
> direction is consistent with DESIGN.md §4.4's operad commitment and §4.6's
> sink rule. Section-level ideas, contributions, and concerns follow in
> `kimi` callouts throughout; none of them disputes the selected direction.

## 2. What “product” must mean

The word *product* is overloaded and must not silently select one of these
unrelated operations.

### 2.1 Machine/box product

For state machines or Moore-style boxes

```text
A : S_A × I_A → S_A × O_A
B : S_B × I_B → S_B × O_B
```

the parallel product is

```text
A ⊗ B : (S_A × S_B) × (I_A ⊕ I_B)
          → (S_A × S_B) × (O_A ⊕ O_B)
```

where the tagged/disjoint interfaces avoid accidental same-name merging. This
is the composition operation intended by this note.

**Proposed Sembla composition syntax (not implemented):**

```lean
sembla_component IndependentEpidemicPolicy := Population ⊗ Policy
```

This creates two independent state owners and qualified ports such as
`population.infection_count` and `policy.infection_count`; it creates neither
of the two feedback wires. Adding those wires produces `EpidemicPolicy` and
changes its behavior.

### 2.2 Factored attributes on one table

A table with

**Current/implemented Sembla syntax:**

```lean
health : {susceptible, infected}
custody : {community, prison}
```

has a factored row state. This is not the product of two reusable boxes, even
though its possible row values form a product of attribute domains.

### 2.3 Relational Cartesian product

Taking every row of table `A` with every row of table `B` is a relational
operation with potentially multiplicative cardinality. It is not machine
composition and should never be selected by `⊗` or `product` without an
explicit relational operator.

### 2.4 Model product

A complete model currently owns global `dt`, parameters, summaries, boxes, and
wires. Product should initially be defined for **components/boxes under one
owning model**, not arbitrary complete models. Model product would additionally
need rules for incompatible `dt`, parameter sharing, duplicate defaults/priors,
summary identity, and initialization.

The concrete distinctions are:

| Expression | Meaning | Selected by `⊗`? |
|---|---|---|
| `Population ⊗ Policy` | independent machine/component product | yes |
| one `Person` row with `(health, custody)` | factored row attributes | no |
| `Person CROSS JOIN Employer` | relational Cartesian product | no |
| `policyFeedbackSIR ⊗ hospitalModel` | product of complete models with separate global contracts | not initially |
| categorical product of two attributed C-sets | pointwise/slice construction whose attribute behavior may differ from machine product | no |

**Relational pseudocode—not composition syntax:**

```sql
SELECT * FROM Person CROSS JOIN Employer;
```

The ACSet distinction is not academic: Topos's
[“Acsets with variables”](https://topos.institute/blog/2023-06-20-acsets-with-variables/)
shows that categorical products of attributed data can change attribute types
or impose attribute-equality behavior. Sembla's `⊗` must therefore remain the
explicit machine operation defined in §2.1.

> [!kimi] kimi: A fifth overloading of “product” worth excluding by name
> To the CTMC-literate reader, "product" also suggests the **Kronecker/tensor
> product of generator matrices** used by stochastic automata networks (SANs)
> and superposed stochastic Petri nets to build a joint generator without
> enumerating the joint state space. §7.3's synchronized families are exactly
> SAN "synchronizing events," and the Kronecker literature is the standard
> answer to the "Cartesian rule explosion" this note avoids by lifting rule
> families rather than enumerating pairs. Naming that lineage (SANs, PEPA)
> gives the design a formal reference base, and clarifies that the machine
> product of §2.1 is a labeled parallel composition with (optionally) shared
> events — not generator Kronecker algebra, and not any of §2.2–§2.4.

## 3. Current implementation baseline

The current Lean IR is flat:

```text
Box {
  tables,
  transitions,
  inputs,
  outputs,
  views
}

Model {
  dt,
  params,
  boxes,
  wires,
  summaries
}
```

The Rust IR has the same shape. `Box` has no child/component field, and `Model`
has no composition expression. The surface language collects boxes and wires
directly into that flat representation.

The checked-in [`sir_policy.json`](../../../examples/sir_policy.json) is a separate
model with the same two-box/two-wire topology as the running Lean example. It is
not a serialization of `Step05_PolicyFeedback.lean`: its thresholds, restriction
encoding, and population sizes differ. It is used here only to show the current
flat executable IR shape.

**Current executable IR (abridged):**

```json
{
  "name": "sir_workplace_policy_feedback",
  "dt": 0.25,
  "boxes": [
    { "name": "population", "outputs": [{ "name": "infection_count" }] },
    { "name": "policy", "outputs": [{ "name": "restriction_modifier" }] }
  ],
  "wires": [
    {
      "from": { "box": "population", "port": "infection_count" },
      "to": { "box": "policy", "port": "infection_count" }
    },
    {
      "from": { "box": "policy", "port": "restriction_modifier" },
      "to": { "box": "population", "port": "restriction_modifier" }
    }
  ]
}
```

No reusable `Population` or `Policy` definition survives in that artifact;
there are only the concrete `population` and `policy` boxes. That is the gap the
proposed composition-source layer must fill.

The current executable composition contract is nevertheless useful:

1. all boxes read the same tick-start committed snapshot and current input
   mailboxes;
2. transition candidates are evaluated without within-tick visibility of other
   effects;
3. accepted effects commit at the tick barrier;
4. outputs are built from committed state;
5. wire deliveries become inputs for the next tick; and
6. views/summaries observe as sinks.

Consequences already relied upon:

- feedback cycles are guarded by one tick of delay;
- there are no same-tick wire cascades;
- unwired boxes do not exchange state;
- input ports have at most one driver;
- current wire schemas must match exactly and positionally; and
- tick-zero inputs are empty.

The current implementation is narrower than the long-term “finite tables on
wires” commitment: v0.1 output builders produce aggregate rows (`count`/`sum`)
rather than arbitrary finite-table streams. General table transport, keyed
message construction, and row-level cross-boundary messaging remain separate
extensions.

This flat contract should remain the initial execution target of any new
composition surface.

### 3.1 Existing semantic gaps

The current representation does not provide:

- reusable component definitions or multiple named instances;
- product/tensor syntax;
- nested composite boxes;
- port exposure, hiding, renaming, or adapters;
- scheduler annotations on reusable leaves;
- cross-component atomic transition families;
- constrained products or invariants;
- hierarchy-aware widgets or diagnostics; or
- composition-stable stochastic rule identity.

The last item is especially important. Rule IDs are currently assigned densely
by global box order followed by transition order. Philox draws use those IDs.
Inserting or reordering an unrelated sibling can therefore change later random
streams. A product operator cannot honestly promise refactoring invariance until
this is versioned and corrected.

## 4. Semantic vocabulary

A composition design should distinguish the following concepts.

### Component definition

A reusable declaration containing:

- private state schemas;
- local transitions;
- typed input and output ports;
- parameters or explicit parameter requirements;
- views/observations;
- local transition identities; and
- a scheduler requirement or capability.

In the proposed notation, `Population` is a definition; today's
`box population where ...` body supplies the concrete leaf behavior.

### Instance

A use of a component definition with:

- a stable instance identity distinct from its display name;
- parameter bindings;
- optional visible renaming; and
- initialization data.

In `instance city := Population`, `city` is an independently initialized use of
`Population`. Two instances of one definition are independent. Intentional
stochastic correlation may use an explicit correlation key, but shared mutable
state is not part of product composition and is out of scope for the initial
design. Any future shared-state mechanism would require separate ownership,
capability, conflict, and scheduler semantics.

### Primitive/leaf component

A component executed by one scheduler domain. Existing flat `population` and
`policy` boxes are the closest current analogues.

### Composite component

An internal graph of instances, products, wires, and boundary-port mappings
that can itself be instantiated as a component. `EpidemicPolicy` is the running
composite; `CareNetwork` can instantiate it alongside `Hospital`.

### Product/tensor

Parallel composition with product state and disjoint tagged interfaces. Product
does not create wires or shared state by itself. Thus
`Population ⊗ Policy` is not yet the feedback system.

### Wire

A delayed table channel from one output port to one input port. It is stateful:
the in-flight/next-tick mailbox is part of composite execution state. The wire
`population.infection_count -> policy.infection_count` is one such mailbox.

### Exposure

A zero-delay structural alias between a direct child port and the composite
boundary. For example, exposing `population.infection_count` lets a parent
connect to the same child output without allocating a second mailbox or adding
another tick.

Child ports are private outside their immediate composite unless that composite
exposes them. A parent may wire, expose, or hide a direct child's boundary port;
it may not reach through that child to an unexposed descendant port.

### Hiding

Removal of a direct child boundary port from the new composite's public
interface without changing internal execution. Because `EpidemicPolicy` exposes
`restriction_modifier`, a parent may hide `epidemic.restriction_modifier`; it
may not name `epidemic.policy.restriction_modifier` directly. Hiding does not
delete policy state or the internal feedback mailbox. A hidden input must be
internally wired or explicitly closed by a typed source; hiding must not invent
a default value.

### Renaming

A change to a visible label. Renaming `infection_count` to
`regional_infection_count` changes presentation, not stochastic identity or
transition behavior.

### Scheduler domain

The smallest component region advanced as one numerical solver/island during an
outer macro-step. In the later `CareNetwork` example, `Population` and
`Hospital` remain separate scheduler domains even when nested together.

### Constrained product

A product plus a predicate identifying legal joint states. For example,
`CapacitySafeCare := restrict CareNetwork` can require occupied beds not to
exceed staffed beds. It is a restricted subsystem, not merely a product
followed by post-hoc error correction.

The same running composite can be displayed as typed source records.

**Proposed composition-source IR (illustrative):**

```text
Definition
  Population
  Policy

Instance
  EpidemicPolicy.population : Population
  EpidemicPolicy.policy     : Policy

Wire [owner = EpidemicPolicy]
  population.infection_count      -> policy.infection_count          [delay = 1]
  policy.restriction_modifier     -> population.restriction_modifier [delay = 1]

Exposure [owner = EpidemicPolicy]
  population.infection_count -> EpidemicPolicy.infection_count       [delay = 0]
  policy.restriction_modifier -> EpidemicPolicy.restriction_modifier [delay = 0]
```

These are source-graph relationships, not four mutable runtime tables. The
linker resolves them into the flat boxes, ports, and mailboxes shown in §3.

## 5. Core product semantics

Let `A` and `B` be components with disjoint ownership by default.

### State

```text
State(A ⊗ B) = State(A) × State(B)
```

For the unwired running product:

```text
State(Population ⊗ Policy)
  = (Person/Employer tables, population-local mailboxes)
  × (Controller table, policy-local mailboxes)
```

The state is a pair of independently owned ACSet instances; it is not the
categorical product of their attributed data. When the feedback wires are added,
their two next-tick mailboxes become additional composite state.

### Interfaces

```text
Inputs(A ⊗ B)  = tag_A(Inputs(A))  ⊕ tag_B(Inputs(B))
Outputs(A ⊗ B) = tag_A(Outputs(A)) ⊕ tag_B(Outputs(B))
```

Concretely, `population.infection_count` and `policy.infection_count` remain
different ports even though the visible suffix is the same. Tags are structured
instance identities, not delimiter-concatenated strings, and same visible names
do not merge automatically.

### Transitions

A local transition of `A` lifts to the product as:

```text
(a, b) → (a′, b)
```

A local transition of `B` lifts as:

```text
(a, b) → (a, b′)
```

For example:

```text
population.infect : (population, policy) -> (population′, policy)
policy.restrict   : (population, policy) -> (population, policy′)
```

At the Moore-machine level, the product macro-step is the paired child step
from one shared old snapshot. Within each stochastic child step, the *declared
local rule families* form a tagged lifted union; they are not replaced by an
enumeration of every pair such as `(infect, restrict)` or `(recover, reopen)`.

Within a tau-leaped macro-tick, independent accepted events from both components
may commit together when their writes and claims are compatible. This is
consistent with the paired macro-step and avoids Cartesian rule explosion.

### Product noninterference law

For an unwired product with disjoint resources, projecting a run onto `A` must
match running `A` alone under the same parameters, inputs, entity identities,
scheduler, and stochastic identity. The analogous law holds for `B`.

This law is stronger than comparing final values: it should compare accepted
transition/draw coordinates, committed states, outputs, and observations.

> [!kimi] kimi: The noninterference law is falsifiable today — and fails on draws
> Worth stating sharply: with current dense rule IDs, `proj_A(run(A ⊗ B))`
> does **not** match `run(A)` on draw coordinates even for an unwired
> disjoint product, because `A`'s transitions receive different rule IDs when
> `B`'s precede them. Committed states may still coincide (hazards are
> rate-equivalent), so a state-only comparison could pass while the law as
> written fails. Two consequences: (1) the §16 test "sibling
> insertion/reordering preserving draw traces" will red against the current
> runtime and should be gated on the Phase 0 identity work rather than
> discovered during Phase 1; (2) until then, the honest statement of the law
> needs an explicit "modulo identity map" quotient, or tests should compare
> states only and say so.

## 6. Wiring and feedback semantics

A wire is not transition synchronization. It transports an output table after
one component has completed a macro-step, and the receiver sees it only during
the next macro-step.

**Current/implemented Sembla syntax:**

```lean
wire population infection_count -> policy infection_count
wire policy restriction_modifier -> population restriction_modifier
```

**Semantic pseudocode for the first wire:**

```text
tick 12 start:
  policy.infection_count contains the table delivered after tick 11

tick 12 commit:
  population finishes with infected = 137
  policy reacts only to its tick-11 input

after tick 12:
  [{ infected = 137 }] is installed in policy.infection_count for tick 13

tick 13 start:
  policy may now enable restrict
```

Therefore a transition in `population` cannot directly enable a transition in
`policy` during the same tick through an ordinary wire.

### Cycles

Feedback cycles remain legal because every actual wire is delayed. In the
running two-wire loop, a population count takes one tick to reach policy and a
new restriction takes another tick to return to population: the round trip is
two ticks. Exposure and renaming are structural aliases and add no delay. If a
future combinational adapter is added, zero-delay cycle detection becomes
mandatory.

### Fan-in and fan-out

The initial generalized design should preserve current behavior:

- fan-out from one output may be allowed;
- an input has at most one source; and
- multi-driver union/reduction requires an explicit typed merge component.

**Proposed Sembla composition syntax (not implemented):**

```lean
instance regional_counts := SumInfectionCounts
wire north.infection_count -> regional_counts.left
wire south.infection_count -> regional_counts.right
wire regional_counts.total -> policy.infection_count
```

The `SumInfectionCounts` leaf owns the deterministic, commutative reduction;
two wires may not target `policy.infection_count` directly. Implicit same-name
merging or last-writer-wins is forbidden.

### Schema compatibility

Retain exact ordered-schema equality initially. Reordering, projection,
renaming, width subtyping, enum coercion, and aggregation should be explicit
adapter components rather than implicit port variance.

**Proposed Sembla composition syntax (not implemented):**

```lean
instance select_count := ProjectInfectionCount
wire surveillance.daily_report -> select_count.report
wire select_count.infection_count -> policy.infection_count
```

Here the adapter—not the wire—projects and renames a richer report to the exact
`{ infected : Int }` schema. A future ACSet schema morphism/data-migration layer
could specify such adapters, while exact equality remains the first linker rule.

> [!kimi] kimi: Two wiring notes — loop delay and merge components
> **Loop delay.** A feedback pair `A → B → A` carries **two** ticks of delay
> around the loop: `B` reacts to last tick's `A.out`, and `A` sees last
> tick's `B.out`. For stiff feedback (a policy responding strongly to a
> fast-moving count) this is a classic source of discretization oscillation
> that has nothing to do with the model. This belongs with the `dt`
> discipline guidance (DESIGN.md open questions 2 and 7); a cheap diagnostic,
> echoing the deferred-loser saturation counter of §5.1, would flag feedback
> loops whose round-trip delay exceeds the fastest hazard they modulate.
> **Merge components.** The explicit fan-in merge is itself a leaf component,
> so its reduction must satisfy the §4.2 commutative-monoid restriction —
> otherwise Level A canonical ordering is unenforceable at the merge. Worth
> one normative sentence here so "explicit merge" is not read as "arbitrary
> order-dependent fold."

## 7. Transition semantics under composition

Composition must specify transitions at three levels.

### 7.1 Lifted local transitions

Most transitions remain owned by one leaf instance. Their guards, hazards,
claims, and effects read only that instance's state and declared inputs.

**Current/implemented Sembla syntax inside the flat `population` box:**

```lean
infect on Person : health: S →[
  β · freq (health = I) over employer ·
    (1.0 - inputSum restriction_modifier field restriction)
] I
```

Under the proposed source graph, the same declaration is reported as
`population.infect`; qualification and stable identity are linker behavior, not
a new independent transition. A product or nesting boundary must not rewrite
its stochastic identity or add an execution delay. Local effect right-hand
sides continue to read the same old snapshot, and accepted effects commit
simultaneously.

Qualified reporting may show:

```text
instance: population
transition: infect
semantic identity: <Population-id, population-instance-id, infect-id>
```

The display path can change under renaming; the semantic identity cannot.

### 7.2 Wired influence

The `inputSum restriction_modifier` expression above reads the table delivered
at the previous tick boundary. It never reads mutable `policy` state directly.
Wiring therefore changes a local hazard through declared input data but creates
neither an implicit joint transition nor a shared clock.

### 7.3 Candidate design: synchronized cross-component transition families

Extend the running domain with a `Hospital` component. Admitting one person must
atomically change `population.Person.care` and `hospital.Bed.occupied`. Two
ordinary wires cannot represent that same-tick all-or-nothing event: after one
leg commits, the other would react only in a later tick.

**Proposed Sembla composition syntax (not implemented):**

```lean
family admit_patient in CareNetwork where
  match request in admission_requests by request_id
  match person by request.person_id
  match bed by request.bed_id
  guard person.care = waiting ∧ bed.occupied = false ∧
    bed.hospital_id = person.hospital_id
  hazard admission_rate
  leg population.mark_admitted
  leg hospital.occupy_bed
  claims person, bed
```

The request row supplies the exact `(person_id, bed_id)` participant tuple; the
family does not search a Cartesian product of people and beds.

**Semantic contrast:**

```text
two local transitions + wires:  (waiting, free) -> (admitted, free) -> later (admitted, occupied)
synchronized family:            (waiting, free) -> atomically (admitted, occupied)
```

A complete family design would need to freeze all of these mechanics:

- participant rows/events are selected only through declared keys or explicit
  selectors, never an implicit Cartesian search;
- the participant tuple has a deterministic event key derived from its declared
  join keys/entity identities;
- the family guard and one hazard/clock govern that matched tuple;
- a leg bound into the family is not also independently scheduled unless an
  explicit separate local transition is declared;
- each leg reads/writes only state it owns;
- all participant claims are combined;
- failure to win any required claim prevents every leg from firing;
- the family RNG coordinate includes stable family, instance, event, and draw
  identities;
- all effect expressions read the old snapshot;
- all legs commit atomically or none commit; and
- the event is reported once, with participant details.

Whether guards/effects are declared directly on the family or supplied by typed
leg templates remains an open design choice. Until row correlation, candidate
enumeration, clock ownership, and reporting are specified, synchronized
families should remain deferred. If adopted, they should initially be restricted
to one scheduler domain or implemented by an explicit global boundary
coordinator. They must never be inferred merely from matching transition names
or connected ports.

> [!kimi] kimi: Three additions to the family mechanics list
> (1) **Clock semantics.** State the CTMC-level meaning: a family is *one*
> global exponential clock that enters the argmin race of **every**
> participating component. That is what makes its tau-leaped firing
> consistent with local events in both children, and it is the property a
> future proof or differential test must encode.
> (2) **Match compilation.** "Never an implicit Cartesian search" is right;
> strengthen it to "match clauses compile to the same declared-key join
> kernel as §4.2 map/filter/join." Otherwise `family` quietly becomes a
> second expression language with its own planner.
> (3) **CRN consequence.** Refactoring between "two transitions + wire" and
> "one family" changes the draw structure, so the common-random-number
> pairing of DESIGN.md §5.3 does not survive that rewrite. Since this note
> already declares the two representations non-equivalent, add one sentence
> telling scenario authors the choice is part of model identity, not a
> behavior-preserving refactor.

### 7.4 Conflicts and claims

Product components own disjoint resources by default. Shared or cross-component
resources require explicit capabilities/claims.

A generalized resource identity should include stable owner-instance and local
resource identities, not box list positions. Candidate ordering must remain
deterministic and independent of iteration order.

Every potentially overlapping state-cell write should either:

- be statically proven disjoint;
- participate in an explicit exclusive claim; or
- use a declared commutative merge operation.

Last-writer-wins is not an acceptable composition semantics. Duplicate or
uncovered overlapping writes should fail validation, not be discovered as an
incidental runtime double-write.

> [!kimi] kimi: Hashed rule IDs change the *meaning* of tie-breaks
> Today's conflict tie-break `(time, rule-id, entity-id)` (DESIGN.md §5.1)
> makes declaration order the arbiter of exact-time ties. After §10's
> identity migration, ties are decided by hashed/allocated IDs: still
> deterministic and iteration-order independent, but no longer related to
> anything the author wrote. Two things follow: (a) authors must never rely
> on declaration order for cross-component priority — meaningful priority
> belongs in an explicit ordering key, which §5.1's pluggable queue
> disciplines already provide; and (b) the migration must keep allocated IDs
> disjoint from the reserved `u32::MAX - 1` / `u32::MAX` sweep-seed
> namespaces (`rng.rs`), including under hash collision.

## 8. Constrained products and invariants

A constrained product is conceptually:

```text
restrict (A ⊗ B) by invariant P
```

**Proposed Sembla composition syntax (not implemented):**

```lean
sembla_component CapacitySafeCare :=
  restrict CareNetwork where
    invariant within_capacity :=
      CareNetwork.hospital.occupied_beds ≤ CareNetwork.hospital.staffed_beds
```

For `staffed_beds = 1`, `(waiting = 2, occupied = 0)` is a legal old state.
Two admissions that each see one free bed are individually plausible but their
joint prospective state `occupied = 2` is illegal. They therefore need a shared
capacity claim or one coordinated commit; two post-commit assertions would be
too late.

**Proposed Sembla composition syntax (not implemented), additive contrast:**

```lean
sembla_component BalancedLedger :=
  restrict (DebitLedger ⊗ CreditLedger) where
    invariant balanced :=
      DebitLedger.exposed_total = CreditLedger.exposed_total
```

Here each child explicitly exposes a typed invariant projection; the constraint
does not inspect arbitrary hidden tables. A constraint such as “every infected
Person row must be in prison” is instead a row-level/relational invariant inside
a population component. It must not be represented as `Health ⊗ Custody` unless
those are genuinely separate machines with an explicit entity-correspondence
relation.

Two kinds of declaration must remain distinct.

### Observational assertion

Reports a violation but does not alter transition selection or state. Useful for
model debugging and scientific checks.

### Semantic invariant

Restricts legal initial states and legal committed transitions. It requires:

1. valid initialization;
2. preservation by every local transition;
3. preservation by every synchronized transition family; and
4. preservation by every set of transitions permitted to commit concurrently.

Individually preserving transitions may jointly violate an invariant, so checking
only each transition in isolation is insufficient when they can commit together.

The semantics must not use rollback, repair, projection to the nearest legal
state, or post-commit filtering unless such behavior is separately named and
modeled. If preservation cannot be proved or validated, either:

- require an atomic coordinated transition/scheduler domain;
- evaluate a deterministic assertion against the prospective pre-commit state
  and, on failure, leave committed state and next-tick mailboxes unchanged while
  returning a model error; or
- reject the composition.

Cross-component invariants cannot inspect hidden child state without an explicit
capability. Constraints involving arbitrary joins or quantification belong to a
separate relational-constraint design.

> [!kimi] kimi: The easy/hard invariant dichotomy is worth naming
> The warning that individually preserving transitions can jointly violate an
> invariant has a useful decidable split. **Conservation/additive**
> invariants (sum of signed effects = 0, e.g. `BalancedLedger`) are preserved
> by any concurrent commit set whenever each transition preserves them,
> because effects commit additively — per-transition checking suffices.
> **Bound/threshold** invariants (`count ≤ k`, `capacity ≥ 0`) are the
> dangerous class: two transitions each legal from the old snapshot can
> jointly exceed the bound, and no per-transition check catches it. Those
> need either a shared claim on the bounded quantity or a prospective-state
> check. Naming the split lets Phase 5 ship the additive class with cheap
> obligations and reserve machinery for the threshold class. Separately: the
> "evaluate an assertion against the prospective pre-commit state" fallback
> presumes a single canonical prospective state. That exists under Level A/B
> ordering but not necessarily under a future Level C/atomics path — specify
> the fallback as evaluating on the canonical (Level A) order, or restrict it
> to levels where the pre-commit state is canonical.

## 9. Nesting, exposure, and flattening

A nested component is an authoring and reasoning boundary around an internal
instance graph.

**Proposed Sembla composition syntax (not implemented):**

```lean
sembla_component RegionalResponse where
  instance epidemic := EpidemicPolicy

  expose epidemic.infection_count as regional_infection_count
  hide epidemic.restriction_modifier
```

A parent may now instantiate `RegionalResponse`, but the inner population and
policy remain the same semantic leaf instances.

### Boundary aliases

Exposing `epidemic.infection_count` as `regional_infection_count` is a typed
alias. It does not create a second mailbox. Otherwise each nesting level would
accidentally add a tick.

### Hidden state

Hiding the direct child boundary port `epidemic.restriction_modifier` removes it
from the new public interface; it does not remove policy state or either
feedback mailbox. Hidden child state and internal mailboxes remain part of
semantic state even when not externally addressable. Hiding affects interface/observation, not execution.

### Flattening requirement

Flattening erases hierarchy while preserving:

- leaf instance identities;
- local and synchronized transition identities;
- scheduler domains;
- state ownership;
- resource identities and claims;
- actual delayed wire mailboxes;
- zero-delay exposure maps;
- observation identities; and
- source/provenance mappings.

For the running example, a source-to-plan map could contain:

| Source declaration | Canonical flat-plan object | Runtime state added? |
|---|---|---|
| `RegionalResponse.epidemic.population` | box `population` with stable instance ID | existing leaf state |
| `population.infection_count -> policy.infection_count` | mailbox `m_population_policy` | yes, one delayed mailbox |
| `expose epidemic.infection_count as regional_infection_count` | boundary/source-map alias | no |
| `hide epidemic.restriction_modifier` | visibility entry | no |

The flattened plan must produce the same committed leaf states, outputs,
mailboxes, draw coordinates, conflicts, and observations as the hierarchical
meaning. Flattening should not rely on concatenating names or on incidental
traversal order.

> [!kimi] kimi: Flattening needs mailbox identities and a total order
> Two strengthenings. (1) Mailboxes are part of semantic state and covered by
> state hashes, so they need the same treatment as rule IDs: derive mailbox
> identity deterministically from a stable wire identity plus both endpoints,
> `(wire-id, source-instance-id, output-port-id, target-instance-id,
> input-port-id)`. Including the target disambiguates fan-out; stable port and
> wire IDs keep renaming or reassociation from changing mailbox identity. (2) "Should not rely on incidental traversal order" is better
> replaced by a constructive rule: the canonical plan sorts instances, wires,
> transitions, and mailboxes by stable identity, and the plan hash covers
> that canonical serialization. Then §15's associativity/symmetry laws become
> **byte-equality of canonical plans**, and §16's "structural and literal
> linked-plan twins" test has a precise meaning instead of a per-case
> equivalence argument.

## 10. Stochastic identity and reproducibility

Current dense declaration-order rule IDs are incompatible with strong product
and nesting laws. Reordering siblings or inserting an independent component can
change Philox coordinates.

A compositional source identity should derive from persisted identifiers such as:

```text
component-definition-id
× instance-id
× local-transition-or-family-id
× event/entity key
× draw-site-id
```

For example, renaming display path `regionA.population.infect` to
`north.population.infect` must leave the semantic tuple unchanged:

```text
<Population-v1, population-instance-7, infect-v1, person-1042, race-draw>
```

Today the corresponding draw uses a dense global `rule_id`; the tuple above is
a proposed identity contract, not current runtime behavior.

Requirements:

- display renaming does not change identity;
- hierarchy reassociation does not change identity;
- exposure/hiding does not change identity;
- flattening does not change identity;
- distinct instances receive distinct instance identities;
- intentional common-random-number sharing uses an explicit correlation key;
- RNG identity is either carried directly in a versioned wider coordinate or
  mapped to the runtime's current `u32` key by a persisted, insertion-invariant,
  collision-checked allocation;
- compact ordinals used only for indexing/reporting are not silently reused as
  RNG identities; and
- legacy positional-ID artifacts retain an explicitly versioned interpretation.

The run manifest should record both the composition-source and linked executable
identity/hashes when a separate source representation exists.

### Hash equivalence

Existing whole-state and IR hashes include names/order and therefore should not
be assumed alpha- or hierarchy-invariant. The design must name which observations
composition laws preserve:

- leaf table contents;
- external output traces;
- views and summaries;
- fired transition reports;
- wire mailbox state;
- draw coordinates;
- structural source hash; and/or
- executable flat-plan hash.

Different claims may use different equivalence quotients; they must not be
collapsed into a vague statement that “the hashes match.”

> [!kimi] kimi: Prefer content-addressed `u32` derivation over a registry
> The "persisted, insertion-invariant, collision-checked allocation" mapping
> stable identities to the runtime's `u32` rule word admits two readings. A
> **persisted next-free registry** is insertion-tolerant but history
> dependent: a second frontend, or a clean rebuild without the registry,
> cannot reproduce the same plan from source alone. A **content-addressed
> derivation** — e.g. `rule_u32 = blake3(identity-tuple)[0..4]`, with a
> documented deterministic probe for collisions and a link-time uniqueness
> check that also excludes the reserved sweep-seed namespaces — is a pure
> function of the identity set. That is what makes composition laws checkable
> at the serialized boundary (the cost listed under Option A in §12) and
> keeps the manifest's identity tuple a function of the model rather than of
> its build history. The Philox counter layout `[tick, rule, entity, draw]`
> is fully allocated at 128 bits, so widening the rule word instead would be
> an RNG format change, not a free alternative.

## 11. Heterogeneous schedulers

The original design allows leaf components with tau-leap, exact CTMC/DES, or ODE
schedulers. Composition needs a common outer synchronization interval.

At each outer boundary:

1. received inputs are fixed for the interval;
2. each scheduler domain advances internally to the next boundary;
3. no intermediate state or output is visible across domains;
4. domains commit at the boundary;
5. boundary outputs become next-interval mailboxes; and
6. observations evaluate afterward.

An exact or ODE domain may sub-step internally. Moving a nesting boundary around
an unchanged domain is structural; splitting or merging scheduler domains is a
semantic/numerical transformation requiring its own equivalence argument.

A synchronized transition family spanning scheduler domains must either execute
as an explicit boundary transaction or be rejected. Mid-step cross-domain
writes would violate the common causality contract.

Scheduler algorithm, parameters, hardware/precision contract, and domain plan
must appear in run provenance.

**Proposed Sembla composition syntax (not implemented):**

```lean
sembla_component CareNetwork where
  instance population := Population (scheduler := tauLeap (dt := 0.25))
  instance hospital := Hospital (scheduler := exactCTMC)
  instance policy := Policy (scheduler := discrete)
  outer_dt := 1.0
  ...
```

**Semantic pseudocode:**

```text
outer interval [0.0, 1.0):
  Population may take tau-leap substeps at 0.25, 0.50, 0.75, 1.00
  Hospital may fire exact internal events at 0.20 and 0.70
  Policy uses the input snapshot fixed at 0.0
boundary 1.0:
  all domains commit and exchange output tables
outer interval [1.0, 2.0):
  receivers first use the tables delivered at boundary 1.0
```

Wrapping an unchanged domain in another composite is structural. Splitting one
domain or merging `Population` and `Hospital` changes numerical semantics.

> [!kimi] kimi: Composed “exact” domains are not the exact joint CTMC
> §11's boundary protocol is the strong-coupling scheme of co-simulation
> (FMI/HLA master algorithms: frozen inputs, advance, exchange at
> communication points) — a lineage worth citing, because its error results
> come with it. Zero-order-hold coupling across a macro-step is first-order:
> even when a domain integrates its own CTMC exactly *within* the interval,
> the wired hybrid converges to the joint CTMC only as the outer interval
> shrinks. Two consequences: the outer interval is a **semantic parameter of
> the composed model** (as `dt` is in DESIGN.md §4.3) and belongs in
> provenance as such; and the "exact path as validator" story (DESIGN.md
> §4.3) needs a caveat — a monolithic exact run validates a wired hybrid only
> up to coupling error, so that check is a convergence check, not a bitwise
> one. The good news the note already implies: one-tick wire delay excludes
> algebraic loops by construction — the co-simulation failure mode that
> usually forces iteration or rollback.

## Related work: Topos Institute and the AlgebraicJulia ecosystem

The recommended source-graph/interpreter split has close maintained precedents.
This note borrows their distinctions without claiming that their software
already implements Sembla's stochastic runtime contract.

- [ACSets](https://doi.org/10.32408/compositionality-4-5) are typed relational
  graph/data structures suitable for definitions, instances, ports, wires,
  boundary maps, and provenance. [ACSets.jl](https://algebraicjulia.github.io/ACSets.jl/stable/)
  and [Catlab](https://algebraicjulia.github.io/Catlab.jl/stable/) provide
  schemas, serialization, homomorphisms, (co)limits, and data migration.
- Catlab demonstrates
  [wiring diagrams as attributed C-sets](https://algebraicjulia.github.io/Catlab.jl/stable/generated/wiring_diagrams/wd_cset/).
  This is a concrete precedent for Option C/D's recursively nestable source
  syntax and separate interpretation.
- [Operadic modeling of dynamical systems](https://arxiv.org/abs/2105.12282)
  and [AlgebraicDynamics.jl](https://algebraicjulia.github.io/AlgebraicDynamics.jl/stable/)
  separate a composition pattern from the machine semantics interpreting it.
- [Structured cospans](https://arxiv.org/abs/2304.00447) formalize open systems
  and boundary gluing. They inform exposure and shared-boundary composition,
  but pushout/variable sharing is not Sembla's directional one-tick channel.
- CatColab's [v0.4](https://topos.institute/blog/2026-01-08-catcolab-0-4-robin/),
  [v0.5](https://topos.institute/blog/2026-03-23-catcolab-0-5-sandpiper/), and
  [v0.6](https://topos.institute/blog/2026-06-01-catcolab-0-6-starling/)
  releases provide practical reusable-model instantiation and composition by
  sharing/identifying variables. That suggests a future explicit `Share` or
  `Identify` constructor, not implicit same-name merging and not ordinary
  delayed `wire`.

A minimal ACSet-like representation of the running composite would look like
this.

**Conceptual ACSet/Catlab-style notation (explanatory only):**

```text
objects:
  Definition, DefinitionPort, Instance, InstancePort, Wire, Exposure

foreign keys:
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

attributes:
  DefinitionPort.direction : {input, output}
  DefinitionPort.schema_id : SchemaId
  Wire.delay               : Nat
```

**Concrete records for `EpidemicPolicy`:**

```text
DefinitionPort(Population.infection_count, output, InfectionCountSchema)
DefinitionPort(Population.restriction_modifier, input, RestrictionSchema)
DefinitionPort(Policy.infection_count, input, InfectionCountSchema)
DefinitionPort(Policy.restriction_modifier, output, RestrictionSchema)
DefinitionPort(EpidemicPolicy.infection_count, output, InfectionCountSchema)
DefinitionPort(EpidemicPolicy.restriction_modifier, output, RestrictionSchema)
Instance(population, parent=EpidemicPolicy, definition=Population)
Instance(policy, parent=EpidemicPolicy, definition=Policy)
InstancePort(population.infection_count, population, Population.infection_count)
InstancePort(policy.infection_count, policy, Policy.infection_count)
InstancePort(policy.restriction_modifier, policy, Policy.restriction_modifier)
InstancePort(population.restriction_modifier, population, Population.restriction_modifier)
Wire(EpidemicPolicy, population.infection_count, policy.infection_count, delay=1)
Wire(EpidemicPolicy, policy.restriction_modifier, population.restriction_modifier, delay=1)
Exposure(EpidemicPolicy, population.infection_count, EpidemicPolicy.infection_count)
Exposure(EpidemicPolicy, policy.restriction_modifier, EpidemicPolicy.restriction_modifier)
```

This representation can validate ownership, typing, and connectivity. It does
not determine tau-leap scheduling, mailbox execution, stable Philox coordinates,
atomic families, or CUDA lowering; those remain obligations of the Sembla
linker and executable semantics.

As of 2026-07-21 the relevant software remains active: CatColab v0.6.1,
Catlab v0.17.6, and ACSets.jl v0.2.29 are the latest checked releases. Direct
Julia runtime adoption is not implied; an ACSet-compatible schema,
interchange artifact, or development-time oracle are separate choices.

## 12. Implementation approaches

The same author-level `EpidemicPolicy` example makes the representation choice
concrete:

| Option | Serialized/executable shape for the example | Where hierarchy survives |
|---|---|---|
| A | proposed Lean elaborates immediately to current `Model { boxes, wires }` | only a source/side provenance map |
| B | recursive `CompositeBox(EpidemicPolicy, children=[population, policy])` executes directly | runtime IR |
| C | algebraic expression such as `DelayedFeedback(Wire(Tensor(...)))` | primary AST interpreted multiple ways |
| D | typed source records link deterministically to current-shaped flat `Model` plus embedded identity/source map | source artifact and provenance, not execution recursion |

**Semantic pseudocode for the common author input:**

```text
EpidemicPolicy
  = wire(Population.infection_count, Policy.infection_count)
  + wire(Policy.restriction_modifier, Population.restriction_modifier)
```

### Option A — Surface-only composition lowered directly to flat IR

Lean syntax defines reusable components, product, instances, wiring, and
exposure, then immediately emits the existing flat `Model`.

**Advantages**

- smallest implementation and migration surface;
- existing Rust validator/runtime/CUDA can remain unchanged initially;
- old JSON and direct flat models remain valid; and
- fast path to authoring reuse and product laws.

**Costs and risks**

- hierarchy/provenance disappears unless carried in a side map;
- other frontends cannot share the composition language;
- diagnostics/widgets must reconstruct source structure;
- stable stochastic identity still requires an executable-ID migration; and
- composition laws may exist only in Lean rather than at the serialized boundary.

**Best use:** a narrow first experiment, not the final multi-frontend contract.

### Option B — Recursive hierarchical executable IR

Make `Box` recursively contain component instances/composites and teach every
runtime layer to execute hierarchy.

**Advantages**

- hierarchy survives serialization and runtime inspection;
- natural place for nested schedulers, interfaces, and composite widgets; and
- no separate flattening artifact is required.

**Costs and risks**

- changes Lean IR, JSON, Rust validation, CPU runtime, CUDA, initialization,
  state addressing/hashing, manifests, CLI reporting, and every fixture;
- recursive scheduling and identity become runtime concerns;
- equivalent hierarchies can execute differently unless normalized; and
- hierarchy and flat execution may become competing semantics.

**Assessment:** highest risk; choose only if runtime-visible hierarchy is a
confirmed requirement.

### Option C — Free composition AST as the primary IR

Represent primitive components plus algebraic constructors such as:

**Proposed composition-source AST (illustrative):**

```text
Primitive
Tensor
Wire
DelayedFeedback
Rename
Hide
Restrict
Instantiate
```

and define semantics by interpretation.

**Advantages**

- clean mathematical home for composition laws and Lean proofs;
- preserves author intent and reusable structure;
- supports multiple interpreters (flat runtime plan, widgets, documentation);
  and
- separates composition from primitive execution.

**Costs and risks**

- equivalent expressions require canonical normalization or explicit
  isomorphisms;
- unrestricted categorical generality may outrun concrete modeling demand;
- diagnostics and constrained products remain substantial work; and
- Rust or another frontend needs access to the representation if it is the
  public contract.

**Best use:** formal source representation, provided its constructor set remains
small and demand-driven.

### Option D — Hybrid composition source IR plus canonical flat executable IR

Use a typed, frontend-agnostic composition-source graph/AST, validated and linked
once into a canonical flat executable plan. Initially, the existing flat IR can
serve as that plan after stable IDs/provenance are added.

**Advantages**

- reusable/nested composition and formal laws at the source layer;
- small execution core and minimal CPU/CUDA disruption;
- deterministic normalization gives one runtime semantics;
- hierarchy-aware diagnostics/widgets remain possible through source maps; and
- direct flat IR remains available to machine writers.

**Costs and risks**

- requires a versioned linker and two related artifact hashes;
- source-to-plan provenance must be first-class;
- users must understand source identity versus executable identity; and
- linker preservation becomes a proof/test obligation.

**Assessment:** recommended long-term direction.

> [!kimi] kimi: Put source provenance *inside* the plan artifact
> Option D's "two related artifact hashes" and Option A's "hierarchy
> disappears unless carried in a side map" share a failure mode: sidecar
> provenance gets separated from its artifact when results are copied,
> archived, or diffed. Recommend the canonical plan **embed** the
> composition-source hash and the identity map inline (a versioned,
> append-only field per §5.4's rules), so the manifest continues to record
> exactly one executable hash and the source↔plan link survives artifact
> movement. This also de-risks the Lean-only first step (open decision 1):
> if Phase 1 ships Lean surface → flat plan, emitting the *plan with embedded
> identity map* from day one establishes the artifact contract before the
> frontend-agnostic source graph exists, so interim user models never become
> a backwards-compatibility constraint on the source IR.

### Option E — Textual cloning or table merging

Treat product as copying declarations together or merging boxes/tables.

**Advantages:** superficially cheap.

**Failure modes:** duplicate names, unstable rule IDs, altered Ref scope, changed
conflict domains, interface loss, state-hash drift, and accidental scheduler
merging.

**Assessment:** reject as a semantic composition mechanism. Any merging should
be an explicit compiler transformation with a proved/tested equivalence contract.

## 13. Recommended hybrid pipeline

```text
component definitions
        ↓ instantiate / tensor / wire / hide / rename / restrict
composition source graph
        ↓ resolve types, identities, parameters, interfaces, constraints
validated composition graph
        ↓ deterministic versioned linker
canonical executable plan
        ↓ existing validator + CPU/CUDA execution
results + source/identity provenance
```

For `EpidemicPolicy`, the source has two leaf-definition records, one composite
definition, two instance records, two wire records, and two exposure records. Linking produces the same
current-shaped `population` and `policy` boxes plus exactly two mailboxes. The
plan embeds mappings such as:

```text
source instance population -> flat box id 17
source transition population.infect -> flat rule id 0x8bd13f2a
source wire population.infection_count -> policy.infection_count -> mailbox id 41
source hash -> 6f...c2
```

The identifiers are illustrative; their deterministic derivation remains an
open decision. The important invariant is that regrouping or renaming the
source does not silently allocate different semantic objects.

Only the canonical plan executes. The source graph is not a second mutable copy
of runtime state.

The linker must reject:

- duplicate definition or instance identities;
- recursive definition cycles;
- unresolved/hidden required inputs;
- schema-incompatible wires;
- multiple drivers without an explicit merge component;
- unstable or colliding stochastic IDs;
- invalid scheduler-spanning synchronized families;
- unsatisfied invariant obligations; and
- ambiguous parameter sharing.

## 14. Illustrative surface directions

Every block in this section is **proposed Sembla composition syntax and is not
implemented**. Leaf bodies deliberately resemble the current DSL so the design
can reuse existing model declarations rather than inventing a second transition
language.

### Reusable components and product

**Proposed Sembla composition syntax (not implemented):**

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

sembla_component IndependentEpidemicPolicy := Population ⊗ Policy
```

`IndependentEpidemicPolicy` owns both states but has four unconnected qualified
ports. It behaves as two side-by-side machines.

### Instances and delayed wiring

**Proposed Sembla composition syntax (not implemented):**

```lean
sembla_component EpidemicPolicy where
  instance population := Population
  instance policy := Policy

  wire population.infection_count -> policy.infection_count
  wire policy.restriction_modifier -> population.restriction_modifier

  expose population.infection_count as infection_count
  expose policy.restriction_modifier as restriction_modifier
```

This is the proposed reusable form of the current flat two-box model. It has two
mailboxes and a two-tick round trip; the exposure is a zero-delay alias.

### Multiple instances remain independent

**Proposed Sembla composition syntax (not implemented):**

```lean
sembla_component TwoRegions where
  instance north := EpidemicPolicy
  instance south := EpidemicPolicy

  expose north.infection_count as north_infection_count
  expose south.infection_count as south_infection_count
```

`north.population` and `south.population` have distinct state and stochastic
instance identities even though both instantiate the same definitions.

### Nesting and hiding

**Proposed Sembla composition syntax (not implemented):**

```lean
sembla_component PublicPolicyModel where
  instance internal := EpidemicPolicy
  expose internal.infection_count as infection_count
  hide internal.restriction_modifier
```

Exposure and hiding add no delay. Only `wire` allocates a delayed mailbox, and
hiding does not remove internal state.

### Constrained care network

**Proposed Sembla composition syntax (not implemented):**

```lean
sembla_component CapacitySafeCare :=
  restrict CareNetwork where
    invariant within_capacity :=
      CareNetwork.hospital.occupied_beds ≤ CareNetwork.hospital.staffed_beds
```

### Candidate synchronized family

**Proposed Sembla composition syntax (not implemented):**

```lean
family admit_patient in CareNetwork where
  match request in admission_requests by request_id
  match person by request.person_id
  match bed by request.bed_id
  guard person.care = waiting ∧ bed.occupied = false ∧
    bed.hospital_id = person.hospital_id
  hazard admission_rate
  leg population.mark_admitted
  leg hospital.occupy_bed
  claims person, bed
```

The request row selects one person and one bed without a Cartesian search.
This family syntax remains illustrative until row correlation, event identity,
clock ownership, ACSet-rewrite correspondence, and scheduler coordination are
selected. It is intentionally different from a delayed wire.

## 15. Algebraic laws and acceptance obligations

The selected implementation should make the following laws precise.

### Product laws

- **Identity:** `A ⊗ I ≅ A`.
- **Associativity:** `(A ⊗ B) ⊗ C ≅ A ⊗ (B ⊗ C)`.
- **Symmetry:** `A ⊗ B ≅ B ⊗ A`, only up to explicit stable-identity/interface
  isomorphism unless serialization/report order is quotiented.
- **Projection/noninterference:** each projection of an unwired disjoint product
  matches the corresponding component run.

### Wiring laws

- every real wire contributes exactly one tick of delay;
- exposure and renaming contribute zero delay;
- feedback has no within-tick cascade;
- fan-in occurs only through explicit merge components; and
- nesting/flattening preserves mailbox state and external traces.

### Transition laws

- lifted singleton transitions equal their primitive behavior;
- disjoint accepted effects commute;
- all effects read the old snapshot;
- synchronized families commit atomically;
- losing one required claim rejects all family legs;
- conflict outcomes are iteration-order independent; and
- stable draw coordinates survive permitted renaming, regrouping, and flattening.

### Constraint laws

- initialization satisfies every semantic invariant;
- every local and synchronized family preserves invariants;
- every permitted concurrent commit set preserves invariants; and
- an invariant failure is a deterministic model error, never silent repair.

### Scheduler laws

- structural nesting around unchanged leaf domains preserves behavior;
- changing scheduler algorithms/domains is an explicit semantic change;
- no scheduler exposes internal substeps across a boundary; and
- CPU/CUDA implementations agree for the selected determinism contract.

The laws should be executable against concrete terms, not only stated with
metavariables:

```text
link((Population ⊗ Policy) ⊗ Hospital)
  == link(Population ⊗ (Policy ⊗ Hospital))

project_population(run(Population ⊗ Policy))
  == run(Population)                         -- before feedback wires are added

run(flatten(RegionalResponse))
  == run(RegionalResponse)                   -- states, mailboxes, draws, outputs

cut(population.infection_count -> policy.infection_count, recorded_trace)
  == run(policy with that input trace)
```

Equality here means the specific quotient named for each law; the strongest
forms require byte-identical canonical plans or draw traces.

> [!kimi] kimi: Two candidate laws worth adding to the list
> **Canonical-report quotient (strengthens symmetry).** Choose the cheap
> reading of open decision 6 now: serialize and report instances, ports,
> transitions, and wires in stable-identity order everywhere. Then symmetry
> is a user-visible byte-level law, twin-artifact tests need no isomorphism
> argument, and "up to isomorphism" never enters the acceptance vocabulary.
> **Wire-cut/trace-replay law.** For any wire, replacing the driver with an
> input preloaded with the recorded delivery trace reproduces the receiver's
> run exactly, draw coordinates included. This follows from the one-tick
> mailbox contract, but stating it as a law makes it a tool: every component
> becomes independently testable against golden wire traces — the natural
> unit-test granularity for composition, and a way to reproduce failures from
> manifests without rerunning whole models.

## 16. Test strategy

Future implementation should add tests at several layers.

### Lean/frontend

- reusable definition and multiple-instance elaboration;
- product identity/associativity after canonical linking;
- qualified diagnostics and widget anchors;
- exposure/hiding/renaming source maps;
- stable instance and transition identities;
- constrained-product positive and negative cases; and
- structural and literal linked-plan twins.

### IR validation

- duplicate/colliding identities;
- recursive definitions;
- parameter-binding ambiguity;
- schema mismatches and hidden required inputs;
- fan-in without merge;
- invalid synchronized-family ownership/claims;
- scheduler-domain violations; and
- invariant declaration/preservation failures.

### Runtime

- independent product projection;
- one-tick pulse/feedback behavior;
- nesting versus flattening equivalence;
- sibling insertion/reordering preserving draw traces;
- synchronized-family atomicity;
- cross-component claim conflicts and deterministic ties;
- simultaneous-effect invariant closure; and
- external outputs/views/summaries across hiding/renaming.

### CPU/CUDA differential

- product components firing concurrently;
- delayed cycles;
- cross-component claims;
- synchronized multi-leg effects;
- stable stochastic coordinates; and
- linked plans produced from differently associated but equivalent source graphs.

A compact end-to-end fixture set makes those obligations concrete:

| Fixture | Required observation |
|---|---|
| `IndependentEpidemicPolicy` | projecting population reproduces the standalone population run |
| `EpidemicPolicy` | count reaches policy after one tick; restriction returns after the second |
| `RegionalResponse` and its linked flat plan | identical mailbox trace, state, reports, and draw coordinates |
| `TwoRegions` with instances swapped/renamed | canonical linked plan and draw trace unchanged under the selected quotient |
| `CapacitySafeCare` with one bed and two candidates | no commit can produce `occupied_beds = 2` |
| `admit_patient` | patient and bed legs either both commit or neither commits |
| heterogeneous `CareNetwork` | no domain observes another domain's internal substeps |

Tests should compare draw coordinates, mailboxes, committed leaf state, output
traces, and observations—not only selected final hashes.

> [!kimi] kimi: One generator, two oracles, plus the sharpest identity test
> The laws of §15 are all property-testable against the CPU oracle before any
> Lean proof exists: generate small random compositions (instances, wirings,
> nestings, reassociations, renamings), link, run, and check projection,
> flattening, and delay laws on committed states **and** draw coordinates.
> Feed the same generated corpus through the existing CPU/CUDA differential
> harness (`docs/performance/cuda-differential-harness.md`) so composition equivalence
> and backend equivalence share one generator. Include the adversarial
> identity case as a first-class property: alpha-rename every instance/port,
> reassociate all nesting, flatten, and require **byte-identical draw
> traces** — the cheapest test that would have caught the current
> dense-rule-ID defect, and worth running from Phase 1 onward.

## 17. Suggested implementation sequence

### Phase 0 — semantic prerequisites

1. Define composition observational equivalence.
2. Define stable stochastic/instance/transition identities.
3. Scope refactoring invariance to scheduler-preserving transformations.
4. Define source-plan versioning and provenance.

**Visible example:** the tuple
`<Population-v1, population-instance-7, infect-v1, person-1042, race-draw>`
has a specified serialized meaning before any component syntax ships.

### Phase 1 — reusable flat components and product

1. Add reusable component definitions and instances.
2. Add tensor/product with disjoint tagged interfaces.
3. Link to the current flat IR.
4. Preserve direct flat models and old artifact interpretation.
5. Add identity, associativity, and noninterference tests.

**Visible example:** `IndependentEpidemicPolicy := Population ⊗ Policy` links
to two unwired current-shaped boxes.

### Phase 2 — nesting and interface control

1. Add composite definitions.
2. Add expose, hide, and rename.
3. Preserve actual wire mailboxes without boundary-added delay.
4. Add hierarchy-aware source maps, diagnostics, and widgets.

**Visible example:** `RegionalResponse` exposes `regional_infection_count`,
hides its direct child's `restriction_modifier` boundary port, and still links
to exactly the two original feedback mailboxes.

### Phase 3 — generalized wiring adapters

1. Keep exact schemas and single drivers as the default.
2. Add explicit rename/project/reorder adapters only when demanded.
3. Add explicit deterministic merge/reducer components for fan-in.

**Visible example:** `ProjectInfectionCount` converts `daily_report` into the
exact `{ infected : Int }` input, while `SumInfectionCounts` performs explicit
regional fan-in.

### Phase 4 — synchronized transition families and global claims

1. Introduce stable global resource identities/capabilities.
2. Add atomic cross-component family semantics.
3. Extend CPU/CUDA conflict resolution and differential tests.

**Visible example:** `admit_patient` atomically changes one person and one bed,
with one event identity and one combined claim set.

### Phase 5 — constrained products

1. Add observational assertions.
2. Add semantic invariants and initialization checks.
3. Add transition/concurrency preservation obligations.
4. Add coordinating domains for constraints that cannot be preserved locally.

**Visible example:** `CapacitySafeCare` rejects initialization or a concurrent
commit that would make `occupied_beds > staffed_beds`.

### Phase 6 — heterogeneous schedulers

1. Add leaf scheduler declarations and common outer boundaries.
2. Define stable internal event/draw coordinates.
3. Gate scheduler-spanning families.
4. Record the complete plan in manifests.

**Visible example:** `CareNetwork` advances tau-leap population and exact
hospital domains independently, exchanging only at `outer_dt` boundaries.

A recursive executable IR should be reconsidered only if runtime-visible
hierarchy remains necessary after the hybrid linker and provenance maps exist.

> [!kimi] kimi: Two sequencing adjustments
> (1) Phase 1's "link to the current flat IR" is not cost-free for the IR:
> the plan must carry the identity map, so a **versioned IR bump with an
> explicit legacy interpretation for positional rule IDs** (per §10) is a
> Phase 1 deliverable and deserves a DECISIONS.md entry when scheduled —
> otherwise "current flat IR" reads as "no IR change," which contradicts
> §10. (2) Consider pulling **observational assertions** (Phase 5, step 1)
> forward to Phase 2–3. They need only exposure plus expression evaluation
> over committed state, carry no preservation machinery, and pay off
> immediately as model-debugging surface while semantic invariants are still
> being designed. The note already separates the two kinds (§8); the sequence
> can exploit the same split.

## 18. Open decisions

Before implementation, resolve:

1. Is the composition-source graph serialized and frontend-agnostic from day
   one, or initially Lean-only?
2. Which observations define refactoring equivalence: external traces only,
   leaf states, draw coordinates, fired reports, or selected hashes?
3. How are stable IDs persisted and migrated from legacy dense `u32` rule IDs?
4. Can product instances share parameters by explicit binding, or only receive
   copied values?
5. Are component definitions allowed to declare `dt`, or only scheduler
   requirements under a model-level outer `dt`?
6. Is product symmetry a user-visible law or only an isomorphism that may reorder
   reports/serialization?
7. Are synchronized cross-component families required for the first composition
   release?
8. Which invariants are statically provable, runtime-asserted, or rejected?
9. Must hierarchy survive into runtime/manifest tooling, or is source-map
   provenance sufficient?
10. How should current state hashes be compared across alpha-renaming and
    flattening?
11. Is the source graph itself an ACSet, losslessly mapped to a versioned ACSet
    schema, or only conceptually ACSet-like?
12. Is AlgebraicJulia used only as prior art, as an interchange/development-time
    oracle, or as a required tool in any build path?
13. Does variable sharing/identification become an explicit future constructor,
    and how is it kept distinct from delayed `wire` and synchronized `family`?

> [!kimi] kimi: Two more open decisions to record
> 14. **Where does the denotational semantics live after composition?**
>     DESIGN.md §4.5 makes the deep-embedded IR the semantic ground truth.
>     Once a composition-source layer exists, ground truth must either move
>     to the source graph (with the linker proved/tested as meaning-
>     preserving) or remain on the canonical plan (with source-level laws
>     stated modulo linking). This is the theorem-shaped version of open
>     decision 1 and should be decided alongside it.
> 15. **Entity-ID namespacing across boundaries.** Birth/death allocates
>     entity IDs as a function of `(tick, parent, slot)` (DESIGN.md §4.2) —
>     unique within an owner, not across instances. §3 defers row-level
>     cross-boundary messaging, but when it lands, delivered rows carry
>     entity-keyed columns the receiver joins against its own state; without
>     owner-namespaced entity IDs this is exactly the silent same-name merge
>     §5 forbids for ports. Reserve the namespacing rule now so the wire
>     schema contract does not have to change when table transport
>     generalizes.

## 19. Recommendation

Adopt the hybrid direction, but implement it incrementally:

- preserve the current flat runtime as the initial executable core;
- make machine product—not table Cartesian product—the fundamental operation;
- add reusable typed component definitions and stable instances first;
- define local transition lifting and one-tick wiring before adding
  cross-component atomic families;
- make exposure/hiding structural and delay-free;
- replace declaration-order RNG identity before claiming strong product laws;
- treat scheduler domains as semantic boundaries;
- express inadmissible combinations as constrained products/invariants, not by
  enumerating giant product enums; and
- require every hierarchy/composition feature to pass flattening, causality,
  identity, and CPU/CUDA differential tests; and
- keep `EpidemicPolicy`, `RegionalResponse`, and `CapacitySafeCare` as linked
  documentation/test fixtures so every new abstraction has a visible source
  term and an observable execution consequence.

This sequence restores the originally proposed compositional architecture
without prematurely forcing recursion into every runtime layer or weakening the
existing deterministic execution contract.
