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

### A7. Command-style mathematical syntax is the human surface

*(Adopted 2026-07-20; implementation record in
`docs/design/surface-syntax-options.md`.)*

**Consideration.** The original `model%` syntax was a dependable semantic
kernel, but its nested category lists, keyword-marked references, and quoted
wire names made complete models read like construction data rather than
mathematics. Any replacement still had to preserve literal JSON bytes, stable
rule/effect order, positioned diagnostics, legacy models, and the
frontend-agnostic IR boundary.

**Rationale.** Human-authored models use indentation-structured `sembla_model`
commands with real parameter bindings and tilde priors (B), reaction arrows
(A), and keyed `freq` notation (C(ii)). These are layered adapters to one
surface-model elaboration kernel: migration is accepted only when the emitted
IR is byte-identical. Runtime model/table names derive through a frozen
snake-case and Greek-transliteration contract, with explicit `(name := "…")`
overrides and collision diagnostics. `model%` remains the supported
compatibility/kernel form and direct IR constructors remain the machine-writer
path. Keyed comprehensions with row binders (C(i)) are deferred until a real
model requires them; a do-notation builder (E) is rejected/deferred for human
authoring because imperative construction is not the mathematical surface.

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

### G6. Parameter-sampler transcendentals are software-pinned

*(Adopted 2026-07-19.)*

**Consideration.** CI's first cross-platform run found a one-ULP difference in
a frozen LogNormal draw between macOS/aarch64 and Linux/x86_64. Platform C
libraries do not promise identical `ln`, `cos`, or `exp` results, while θ
portability is load-bearing for the NPE workflow: training pairs generated on
a GPU host must contain exactly the same θ as the corresponding development
sweep.

**Rationale.** The cold parameter-draw path uses the pure-Rust `libm` crate,
pinned to an exact version, for `ln`, `cos`, and `exp`. This is narrow
Level-B-style software pinning where its cost is immaterial; `sqrt` remains the
standard-library operation because IEEE-754 square root is exactly rounded.
The dependency policy gains exactly one approved entry, `libm`, alongside
`sha2`, and a source guard prevents new platform-backed transcendental method
calls. A `libm` version bump is therefore a frozen-vector-breaking change.

The source audit also found one pre-existing `f64::ln` in `rng::exp_f64`, used
by result-bearing simulation racing clocks, contrary to the PRD's
authoring-time scope statement. Pinning simulation transcendentals is outside
this sampler-only change, so the guard records that exact call as its sole
documented exemption rather than changing simulation outputs.

The workspace gate contains two Rust legacy goldens that embed sampled θ bytes.
This change regenerates exactly the affected draw and parameter-manifest
fixtures so the sampler change remains independently testable; the broader
Python/NPE fixture audit, reference-artifact regeneration, and cross-platform
CI proof remain the downstream fixture PRD's responsibility.

**Breaking change.** θ draws from sweeps generated before this commit are not
bit-reproducible after it. No compatibility shim is provided; downstream
fixtures are regenerated explicitly where their owning change requires it.

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

### I7. Allowed-file lists are derived from the acceptance criteria (2026-07-28)

**Decision.** A PRD's allowed-file list is written by walking its own acceptance
criteria and asking, for each, which file must change to satisfy it. It is not
written from where the author expects the code change to land.

**Alternatives.** Relying on §M2's carve-out to repair lists mid-run is
rejected: it works, but each repair costs a stopped run and up to five wasted
attempts. Dropping the allowed-file restriction is also rejected — it is what
stops an implementation quietly widening its own scope, and it has caught real
cases.

**Reason.** Four runs have now stalled on the same defect, and in every case the
implementation was complete and passing every functional criterion:

- `prds-evaluator-throughput/0001` — the baseline moved mid-run (§M2's original
  case);
- `prds-cuda-host-path/0001` — the list omitted `main.rs`, but retaining a
  `StateStore` across ticks required changing an owned-value API that crosses
  into the CLI;
- `prds-device-observation/0002` — the list omitted the differential-corpus
  runner, while §5 required removing the CUDA rejection of grouped views and §6
  required adding the grouped model to the corpus. That script is where both
  live, and it *asserted* the rejection §5 removes, so the PRD was
  self-contradictory as written.

The common cause is not carelessness about files. It is that the author reasons
about the *change* and then writes the file list from that mental model, while
the acceptance criteria demand more: a corpus entry, a registry addition, a
fixture, an API the change necessarily crosses. **The criteria are the
specification; the file list must be derived from them, not from the diff the
author imagines.**

A useful check: for every criterion that says *add*, *register*, *cover*, or
*record*, name the file that owns that list and confirm it is present.

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

## J. Composition and the Option D architecture (accepted 2026-07-21)

### J1. Option D pipeline accepted

**Decision.** Composition uses serialized composition source → one canonical
Lean 4 linker → a versioned flat executable plan → Rust validation and
execution. The first release is limited to the architecture document's Phases
0–4: decisions, the plan envelope and stable identity, composition source, the
linker with product, wires, and nesting, specification statements, surface
syntax, and the artifact bundle. Recursive runtime hierarchy is rejected
because it would force hierarchy into validation, addressing, scheduling,
execution, hashing, reporting, and fixtures while risking distinct flat and
hierarchical semantics. A free composition AST as the executable contract is
also rejected because every runtime and backend would need an interpreter for
generality beyond the concrete model needs. Algebraic structure remains at the
source layer, while execution has one canonical flat contract.

### J2. Hash algorithm

**Decision.** All new hashes use SHA-256 with domain separation:
`SHA-256(domain-string ++ 0x00 ++ payload)`. Persisted hashes are records
`{algorithm: "sha256", domain, digest}` with lowercase hex digests. blake3 is
rejected because `sha2` is already the approved manifest dependency and
`scripts/check.sh` enforces the dependency policy. One algorithm and explicit
domains keep every persisted digest interpretable without adding a new
cryptographic dependency.

### J3. Stable identity grammar

**Decision.** A **slug** is `[a-z][a-z0-9_]*` in ASCII, matching existing
runtime snake_case names, with no leading digit or underscore. A **stable
declaration ID** is `<kind>:<slug>` with the prefixes `model:`, `def:`,
`inst:`, `port:`, `wire:`, and `expose:`, for example `def:population`, `inst:north`,
`port:infection_count`, and `wire:count_to_policy`. A **transition local ID**
is the transition's existing runtime `name` slug, which is already stable and
referenced by `fired:` columns. An **occurrence ID** is `occ:` followed by the
slash-joined chain of instance-ID slugs from the root definition: depth 1 is
`occ:population`, nested examples are `occ:epidemic/population` and
`occ:north/population`, and the root definition itself is the empty chain
`occ:`. Occurrence chains are built from instance declaration IDs, never
display names and never traversal positions. No identity is ever derived from
a display name or a traversal position. A **transition occurrence
identity** is `<occurrence-id>#<transition-name>`, for example
`occ:population#infect` and `occ:north/population#infect`. A **wire occurrence
identity** is `<owner-occurrence>#wire:<wire-slug>`, for example
`occ:#wire:count_to_policy` for a root-owned wire and
`occ:north#wire:count_to_policy` for the same wire inside instance `north`. A
**mailbox identity** is
`mbox:<wire-occurrence>|<source-occurrence>.<port-slug>|<target-occurrence>.<port-slug>`;
including both endpoints disambiguates fan-out. A **plan leaf name** is the
occurrence chain slugs joined by `/`, such as `population` or
`epidemic/population`; because slugs contain no `/` or `.`, it cannot collide
with existing `box.table.attr` report naming, and display names remain
non-semantic source-map data.

### J4. RNG strategy (doc open question 2 resolved)

**Decision.** The Philox coordinate layout
`[tick, rule_word, entity_id, draw_idx]` is unchanged. For versioned plans the
`u32` rule word is content-addressed by the following frozen construction:

```text
rule_word = big-endian u32 of the first 4 bytes of
  SHA-256( "sembla.rule-word/v1" ++ 0x00 ++ transition-occurrence-identity )
```

Words equal to `u32::MAX - 1` or `u32::MAX`, which are reserved sweep/prior
namespaces, and any collision between two accepted identities are
deterministic link/validation errors; identities are never reassigned. A
persisted next-free registry is rejected because it is history-dependent and
would let unrelated insertion order affect identity. Widening the coordinate
is rejected because it would change the RNG format rather than preserve the
existing Philox contract.

### J5. Canonical JSON (`sembla.canonical-json/v1`)

**Decision.** Every new composition source, executable plan, and bundle
manifest uses UTF-8 with no BOM, no insignificant whitespace, and no trailing
newline, so file bytes are exactly the canonical bytes. Object keys are sorted
by byte-wise lexicographic order. Optional absent fields are omitted, never
`null`, and related optional fields form all-present-or-all-absent tuples whose
partial forms readers reject. Plan arrays use canonical order by stable
identity—leaves by occurrence path, transitions by `(leaf, name)`, and
wires/mailboxes by identity string—while source collections preserve author
order. Strings escape only `"`, `\`, and control characters
(`\b \t \n \f \r`, otherwise `\u00xx` lowercase), with every other character
literal UTF-8, matching serde_json-style escaping. Integers use plain decimal
without leading zeros or `+`; non-integer numerics use the existing Lean
canonical model writer's exact conventions so values such as `dt: 0.25`
round-trip through `serde_json` with its enabled `float_roundtrip` behavior.
Versioned parsers reject unknown fields rather than assigning them inert or
best-effort meanings.

### J6. Version strings

**Decision.** The required version strings and hash domains are frozen as
follows and are part of the artifact contract.

| Concern | String |
|---|---|
| Composition source schema | `sembla.composition-source/v1` |
| Executable plan schema | `sembla.executable-plan/v1` |
| Linker semantics | `sembla.linker/v1` |
| Stable identity scheme | `sembla.identity/stable-v1` |
| Legacy identity scheme | `sembla.identity/legacy-positional-v1` |
| Canonical encoding | `sembla.canonical-json/v1` |
| Source map schema | `sembla.source-map/v1` |
| Hash domains | `sembla.source-artifact/v1`, `sembla.plan-core/v1`, `sembla.plan-envelope/v1`, `sembla.bundle-root/v1`, `sembla.rule-word/v1` |

Unknown or missing required version strings are always rejected with a
deterministic error, never interpreted by best effort. `required_features` and
`enabled_features` must be present and exactly `[]` in V1; any entry is a
deterministic rejection naming the feature.

### J7. Plan origins and the legacy path

**Decision.** Versioned plan envelopes have exactly two origins in V1:
`linked` and `direct_stable`. Unversioned model JSON, including everything
currently in `examples/`, is the envelope-free `legacy` path. That path keeps
dense declaration-order rule IDs under the scheme name
`sembla.identity/legacy-positional-v1`. Its behavior remains byte-identical
forever and it is never silently upgraded. The architecture document's
`normalized legacy` origin is deferred rather than accepted into V1. Keeping
legacy outside the versioned envelope prevents a migration label from changing
an existing compatibility contract.

### J8. Parameter bindings (doc open question 5 resolved)

**Decision.** An instance binds each component parameter requirement to a
model-level parameter name, never a literal. Binding two instances to the same
model parameter is explicit sharing. Distinct per-instance values require
distinct model parameters. Literal bindings and an implicit per-instance
parameter namespace are rejected because they would change parameter
resolution and sharing semantics. The existing θ, `Param` resolution, priors,
and sweep behavior are unchanged.

### J9. `dt` and scheduler domains (doc open question 6 resolved)

**Decision.** Components never declare `dt`; the root composition declares
`outer_dt`. V1 has exactly one scheduler domain, `domain:global`, using the
algorithm `tau_leap` and containing every leaf. Per-component time steps and
heterogeneous scheduler domains are rejected because they require scheduling
semantics outside this release. One root time step preserves the existing
runtime's single global scheduling contract.

### J10. Composition laws are byte-equality (doc open question 4 resolved)

**Decision.** Plan collections are sorted by stable identity, so product
associativity, product symmetry, alpha-renaming, and declaration-permutation
laws are byte-equality of canonical plan cores. Isomorphism-only laws are
rejected because canonical ordering supplies a stronger mechanical contract.
The plan **semantic** hash uses domain `sembla.plan-core/v1` and covers exactly
`{schema_version, identity_scheme, model, identity}`. It excludes `origin` and
`linked_provenance`, and source maps and display names never enter this hash.
The **envelope** hash uses domain `sembla.plan-envelope/v1` and covers the whole
envelope. This separation lets equivalent linked and direct-stable cores share
semantic identity while their complete provenance-bearing artifacts remain
distinguishable.

### J11. Hashes live outside the plan

**Decision.** Plan files never embed their own hash records. Hashes are
computed over exact canonical file bytes and recorded in run manifests and
bundle manifests. Embedding a plan's own digest and the architecture document
§5.3 self-reference rules are rejected because they make the hashed value
self-referential. External recording keeps the canonical plan bytes and their
integrity records unambiguous.

### J12. Deferred constructs

**Decision.** Synchronized transition families, `Share`/`Identify`, semantic
invariants and constrained products, observational assertions, heterogeneous
schedulers, explicit adapter/merge components, dynamic component topology,
ACSet storage or any Julia dependency, non-Lean source producers, and any
non-Lean linker are all out of scope for this release. They are deferred rather
than admitted as partially implemented alternatives. Under DESIGN.md §5.5's
no-inert-syntax rule, every one must be rejected with a deterministic error if
it appears in an artifact. No deferred construct may be accepted and then
silently ignored.

### J13. Observation quotient and proof obligations (2026-07)

**Decision.** Composition V1 observational equivalence includes every field of
the observation contract: leaf and mailbox state, external outputs, firing
order, Philox draw coordinates, and named scalar observations. Hashes remain
consequences of canonical artifacts rather than observations, consistent with
§J10. The independent static source and plan denotations are executable-checked
across the linkable fixture corpus. Full behavioral preservation is
stated-deferred under the project proof policy. Per the architecture document
§14.3, rollout remains gated on the executable preservation checks until that
behavioral proof is completed.

### J14. Composition integration (2026-07-22)

1. **CUDA keying.** **Decision.** The CUDA backend follows the same split PRD
   0004 of the composition track gave the CPU runtime: the content-addressed
   `rule_word` is used wherever a rule identity enters a Philox counter or an
   ordering/tie-break key; the dense `rule_id` ordinal remains for indexing,
   buffer layout, codegen specialization, and diagnostics. **Alternatives.**
   Dense positional words for RNG and content-addressed words for indexing are
   rejected. **Reason.** Legacy models have `rule_word == rule_id`, so legacy
   CUDA behavior is bit-identical — the existing differential goldens prove it.

2. **GPU evidence discipline.** **Decision.** PRDs in this folder split
   acceptance into **local criteria** (must pass in the managed run without a
   GPU: compilation, corpus listing, graceful skips, legacy goldens unchanged)
   and **hardware criteria** (recorded in the runbook/evidence conventions,
   executed manually later). A PRD is approvable on local criteria alone;
   hardware criteria must be *listed* in its implementation notes as pending.
   **Alternatives.** Requiring unavailable GPU hardware for approval or
   presenting the stub workflow as evidence is rejected. **Reason.** Hosted CI
   has no GPU, so the local-versus-hardware acceptance split keeps approval
   honest without hiding later hardware obligations.

3. **Mixed identity schemes never compare.** **Decision.** `compare` rejects a
   legacy model in one arm and a plan envelope in the other with a deterministic
   error. **Alternatives.** Silently pairing or normalizing mixed arms is
   rejected. **Reason.** CRN pairing across the legacy/stable identity boundary
   is meaningless and must not be silently computed.

4. **Plan sweeps carry the plan tuple, not `ir_hash`.** **Decision.** A sweep
   over a plan envelope records the existing `PlanIdentityTuple` (and
   `LinkedSourceTuple` when origin is `linked`) in its manifests via the
   existing `plan_identity_tuples` helper; the legacy `ir_hash` field stays
   absent. **Alternatives.** Reusing `ir_hash` or inventing a parallel plan-
   identity field is rejected. **Reason.** The existing tuples are the canonical
   plan identity contract, and legacy sweeps are byte-unchanged.

5. **No `--dt` for plans anywhere.** **Decision.** `--dt` overrides never apply
   to plans (already enforced for `run`; the same rejection extends to
   `diff-backends`). **Alternatives.** Mutating a plan envelope at the CLI is
   rejected. **Reason.** A plan is edited and re-linked/re-canonicalized, never
   mutated at the CLI.

6. **`--all-plan-fixtures` is a sibling flag.** **Decision.**
   `diff-backends --all-examples` keeps its exact current behavior; a new
   `--all-plan-fixtures` flag walks `fixtures/plans/*.plan.json` and
   `fixtures/plans/linked/*.plan.json`. The corpus runbook runs both.
   **Alternatives.** Giving `--all-examples` a new meaning is rejected.
   **Reason.** A sibling flag preserves the legacy corpus contract while making
   the plan corpus explicit.

7. **Structure-widget-only scope.** **Decision.** The composition widget is a
   structure widget in the DESIGN.md §3 taxonomy (zero runtime cost, rendered
   from elaborated values). **Alternatives.** Behavior widgets, simulation
   calls, and new frontend dependencies beyond the existing ProofWidgets stack
   are rejected. **Reason.** Widgets render structure only; integration adds no
   runtime semantics.

## K. Demographic slot modeling (accepted 2026-07-23)

### K1. Fixed-pool slot architecture

**Decision.** Demographic turnover (births, deaths, and migration) is modeled
inside a fixed table of reusable person slots. A row is vacant or present;
entries activate vacant rows and exits vacate them. Person identity is
`(slot ordinal, generation)`: the row ordinal is the permanent slot ID and
Philox entity coordinate, never a person ID, and `generation` increments on
each activation. Capacity exhaustion is an explicit observed failure reported
through saturation diagnostics, never a silent condition.

**Alternatives.** Dynamic row allocation is rejected. The roadmap's
stream-compaction birth/death design is not selected and remains deferred under
its original demand trigger.

**Reason.** Reusing a bounded slot pool preserves stable Philox coordinates and
makes turnover and exhaustion explicit without introducing dynamic-allocation
or compaction semantics.

### K2. Age representation

**Decision.** Age is represented as `age_months : Int` and advanced by a
deterministic monthly transition. Mutable age and an authoritative birth date
are never stored together without a checked consistency contract.

**Alternatives.** Derived age through `Expr::Tick` plus `birth_month_index` is
deferred. PRD 0009's benchmark must first show that ageing-write cost is
material before `Expr::Tick` is designed as a flagged semantic construct across
validation, CPU, CUDA, and reproducibility.

**Reason.** The mutable representation has defined semantics using existing
constructs, while the cross-cutting cost and contract of a tick expression are
not justified without measurement.

### K3. State artifact format

**Decision.** State artifacts use `sembla.state/v1`: the 12-byte ASCII
`SEMBLA_STATE` magic, a canonical-JSON header, and raw little-endian column
blobs matching runtime `ColumnData`. The hash domain is
`sembla.state-artifact/v1` over exact file bytes. The artifact contains no
execution metadata. For artifact-loaded runs, every declared `rows :=` count is
enforced exactly rather than treated as a size hint.

**Alternatives.** Self-describing execution metadata, non-canonical headers,
converted column encodings, trailing data, and best-effort row-count repair are
rejected.

**Reason.** A byte-frozen state format can be validated and hashed
unambiguously while manifests remain the sole owners of execution provenance.

### K4. Chained annual runs, not checkpoints

**Decision.** Standing-no #6 remains: there is no run-management or replay
subsystem. Annual calibration windows are separate runs chained by hashed state
artifacts and recorded with `initial_state` and `exported_state` manifest
tuples. A chained 12+12 run pair is explicitly not bitwise-equal to a continuous
24-tick run because tick coordinates restart; this distinction is documented
and test-asserted.

**Alternatives.** Checkpoint/restart semantics and claims that chained and
continuous runs are interchangeable are rejected.

**Reason.** Artifact chaining supplies the required annual boundary and
provenance without adding hidden continuation state or contradicting the
coordinate-based execution contract.

### K5. Rate heterogeneity via loaded columns and per-run θ

**Decision.** Per-slot age, sex, and area rate multipliers are `Real` attribute
columns supplied in the initial state and referenced by hazards. Annual rate
variation is represented by per-run θ across chained runs.

**Alternatives.** A time-indexed rate-table construct is rejected for now; no
customer under the selected monthly/annual design requires it.

**Reason.** Loaded columns and existing parameters express the required
heterogeneity and annual updates without adding a new time-indexed lookup
semantics.

### K6. `grouped-observations` is the first §5.5 feature flag

**Decision.** `grouped-observations` is a runtime option threaded through
validation and execution and recorded in sorted order in manifests. Models that
use grouped views without the flag are rejected with a diagnostic naming it.
The plan validator's known-feature set grows from empty to
`{"grouped-observations"}`; this is the one sanctioned revision to §J's
"exactly `[]`" rule, and unknown features still reject. Composition sources may
not carry grouped views in this track. Grouped observation is a sink, extending
the §4.6 invariant mechanically. CUDA supports grouped count observations when
every key axis is exactly boundable: Enum axes use schema cardinality, Ref axes
use the target table's runtime row count, and banded Int axes use a device
min/max reduction over the column. One dense histogram is limited to 1,048,576
counters; a larger exact product rejects deterministically with the qualified
view name, computed size, and limit rather than falling back. CUDA preserves
the host's underlying `i128` key values and declaration-axis lexicographic
order, and it omits zero-count groups.

**Alternatives.** A Cargo feature, inert syntax, unrecorded enablement,
composition-source support, guessed or declared Int bounds, zero-count output,
and implicit CUDA fallback are rejected.

**Reason.** The first provisional-meaning construct exercises §5.5's complete
flag contract while preserving observation non-feedback and explicit backend
limits. Exact runtime bounds keep grouped device observation within the closed
commutative-monoid fragment without changing its semantics.

### K7. Contest surface syntax, `race_time` only

**Decision.** `contest <ref-attr> by race_time` in a transition body lowers to
the existing `ResourceClaim` and `ClaimOrdering::RaceTime` IR. This resolves
DESIGN.md open question §10.1 for the race-time case.

**Alternatives.** Keyed orderings and queue disciplines remain in v0.5 scope
and are not exposed by this track.

**Reason.** Race-time contests already have complete IR and runtime meaning, so
the surface can expose them without prematurely choosing the keyed-ordering
language.

### K8. Surface gaps closed without flags

**Decision.** Arithmetic `set` effects and `Int` parameter declarations expose
IR/runtime capability present since v0.1: `Effect::SetAttr` accepts a full
`Expr`, and `ParamType.int` exists. They require no feature flag.

**Alternatives.** Treating these surface omissions as provisional semantics or
placing them behind default-off flags is rejected.

**Reason.** Under §5.5's rationale, flags govern constructs whose meaning is not
yet settled, not syntax for semantics the system already defines.

### K9. Deferred constructs and their triggers

**Decision.** The folder's deferred list and triggers remain verbatim:

> `Expr::Tick`/derived age (trigger: PRD 0009 measures ageing-write cost as
> material); categorical draws, cross-row writes, mother-linked births,
> vacant-slot claiming, non-exclusive `Ref` reassignment, household refs
> (trigger: aggregate model shows identity linkage is scientifically required
> → design-options note first); paired migration events/quotas (trigger:
> reported balance residual unacceptable → Option D Phase 6); keyed contest
> orderings (v0.5); event-stream sinks; sub-annual rate tables.

CUDA grouped-observation support is discharged by the device-observation
follow-up under §K6's exact-boundability and deterministic 1,048,576-counter
limit; it is no longer deferred. The household-identity trigger produces a
design-options note, not PRDs, as its first artifact. Option D Phase 6 is the
synchronized-family path for paired migration.

**Alternatives.** Half-building any deferred construct, admitting inert syntax,
or advancing one without its named trigger is rejected.

**Reason.** Named evidence and design triggers keep aggregate-model scope from
silently acquiring individual-linkage, synchronization, event-stream,
time-table, or backend semantics.

### K10. Aggregate-model interpretation caveats

**Decision.** The birth-activation hazard is a rate per eligible vacant slot,
not a fertility hazard, and must not be interpreted as one without an explicit
scaling derivation. The one-tick event-marker lockout—new entrants are
ineligible for events while their marker persists—is a documented and measured
model trade-off counted by PRD 0007, not a framework bug. National internal-
migration balance holds only in expectation; the residual is always reported
and never silently reconciled.

**Alternatives.** Fertility interpretation without scaling, hiding the lockout,
and silently forcing exact national migration balance are rejected.

**Reason.** These caveats distinguish aggregate approximation choices from
framework semantics and keep their measurable consequences visible.

## L. CUDA validation parallelism (accepted 2026-07-25)

### L1. The defect is validation, not execution

**Decision.** The generated CUDA simulation kernels are parallel. Four
validation kernels (`sembla_validate_claims`, `sembla_validate_transition`,
`sembla_validate_effects`, and `sembla_validate_outputs`) instead execute a
per-row loop on a single thread. Their cost is O(rows) serial per claim and per
fallible expression per tick. The defect is validation, not execution.

**Alternatives.** Attributing the slowdown to host/device transfer, to `f64`
arithmetic, or to the model's rule count is rejected. Each is contradicted by
the measurement that SIR at 26M rows on the same GPU class runs at ~1,380
ticks/sec while generating none of these loops.

**Reason.** The emitted source and the sustained 100% `utilization.gpu` reading
(which reports kernel residency, not occupancy) jointly identify a serial
kernel, not a stall.

### L2. Validation remains a separate pass

**Decision.** Per-row validation is parallelised in place. It is not fused into
the execution kernels that already visit each row.

**Alternatives.** Fusion, which is faster in principle, is rejected.

**Reason.** Fusion entangles two independent concerns for a speedup not required
to clear the §L4 gate, and the CPU oracle keeps them separate. Divergence in
structure makes differential reasoning harder for no gain now.

### L3. Failure reporting is order-independent by construction

**Decision.** Validation reports the minimum failing candidate index, computed
by parallel reduction, not the first writer.

**Alternatives.** Reporting any failing candidate, or making the reported index
depend on launch configuration, is rejected.

**Reason.** Diagnostics are part of the observable contract compared by the
differential harness. A diagnostic that varies with block count breaks Level A
determinism as surely as a differing state hash.

### L4. The gate is "worth using", not a throughput target

**Decision.** The track succeeds when CUDA at the frozen case is at least **3×
faster than the same host's CPU** on the same model, binary, commit, seed, and
state artifact.

**Alternatives.** A ticks/sec target and parity with SIR throughput are rejected.

**Reason.** The decision the roadmap needs is whether the GPU is worth using for
this model class. An absolute target invites tuning beyond the question being
asked.

### L5. Grouped observations stay CPU-only

**Decision.** Unchanged from §K6 and §K9, grouped observations stay CPU-only.
This track admits the demographic model to the differential corpus in its
no-grouped configuration only.

**Alternatives.** Opportunistically adding CUDA grouped support here is
rejected.

**Reason.** It is a separate deferred construct with its own follow-up folder.
Bundling it would hide a semantic change inside a performance fix.

### Frozen benchmark case

Later PRDs in this track must use this case unchanged:

```text
model:    fixtures/demographic/benchmark/demographic_slots.no-grouped.json
scale:    10,000,000 slots
ticks:    24
seed:     9009
areas:    4      present fraction: 0.8
streams:  birth:600,overseas:250,internal:150
command:  scripts/bench-demographic.sh --scales 10000000 --ticks 24 --seed 9009
```

Both arms run on one host in one session. Replicates: **three per backend**, and
the reported figure is the median; a single run is not evidence for a gate (the
ageing-share readings this year show why).

### L6. §L4 verdict at PRD 0002: not met, and the diagnosis was wrong (2026-07-26)

**Decision.** The §L4 gate is **not met**. Measured on one Hyperstack host
(H100 PCIe, AMD EPYC 9554, commit `dbc665f`, one shared 10M state artifact, one
release binary): CUDA `5647.9s`, CPU `434.6s` for 24 ticks of the no-grouped
model — CUDA is **13.0x slower** than the same host's CPU, against a gate
requiring 3x faster. Single replicate per arm: the protocol was stopped after
replicate 1 once the magnitude made further replicates uninformative, so this is
recorded as a **measured verdict, not §L4 gate evidence**.

**PRD 0002 was correct and irrelevant.** An `nsys` kernel profile at 500k rows
shows its four parallelised kernels (`validate_claims`, `validate_transition`,
`validate_effects`, `validate_outputs`) now consume **0.0%** of GPU time. They
were genuinely serial and are genuinely fixed. They were never the bottleneck.

**The measured distribution** (500k rows, 2 ticks, 99.9% of GPU time):

| Kernel | Share | Character |
|---|---:|---|
| `sembla_check_candidate_errors` | 37.7% | single-threaded; walks the candidate array (rules x rows) |
| `sembla_prepare_effects` | 33.7% | single-threaded; two per-row loops |
| `sembla_resolve_conflicts` | 28.5% | **already parallel** — slow for another reason |

**Alternatives rejected.** Closing the track (the cause is now identified and
addressable); proceeding with PRD 0006 as drafted (it targets only
`prepare_effects`, one of three); and treating this as §L4 gate evidence (one
replicate).

**Reason.** §L1 attributed the cost to the four validation kernels on the
strength of emitted-source structure plus a sustained 100% `utilization.gpu`
reading. Both observations were real; the inference from them was not tested
before a PRD was scoped, implemented, and measured on rented hardware. §L1 is
**superseded** by this measured distribution.

**Consequent rule.** No CUDA performance PRD is scoped without a kernel profile
first. Profiling this case cost about fifteen minutes and one dollar and would
have prevented the entire 0002 cycle. Structural reasoning about which kernel
dominates is a hypothesis, not a finding.

Note also that cost is **superlinear** in rows: 2.7 s/tick at 500k against
235.3 s/tick at 10M, i.e. 87x cost for 20x rows. The 10M kernel distribution may
therefore differ from the 500k profile above, and the rewritten PRD must profile
at the scale it intends to fix.

### L7. §L4 verdict after PRD 0006: not met at 2.56x, and the bottleneck has moved off the GPU (2026-07-26)

**Decision.** The §L4 gate is **not met**. Three replicates per backend on one
host (commit `206900c`, one shared 10M artifact, one binary): CUDA median
`171.2s`, CPU median `438.7s`, ratio **2.56x** against a required 3x. Spreads
were 5.3% and 2.8%, so this is a measured miss rather than a noisy one.

**PRD 0006 succeeded regardless.** 33.0x faster than the pre-fix `5647.9s`, with
**byte-identical output hashes** — the segmented argmin selects the same winners
as the quadratic scan, satisfying §E3. `resolve_conflicts` now scales linearly.

**The bottleneck is no longer the GPU.** At 5M rows over 2 ticks, all GPU
kernels together account for **9.1 ms of 10,620 ms** of wall time — 0.09%. Of
the CUDA API time, 93.8% is `cuMemcpyDtoHAsync` device-to-host readback; the
remaining ~96% of wall time is host-side work outside CUDA. The post-fix kernel
ranking is flat, with no kernel above 11.5%.

**Alternatives rejected.** Lowering the gate to fit 2.56x — it was set at "worth
using" deliberately, before any result was known, and moving it now would make
it meaningless. Continuing CUDA kernel optimisation — at 0.09% of wall time
there is nothing left to win there. Closing the CUDA track as unviable — 2.56x
faster is the right side of parity and the remaining cost is addressable.

**Reason.** The gate asked whether the GPU is worth using for this model class.
The measured answer is "yes, moderately, and the reason it isn't more is not the
GPU". That is a more useful finding than a pass would have been.

**Consequent direction.** The next work is host-side profiling, which is free and
local — no GPU required. PRD 0007's kernel parallelisation, while correct and
merged, is not expected to move any measurable number.

### L8. §L4 verdict after the host-evaluator track: met at 4.21x, with no GPU change (2026-07-27)

**Decision.** The §L4 gate is **met**. Three replicates per backend on one host
(commit `917d930`, one shared 10M artifact, one binary, the unchanged frozen
protocol): CUDA median `31.82s`, CPU median `133.86s`, ratio **4.207x** against
a required 3x. Spreads were 6.1% and 0.2%. All seven collector assertions pass,
including byte-identical results, summaries, and execution hashes across both
backends and every replicate. Evidence:
`docs/evidence/demographic-bench/hyperstack-l4-20260726T140326Z/`.

**No GPU code changed between §L7 and this measurement.** CUDA went `171.2s →
31.8s` (5.4x) and CPU `438.7s → 133.9s` (3.3x) entirely from
`docs/prds-host-evaluator-performance` PRDs 0001–0004: resolving column
references once per column rather than once per row, the same for `Ref`
columns, and computing per-tick state hashes only when a consumer reads them.

**Why the ratio moved when both backends improved.** §L7 established that ~96%
of CUDA wall time was host-side work. The CUDA path pays that cost *in addition
to* its device work, so removing it helps CUDA proportionally more than CPU.
This is the mechanism §L7 predicted, now confirmed at the frozen scale.

**Where CUDA time now goes.** Per-phase instrumentation at 5M rows over 2 ticks
(`docs/prds-execution-timing` PRD 0001) attributes what §L7 could only record as
unaccounted: `state_reconstruct` **45.8%**, `observe_views` 20.2%,
`state_transfer` 13.9%, `readback_control` 7.8%, `report` 7.4%, `other` 4.3%,
`kernels` **0.56%**. Kernel time is `9.4ms` of `1674ms`, essentially unchanged
from §L7's `9.1ms` — the share rose only because the denominator shrank.

`state_reconstruct` is `unpack_state` + `StateStore::new` rebuilding the full
host state every tick so host-side observation has something to read. It is
host allocation, not PCIe: transfer is a third of its cost.

**Alternatives rejected.** Raising the gate now that it passes — the same
argument §L7 used against lowering it applies symmetrically, and a bar moved
after seeing the result is not a bar. Treating 4.21x as the ceiling — three
named costs above it remain addressable.

**Reason.** The gate asked whether the GPU is worth using for this model class.
The answer is now yes, by the criterion set before any result was known.

**Consequent direction.** Gate-clearing no longer justifies further work; the
remaining items must be argued on their own merits. `state_reconstruct` is the
largest and is a local, bit-identical change. Note also that further CPU-side
optimisation now *lowers* this ratio while improving the product, so §L4 has
served its purpose and should not be used to steer work from here.

**Recorded, not decided.** The ageing cost share measured **0.328** median
(0.321 / 0.328 / 0.330), materially above earlier measurements and well above
§K2's 10% threshold. §K2 is not revisited here.

### L9. §L4 verdict after the throughput tracks: not met at 1.91x, and the gate has outlived its question (2026-07-27)

**Decision.** The §L4 gate is **not met**. Three replicates per backend on one
host (commit `00389a7`, one shared 10M artifact, one binary, the unchanged
frozen protocol): CUDA median `26.37s`, CPU median `50.48s`, ratio **1.914x**
against a required 3x. All seven collector assertions pass, including
byte-identical results across both backends and every replicate. Evidence:
`docs/evidence/demographic-bench/hyperstack-l4-20260727T120050Z/`.

**Nothing regressed. Both backends got substantially faster.**

| | §L7 | §L8 | §L9 |
|---|---:|---:|---:|
| CUDA median | 171.2s | 31.82s | **26.37s** |
| CPU median | 438.7s | 133.86s | **50.48s** |
| ratio | 2.56x | 4.207x | **1.914x** |

Since §L8, CUDA improved 1.21x and CPU improved 2.65x. The gate fell because it
measures a *ratio*, and the slower backend improved faster.

**This is the failure §L8 predicted in writing.** It recorded that "further
CPU-side work now *lowers* this ratio while improving the product, so §L4 has
served its purpose and should not be used to steer work from here." Seven PRDs
in `prds-evaluator-throughput` then did exactly that.

**PRD 0001 of `prds-cuda-host-path` succeeded and is verified on hardware.** At
5M rows over 2 ticks: wall `1674.4ms -> 936.1ms` (**1.79x**), with
`state_reconstruct` `766.8ms -> 220.7ms` (**-71%**) and `state_transfer` flat at
`232.6ms -> 239.6ms`. That flat transfer is the control the PRD specified: it
confirms the gain came from removing host allocation, not from anything changing
about the device copy. `observe_views` also fell `338.9ms -> 93.5ms`, inherited
from the CPU evaluator work because it runs on the host in both backends.

**Kernels are 9.3ms of 936.1ms — 1.0%.** Unchanged in absolute terms across
§L7, §L8 and §L9. The GPU has never been the constraint and still is not.

**Alternatives rejected.** Treating this as a regression — nothing got slower and
the product improved by every absolute measure. Lowering the bar to fit 1.91x —
§L7 rejected that reasoning when the gate failed and §L8 rejected its mirror
when the gate passed; a bar moved to suit the result is not a bar. Reverting
CPU-side work to restore the ratio — that would make the system slower to satisfy
a number.

**Reason.** §L4 asked whether the GPU is worth using for this model class. It
has now answered that question three times with three different verdicts while
the underlying answer — yes, and the host is what limits it — never changed. A
criterion whose verdict inverts because unrelated work improved is measuring the
wrong thing.

**Consequent direction.** §L4 is **retired as a steering criterion**. It is not
re-run by default and no PRD should cite it as justification or success. Absolute
per-run wall time, and the per-phase attribution from `--timing-json`, replace it.
Should a successor gate be wanted, it must be an absolute target — "50M slots in
under N seconds" — not a ratio between two implementations that are both moving.

**Recorded, not decided.** The ageing cost share measured **0.402** median
(0.392 / 0.402 / 0.408), the third consecutive rise: 0.122, then 0.328, now
0.402, against §K2's 10% threshold. §K2 is not revisited here, but three
measurements moving one way is now worth its own look.

### L10. Ungrouped CUDA observation is gated conservatively from the IR (2026-07-27)

**Decision.** A CUDA run may observe ungrouped views on the device and omit its
per-tick state download only when every declared view is positively recognised
as either a filtered `Count` or filtered `Min`/`Max` over an `Int` row-local,
infallible expression. The row-local decision reuses the evaluator's existing
predicate. Any `Expr::Agg` or `Expr::Input`, grouped view, `Sum`, unrecognised or
row-fallible expression, or model with no scalar views forces complete host
observation and state download. Eligibility is computed once from validated IR
and reported per run and per view.

`Sum` over `Real` stays in canonical ascending row order. `Sum` over `Int` stays
sequential because reassociation can overflow where the canonical pass does not.
Real extrema are excluded: the CPU uses its specified total ordering, while
ordinary floating-point min/max have asymmetric NaN behavior. Counts and Int
extrema are commutative monoids within the existing closed CUDA fragment and
return only one scalar per view.

**Alternatives.** Benchmark-name checks, partial device observation when one
view still needs host state, per-row result download, extending the CUDA
fragment, and implicit backend fallback are rejected. Grouped views remain
unchanged under §K6 and §K9.

**Reason.** The all-or-nothing IR gate preserves exact observation semantics
while removing transfers only when no host consumer needs the per-tick state.
The existing CPU/CUDA differential comparison remains the executable safety
argument.

### L11. Device-side observation verified on hardware: CUDA 1.87x, CPU flat (2026-07-28)

**Decision.** The §J14.2 hardware criteria for `prds-device-observation/0001`
and `0002` are **satisfied**. Both compile with `--features cuda`, both produce
byte-identical output against the CPU oracle, and the per-tick state download is
gone in both the no-grouped and grouped configurations. Evidence:
`docs/evidence/demographic-bench/hyperstack-l4-20260728T072119Z/`, commit
`04ada45`, one host, one session, all six collector assertions passing.

| | §L8 | §L9 | **§L11** |
|---|---:|---:|---:|
| CUDA median | 31.82s | 26.37s | **14.10s** |
| CPU median | 133.86s | 50.48s | **49.07s** |
| ratio | 4.207x | 1.914x | **3.480x** |

**CUDA improved 1.87x while CPU stayed flat at 1.03x.** That asymmetry is the
result, not a side effect: the work removed host cost that only the CUDA path
was paying, so the CPU arm had nothing to gain and gained nothing.

Phase attribution at 5M rows over 2 ticks, against §L9's baseline:

| phase | §L9 | no-grouped | grouped |
|---|---:|---:|---:|
| `state_transfer` | 239.6 | **0.0** | **0.0** |
| `state_reconstruct` | 220.7 | **0.0** | **0.0** |
| `observe_views` | 93.5 | **0.0** | **0.0** |
| `readback_control` | 206.3 | 193.1 | 197.0 |
| `report` | 119.9 | 141.8 | 119.5 |
| `other` | 46.9 | 21.2 | 21.4 |
| `kernels` | 9.3 | 10.4 | 13.6 |
| **wall** | **936.2** | **366.5** | **351.4** |

Three phases are exactly zero, not merely reduced: the download is skipped, not
made faster. **The grouped and no-grouped totals are within run-to-run noise of
each other**, so device-side grouped observation costs approximately nothing —
against 1,335ms of host `observe_views` for the same work on CPU. The
configuration the calibration workflow actually uses is the one where the GPU
wins hardest, and it was rejected outright before `0002`.

**§L4 reads MET at 3.480x and remains retired.** §L9 retired it as a steering
criterion and that stands. The ratio rose because the CUDA arm improved while
CPU held still, so this time the number and the product moved together — but
that is which backend happened to be worked on, not the gate becoming
informative. No PRD should cite 3.480x as justification.

**Correction to the projection method.** A linear extrapolation from the 5M/2-tick
profile predicted 12.7s for the 10M/24-tick case; the measurement is 14.10s,
**11% low**. The cause is superlinearity between 5M and 10M, not larger startup.
`readback_control` + `report` were 34.8% of the §L9 profile and are **91.4%** of
this one, and those are exactly the phases that allocate a fresh
multi-hundred-megabyte host buffer per tick — 400MB/tick at 10M, where
`alloc_spike` measured page-fault effects up to 4.04x. **Linear extrapolation
from a 5M profile now understates larger cases, and increasingly so with scale.**
Treat projections above 10M as optimistic.

**Recorded, not decided.** The ageing cost share measured **0.411** median
(0.404 / 0.411 / 0.413), a fourth consecutive rise: 0.122, 0.328, 0.402, now
0.411, against §K2's 10% threshold.

**Consequent direction.** `readback_control` is now 53% of CUDA wall time and
`report` 33%, together 91% — and both exist to move and then count the `wins`
and `deferred` buffers, 200MB per tick at 5M, to produce at most 13 integers of
purely diagnostic output. That is the next PRD.

### L12. Parallel validation deadlocks on hardware under a multi-thread launch geometry (2026-07-28)

**Decision.** `prds-cuda-validation-parallelism/0002` is **defective on
hardware** and its §J14.2 criteria are not satisfied. The generated
`sembla_record_validation_failure` acquires a spin lock:

```cuda
// codegen.rs:2795
while (atomicCAS(status + 4, 0ULL, 1ULL) != 0ULL) { }
```

On an H100 (sm_90) the differential corpus passes `claim_key_overflow` at launch
geometry `1x1` and **hangs indefinitely** at `1x32` — one full warp contending
for the lock. Observed for 2h31m at 100% GPU utilisation and 489MiB before the
run was killed; the same suite completed in 23.04s on 2026-07-19 before this
code existed. `git log -S` attributes the construct to `8feb168`. Evidence:
`docs/evidence/cuda-validation-deadlock-20260728/`, which also carries the
23-second reproduction.

The in-source comment asserts the pattern is safe because independent thread
scheduling is available on sm_70+. ITS is present and the pattern still hangs,
so **the comment states a necessary condition as if it were sufficient**.

**Alternatives.** Widening the launch geometry to avoid intra-warp contention —
that hides the defect and the corpus exists to find it. Removing the geometry
sweep from the corpus — it is the only thing that caught this. Raising a test
timeout — a deadlock is not slowness.

**Reason.** The fix direction is to select the reported failure by pure atomics
with no critical section, as the surrounding `atomicMin`/`atomicMax` extrema
already do, rather than to make the lock work.

**This is the §J14.2 split proving itself.** `0002` was approved on local
criteria with hardware criteria listed pending, and this was its first execution
on a GPU. A defect that no host test could reach was found by the first hardware
run, on the first case, in the first geometry that contends. **The reproduction
costs 23 seconds; the discovery cost 2.5 hours of GPU time only because nothing
had run it.** Corpus coverage is now automated behind `BENCH_CORPUS=1` so this
cannot go unrun again.

**Local correction verdict (2026-07-28).** The mutex word is now a reduction
phase, and every logical validator runs four stream-ordered passes: minimum
scan, minimum ordering identity within that scan, minimum branch within that
prefix, then payload recovery by the exact winning key. The generated source
contains no `atomicCAS(status + 4, ...)` loop or critical section; the unrelated
monotone i64 extrema CAS reductions remain unchanged. Multi-block, multi-warp
`4x128` joins the diagnostic geometry corpus, whose ignored lib test is bounded
by a killable child-process deadline. Local gates establish code generation and
CPU-oracle invariants only. The §J14.2 hardware verdict remains pending the
runnable `BENCH_CORPUS=1 bash run-demographic-benchmark.sh` command.

## M. Performance methodology

### M1. Optimisation is scoped from direct measurement, not from profile shares (2026-07-27)

**Decision.** No performance work in this project is scoped from a profiler's
percentages alone. A candidate change must first be measured directly — a
hand-written arm computing the same result by the proposed method, asserted
equal to the current one — and it is that measurement, not a profile share, that
justifies a PRD. Profiles remain the right tool for *finding* candidates and the
wrong one for *sizing* them. Measurement on one model shape is not sufficient
evidence for a constant applied to every model: such constants require at least
two materially different shapes, reported separately rather than averaged.

**Alternatives.** Continuing to scope from profile shares, which is cheaper and
needs no throwaway code, is rejected. Requiring a full implementation before
committing to one is also rejected: the point is a falsifiable measurement, not
a finished change.

**Reason.** Three failures, each expensive, and each with the same shape.

The CUDA folder scoped two PRDs from source structure and both picked the wrong
target (§L6, §L7). Then `prds-host-evaluator-performance` PRD 0004 removed a
provable 8 MB-per-operand copy: the allocator symbols in the profile duly
dropped ~20%, and the measured runtime did not move at all. Two later spikes
explained why and generalised it — first-touch page-fault time is attributed to
the code writing the memory rather than to `malloc`, so profiles *systematically
understate* allocation cost, and `alloc_spike` found 4.04× available where the
symbols suggested ~14%.

The corollary is that a negative result is worth paying for. `bitset_spike` and
`narrow_spike` each cost minutes and each closed off a plausible PRD, and
`rng_batch_spike` established that a bottleneck was irreducible rather than
badly arranged. A measurement that says "do not do this" is as valuable as one
that says "do this", and much cheaper than discovering it in review.

A screen that can only pass is not a screen: where a spike judges quality rather
than speed, it must include a control expected to fail. `rng_variants_spike`'s
deliberately weakened variant failed by ~40×, which is the only reason to
believe the screen could reject anything.

**Consequence.** Spikes are committed, runnable, and cited by the PRDs they
justify — see `crates/sembla-runtime/examples/` and `docs/performance-model.md`.
They are not implementations and carry no acceptance obligations, but they are
evidence and are kept.

### M2. The baseline is frozen for the duration of a managed run (2026-07-27)

**Decision.** While a `/piprd run` is in progress, the operator does not commit
to the branch it is running on. A PRD's allowed-file restriction is enforced by
diffing HEAD against the baseline recorded at run start, so any commit the
operator lands mid-run appears to the reviewer as the implementation exceeding
its scope. Work that cannot wait is done on another branch and merged after the
run stops.

Relatedly, a PRD may **not** be failed for a defect in its own baseline. Where a
required gate fails on clean HEAD, the correct outcome is to stop and report it,
not to expand scope or to loop. PRDs in performance folders now say so
explicitly in their allowed-files section.

**Alternatives.** Having the runner re-derive its baseline, or having reviewers
attribute commits by author, are rejected as tooling changes to compensate for
an operator error that is free to avoid.

**Reason.** It happened. A run of `prds-evaluator-throughput/0001` started at
23:40:26 UTC; at 00:29:58 a commit landed on `main` fixing a quality-gate
failure in eight spike examples — files outside that PRD's allowed list. Every
subsequent review correctly flagged a scope violation, and the run exhausted all
five attempts on a blocker no in-run action could clear, because the offending
commit was not the implementer's to revise. The implementation itself had passed
every other criterion on the first attempt and was lost.

This is the second failure of the same shape. §L4's quiescence requirement, as
first written, demanded that no agent session be active — a condition a run
cannot create from inside itself — and stalled a run at five attempts before
being made advisory. **An acceptance criterion that no in-run action can satisfy
is a defect in the criterion**, and both the runner and the operator should treat
repeated identical blockers as evidence of one.

**Amended 2026-07-27, after a third failure of the same shape.** A run of
`prds-cuda-host-path/0001` found that the PRD's allowed-file list made the PRD
unachievable: the change required touching a file the list omitted. The runner
asked, the operator authorised the exception, and the implementer acted on it —
but the authorisation was never written into the PRD. Every subsequent review
correctly found a prohibited file, and the run exhausted its attempts, because
neither reverting authorised work nor editing the PRD was available to the
implementer.

So this decision's freeze has one carve-out. **An out-of-band authorisation must
be written into the PRD before the run continues.** Recording an approved scope
exception is not moving the baseline; it is the operator completing an
authorisation they have already given, and deferring it guarantees the run
fails. The operator makes that edit and only that edit.

The pattern across all three is one thing: **a reviewer enforcing a written
specification cannot be argued with by an implementer.** Where the specification
is wrong, only the operator can fix it, and must do so promptly rather than at
the end of the run.

**Extended 2026-07-28: the freeze covers the working tree, not just commits.**
A fourth run stalled because the operator committed `DECISIONS.md` between
attempts to record an unrelated decision, and `git add` swept up §K6/§K9
amendments the implementer had left uncommitted. The PRD required those
amendments to share a commit with the code change; half of them were now in the
operator's pushed commit, so the requirement had become unsatisfiable.

So: **before committing anything between attempts, check what is actually in the
working tree.** An implementer's uncommitted work is part of the run. Stage
specific paths rather than whole files, and never assume a file you edited
contains only your edit.

And a note on criteria of this shape: **requirements about commit packaging are
fragile and rarely worth their cost.** "Both present at HEAD" states the real
goal — the decision record must not contradict the code — without creating a
condition that an operator's ordinary activity can destroy.


### M3. A pending hardware criterion must name the command that runs it (2026-07-28)

**Decision.** A PRD that defers a criterion under §J14.2 must name the exact
command that will execute it, and that command must exist and be reachable from
the session tooling. **If no automation exists, building it is in scope for the
PRD that defers the criterion**, not for a later unnamed session.

**Reason.** §J14.2 makes deferral honest but not bounded. It requires hardware
criteria to be *listed* as pending; nothing required them to be *runnable*, and
a criterion that is listed and unrunnable is indistinguishable, from the record,
from one that passed.

It happened. `crates/sembla-cuda/scripts/run-differential-corpus.sh` existed
well before 2026-07-19, and `prds-device-observation/0002` extended it with the
grouped configuration. **No collector ever invoked it.** Several PRDs carried
"CPU/CUDA differential equality" as hardware-pending across three GPU sessions
with nothing anywhere that would run it. It was finally run on 2026-07-28, only
because the collector was taught to, and it found a deadlock (§L12) on the first
case of the first geometry that contends.

So the corpus had been unrun for long enough that a hang could be introduced,
approved, and carried through three sessions without detection. The defect was
not hard to find; nothing was looking.

**Alternatives.** Relying on the operator to remember — three sessions did not.
A runbook checklist item — the module README already described the corpus and it
still never ran; documentation does not execute. Requiring hardware for approval
— §J14.2 rejected that for good reasons that have not changed.

**Consequent.** `BENCH_CORPUS=1` now runs the corpus from the collector, before
the gate, aborting it on disagreement. Hardware criteria in this repository name
a runnable command or they are not deferred.

### M4. A check is not trusted until it has been made to fail (2026-07-28)

**Decision.** A verification added to the collector, the payload, or a PRD's
acceptance tooling must be demonstrated to fail on a deliberately perturbed
input before its passing result is cited as evidence.

**Reason.** A check that cannot fail is worse than no check, because it emits a
passing result that is then quoted in a decision record.

It nearly happened, in the same session and in the check written to verify §L11.
The first version of the grouped CPU/CUDA parity comparison compared
`profile-grouped-cuda.csv` against `profile-grouped-cpu.csv`. **Neither file
contains any grouped data.** Grouped views are written to
`<stem>.grouped.<view>.csv` sidecars (`main.rs:2202`), so the comparison would
have passed unconditionally, on every run, and printed "grouped CPU/CUDA outputs
identical" — the exact sentence the PRD needed and the one nobody would have
re-derived. It was caught by running it against a byte-perturbed copy and
requiring it to report a mismatch.

**Alternatives.** Careful reading — that is how the bug was written in the first
place, by someone who had just read the code that names the sidecars. Reviewing
the check's output — it produces the correct-looking output either way, which is
precisely the failure mode.

**Note.** This generalises §M1. §M1 says do not infer a result from a proxy;
this says do not infer a *check's validity* from its passing. Both are the same
error at different levels: accepting a number without establishing what would
have changed it.
