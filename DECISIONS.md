# Sembla: Design Decisions and Rationale

The "why" companion to `DESIGN.md`. Each entry states the decision, the
**consideration** (what was at stake, the alternatives, the tension), and the
**rationale** (why this choice won). Many of these were forced by adversarial
review; where a decision was a concession that reversed an earlier position,
that is recorded honestly.

Section numbers reference `DESIGN.md`.

---

## A. Frontend and language

### A1. Lean 4 as the frontend

**Consideration.** The original pitch justified Lean by "nice syntax" and
"widgets" — both replicable elsewhere. Julia offers unicode math notation,
macros, and a mature numerics/ODE ecosystem; Pluto.jl offers reactive
slider→result loops; Python offers the entire scientific stack. If the only
gains were syntax and interactivity, Lean would be the wrong choice — it is
the least stable, most theorem-prover-centric option, with a batch-compiled
toolchain that is the worst-case substrate for live interaction.

**Rationale.** Two things survived scrutiny as genuinely Lean-specific:
(1) the **infoview** renders context-dependent interactive output for the
syntactic node under the cursor, driven by elaborator state, inside an
ordinary source file — Pluto is cell-granular and notebook-shaped, not
cursor/source-granular, so nothing in the Julia/Python world matches it;
(2) Lean can host a **formal semantics** for the DSL, which no other frontend
can. Everything else Lean was pitched for (syntax, sliders) is a bonus that
rides along, not the justification.

### A2. Lean is *not* for live behavior loops (in v1)

**Consideration.** The headline pitch was sliders → prior-predictive checks →
updated source. The widget demo that inspired this works because its loop is
editor→elaborator→small pure computation→render, local and instant. The real
loop is editor→IR→GPU→simulate 26M agents→summarize→render; the widget layer
is the cheapest link, and the latency lives in the runtime.

**Rationale.** We split widgets into **structure widgets** (state diagrams,
wiring views, prior densities — elaborator-only, zero runtime, genuinely
great in Lean) and **behavior widgets** (slider→simulate→plot — gated on
runtime performance). Only structure widgets are v0.1. The behavior loop's
feasibility is owned by the runtime, not the frontend — so it cannot be a
reason *for* Lean, and it cannot be promised until the GPU backend exists.

### A3. Lean as semantic ground truth; proofs deferred but specification paid now

**Consideration.** The original pitch explicitly said it "wouldn't rely on
Lean's proof functionality." But the strongest Lean-specific arguments —
verified program transformations (e.g. a gradient that carries a theorem),
and the future in which AI agents discharge proofs cheaply — all require a
formal *specification* to point proofs at. An agent cannot prove properties
of a pile of untyped macros; it needs the DSL's meaning defined as a
mathematical object.

**Rationale.** We adopted position (C): the IR is a **deep embedding** with a
denotational semantics defined in Lean; proofs are deferred, but the
specification is written from day one. This reverses the original "no proofs"
stance, and we accepted the reversal explicitly because it is the only thing
that makes Lean *load-bearing* rather than decorative. Cost knowingly
accepted: v1 must include a semantics, and every guarantee is over ℝ and
stops at the IR boundary (the Rust/GPU compiler is trusted, not verified).

### A4. Gradients/HMC do not constrain the v1 IR

**Consideration.** A verified symbolic gradient (SciLean-style) is a real
Lean-specific capability and reopens the HMC door — a legitimate reason to
value Lean. But discrete-state agent transitions (a categorical draw from
Employed→Unemployed) are discontinuous and have no useful gradient; HMC
applies only to the continuous fragment.

**Rationale.** Gradients are **option value**, not a v1 requirement. They get
no vote in the v1 IR design. Keeping them out prevents a feature justified by
a secondary corner (ODE-like blocks) from distorting the core (discrete
microsimulation). The IR is designed so gradients *can* be added later
without rework, but nothing waits on them.

### A5. Frontend-agnostic IR

**Consideration.** If Lean's widget round-trip proves miserable, or the
toolchain churns (widget APIs have historically been unstable), the project
must survive a frontend swap. Coupling the semantics tightly to Lean's type
theory would make that swap catastrophic.

**Rationale.** The IR is the contract; nothing in the Rust backend depends on
Lean. This is the hedge against Lean's instability and the kill-switch for
the frontend if the interaction story disappoints. It also enforces
discipline: the semantics lives in the IR's meaning function, not in
elaboration accidents.

### A6. Units belong in Lean, not in the Rust validator

*(Adopted 2026-07-17 from the PFCLBS review; DESIGN.md §8.)*

**Consideration.** The case for dimensional checking is strong and specific:
§4.3 writes `hazard 0.02 / year`, `dt` is a *semantic* parameter rather than a
performance knob, and a rate/`dt` unit mismatch is precisely the error a working
modeler makes — silently, with plausible-looking output. PFCLBS has a real
implementation to copy (units, refinements, and dimensioned literals in
`sks_validate`), so the tempting move is to port it into `sembla-ir`.

**Rationale.** Right goal, wrong building. Porting it would put a dimensional
type system in the backend validator — duplicating, in Rust, the one thing the
frontend was chosen to do (§A1: Lean hosts the semantics), and growing the IR's
trusted surface in the process. Sembla has a type theory available upstream of
the IR; PFCLBS does not, which is exactly why its units live in the validator.
Copying that placement would import a workaround for a constraint we don't have.

There is a scale argument too: PFCLBS's units are embedded in a ~44k-line
validator. Sembla's whole IR crate is ~1.9k lines. "Just port the unit system"
is not a small change to `sembla-ir`; it is a second `sembla-ir`.

So: units are a **frontend obligation**, discharged during elaboration, with the
IR receiving already-dimensioned values. The door stays open in Lean; it stays
shut in `sembla-ir`. If the Lean frontend is ever swapped out (§A5), the new
frontend inherits the obligation — which is the correct place for the cost to
land, since it is the frontend that promises the modeler this safety.

---

## B. State and data model

### B1. ACSets (attributed C-sets) as the state model

**Consideration.** Alternatives: object-graphs of agent structs (pointer-rich,
GPU-hostile, non-deterministic layout); a bespoke entity-component system; or
a relational/columnar model. The domain wants dynamic relationships
(works_at, lives_in) and heterogeneous per-entity attributes.

**Rationale.** ACSets give a *categorical* data model (schema = a category,
state = a functor to Set) that is simultaneously an ordinary typed columnar
database. This is the pivotal convergence of the whole design: the formal
object and the performant representation are the same thing.

### B2. Columnar/SoA layout is the semantics, not an implementation hack

**Consideration.** The original pitch treated "store individuals in an indexed
columnar format" as a performance concession bolted onto an
individuals-as-systems semantics. That framing creates a permanent
impedance mismatch between what the model *means* and how it *runs*.

**Rationale.** Struct-of-arrays layout *is* the ACSet, one identity functor
from the math. Making it the semantics (not a hack beneath it) removes the
mismatch, makes GPU-friendliness structural rather than retrofitted, and
means the model's meaning and its execution never diverge.

### B3. "An individual is a row, not a system; the population is the system"

**Consideration.** The founding aesthetic was "each individual is a system
with wires." But interaction topology is dynamic (contacts change, people
change employers, are born, die), and classical wiring diagrams fix
who-talks-to-whom before the semantics runs. Encoding "meet a random contact"
as a wire forces either a global matchmaker (the banned global in disguise)
or mode-dependent interfaces (the research frontier, not the settled part).

**Rationale.** We demoted composable-wiring to the *population* level and made
individuals rows in a population's ACSet state. Interactions within a
population are queries/rewrites over tables, not messages across interfaces.
The surface DSL still *reads* "an Individual is a system with states and
reactions" (Poly at the syntax layer), but it elaborates to relational
kernels. This preserves the aesthetic where it is honest (syntax, macro
level) and abandons it where it would lie (individual granularity).

### B4. Uniform data substance: state, wires, and messages are all tables

**Consideration.** Wires could carry fixed-width values (scalars/vectors) —
simple, but then individual-granular cross-boundary interaction is
inexpressible without widening interfaces to the whole state. Or wires could
carry a different data type than state, creating two data models to compile.

**Rationale.** Wires carry **streams of finite tables**; interface types are
relation schemas. One data substance throughout — state is tables, aggregates
are tables, wire messages are tables, the interior of a box is queries over
tables. This is GPU-friendly, lands adjacent to streaming-relational-dataflow
theory (DBSP), and lets cross-boundary interaction happen without state
sharing (§B3's no-globals principle survives).

---

## C. Dynamics, time, and stochastics

### C1. Restricted relational-kernel fragment, not general graph rewriting

**Consideration.** AlgebraicJulia's graph-rewriting ABM (DPO rewriting on
ACSets) natively handles birth, death, and dynamic rewiring — exactly the
things that killed individual-level wiring (§B3). It was the single most
relevant prior art. But general subgraph pattern matching is subgraph
isomorphism: combinatorial, allocation-heavy, branch-heavy — the anti-GPU
workload. Nothing in that line runs at 26M agents.

**Rationale.** We adopted the ACSet *data model* and the rewriting
*intuition* (birth/death as row create/delete), but restricted the rule
language to a fragment that compiles to columnar kernels: map, filter,
join-on-declared-keys, group-by with commutative-monoid aggregation, stream
compaction for birth/death. The line we drew: patterns are one primary entity
plus a bounded neighborhood via declared foreign keys; multi-entity
interaction only via declared join keys; no unbounded patterns. Every
restriction refused costs a GPU compilation strategy; every one accepted
costs some expressible model. This trade is the core research bet of the
project.

### C2. Hazard rates, not per-tick probabilities

**Consideration.** Transitions could declare per-tick probabilities (familiar
to ABM practitioners) or continuous-time hazard rates λ (exponential clocks).
Probabilities are tick-size-dependent and don't compose cleanly across
subsystems with different natural timescales.

**Rationale.** Hazard rates are the native statistical dialect of policy
microsimulation (survival analysis, duration/mortality/transition models —
exactly how the econometric estimates arrive). They induce clean CTMC
semantics, compose across timescales, and make dt a visible semantic
parameter rather than a hidden assumption. `with prob p` remains as sugar
desugaring to λ = −ln(1−p)/dt, so the familiar form is still available.

### C3. Racing clocks (CTMC) as ground-truth semantics

**Consideration.** How do concurrent transitions resolve? Options: synchronous
update (all fire together — but then conflicts need ad-hoc resolution, and
the sync-vs-sequential updating choice is known to change ABM outcomes);
sequential random-order (Mesa/NetLogo — but order is an arbitrary artifact);
or continuous-time racing clocks.

**Rationale.** Independent exponential clocks with earliest-wins is the CTMC —
a principled ground truth. Sequential random-order updating is recovered as
its discrete shadow (so we don't *lose* that mode, we *explain* it).
Bonus payoffs: queueing systems are CTMCs natively (§F), and the mean-field
limit of a population CTMC is an ODE system (Kurtz), making the agent→ODE
passage a theorem rather than an analogy — the third time coarse-graining
showed up as load-bearing structure.

### C4. Tau-leaping as the executed approximation

**Consideration.** Exact CTMC simulation (Gillespie) resamples every clock
after every single event — a strictly sequential event loop, the anti-GPU
workload. Exact fidelity is incompatible with parallel execution.

**Rationale.** Freeze rates at tick start, fire everything whose sampled time
lands in the window dt, resolve conflicts by argmin, let losers re-race next
tick (no within-tick cascades). This is parallel and GPU-legal, at the cost
of O(dt²) discretization error. Consequence made explicit: **dt is a semantic
parameter**, documented as such, not merely a performance knob. Exact
Gillespie survives as a per-box slow path for small subsystems and as a
validator.

### C5. No within-tick cascades (uniform one-tick delay)

**Consideration.** Should an effect applied early in a tick be visible to
transitions later in the same tick? Allowing it reintroduces order-dependence
(who goes first matters) and defeats parallelism and determinism.

**Rationale.** Double-buffered read-old/write-new, everywhere — across wires
*and within boxes*. Nothing sees same-tick writes. This is what makes
box boundaries semantically invisible (§D2), makes execution order-free
(hence parallelizable and deterministic), and matches the only sane GPU
execution model anyway. The cost — cross-boundary interactions are one tick
delayed — is uniform and documented, and made a *feature* of the semantics
rather than a bug (it is why refactoring is safe).

### C6. Scheduled clocks for non-exponential durations

**Consideration.** Racing exponential clocks give memoryless durations. Real
processes (court hearings, scheduled appointments, statutory deadlines,
lognormal service times) are aggressively non-memoryless. Pure CTMC can't
express them.

**Rationale.** Two compatible extensions: phase-type approximation (chained
exponential stages — stays pure CTMC) and scheduled clocks (sample a full
duration from any distribution at stage entry via Philox-at-entry-coordinates,
store the firing date, re-check the guard at firing so early exits cancel for
free). Scheduled clocks are a generalized semi-Markov process — the standard
DES semantics — and are *easier* on the runtime than races (no staleness).
Deferred past v0.1 but the semantics has a defined home for them.

---

## D. Composition

### D1. Operadic composition (boxes within boxes)

**Consideration.** With individuals demoted to rows (§B3), does "systems
compose" survive at all? Or does everything collapse into one monolithic box?

**Rationale.** Operad-style wiring (Spivak) is precisely the device for "a
box's interior can be anything presenting the right interface." A
population-as-relational-machine wraps as a Moore machine and sits in a
wiring diagram beside an ODE block or a policy module. This rescues the
"compose systems, take products" pitch at the *macro* level, and enables
heterogeneous-fidelity hybrids: a 26M-agent population tau-leaped on GPU
wired to a 30k-case court run exactly on CPU. The operad earns its keep as
the glue between subsystems of different scale and solver — which is where
it does real work, not at individual granularity.

### D2. Boundary invariance as a first-class property

**Consideration.** A compositional ideal says refactoring the box hierarchy
shouldn't change semantics. But if within-box interaction were instantaneous
while cross-box interaction were tick-delayed, moving a boundary would change
outputs — breaking the ideal.

**Rationale.** Because delay is *uniform* everywhere (§C5), moving a box
boundary never changes observable semantics. This is elevated from a hope to
a theorem candidate and a v0.1 acceptance test (a two-box model and its
hand-merged single-box equivalent must produce bitwise-identical state hashes
every tick). It is the concrete proof that composition is real, not
cosmetic.

### D3. No globals except the tick and θ

**Consideration.** The founding principle was "no global variables." But a
tax schedule, interest rate, or policy lever is exactly what policy users
want to grab and vary — and some global notion of time/step is unavoidable.

**Rationale.** We distinguished *mutable* globals (banned — broadcast as wires
instead) from *per-run constants* (parameters — read-only, hence not globals;
see §G). The only sanctioned globals are the synchronous tick and the
parameter vector θ. This keeps the no-globals principle honest (it applies to
mutable state) while admitting the constants policy work actually needs.

---

## E. Reproducibility and execution

### E1. Philox counter-based RNG, keyed by coordinates

**Consideration.** Stateful RNG streams make randomness order-dependent: which
agent draws first affects what it draws, so parallel or reordered execution
diverges. Reproducibility would then be a scheduling problem entangled with
performance.

**Rationale.** Counter-based Philox makes each draw a **pure function** of
`(seed, tick, rule_id, entity_id, draw_idx)`. Randomness becomes
order-independent by construction, so reproducibility reduces *entirely* to
floating-point reduction order — a separate, tractable problem. This is the
single load-bearing trick that makes determinism and GPU parallelism
compatible instead of opposed.

### E2. Three determinism levels

**Consideration.** "Reproducible" is ambiguous: same binary/same GPU? across
GPU generations? CPU vs GPU bit-identical? Each is a wildly different cost,
and forcing the strictest everywhere would cripple performance. The original
pitch wanted tiered guarantees trading replication against performance — a
good instinct, but the levels dictate the IR and scheduler, so they had to be
defined up front, not bolted on.

**Rationale.** Level A (audit: bitwise, same binary+GPU, fixed reduction
trees, sorted scatters), Level B (portable-bitwise across hardware:
software-pinned FP, no fast-math — slow, for published results), Level C
(fast: atomics allowed, same draws but FP summation jitter). Same IR, three
schedulers. Because randomness is coordinate-pure (§E1), *which* events happen
never varies across levels — only FP accumulation order does. For the policy
audit use case, Level A ("rerun the published seed, get the published
numbers") is likely sufficient, which is the cheapest strict option.

### E3. Conflict resolution via declared contested resources + argmin

**Consideration.** Synchronous parallel semantics means two transitions can
claim one entity in a tick (two employers hire one worker). Order-free writes
make "last writer wins" unavailable by design. Something principled must
break the tie deterministically.

**Rationale.** Transitions **declare the resources they contest** (checked at
elaboration — this is also where dependent types earn a wage: a transition's
claimed resources must cover its writes). Each contested resource resolves by
**argmin over sampled firing times** with a lexicographic tie-break
`(key, rule_id, entity_id)` — a segmented min-reduction, one of the most
GPU-native operations there is, and Level-A deterministic. The resolution key
is pluggable, which is how queue disciplines drop out (§F).

### E4. User-proposed race semantics adopted as the canonical merge

**Consideration.** An earlier framing offered per-field commutative merge
monoids for write conflicts — general but ergonomically heavy. The user
proposed instead "whichever transition happens first prevails."

**Rationale.** This is exactly racing clocks (§C3) applied to conflict
resolution, and it is strictly better: it unifies conflict resolution with
the stochastic semantics (one mechanism, not two), is the canonical CTMC
answer, and resolves via argmin (§E3). Adopted as *the* conflict mechanism,
with the merge-monoid framing retained only as the general backstop.

### E5. Common random numbers as a free corollary

**Consideration.** The core use case is comparing policy *designs* —
counterfactual analysis, where noise between runs can swamp the actual effect
of the design change.

**Rationale.** Coordinate-keyed Philox (§E1) gives exact CRN for free: the
same `(seed, tick, rule, entity)` yields the same draw across scenarios, so
the same simulated person experiences the same shocks under both designs —
perfectly paired counterfactuals at individual granularity, with large
variance reduction. Most frameworks bolt CRN on badly or can't; here it is a
corollary of the reproducibility design and may be the single most valuable
feature for policy work. Later extended to *parameter*-vector contrasts (§G).

### E6. The governing invariant: order-free operations only

**Consideration.** Each of the above (columnar state, coordinate RNG,
commutative aggregation, argmin conflicts, tau-leaping, uniform delay) could
look like an independent choice.

**Rationale.** They are one choice: *the semantics may only use operations
whose parallel execution is order-free or has a canonical order.* This single
invariant is what collapses "GPU performance" and "bitwise reproducibility"
from two hard problems into one. Every dynamics-layer decision is downstream
of it.

### E7. The run manifest: record the contract, don't just claim it

*(Adopted 2026-07-17 from the PFCLBS review; DESIGN.md §5.4.)*

**Consideration.** §2 states the contract as seed + IR + θ + level ⇒
reproducible results, and v0.1 genuinely delivers it — determinism tests pass,
hashes are stable. So the manifest looks like paperwork for a property already
proven, and the counter-argument was real: artifacts you don't need are
liabilities, and Sembla's whole strategy is refusing scope.

**Rationale.** The property is proven *in the test suite*, where both sides of
the equation are in scope. In the artifact a user actually keeps, only the
right-hand side survives: `sembla run --out results.csv` writes a CSV and prints
its hashes to stdout, where they evaporate. Nobody holding that CSV can say
which IR, seed, θ, or `dt` produced it. A contract nothing records is a
convention, not a contract — and reproducibility is claimed as a *semantic
property* (§1), not a lucky property of our CI.

What forced the timing rather than the decision: v0.2 runs the CPU oracle
against the GPU backend under a precision strategy ADR 0001 leaves open between
native `f64`, double-single, and a tiered path. A result that cannot name its
own precision cannot participate in that gate. One backend makes this a sidecar
file; two make it a retrofit across both. The structural rules (algorithm IDs
beside hashes, per-concern schema versions, append-only all-or-nothing tuples)
are borrowed from PFCLBS's `ReplayManifest` — which earned them across five
milestones of format evolution, at a cost we can decline to repay.

The scope boundary is explicit and holds: **one file, not an archive.** No
replay bundles, no event capture, no provenance database. Sembla records what
its own contract claims. Run management is someone else's product (§8).

### E8. Default-off flags as runtime options, recorded in the manifest

*(Adopted 2026-07-17 from the PFCLBS review; DESIGN.md §5.5.)*

**Consideration.** v0.1 has zero feature flags. Writing a flag policy for zero
flags is exactly the over-building this project keeps refusing, and the obvious
mechanism — Cargo features — is free, idiomatic, and already understood by
every Rust contributor.

**Rationale.** The mechanism choice is the part that cannot be deferred, and
Cargo features are the wrong one for a reason specific to Sembla rather than to
taste: they change the compiled surface per build and are **invisible to the run
manifest** (§E7). A flag changes what a model *means*. If a flag can be on
without the artifact saying so, then seed + IR + θ + level no longer determines
the result and the §2 contract is false — quietly, and only for the runs where
it matters. So flags are runtime options threaded through validation and
execution, and every enabled flag is recorded. PFCLBS reached the identical
conclusion from the identical constraint (replay visibility), having considered
Cargo features first.

The "no inert syntax" rule — accepted syntax is never accepted-and-ignored —
is the other half, and it is *more* binding here than at PFCLBS: §4.5 commits
every construct to a Lean meaning, so syntax that elaborates to nothing is a
lie told in the one place the project promises not to. A default-off flag is the
honest marker for "meaning is provisional here."

What we deliberately did *not* adopt is the machinery: no flag registry, no
retirement tooling, no discovery inventory. The rule plus one manifest field.
The first flag (v0.3 birth/death) validates it; the policy is written now only
because retrofitting flag-visibility onto a shipped manifest is the avoidable
version of this work.

### E9. Two execution paths, not a profile matrix

*(Adopted 2026-07-17 from the PFCLBS review; DESIGN.md §8.)*

**Consideration.** PFCLBS demonstrates a genuinely impressive specialization
framework: tree-walked, prepared, specialized, generated, and hybrid execution
paths, plus SIMD and WGSL kernels, with capability-dependent fallback and
differential gates holding them together. Every tier exists because someone
measured something. It is the most obviously enviable thing in the repository.

**Rationale.** It is also the clearest case of a cost Sembla should not buy. The
bill is not paid by the maintainers — it is paid by *users*, who must read
manifests to learn which semantics they actually ran, and by anyone trying to
tell stable defaults from feature-gated extensions. PFCLBS's own comparison
lists this as a disadvantage. More decisively: tiers are what you build when you
lack a normalized kernel IR to optimize *through*. Sembla's closed kernel
fragment (§4.2) is a bet that one narrow IR plus certified rewrites (§7)
beats N hand-specialized paths — so shipping the matrix would concede the bet
before testing it.

Two paths, oracle and GPU, held together by differential testing. The one habit
worth keeping is narrow and already taken: record which path ran and whether it
fell back (§E7). Revisit only when a single kernel IR makes a third path cheap
rather than combinatorial.

---

## F. Domain validation (queueing / courts)

### F1. Queue disciplines are conflict-resolution ordering keys

**Consideration.** Modeling people flowing through courts under different
designs needs queues: a server (judge) taken by one of many waiting cases,
under FIFO / priority / random disciplines. A naive framework needs bespoke
queue machinery.

**Rationale.** A free server is a contested resource (§E3); the queue
discipline is just the ordering key. FIFO = argmin by arrival time; priority
= argmin by (severity, arrival); random = argmin by a Philox draw. Capacity-c
(c judges) generalizes argmin to top-k — still a commutative merge, still
Level-A deterministic. The conflict mechanism built for hiring conflicts
*is* the queueing engine, validating that the semantics generalizes.

### F2. Small subsystems run exact; the operad makes it principled

**Consideration.** Courts see ~10k active cases while the population is 26M.
Tau-leaping's error and its one-event-per-resource-per-tick throughput cap
matter more in a busy queue than in a diffuse epidemic.

**Rationale.** A court box is small enough to run *exact* (sequential
Gillespie/DES on CPU, no tau-leap error) while the population box runs
tau-leaped on GPU — different solvers, different hardware, one semantics,
glued by the operad (§D1). Plus a required runtime diagnostic: count deferred
conflict losers per resource and warn on saturation, turning the tau-leap
throughput bias from a silent error into a visible one.

---

## G. Parameters and calibration (the amendment)

### G1. Parameters are first-class in the IR, never inlined

**Consideration.** The initial frontend PRD elaborated `param β := 0.3` to a
literal baked into the IR. This is fine for one run and fatal for everything
parameters exist for: calibration, prior-predictive checks, sensitivity
sweeps, and sliders all run the *same* IR under many θ. Inlining means
re-elaborating through Lean per draw (thousands of compiler invocations) and,
worse, every θ becomes a *different IR* — which silently voids the
reproducibility contract ("seed + IR ⇒ results" is meaningless when the IR
varies per draw).

**Rationale.** Parameters became first-class: a declared `params` block and an
`Expr::Param` reference form, with the run contract upgraded to
**seed + IR + θ + level ⇒ reproducible results**. θ is supplied at run time.
This was caught and fixed *before* the IR golden fixtures freeze, which is the
last cheap moment to change the contract. It is recorded as a genuine design
error, not a mere omission.

### G2. The parameter / state / input trichotomy

**Consideration.** The original "no global parameters" rule was never formally
reconciled with the obvious need for policy levers. Without a named
distinction, parameters, state, and wired inputs blur together.

**Rationale.** A **parameter** is per-run constant and read-only (the unit of
calibration, priors, sweeps, sliders); a **state** evolves during execution;
an **input** arrives on a wire. A parameter isn't a global *variable* because
nothing can write it during execution — so the no-globals principle (§D3) is
satisfied. This also gives the slider a precise definition: it edits θ in the
run configuration, never the IR and never live state.

### G3. Priors declared in the model

**Consideration.** The original pitch wanted widgets rendering prior
distributions, but nothing in the design declared a prior anywhere — so the
headline widget had no data to render.

**Rationale.** Priors are declarative metadata on parameter declarations
(`param β prior LogNormal(...)`), carried into the IR. This makes the prior a
property of the model (where it belongs), lets the structure widget render it
with zero runtime (§A2), and gives the prior-predictive sweep (§G4) its
sampling distributions.

### G4. Prior-predictive sweep as CLI plumbing, calibration method deferred

**Consideration.** Should the framework commit to a calibration algorithm
(ABC, SBI, gradient-based)? Gradient-based would reach into and constrain the
IR; the others treat the runtime as a black box.

**Rationale.** First-class parameters + a sweep runner give black-box methods
everything they need, so the *method* choice stays open (open question §10.4)
without blocking anything. The sweep itself is pure plumbing over existing
pieces: sample θ from declared priors via a *reserved Philox namespace*
(`rule_id = u32::MAX`) so parameter draws are reproducible and can never
collide with simulation draws, run the same IR per draw, collect outputs. It
composes with CRN (§E5): draws share simulation coordinates, so output
variation across draws is attributable to θ alone. Gradient-based calibration
remains deliberately deferred (§A4).

### G5. Calibration method: amortized NPE, run externally

*(Adopted 2026-07-18; resolves open question §10.4 and the ROADMAP v0.4 method
fork.)*

**Consideration.** §G4 deliberately deferred the method choice, having built
the plumbing (first-class parameters, declared priors, the sweep runner) that
any black-box method needs. The candidates: ABC (simple, but
rejection-wasteful near a tolerance), gradient-based calibration (would reach
into the IR and require the differentiable fragment §A4 keeps deferred), or
simulation-based inference with neural density estimators. Within the neural
family, sequential variants (SNPE) focus simulation effort around one observed
dataset but couple the trainer to the runner through a proposal-feedback loop;
amortized NPE trains once on prior-predictive pairs and then answers any
observation instantly.

**Rationale.** **Amortized NPE, with the workflow outside the framework.** It
is the exact consumer of infrastructure already shipped: training data is
(θᵢ, xᵢ) pairs with θ drawn from declared priors — precisely the
prior-predictive sweep — and the measured H100 throughput (~1,380 ticks/sec at
26M rows, ADR 0001) makes large training corpora cheap, which is
amortization's main cost. It never touches the IR, so §A4 stays intact. The
posterior workflow is an external Python pipeline (the `sbi` stack) fed by a
thin, versioned `(θ, x)` export beside the run manifest; Sembla stays
semantics + runtime. This re-opens standing-no #5 (calibration export formats)
explicitly and narrowly — the method being chosen was that entry's stated
condition. Sequential methods stay reachable at zero IR cost via a sweep mode
that accepts externally supplied θ draws.

Two consequences are recorded now because each is easy to get silently wrong:

1. **Training pairs need independent noise.** The sweep's CRN default shares
   simulation coordinates across draws so output variation is attributable to
   θ alone (§G4) — ideal for policy contrasts, wrong for NPE training data.
   Pairs generated under one shared noise realization teach the estimator a
   deterministic θ→x map, and the learned posterior comes out overconfident.
   NPE data generation therefore varies a per-draw replica index that enters
   the seed coordinate (§5.3 machinery); CRN mode remains the default
   elsewhere.
2. **The conditioning data `x` is the declared-summaries construct** (§4.6),
   not a parallel format: hand-declared summaries first, embedding networks
   over per-tick views later, with no IR change between the two.

A corollary for §A2's widget taxonomy: a trained amortized posterior evaluates
in milliseconds, so posterior-conditioned behavior widgets can query the flow
without re-simulating — a latency path that did not exist when behavior
widgets were gated solely on runtime speed.

---

## H. Scope and sequencing (v0.1)

### H1. Composition is the one feature that cannot be cut

**Consideration.** v0.1 must ship, so something must be sacrificed. Of the
candidates (composition, GPU, ODE blocks, birth/death, calibration, extra
determinism levels, proofs), which is load-bearing for the project's
*identity*?

**Rationale.** Composition. If the IR, runtime, and elaborator are all built
single-box, composition arrives later as a refactor of everything. Choosing
it confirms the project is a *semantics* project (composition with real
meaning), not merely a fast ABM runner (many exist). Everything else can be
added incrementally; this cannot.

### H2. Composition in minimal viable form: two boxes, one feedback wire

**Consideration.** "Keep composition" could mean building the full
wiring-diagram language and nesting UI — too much for v0.1.

**Rationale.** Exactly two boxes and one feedback wire exercise *every* piece
of compositional machinery (table-typed ports, one-tick delay, traced/
feedback structure, boundary invariance, "a composed system is a system").
If two boxes with feedback work, the operad generalizes; if they don't, no
syntax would have saved it. Composition is *proven* in v0.1 and *generalized*
in v0.2.

### H3. GPU backend cut to a throwaway spike; CPU oracle built first

**Consideration.** The GPU runtime is a headline goal, but building it in v0.1
alongside a compiler and a frontend is three projects at once. Yet the GPU
performance thesis is a real risk that shouldn't be deferred blindly.

**Rationale.** Two arguments made the CPU-first cut correct. (1) The CPU
interpreter must exist *anyway* as the differential-testing oracle for the
eventual GPU backend — it is the executable counterpart of the Lean
semantics and the backbone of the determinism story; building it first is
correct ordering, not deferral. (2) The GPU *risk* is throughput, not
compilability (the semantics is GPU-legal by construction, §E6) — and
throughput is answerable by a 1–2 week standalone benchmark (raw kernels:
26M-row map + segmented argmin + Philox) with no IR, no Lean, nothing thrown
away except the spike. So v0.1 validates the performance thesis without
building anything it would discard.

### H4. The expressiveness cliff is deliberate

**Consideration.** The restricted kernel fragment (§C1) cannot express
unbounded patterns, negative application conditions beyond anti-joins, or
within-tick recursion (transitive closure, unbounded market renegotiation).

**Rationale.** These are exactly the constructs that escape efficient columnar
compilation. Excluding them is the price of GPU compilability, paid
knowingly. A model needing them in one tick is a design smell to catch at
elaboration; the escape hatch is approximation across ticks or an opt-in slow
path. The test of whether the restriction is livable: name a model you intend
to build that the fragment can't express — none surfaced for the v0.1 use
cases.

### H5. Rust backend

**Consideration.** The runtime could be Julia (matching AlgebraicJulia),
C++, or Rust.

**Rationale.** Rust gives deterministic control over memory layout and
floating-point behavior (both load-bearing for §E), a strong story for the
eventual GPU path (wgpu/CUDA bindings), no GC pauses, and safety for a
long-lived systems codebase. Julia was declined for the runtime for the same
reasons its reproducibility story is weak; it remains a reference point for
the *frontend* alternative that lost to Lean (§A1).

### H6. Optimization = certified equivalence (the unifying thesis)

**Consideration.** Why invest in a formal semantics at all if proofs are
deferred?

**Rationale.** Because the compiler's optimizations should be *exactly* the
equivalences the theory certifies. The worked example: infection probability
depends only on the *count* of infectious coworkers, so the quadratic
self-join can be rewritten as group-by-then-broadcast (linear) — and this is
an exact lumping (bisimulation/lumpability), the *same* mathematics as
macro-level coarse-graining, showing up as a query-plan optimization whose
correctness is a statable theorem. This is the thesis that makes the Lean
investment pay rent, and it is why coarse-graining kept recurring as
load-bearing rather than decorative.

---

## I. PRD authoring decisions

### I1. Backend-first ordering (Lean appears last)

**Consideration.** The PRDs could start from the frontend (the user-facing
part) or the backend.

**Rationale.** Backend-first (IR → runtime → models, then Lean at PRD 0010)
means PRDs 0002–0009 are testable in a plain Rust environment, and the Lean
frontend's correctness is defined as *parity with already-proven fixtures*
rather than something new to validate. It also front-loads the risky
semantic core and defers the least-stable dependency (Lean toolchain).

### I2. Self-contained PRDs with restated context

**Consideration.** The implementing agent (pi-piprd) may start each PRD cold,
without the conversation's context.

**Rationale.** Every PRD restates its context and cites the DESIGN.md sections
it implements, so it stands alone. Cross-PRD invariants that must not drift
(determinism rules, crate names, rule_id assignment, the state hash, the
parameter contract, reserved RNG namespaces) live *once* in the PRDs README
and are declared binding, so they aren't re-specified (and re-diverged) per
file.

### I3. Mechanical acceptance criteria

**Consideration.** pi-piprd's review stage is another model judging work
against the criteria. Vague criteria make that loop thrash.

**Rationale.** Criteria are mechanical wherever possible: `cargo test` green,
byte-identical hashes on repeat runs, specific CLI invocations with expected
exit codes, hand-computed micro-cases with precomputed expected values. The
genuinely unautomatable bits (widget rendering) are tested at the props-data
level with documented manual steps as backup.

### I4. Load-bearing results encoded as tests, not prose

**Consideration.** The conversation's hard-won insights could be left as
design commentary.

**Rationale.** The critical properties are pinned as required tests: the
lumping rewrite (naive O(n²) vs group-by must match), bitwise boundary
invariance (two-box vs merged), CRN paired counterfactuals, θ-changes-results-
without-touching-IR, and the GPU spike must report "unanswered" rather than
pass off software-rasterizer numbers as a verdict. A property with a failing
test is real; a property in prose is a wish.

### I5. GPU spike quarantined outside the workspace

**Consideration.** The throwaway spike could live in the workspace for
convenience.

**Rationale.** It is explicitly excluded from the Cargo workspace and never
depended on, so `cargo build --workspace` never compiles it and it cannot
accidentally become load-bearing. Its only durable artifact is a RESULTS.md —
enforcing its throwaway status structurally, not just by intention.

### I6. Golden fixtures freeze the IR contract

**Consideration.** The IR's concrete JSON encoding (enum tag spelling, field
names) needs *a* definition, but over-specifying it in prose is brittle.

**Rationale.** PRD 0002 leaves tag spelling to the implementer but freezes it
with checked-in golden fixtures and round-trip tests. The first run of 0002
sets the contract; everything downstream (especially the Lean parity check in
0010) builds against it. Flagged for a human glance before 0010, because it
is the one artifact whose first draft becomes permanent.
