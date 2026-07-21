# Revision plan: examples throughout `composition-options.md`

## Outcome

Revise `docs/design/composition-options.md` so that a reader encounters one concrete composed model repeatedly while learning each semantic point, rather than waiting until §14 for sketches. Preserve the document’s options/discussion status: only the existing flat `sembla_model` form and flat JSON IR may be described as current; every component/product/nesting/family/constraint/scheduler/source-graph form must be visibly marked proposed or conceptual.

This is a documentation-only plan. No project/source file was modified.

## Repository-grounded syntax and constraints

### What can be shown as current

The implemented human surface is the indentation-based `sembla_model` command (`frontend/README.md`, “Public command surface”; `docs/design/surface-syntax-options.md` §§1–3). Its relevant forms are:

```lean
-- Current/implemented Sembla syntax (flat model)
sembla_model sirPolicy (dt := 0.25) where
  box population where
    input restriction_modifier where
      modifier_offset : ℝ
    output infection_count from Person where
      infected : Int := count where health = I
  box policy where
    input infection_count where
      infected : Int
    output restriction_modifier from Controller where
      modifier_offset : ℝ := sum (modifier - 1.0)
  wire population infection_count -> policy infection_count
  wire policy restriction_modifier -> population restriction_modifier
```

This is grounded in `frontend/Sembla/Models.lean` (`sirPolicy`) and `frontend/README.md`. Current wire syntax is four identifiers plus ASCII `->`; schemas are ordered and exact; destination inputs have at most one driver. Current boxes are declarations inside one model, not reusable definitions or instances.

The current Lean and Rust IRs are flat: `Model` contains `boxes`, `wires`, and `summaries`; `Box` contains tables/transitions/inputs/outputs/views and no children (`frontend/Sembla/IR.lean`, `crates/sembla-ir/src/model.rs`). `examples/two_box.json` is a concrete current executable feedback graph. Runtime tests in `crates/sembla-runtime/tests/composition.rs` establish empty tick-zero mailboxes, exactly one-tick delivery, deterministic feedback, and the carefully scoped merged/composed boundary test.

### What must not be presented as current

The repository has no implemented syntax or IR node for `sembla_component`, `instance`, `⊗`, `expose`, `hide`, `rename`, `restrict`, synchronized `family`, scheduler declarations, a composition-source graph, or Options A–D lowering. These are all design proposals in this note.

### ACSet/Catlab precision

`DESIGN.md` §4.1 and `DECISIONS.md` B1 commit to ACSet state for a leaf box, while `DESIGN.md` §4.4 commits to operad-style macro composition. The note currently barely connects those levels. Examples must explicitly avoid three common category errors:

1. `State(A ⊗ B) = State(A) × State(B)` is a **machine-state pair of independently owned ACSet instances**, not a relational Cartesian product of rows and not automatically the pointwise categorical product of ACSets.
2. Current exact positional schema equality is a Sembla linker/runtime restriction, not a claim that ACSet schema morphisms or Catlab data migration are implemented.
3. A Catlab-like source-graph schema is an explanatory representation candidate, not current Sembla syntax and not a commitment to a particular Catlab API.

## Running example family

Use a small, named family rather than unrelated ledgers, prisons, and policy snippets.

### Primary example: `EpidemicPolicy`

Reuse the repository’s canonical `sirPolicy` shape:

- `Population`: SIR rows; input `restriction_modifier`; output `infection_count`; tau-leap leaf.
- `Policy`: one controller row; input `infection_count`; output `restriction_modifier`; tau-leap/simple controller leaf.
- two delayed feedback wires.
- composite boundary exposes `population_state` and optionally `infection_count`, while hiding `policy_debug`.

This anchors product, multiple instances, wiring/delay, feedback, nesting, exposure/hiding, identity, lowering, source graph, laws, tests, and Phases 0–3.

### Extension: `CareNetwork`

Use one consistent extension for features the canonical two-box example cannot express:

- `Population` (tau-leap GPU), `Hospital` (exact CTMC/DES CPU), and `Policy` (ODE or discrete controller CPU) are scheduler domains coupled only at outer boundaries.
- `admit_patient` is a candidate synchronized family joining a person and bed by declared `hospital_id`; it changes patient status and bed occupancy atomically.
- `CapacitySafe := restrict (Population ⊗ Hospital)` with `occupied_beds ≤ staffed_beds`, using explicit invariant projections rather than hidden-state inspection.

This keeps synchronized families, constraints, and heterogeneous scheduling in one epidemiological/care-delivery domain. Do not retain `BalancedLedger` or `release_person` as the principal examples; they may remain one-line secondary analogies if useful.

## Mandatory notation-labeling policy

Add the following short legend near the end of §1 (after current line 51, before the `kimi` callout), and use the exact lead-in above every code block thereafter:

| Label | Fence | Meaning |
|---|---|---|
| **Current/implemented Sembla syntax** | `lean` | Accepted by today’s `sembla_model`; keep snippets compilable in shape and consistent with `frontend/README.md`. |
| **Current executable IR (abridged)** | `json` | Accepted flat JSON shape; ellipses mean explanatory excerpt, not a runnable fixture. |
| **Proposed Sembla composition syntax (illustrative, not implemented)** | `lean` | Candidate surface only. Never say “write” or “use”; say “a possible surface would be.” |
| **Proposed composition-source IR (illustrative)** | `text` or `json` | Candidate typed source graph/AST, not the current `Model`. |
| **Conceptual ACSet/Catlab-style notation (explanatory only)** | `julia` or `text` | Mathematical correspondence/pseudo-Catlab; neither runnable Sembla nor a frozen Catlab API. |
| **Semantic pseudocode** | `text` | Operational timeline, lowering relation, or test oracle; no parser claim. |

Avoid bare `lean` blocks whose status is only explained several paragraphs earlier. In §14, repeat the labels even though the section already says syntax is not frozen. Use `⊗` only in proposed/semantic blocks; current syntax has only side-by-side `box` declarations.

## Exact insertion and replacement plan

Line numbers refer to the current 1,242-line document and will drift after edits; headings are authoritative.

### 1. §1 Executive summary — insert after current line 51

Insert the notation legend above, followed by a compact “running examples” diagram:

```text
Semantic overview (not surface syntax)
EpidemicPolicy = Population ⊗ Policy
Population.infection_count -[1 tick]-> Policy.infection_count
Policy.restriction_modifier -[1 tick]-> Population.restriction_modifier

CareNetwork = EpidemicPolicy ⊗ Hospital
```

**Teaches:** what will recur, that tensor alone does not wire, and that arrows denote delayed channels. This prevents later snippets from feeling unrelated.

### 2. §2.1 Machine/box product — insert after current line 86

Add a proposed product beside its current flat analogue:

```lean
-- Proposed Sembla composition syntax (illustrative, not implemented)
component Independent := Population ⊗ Policy
```

```lean
-- Current/implemented Sembla syntax (flat analogue, not reusable product)
sembla_model independent (dt := 0.25) where
  box population where
    -- current Population declarations
  box policy where
    -- current Policy declarations
  -- deliberately no wires
```

State explicitly that the first has tagged interfaces such as `population.infection_count` and `policy.infection_count`; the second merely demonstrates today’s executable unwired behavior.

**Teaches:** machine product versus flat juxtaposition; product creates neither wires nor same-name merging.

### 3. §§2.2–2.3 — insert after current lines 98 and 105

Use two tiny counterexamples from the same domain:

```lean
-- Current/implemented Sembla syntax: one ACSet table with factored attributes
system Person (rows := 1_000_000) where
  health : {S, I, R}
  care_status : {Home, Admitted}
```

Then semantic pseudocode:

```text
not product: Person.health × Person.care_status is row-state factoring
not product: every Person row × every Hospital row is a relational join/cartesian product
machine product: Population ⊗ Hospital keeps independently owned machine state
```

**Teaches:** all three overloaded products using one family.

### 4. §3 Current implementation baseline — insert after current line 145 and after line 180

First show a truthful abridged current IR:

```json
// Current executable IR (abridged)
{
  "name": "sir_workplace_policy_feedback",
  "dt": 0.25,
  "boxes": [{"name":"population"}, {"name":"policy"}],
  "wires": [
    {"from":{"box":"population","port":"infection_count"},
     "to":{"box":"policy","port":"infection_count"}},
    {"from":{"box":"policy","port":"restriction_modifier"},
     "to":{"box":"population","port":"restriction_modifier"}}
  ]
}
```

JSON comments make it non-runnable; either use `jsonc` or remove the comment and put the label in prose. Link to `examples/sir_policy.json` and `examples/two_box.json` rather than duplicating full fixtures.

After the execution contract, add a two-tick table for one pulse:

```text
Semantic trace of the current runtime
boundary      Population reads       Policy reads       delivery produced
n             mailbox from n-1       mailbox from n-1   outputs for n+1
n+1           policy output from n   population count n outputs for n+2
```

**Teaches:** current syntax/IR, shared old snapshot, and the precise baseline all proposals must lower to.

### 5. §4 Semantic vocabulary — distribute examples under the terms, not in one block

Insert one-line proposed declarations under these headings:

- **Component definition** (after line 216): `component Population ...` and explain definition identity.
- **Instance** (after line 231):
  ```lean
  -- Proposed Sembla composition syntax (illustrative, not implemented)
  instance north := Population (β := β_north)
  instance south := Population (β := β_south)
  ```
  Explain independent state and distinct stable instance IDs.
- **Composite component** (after line 241): `component EpidemicPolicy where instance ...`.
- **Wire/Exposure/Hiding** (after lines 251/256/262): one line each from the same composite; annotate mailbox allocation `1/0/0`.
- **Scheduler domain** (after line 272): `scheduler population := tauLeap gpu; scheduler hospital := exactCtmc cpu` as proposed syntax.
- **Constrained product** (after line 277): `restrict (Population ⊗ Hospital) where invariant capacity ...`.

**Teaches:** vocabulary by direct recognition and satisfies the request for instances before the semantics section.

### 6. §5 Core product semantics — insert after each mathematical subsection

Under **State** (after current line 290), add:

```text
Semantic example
State(Population ⊗ Policy)
  = (population_acset, policy_acset)
  + no cross-child mailbox until a wire is added
```

Immediately follow with:

```julia
# Conceptual ACSet/Catlab-style notation (explanatory only)
X_population :: ACSet(SchPopulation)
X_policy     :: ACSet(SchPolicy)
machine_state = (X_population, X_policy)  # owned pair; not a row Cartesian product
```

Under **Interfaces**, list the four tagged ports before wiring. Under **Transitions**, show `population.infect` lifting from `(p,q)` to `(p',q)` and `policy.restrict` lifting to `(p,q')`. Under **Product noninterference law**, instantiate the equation:

```text
project_population(run(Population ⊗ Policy, seed))
  == run(Population, seed)  # only after stable identity migration; same inputs/domain
```

Explicitly say current dense IDs prevent claiming draw-coordinate equality today.

**Teaches:** ACSet-safe state interpretation, tagged interfaces, lifted transitions, and the law’s current limitation.

### 7. §6 Wiring and feedback semantics — insert after current line 365 and under Cycles

Use the actual two wires in proposed qualified syntax:

```lean
-- Proposed Sembla composition syntax (illustrative, not implemented)
wire population.infection_count -> policy.infection_count
wire policy.restriction_modifier -> population.restriction_modifier
```

Then give a concrete tick trace with named values (e.g. count `620` emitted at tick 8, read by Policy at tick 9; restriction `-0.6` emitted at tick 9, read by Population at tick 10). State that the round trip is two wire delays, not one.

Under **Fan-in and fan-out**, show rejected implicit fan-in and accepted explicit merge:

```lean
-- Proposed syntax; first form must be rejected
wire north.infection_count -> policy.infection_count
wire south.infection_count -> policy.infection_count
instance totals := SumCounts
wire north.infection_count -> totals.left
wire south.infection_count -> totals.right
wire totals.total -> policy.infection_count
```

Under **Schema compatibility**, juxtapose current exact fields and a future explicit adapter, marking the adapter proposed.

**Teaches:** wiring, one-tick delay, feedback, two-tick loop latency, single-driver rule, explicit merge, and exact schemas.

### 8. §7.1–7.2 Transitions — insert after current lines 425 and 431

Show one current leaf transition copied exactly in style from `sirPolicy`:

```lean
-- Current/implemented Sembla syntax inside a flat box
infect on Person : health: S →[
  β · freq (health = I) over employer ·
    (1.0 + inputSum restriction_modifier field modifier_offset)
] I
```

Annotate: the local rule is current; qualifying it as `population.infect` and carrying stable definition/instance/local IDs are proposed linker behavior. Show `inputSum` reading the previous delivery, never same-tick Policy state.

**Teaches:** current expression syntax embedded in future reusable leaves and wired influence without inventing a joint transition.

### 9. §7.3 Synchronized families — replace current lines 441–449 example

Replace the unrelated prison example with the `CareNetwork` extension:

```lean
-- Proposed Sembla composition syntax (illustrative, deferred and not implemented)
family admit_patient in CareNetwork where
  match person by hospital_id
  match bed by hospital_id
  guard person.care_status = Waiting ∧ bed.occupied = false
  hazard admission_rate
  leg population.mark_admitted
  leg hospital.occupy_bed
  claims person, bed
```

Add a three-row event explanation: one declared-key match, one family clock/RNG event, all claims win then both legs commit; otherwise neither commits. Add conceptual correspondence:

```text
Conceptual ACSet rewriting correspondence (not implemented Catlab syntax)
match L -> X is enumerated by the declared hospital_id join;
Sembla does not thereby adopt unrestricted DPO/SqPO graph matching.
```

**Teaches:** synchronized families, deterministic keyed matching, one clock, atomicity, ownership, and the boundary with conceptual ACSet rewriting.

### 10. §8 Constrained products — replace current lines 534–541 example and add concurrency trace

Use:

```lean
-- Proposed Sembla composition syntax (illustrative, not implemented)
component CapacitySafe :=
  restrict (Population ⊗ Hospital) where
    invariant capacity :=
      Hospital.occupied_beds ≤ Hospital.staffed_beds
```

Because this invariant is primarily Hospital-owned, add a genuinely joint variant using exposed projections, such as `Population.admitted_count = Hospital.occupied_beds`; state both projections are explicit typed boundary observations/capabilities, not arbitrary hidden-table reads.

Then show two concurrent admissions each legal against old occupancy `9/10` but jointly producing `11/10`, motivating a shared bed claim or prospective-state check. Contrast with observational syntax:

```lean
-- Proposed forms; keep distinct
assert occupancy_report := occupied_beds ≤ staffed_beds
invariant capacity := occupied_beds ≤ staffed_beds
```

**Teaches:** constrained product, exposed invariant projections, assertion versus semantic restriction, and concurrent preservation risk.

### 11. §9 Nesting/expose/hide — insert after current line 618 and after line 633

Show a complete small proposed composite:

```lean
-- Proposed Sembla composition syntax (illustrative, not implemented)
component EpidemicPolicy where
  instance population := Population
  instance policy := Policy
  wire population.infection_count -> policy.infection_count
  wire policy.restriction_modifier -> population.restriction_modifier
  expose population.population_state as population_state
  expose population.infection_count as infection_count
  hide policy.policy_debug
```

Add an identity table:

```text
source path                     flat owner/port ID       mailbox?
population.population_state     unchanged leaf ID        no (exposure alias)
policy.policy_debug             unchanged, non-public    no (hidden only)
population -> policy wire       stable wire/mailbox ID   yes (one delayed mailbox)
```

Then show `RegionalModel` nesting two `EpidemicPolicy` instances and expose only regional counts. Explicitly state nesting adds no delay; only actual wires do.

**Teaches:** nesting, expose/hide, multiple composite instances, flattening/source maps, and mailbox identity.

### 12. §10 Stochastic identity — insert after current line 670

Instantiate the identity tuple:

```text
(def=Population/v1, instance=north, local=infect,
 entity=Person#42, draw=hazard_time)
```

Show a rename/reassociation pair with the same semantic tuple but changed display path. Add a “current versus target” two-row table: current dense `rule_id` depends on global declaration order; proposed stable tuple maps to collision-checked runtime key excluding reserved RNG IDs.

**Teaches:** why snippets must distinguish visible names from semantic IDs and why product laws are gated by Phase 0.

### 13. §11 Heterogeneous schedulers — insert after current line 739

Use a concrete outer interval:

```lean
-- Proposed Sembla composition syntax (illustrative, not implemented)
scheduler population := tauLeap (device := gpu)
scheduler hospital := exactCtmc (device := cpu)
scheduler policy := ode (solver := rk4)
outer_dt := 0.25
```

Follow with a boundary trace: inputs frozen at `t=4.00`; each domain substeps privately to `4.25`; outputs exchange only at `4.25` for the next interval. Show that `admit_patient` spanning Population/Hospital is rejected unless represented as an explicit boundary transaction.

**Teaches:** heterogeneous schedulers, macro-boundary causality, private substeps, provenance, and scheduler-spanning-family gate.

### 14. §12 Options A–D — add one lowering micro-example inside every option

Do not merely list prose tradeoffs. Lower the same `EpidemicPolicy` source in four visibly different ways:

- **Option A, after current line 770:** proposed Lean declarations elaborate immediately to today’s `Model { boxes=[population,policy], wires=[...] }` plus a required embedded/side provenance map. Say no reusable graph survives as primary IR.
- **Option B, after current line 792:** proposed recursive executable JSON shape `CompositeBox { children:[...], wires:[...] }`; label it hypothetical and show runtime recursion.
- **Option C, after current line 821:** proposed AST:
  ```text
  Hide(Expose(Wire(Tensor(Instantiate(Population), Instantiate(Policy)), ...), ...), ...)
  ```
  Call feedback `DelayedFeedback` rather than an unrestricted categorical `Trace` unless trace laws are separately justified.
- **Option D, after current line 852:** show source graph -> validated graph -> canonical current-shaped flat plan + embedded source hash/identity map. Mark this recommended, not implemented.

Add a four-row comparison table keyed by “author input / serialized source / executable artifact / where hierarchy survives.”

**Teaches:** Options A–D through one identical composition and makes their differences inspectable. Option E may retain only the rejection example; no new required feature coverage depends on it.

### 15. §13 Recommended hybrid pipeline — insert after current line 907

Add a concrete composition-source graph excerpt:

```json
{
  "definitions": ["Population", "Policy"],
  "instances": [
    {"id":"population@1", "definition":"Population"},
    {"id":"policy@1", "definition":"Policy"}
  ],
  "wires": [
    {"id":"infection-feedback", "from":["population@1","infection_count"],
     "to":["policy@1","infection_count"], "delay_ticks":1}
  ]
}
```

Label it proposed and abridged. Follow with a conceptual ACSet/Catlab-style schema, deliberately pseudo-code and explicitly non-runnable:

```julia
# Conceptual ACSet/Catlab-style notation (explanatory only)
@present SchComposition(FreeSchema) begin
  Definition::Ob; Instance::Ob; Port::Ob; Wire::Ob
  definition::Hom(Instance, Definition)
  source::Hom(Wire, Port); target::Hom(Wire, Port)
  owner::Hom(Port, Instance)
end
```

Mention that a real schema also needs port direction/type, definition-owned ports, exposures, stable IDs, scheduler domains, and provenance. Do not claim Catlab is a dependency or selected serialization.

**Teaches:** source graph structure, referential constraints, how an ACSet representation could fit, and the linker’s concrete input/output.

### 16. §14 Illustrative surface directions — refactor rather than delete

Keep §14 as the single consolidated reference, but make it a payoff/index to examples already introduced. Replace the current disconnected snippets with one complete `EpidemicPolicy` proposed listing and one shorter `CareNetwork` extension. Add cross-references (“product introduced in §5”, “delay traced in §6”, etc.). Repeat the proposed/deferred labels on every block.

Avoid pretending the proposed component body can use `system Person ...` without showing that the leaf body reuses current command declarations. A useful pattern is:

```lean
-- Proposed wrapper; body declarations shown below are current leaf syntax
component Population where
  input restriction_modifier where ...
  system Person (rows := ...) where ...
  infect on Person : ...
```

Explain exactly which lines are existing leaf grammar and which wrapper/qualification constructs are new.

**Teaches:** full family in one place without making §14 the only examples section.

### 17. §15 Laws — append a concrete witness under every law group

Add a compact law table:

- Product: link `(Population ⊗ Policy) ⊗ Hospital` and `Population ⊗ (Policy ⊗ Hospital)` to byte-identical canonical plans after stable-ID sorting.
- Wiring: replay recorded `infection_count` deliveries into Policy and require the same policy state/draw trace (wire-cut law).
- Transition: make one `admit_patient` lose the bed claim and assert neither leg commits.
- Constraint: start at occupancy `9/10`, offer two admissions, and require legal resolution/error rather than `11/10` or repair.
- Scheduler: wrap unchanged Population domain in `EpidemicPolicy` and require behavior preservation; split the domain and explicitly do **not** claim equivalence.

**Teaches:** abstract laws as falsifiable observations over the running family.

### 18. §16 Test strategy — add test-case pseudocode after each test layer

Use the same named fixtures across layers:

```text
frontend/linker: link(associate_left(CareNetwork)) == link(associate_right(CareNetwork))
IR validation:  two drivers of policy.infection_count -> MultipleDrivers
runtime:        emit count=620 at tick 8 -> Policy cannot read it before tick 9
family:         bed claim loses -> person and bed both unchanged
constraint:     concurrent admissions cannot commit occupancy=11 when capacity=10
CPU/CUDA:       same canonical plan -> same selected draw/mailbox/state traces
```

Add one phase-gating note: identity/draw-trace properties are expected red against the current dense-ID runtime and must not be presented as current passing tests. Existing current evidence should link to `crates/sembla-runtime/tests/composition.rs`; proposed tests should not be confused with it.

**Teaches:** exact oracles and which failures each layer owns.

### 19. §17 Phases — add a cumulative “example capability” line to every phase

After each phase list, show what becomes expressible/testable:

- Phase 0: stable identities for `Population.infect` under sibling insertion.
- Phase 1: reusable `Population`, two instances, unwired/product linking.
- Phase 2: full nested `EpidemicPolicy`, expose/hide, flattening.
- Phase 3: `SumCounts` fan-in adapter for north/south populations.
- Phase 4: atomic `CareNetwork.admit_patient`.
- Phase 5: `CapacitySafe` assertion/invariant (noting observational assertions may move earlier).
- Phase 6: tau-leap Population + exact Hospital + ODE/discrete Policy at outer boundaries.

**Teaches:** phases as increments on one system rather than an abstract backlog, and covers the requested phases explicitly.

### 20. §18 Open decisions and §19 Recommendation — add decision-impact references, not new syntax

Under open decisions, identify which example resolves each decision (e.g. symmetry compares associated `CareNetwork` plans; hierarchy decision inspects `EpidemicPolicy` source maps). In §19, close with a six-line progression from current `sirPolicy` flat boxes to proposed hybrid source graph. Do not introduce a third example.

## Editing guidance and risks

1. **Do not overclaim implementation.** The strongest risk is that plausible Lean-looking blocks will be read as accepted syntax. Labels must be local to every block.
2. **Preserve current grammar exactly where labeled current.** Current wires are `wire population infection_count -> policy infection_count`, not dotted endpoints. Dotted endpoints belong only to proposed composition syntax.
3. **Do not conflate flat boxes with reusable instances.** `sirPolicy` is a current analogue and execution witness, not evidence that `instance` exists.
4. **Keep `dt` ownership unresolved where the note says it is open.** Scheduler snippets may show `outer_dt` only as proposed; do not silently decide whether component definitions own `dt`.
5. **Avoid fake Catlab authority.** Use “Catlab-style” and “pseudo-code”; cite it as a possible representation. If runnable Catlab code is desired later, independently verify against the pinned/current Catlab API and add a tested external example.
6. **ACSet product terminology needs an explicit warning.** The runtime pair of owned leaf states is not relational row product. Avoid claiming categorical coproduct/product unless the category and schema embeddings are formally selected.
7. **Delayed feedback is guarded/stateful.** Do not use ordinary `Trace` notation without explaining the one-tick mailbox; prefer `DelayedFeedback` in examples.
8. **Keep scope documentary.** This revision should not select open decisions, add dependencies, change IR, or add tests. It proposes examples and wording only.
9. **Control document length.** Use short snippets (typically 4–12 lines), progressive diffs, and cross-links. §14 should hold the only full listing; earlier sections show focused excerpts.
10. **Existing reviewer callouts can interrupt flow.** Place examples before the relevant callout and ensure the callout still refers to the immediately preceding semantics; do not bury labels inside callouts.

## Suggested next-agent contract

### Goal

Edit only `docs/design/composition-options.md` to implement the distributed-example plan above, using `EpidemicPolicy` plus its `CareNetwork` extension and locally labeling every snippet as current, proposed, conceptual ACSet/Catlab-style, or semantic pseudocode.

### Context/evidence

- Target headings and current line anchors are enumerated above.
- Current syntax authority: `frontend/README.md`, `docs/design/surface-syntax-options.md`, and `frontend/Sembla/Models.lean` (`sirPolicy`).
- Current flat IR authority: `frontend/Sembla/IR.lean`, `crates/sembla-ir/src/model.rs`, and `examples/two_box.json`/`examples/sir_policy.json`.
- Current delay tests: `crates/sembla-runtime/tests/composition.rs`.
- ACSet and operad commitments: `DESIGN.md` §§4.1, 4.4–4.5 and `DECISIONS.md` B1/D1.
- Every requested topic has a mapped insertion: product (§§2/5), instances (§4), wiring/delay/feedback (§6), nesting/expose/hide (§9), constrained product (§8), synchronized families (§7.3), heterogeneous schedulers (§11), Options A–D (§12), source graph (§13), laws (§15), tests (§16), and phases (§17).

### Success criteria

- Examples occur across semantic, architecture, testing, and sequencing sections, not only §14.
- One coherent primary/extension family is used throughout.
- Current code-shaped blocks match repository syntax; proposed constructs are never presented as runnable/current.
- ACSet state, machine product, row Cartesian product, and conceptual source-graph ACSet are distinguished explicitly.
- Each requested feature has at least one concrete snippet or trace and an explanation of what it teaches.
- Existing semantic caveats (dense rule IDs, one-tick mailboxes, exact schemas, scheduler-domain limits, deferred families/invariants) remain intact.

### Validation

Run documentation-focused checks after editing:

```sh
git diff --check -- docs/design/composition-options.md
rg -n "Current/implemented|Proposed Sembla|Conceptual ACSet|Semantic pseudocode" docs/design/composition-options.md
rg -n "Population|Policy|Hospital|admit_patient|CapacitySafe|Option A|Option B|Option C|Option D" docs/design/composition-options.md
```

Then manually compare every block labeled current against `frontend/README.md`/`frontend/Sembla/Models.lean`, and inspect the rendered Markdown for callout/fence integrity. No runtime test is required for a documentation-only edit; if current Lean snippets are promoted to full compilable examples, `cd frontend && lake build` is the next-best validation.

### Stop/escalation rules

Stop once all mapped topics are represented and labels are unambiguous; do not use the revision to settle open IR/API decisions. Escalate only if asked to freeze proposed syntax, claim runnable Catlab code, or choose a scheduler/invariant/family semantics not already selected by the note.

### Resolved assumptions

- `EpidemicPolicy` is based on the canonical current `sirPolicy` behavior but is a proposed reusable wrapper.
- `CareNetwork` is a small extension, not a separate running example.
- Catlab notation is conceptual only.
- The artifact requested in this task is the only changed file; project/source files remain untouched.

## Review checklist

- No blocker found in the plan’s coverage: all requested example categories have explicit insertion points and teaching goals.
- Independent reviewer should pay special attention to notation labels, current wire grammar, ACSet product wording, and whether the expanded document remains readable rather than duplicative.

```acceptance-report
{
  "criteriaSatisfied": [
    {
      "id": "criterion-1",
      "status": "satisfied",
      "evidence": "Produced a documentation revision plan only, with exact heading/line insertion points, a coherent running example family, concrete snippets, notation labels, risks, and validation; no project/source file was modified."
    },
    {
      "id": "criterion-2",
      "status": "satisfied",
      "evidence": "The plan cites current syntax/IR/runtime authorities, maps every requested topic to a specific insertion, and includes an independent-review checklist and next-agent validation contract."
    }
  ],
  "changedFiles": [
    ".pi-subagents/artifacts/outputs/7a0a8bcf-aa1a-48f1-bb04-f91041cedc43/composition-examples/revision-plan.md"
  ],
  "testsAddedOrUpdated": [],
  "commandsRun": [
    {
      "command": "nl -ba docs/design/composition-options.md | grep -E '^ *[0-9]+\\s+#{1,4} '",
      "result": "passed",
      "summary": "Captured authoritative current line numbers for all target headings."
    }
  ],
  "validationOutput": [
    "Reviewed the full 1,242-line target document.",
    "Cross-checked current Lean surface syntax, flat Lean/Rust IR, canonical sirPolicy model, current JSON feedback fixture, and runtime composition tests.",
    "Coverage map includes product, instances, wiring/delay, feedback, nesting/expose/hide, constrained product, synchronized families, heterogeneous schedulers, Options A-D lowering, source graph, laws, tests, and phases."
  ],
  "residualRisks": [
    "No runnable Catlab code was verified; the plan therefore requires all Catlab-style notation to be labeled conceptual pseudo-code.",
    "Line numbers will drift during editing, so headings are designated authoritative.",
    "A later editor must prevent proposed Lean-like syntax from being mistaken for implemented grammar."
  ],
  "noStagedFiles": true,
  "diffSummary": "Added only the requested revision-plan artifact; no project or source files changed.",
  "reviewFindings": [
    "no blockers",
    "review focus: verify local notation labels and ACSet/machine-product distinction in the eventual document edit"
  ],
  "manualNotes": "This task requested a plan and explicitly prohibited project/source edits. No tests were added because there was no implementation change."
}
```
