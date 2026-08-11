# Demographic spine and modular aggregate-first calibration

Status: proposal; descriptive and non-normative
Date: 2026-08-10
Authority: [`DESIGN.md`](../../DESIGN.md) and [`DECISIONS.md`](../../DECISIONS.md) remain binding. This proposal does not amend either authority.
Builds on: [the Australian population model](australian-population-model.md), [its calibration guide](../guides/australian-population-calibration.md), [the composition architecture](option-d-architecture.md), [the model algebra](model-algebra.md), and [the justice implementation sequence](../justice-event-schema-implementation-sequence.md).

## Purpose

Define a strategic path from the current ABS-calibrated Australian population
model to a reusable demographic base for labour-market, justice, health and
other policy microsimulations.

The central recommendation is:

> Estimate every parameter with the cheapest scientifically adequate model,
> use deterministic aggregate dynamics whenever the microsimulation closes at
> that level, and reserve stochastic microsimulation for individual
> heterogeneity, path dependence, constrained interactions and validation.

This is an aggregate-first calibration architecture, not a proposal to replace
microsimulation with one universal aggregate model. Each domain receives its
own aggregate companion and an explicit account of what that companion omits.

## Motivation from the current demographic evidence

The retained 2010–2025 calibration evidence shows a specific failure mode in
the current seventeen-dimensional NPE stage:

- the gravity-only origin–destination WAPE is 22.4%;
- accepted NPE updates worsen it to 29.4%;
- only three of fifteen yearly posteriors are accepted;
- six fail simulation-based calibration; and
- six more have a non-positive posterior median and fail the parameter-domain
  gate.

The detailed evidence remains in
[`calibrated-2026-08-07`](../evidence/australian-population/calibrated-2026-08-07/README.md).
It is not superseded or rewritten by this proposal.

The 56 annual origin–destination cells already identify the fifteen spatial
parameters through the fast Poisson gravity fit. Broad NPE draws then spend
simulation budget relearning those parameters and can move them away from the
aggregate optimum. The two age-profile parameters carry the remaining
identification problem, but the normalized state/direction/sex/age flow
compositions also contain aggregate evidence about them.

This motivates a declared replacement experiment rather than retrospective
tuning of the retained run.

## Strategic principles

### 1. Separate parameter identification from trajectory generation

A stochastic simulator is not intrinsically a better estimator. If an
aggregate likelihood or estimating equation uses all the information relevant
to a parameter, estimate the parameter there and feed its estimate and
uncertainty into the microsimulation.

The microsimulation remains responsible for producing individual trajectories,
joint distributions and policy counterfactuals. It need not rediscover
parameters already identified by published stocks, flows or microdata.

### 2. Give every component an aggregate companion

A scientific component should declare:

1. its detailed micro-state;
2. an aggregate state and aggregation map;
3. deterministic or approximate aggregate dynamics;
4. parameter blocks and their owners;
5. observation and target mappings;
6. the conditions under which aggregate closure is exact; and
7. a measured aggregate-to-micro discrepancy when closure is approximate.

In the language of the [model algebra](model-algebra.md), exact aggregation is
a lumping or an expected-value analogue of a lumping. The desired commuting
relationship is, schematically,

```text
micro state --micro update--> micro state
     |                            |
     h                            h
     v                            v
aggregate ---aggregate update--> aggregate
```

No exactness claim is made unless it is proved structurally or supported by a
predeclared parity test.

### 3. Partition parameters before allocating simulation budget

Every parameter belongs to one of four classes:

| Class | Evidence and method | Default treatment |
|---|---|---|
| Directly estimable | Published occurrence/exposure counts or individual records | Fit outside the simulator |
| Aggregate-dynamical | Stock-flow evolution or aggregate transition system | Fit with the aggregate companion |
| Micro-only | Histories, matching, networks, capacity interactions or latent dependence | Fit with targeted simulation |
| Coupling/scenario | Cross-component effects or external conditions | Estimate from linked evidence or expose as a scenario |

Only the micro-only block should routinely require broad simulator-based
inference.

### 4. Keep calibration modular

Let demographic, labour and justice parameters be
`θ_D`, `θ_L` and `θ_J`, with a smaller block `ψ` for genuine cross-domain
couplings. The default factorisation is conceptually:

```text
p(θ_D | demographic evidence)
× p(θ_L | θ_D, labour evidence)
× p(θ_J | θ_D, θ_L, justice evidence)
× p(ψ | linked cross-domain evidence)
```

This is a modular or cut-posterior discipline. Weak justice evidence must not
improve its fit by moving well-identified interstate-migration parameters.
Feedback is admitted only through a named mechanism with identifying evidence.

### 5. Do not construct one giant aggregate cross-tab

A full aggregate state such as

```text
age × sex × state × education × employment × occupation
× justice status × offence × sentence state × ...
```

recreates the microsimulation's dimensionality. Each component should retain
only the margins, cross-tabs and moments required by its own hazards and public
interface. The microsimulation carries the complete joint distribution.

## Proposed demographic calibration replacement

### Profile the spatial block rather than relearning it

For run year `y`, write:

- `α_y` for the fifteen spatial parameters; and
- `φ = (peak_months, k)` for the age profile.

For every candidate `φ`, recompute age-weighted exposures and refit the spatial
block using the existing Poisson IRLS calculation:

```text
α̂_y(φ) = argmax_α log L_OD,y(α, φ)
```

Then estimate `φ` from the normalized interstate age-sex compositions, with
the spatial block always re-profiled:

```text
φ̂ = argmax_φ Σ_y log L_composition,y(α̂_y(φ), φ)
```

This profile-likelihood or variable-projection construction prevents the age
stage from casually degrading the directly identified O–D fit.

Start with one pooled `peak_months, k` pair over 2010–2024. Test residuals
before adding a COVID regime, a smooth year effect or independent annual
values. Do not begin with thirty annual age-profile quantities.

Use conditional multinomial or Dirichlet-multinomial objectives for the
normalized compositions. This retains the current refusal to force
incompatible source totals—especially the documented 2020 vintage conflict—
into a false joint count likelihood.

Because the separable gravity form is rejected by its own deviance, report
quasi-likelihood, sandwich or otherwise overdispersed uncertainty. Do not
interpret the inverse Poisson Hessian as complete scientific uncertainty.

### Build a deterministic monthly expectation model

The demographic state is nearly closed over cells such as:

```text
presence × residence area × sex × age-month
```

with any lifecycle or event-marker state needed to reproduce the executable
semantics. For competing hazards `λ₁, …, λⱼ`, let `Λ = Σ λⱼ`. The one-tick
probabilities implied by racing exponential clocks are:

```text
P(j fires) = (λⱼ / Λ) × (1 - exp(-Λ Δt))
P(no contested event) = exp(-Λ Δt)
```

The aggregate evaluator should propagate expected cell counts through all
twelve monthly ticks, apply deterministic ageing, and accumulate transition
counts as rewards. It must reproduce the actual tau-leap contest and lifecycle
semantics rather than an unrelated cohort-component approximation.

Where rates depend only on the retained cell and exogenous parameters, this
should produce the exact simulator expectation. Entry activation and other
fixed-pool details must be examined explicitly; if they prevent exact closure,
the evaluator is labelled approximate and its discrepancy is measured.

The evaluator should emit the same ordered summaries used by calibration:

- ending stocks;
- births, deaths and overseas entry/exit;
- all 56 O–D movements; and
- normalized migration and stock age-sex compositions.

### Restrict any remaining simulator inference

After aggregate fitting:

1. run the microsimulation at the aggregate estimate over multiple seeds;
2. compare its mean summaries with the deterministic expectation within Monte
   Carlo standard error;
3. evaluate a small local parameter design if sensitivity parity is needed;
4. fit only a measured residual discrepancy or genuinely micro-only parameter
   block; and
5. use the aggregate estimate as the fallback whenever the residual stage
   fails.

If NPE remains, train it in support-preserving coordinates:

```text
log(base), log(push), log(pull), log(k)
logit((peak - lower) / (upper - lower))
```

A density estimator must not be able to emit a negative rate. Gates are
blockwise: failure of an age-profile correction cannot invalidate the spatial
fit. A micro correction is accepted only if it passes its inference checks and
does not worsen the aggregate O–D objective beyond a predeclared tolerance.

### Consider a separately declared spatial alternative

The 22.4% gravity-only O–D WAPE is evidence of stable dyadic structure not
captured by a separable push–pull model. A possible competing specification is:

```text
log λ_odt = c_t + u_ot + v_dt + δ_od
```

where `δ_od` is a time-invariant O–D affinity estimated from all fifteen years.
It can add stable dyadic information without adding 41 free parameters per
year. This is not part of the first replacement experiment and would amend the
scientific model in `DECISIONS.md` §N5; it must never enter as an invisible
calibration correction.

## Reusable demographic spine

The existing `australian_population` model remains the frozen scientific and
evidence baseline. A reusable successor should be a separately versioned
component rather than a mutation that invalidates existing plans, state
artifacts or goldens.

### State ownership

The demographic spine should own only authoritative lifecycle state:

```text
DemographicPerson
- person_key
- generation
- presence
- age_months
- sex
- residence_area
- entry stream
```

Identity across components is `(person_key, generation)`. Row ordinal alone is
insufficient once a slot can be reused: a new generation must not inherit the
labour, health or justice history of the previous occupant.

Residence, workplace location, justice reporting jurisdiction and custody
location are distinct domain concepts. Downstream components must not overload
`residence_area` for their own geography.

### Lifecycle interface

The spine should expose versioned, table-valued messages or equivalent
component interfaces for:

```text
entered(person_key, generation, entry stream, age, sex, residence)
exited(person_key, generation, exit stream)
died(person_key, generation)
moved(person_key, generation, origin, destination)
population exposure by declared demographic cells
```

Each mutable field has one owner. A downstream component requests an effect on
demography through an explicit causal interface; it does not write demographic
columns directly.

A successor design must decide how domain state is initialized and cleared for
births, arrivals, deaths and emigration. The current one-use preclassified
entry pool is adequate for the 2010–2025 run but should not silently become the
permanent multi-domain lifecycle contract.

### Current composition boundary

The current composition product gives boxes disjoint private state and
one-tick-delayed, table-typed wires. Stable component and transition identities
do not establish shared person identity. `Share`/`Identify` and constrained
products remain deferred by `DECISIONS.md` §J12.

The implemented wire payload is narrower than the architecture's eventual
finite-table interface may suggest. In the current executable IR,
`OutputBuilder.perTable` constructs fields only with aggregate `count` and
`sum` operations. The Rust `build_output` path materializes exactly one row.
Consequently, current macros cannot turn an output into a stream such as:

```text
(person_key = 123, generation = 2, event = entered)
(person_key = 814, generation = 1, event = died)
```

without a new row-projection or filtered-compaction output form in the IR and
its CPU/CUDA implementations. A parallel person table in another box can share
an initial key convention, but current wires cannot keep it synchronized at
individual granularity through entry, movement and exit.

The immediately viable current representation is therefore a single primitive
box for person state that must be read and mutated together. Modularity can
still exist in Lean authoring if the frontend statically fuses separately
authored person facets before emitting ordinary executable IR.

## Frontend-first lowering versus core semantics

Using the six framework needs identified by this proposal, four can support the
near-term demographic, labour and justice-stock model without changing the
executable IR or Rust runtime, provided person-level modules are statically
fused. Three are straightforward macro/library generation; the fourth is a
substantial Lean elaborator or linker feature. Two needs have only bounded
frontend workarounds and ultimately require new runtime semantics.

| Need | Near-term Lean-side lowering | Full general capability |
|---|---|---|
| Common person identity | Fuse person facets into one table in one primitive box | First-class shared-state composition or person-granular wires |
| Lifecycle isolation | Restrict facet behavior to present rows and reset every facet field atomically on entry | Individual lifecycle delivery across private boxes |
| Slot reuse and generation | Generate occupancy, generation, entry reset and invariant logic | Existing executable IR is sufficient |
| Sparse justice/case records | Use person-aligned state or a bounded fixed pool | Dynamic rows, allocation or controlled cross-row operations |
| State-level feedback | Unroll the eight states into guarded transition/input families | Generic enum-keyed lookup or join |
| Different timescales | Use one global fine timestep and generated phase state where adequate | Scheduled clocks or heterogeneous scheduler domains |

This is a scope claim, not a claim that four general-purpose semantics can be
implemented as syntax sugar. If demography, labour and justice must remain
separate executable boxes from the first pilot, current aggregate-only outputs
remove the static-fusion escape hatch: common identity and synchronized
lifecycle then also become IR/runtime work.

### Static person-facet fusion

A frontend-only MVP can introduce an authoring abstraction conceptually shaped
as:

```text
Demographic facet ─┐
Labour facet ──────┼─ Lean elaboration ─> one ordinary Person table and Box
Justice facet ─────┘
```

Each facet contributes namespaced attributes, transitions, views and entry-reset
hooks. The elaborator should:

- form the union of compatible person attributes;
- preserve one declared owner for every mutable attribute;
- require explicit declarations for non-owned person reads and read-only fields
  on any input port already owned by the base box;
- namespace and stabilize transition identities;
- reject conflicting writes and incompatible types;
- require one entry initializer for every reusable field;
- automatically restrict facet transitions and observations to present rows;
- combine all facet entry initializers into the base entry transition's atomic
  effect list; and
- emit only existing `Table`, `Transition`, `Effect.setAttr`, view and port
  constructors.

The combined entry must be one transition, not several transitions with similar
guards: separate racing transitions would not give atomic lifecycle
initialization. On slot reuse that transition increments `generation` and resets
every domain field before the new person becomes visible. Because all effect
right-hand sides read the tick-start row, foundation entry initializers are
restricted to literals or facet parameters; their textual order does not let one
initializer observe age, area or generation written by the same entry.

Facet fields are not generically cleared on exit. A domain transition and a
demographic exit can both fire from the same tick-start person; attaching an
exit cleanup that writes the same domain field would create an accepted-write
conflict. Instead, vacant rows are semantically inactive: generated facet
transitions and views always require `occupancy = present`, row identity includes
`generation`, and the next entry overwrites every facet field atomically. This
avoids stale-state leakage without making labour or justice events compete with
and defer death or emigration. It does not prohibit a domain event and an exit
from both being counted in one tau-leap tick; the row is inactive afterwards,
and a real domain model must measure that approximation through timestep
sensitivity.

This construction is not V1 `Share`/`Identify`. The authored facets disappear
as executable leaves and the plan contains one fused primitive box. It therefore
forgoes independent leaf state, ordinary inter-facet wires and some existing
composition provenance. A polished public feature would require a maintained
decision even if it emits unchanged executable IR; a frontend-neutral,
first-class version would later require a composition-source/linker contract
rather than a Lean-only elaborator convention.

### Fixed geography can be compiled away

For the eight-state Australian model, a high-level expression such as

```text
regional_unemployment[person.residence_area]
```

can elaborate into eight guarded cases and eight scalar fields. An aggregate
output can emit state-specific counts or rates in one row, and generated
state-specific transition families can consume the matching field after the
ordinary wire delay. This is verbose IR but requires no new executable
semantics—the 56 generated interstate transitions already demonstrate the same
finite-family strategy.

This does not solve arbitrary runtime-sized geography. A generic enum-keyed
lookup or own-area aggregate join remains a possible later IR feature if fixed
unrolling becomes a measured blocker.

### Sparse records are only bounded at the frontend

The justice stock-state MVP can place current legal, offence, sentence and
custody state on the fused person row. A macro can also preallocate a declared
fixed number of slots and maintain occupancy/generation columns.

It cannot create the full justice ontology. Current tables have fixed
`sizeHint`; an effect only writes the transition's own row; and an expression
cannot select an arbitrary vacant row and assign it to a person. Unbounded or
sparsely allocated matters, charges, hearings and sentences therefore need a
new bounded-allocation, dynamic-row or controlled cross-row semantic path.
That path would affect validation, state artifacts, deterministic identity,
CPU execution and CUDA execution, not just surface syntax.

### Multiple clocks are only approximated at the frontend

A macro can lower some slower processes into phase attributes or select a
smaller common global timestep and guard monthly processes with generated clock
state. Phase-type chains can approximate some duration distributions while
remaining in the present racing-clock model.

That is not heterogeneous scheduling. `DECISIONS.md` §J9 fixes one global
`outer_dt` and one global tau-leap scheduler. Running the whole Australian
population at a court-scale timestep may be prohibitively expensive, and an
arbitrary duration sampled at stage entry is not supplied by the current effect
language. True scheduled clocks, per-box timesteps or an exact court scheduler
require plan/IR and runtime semantics.

### Recommended implementation boundary

The aggregate calibration replacement—profile likelihood, deterministic
expected-count propagation, modular parameter ownership and parity tests—can
be implemented outside the executable runtime. Phases 0–3 should therefore
make no IR or Rust changes.

For phases 4–6, first test a Lean person-facet fusion layer that emits one
ordinary primitive box, combined lifecycle transitions, fixed-enum regional
families and bounded stock state. Do not call this `Share`/`Identify` or emit a
silently accepted deferred construct. Preserve the existing frozen population
model and record the frontend-only experiment explicitly.

Only measured failures of that labour or justice-stock pilot should trigger
core work on person-granular output streams, shared tables, sparse row
allocation or heterogeneous schedulers. This keeps the scientific driver in
control of the semantic expansion and avoids paying the CPU/CUDA compatibility
cost before a model demonstrates the need.

## Domain extensions

### Labour market

Labour is the preferred first extension because much of its initial state is
one row per working-age person and monthly transitions are compatible with the
demographic clock.

A first labour component may own:

```text
labour-force status
education
occupation and industry
hours and earnings
employer or vacancy reference
workplace geography
```

Employment-status flows stratified by demographic cells can be estimated with
a fast multistate transition model. The microsimulation becomes necessary for
features such as worker–vacancy matching, employer capacity, earnings tails,
household dependence and persistent unobserved heterogeneity.

A VAR or state-space model may be useful here for sufficiently long monthly or
quarterly macro series—unemployment, wages and vacancies. It should supply
macro conditions to labour hazards, not act as the demographic bookkeeping
model.

### Justice

Begin with the stock-state MVP already proposed in the
[justice implementation sequence](../justice-event-schema-implementation-sequence.md):
current legal/custody state by jurisdiction, demographic group and offence or
sentence grouping. Admissions, releases and state changes can first be fitted
with aggregate occurrence/exposure and stock-flow models.

Full justice trajectories require capabilities deliberately outside that MVP:

- sparse one-to-many matters and charges;
- arbitrary event-row insertion;
- cross-row writes;
- scheduled clocks and exact court timing;
- concurrent cases and sentence interactions; and
- append-only event histories.

Those are not presented as simple columns added to the demographic person.
Measured failures of the stock-state pilot should determine which semantic
extension is justified.

### Other domains

Health-state and benefit-receipt components can follow the same pattern:
aggregate multistate calibration first, then targeted micro modelling of
history, household and service-capacity effects. Household formation is likely
to reach the aggregate-closure boundary earlier because kinship and matching
constraints are central rather than residual.

## Joint initial population

Age, sex and state are not sufficient to initialize labour and justice modules
independently. Matching each module's marginals separately does not recover the
cross-domain joint distribution.

Construction should distinguish:

- relationships directly observed jointly;
- relationships reconstructed from overlapping margins;
- unconstrained associations; and
- inconsistent margins caused by source universes, rounding or revision.

Use linked microdata where available. Otherwise use documented maximum-entropy,
iterative-proportional-fitting or constrained-reweighting procedures and retain
an ensemble of plausible initial states. Reserve some cross-tabs for validation.
A microsimulation must not be said to identify a labour–justice association
that is absent from all calibration evidence.

## Current foreclosures and strategic pressure points

### Geography

`DECISIONS.md` §N3 makes demographic residence a writable enum, which enables
genuine interstate moves but prevents a generic hazard from joining to the
population of its own area. This is acceptable for an exogenous demographic
model and increasingly restrictive for labour-market feedback, local service
capacity and agglomeration.

State-specific guarded inputs can encode small fixed systems, but they do not
scale elegantly. A successor spine must either retain this as an explicit
foreclosure, introduce an evidence-justified enum-keyed input/join mechanism,
or adopt a new geography representation that preserves destination choice.

### Sparse dynamic entities

Labour state can often be carried in a parallel person table. Justice matters,
court events, sentences, facilities and employers are sparse or one-to-many.
Fixed preallocation may be acceptable for a bounded pilot but is not assumed to
be the final architecture.

### Timing

Monthly demographic and labour transitions fit the current driver. Court and
custody operations may require daily or event time, scheduled durations and
resource queues. Heterogeneous scheduling remains an evidence-gated semantic
extension, not a capability claimed by this plan.

## Evidence and acceptance contract

Every component-level aggregate companion must report:

1. fitted and held-out target definitions;
2. parameter identifiability and uncertainty;
3. structural goodness-of-fit rather than only optimizer convergence;
4. aggregate-to-micro mean and sensitivity parity;
5. timestep and scale sensitivity where relevant;
6. the residual simulation budget and why it is necessary;
7. fallback behavior for a rejected correction; and
8. model limitations and unobserved joint relationships.

For composed models, additionally require:

- person lifecycle synchronization across components;
- no reuse of state across generations;
- target and parameter ownership without double use;
- standalone-versus-composed non-interference for unwired components;
- CRN counterfactual checks for retained transition identities; and
- held-out validation of cross-domain statistics, not only each module's
  marginals.

## Proposed implementation sequence

### Phase 0 — Freeze the replacement experiment

- Preserve the retained NPE artifacts and reports unchanged.
- Amend `DECISIONS.md` §N5 or add a later superseding decision before changing
  the accepted calibration path.
- Freeze aggregate objectives, parameter transforms, pooling assumptions,
  fallback rules and acceptance thresholds before running the experiment.

**Done when:** the experiment can fail without any threshold or baseline being
rewritten.

### Phase 1 — Profiled aggregate age fit

- Reuse the existing gravity IRLS implementation.
- Re-profile all fifteen spatial parameters for every candidate age profile.
- Fit pooled `peak_months, k` from normalized migration compositions.
- Emit profile surfaces, robust uncertainty and source-level residuals.
- Compare against gravity-only and retained NPE evidence.

**Done when:** the two-dimensional fit is reproducible offline, has a declared
identifiability result, and cannot degrade O–D fit through an unprofiled spatial
update.

### Phase 2 — Deterministic demographic expectation runner

- Implement the twelve-tick aggregate transition and reward calculation.
- Cover movement, mortality, emigration, birth/arrival activation, event
  clearing and ageing.
- Emit the same ordered summary vector as the microsimulation.
- Verify expected summaries against replicated micro-runs at the fitted point
  and a small local design.

**Done when:** every exactness or approximation claim has retained numerical
evidence and a named tolerance.

### Phase 3 — Residual micro-calibration decision

- Determine whether any discrepancy is material relative to data and model
  error.
- If not, remove NPE from demographic parameter identification.
- If it is, fit only the responsible low-dimensional residual block in
  support-preserving coordinates with blockwise gates.

**Done when:** every remaining simulator draw has a documented identification
purpose that the aggregate companion cannot provide.

### Phase 4 — Versioned demographic spine

- Define the person/generation and lifecycle contracts.
- Prototype a Lean person-facet/static-fusion layer that emits one ordinary
  primitive box under the current executable IR.
- Generate atomic entry-reset hooks, automatic present-row guards, field
  ownership checks and reset-completeness checks for every fused facet.
- Decide finite-pool capacity, entry initialization, inactive-vacancy handling
  and any slot reuse policy.
- Produce a successor component without changing frozen
  `australian_population` artifacts.
- Add identity, lifecycle, non-interference and emitted-IR conformance tests.

**Done when:** a fused downstream facet can follow the same person across entry,
movement and exit without owning or duplicating demographic truth, and the
experiment has required no executable IR or runtime change.

### Phase 5 — Labour pilot

- Add a minimal person-aligned employment-state facet to the fused person box.
- Estimate its transition block from aggregate or individual labour evidence.
- Build its aggregate companion and parity tests.
- Measure the scientific and provenance costs of static fusion and whether the
  model actually requires separate person-granular boxes or shared-state
  semantics.

**Done when:** demographic and labour margins plus at least one held-out joint
statistic validate without changing demographic calibration, and any request
for deeper composition semantics points to a retained failing use case.

### Phase 6 — Justice stock-state pilot

- Follow the existing justice semantic ledger and stock-state sequence.
- Link admissions and releases to demographic exposure without claiming full
  event history.
- Retain an ensemble for incompletely observed initial cross-tabs.
- Measure the need for sparse dynamic entities, scheduled clocks and exact
  resource scheduling.

**Done when:** the pilot either validates within its explicit stock-state scope
or produces evidence for a specific runtime-semantic extension.

### Phase 7 — Evidence-gated semantic extensions

Only after the labour or justice pilots expose a measured blocker should the
project consider:

- `Share`/`Identify` or constrained products;
- row-projecting/filtering person-granular output streams;
- enum-keyed aggregate joins;
- dynamic sparse row lifecycles;
- cross-row mutation protocols;
- scheduled clocks; or
- heterogeneous schedulers.

Each addition must be a restricted general primitive justified by a driver
model, consistent with the [roadmap](../ROADMAP.md).

## Non-claims

This proposal does not claim that:

- every domain has an exact aggregate closure;
- aggregate marginals identify unobserved cross-domain dependence;
- a deterministic expectation captures individual or tail outcomes;
- the current fixed-pool model is already a reusable cross-domain person
  kernel;
- current composition identities are person identities;
- full justice event simulation is expressible in the current runtime; or
- demographic, labour and justice feedback is causal without linked evidence
  and an explicit mechanism.

The strategic claim is narrower: making coarse models, parameter ownership and
aggregate-to-micro discrepancy first-class will reduce wasted simulation,
expose non-identifiability earlier, and provide a disciplined path from the
current demographic model to credible composed policy models.
