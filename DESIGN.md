# Sembla: Design Document

**Status:** Draft v1 — converged through an adversarial design review, 2026-07-12.
Amended 2026-07-17: §4.6 (observation as a sink), §5.4 (the run manifest), §5.5
(default-off flags), and two §8 rejections were adopted from a conservative
review of the sibling PFCLBS/SKS repository — see
[`docs/sembla-vs-pfclbs.md`](docs/sembla-vs-pfclbs.md). Each addition records a
commitment Sembla had already made but not made checkable; the rejections are
load-bearing and should be read as part of the adoption.
Amended 2026-07-18: §10.4 resolved (amortized neural posterior estimation, run
as an external workflow — see DECISIONS.md §G5), §10.5 descoped (population
generation is an external pipeline's product), §10.6 gains the
amortized-posterior path.
**Scope:** Semantics, architecture, and v0.1 definition for a compositional simulation
framework with a Lean 4 frontend and a Rust execution backend.

---

## 1. Project identity

Sembla is a **semantics-first simulation framework** for large-scale stochastic
systems, with public-policy microsimulation (e.g., an Australian synthetic
population) as the driving use case. Its distinguishing commitments, in priority
order:

1. **Composition with a real semantics.** Models are built from systems that
   compose (nest, wire, product), and composition has a mathematical meaning that
   refactoring cannot silently change.
2. **Reproducibility as a first-class semantic property**, not a runtime flag —
   with explicitly tiered guarantees trading strictness against speed.
3. **GPU-shaped by construction.** The semantics is restricted, deliberately, to
   operations whose parallel execution is order-free or canonically ordered, so
   GPU acceleration and bitwise replication are the *same* problem.
4. **A frontend (Lean 4) that doubles as the formal home of the semantics**, so
   compiler optimizations correspond to provable equivalences.

The project is explicitly *not*, in v1: a proof-verified compiler, a
differentiable simulator, or a general discrete-event engine — though the design
keeps doors open to each (see §10).

### The one-sentence thesis

> **A synchronous relational machine:** state is a typed columnar database, a
> timestep is a query, composition is wiring boxes that exchange tables, and
> randomness is a pure function of coordinates.

---

## 2. Architecture overview

```
┌─────────────────────────────────────────────────────────┐
│ Lean 4 frontend                                         │
│  • surface DSL: "systems with states and hazard         │
│    transitions" (Poly-flavored syntax)                  │
│  • elaborates to the IR (deep embedding)                │
│  • denotational semantics defined here (ground truth)   │
│  • infoview structure widgets (state diagrams, wiring   │
│    views, prior plots)                                  │
└──────────────────────────┬──────────────────────────────┘
                           │  IR (serialized, versioned)
┌──────────────────────────▼──────────────────────────────┐
│ Rust backend                                            │
│  • CPU reference interpreter = executable semantics     │
│    oracle (v0.1)                                        │
│  • GPU backend, differentially tested against the       │
│    oracle (v0.2+)                                       │
│  • per-box scheduler choice: tau-leaped parallel,       │
│    exact sequential (Gillespie/DES), ODE sub-stepper    │
└─────────────────────────────────────────────────────────┘
```

The IR is the contract: **seed + IR + parameter vector θ + determinism level ⇒
reproducible results.** Parameters are supplied per run, so calibration sweeps,
prior-predictive checks, and sliders all run the *same* IR under many θ.
The IR is frontend-agnostic by design; Lean is the intended frontend, but nothing
in the backend depends on it.

---

## 3. Why Lean (and what Lean is *not* for)

This was the most contested question in the design review. The settled position:

**Lean is chosen for two reasons, ranked:**

1. **The infoview modeling workflow.** Lean's editor integration renders
   context-dependent, interactive output for the syntactic node under the cursor,
   driven by elaborator state. No Julia or Python environment has an equivalent
   (Pluto.jl is cell-granular and notebook-shaped, not source-file /
   cursor-granular). This enables **structure widgets** — rendered from the
   elaborated model with zero runtime cost:
   - state-machine diagram of a system's states and transitions;
   - wiring/composition diagrams of how boxes connect;
   - rendered prior distributions (plots, not just parameter text).
2. **Semantic ground truth.** The DSL's denotational semantics is *defined in
   Lean* as a mathematical object (a deep embedding with a meaning function).
   This makes compiler transformations — operator fusion, the group-by lumping
   rewrite (§7), coarse-grainings, eventually symbolic gradients — into
   *statable theorems*. Proofs are deferred (and expected to get cheaper as
   AI-assisted proving matures), but the specification cost is paid in v1,
   because it cannot be retrofitted.

**Honest accounting (constraints accepted during review):**

- **Behavior widgets** (sliders → run simulation → posterior/prior-predictive
  plots) are an aspiration whose feasibility is owned by *runtime latency*, not
  by the frontend. Structure widgets are v0.1; behavior widgets are gated on the
  runtime being fast enough for an interactive loop.
- Prior *predictive* checks require running the simulation; only prior
  *marginals* are analytic. The widget taxonomy (structure vs. behavior) keeps
  this distinction explicit.
- Any formal guarantee (e.g., a verified gradient) is a theorem **about the
  ℝ-semantics** and **stops at the IR boundary**: the Rust/GPU compiler is
  trusted, not verified, and floating-point execution is not covered. Marketing
  language must respect this.
- Gradients/HMC do **not** constrain the v1 IR. They apply only to the
  differentiable fragment (ODE-like blocks, not discrete-state agents) and are
  option value, not requirements.
- Toolchain risk (widget API churn, VS Code coupling) is accepted; the
  frontend-agnostic IR is the hedge.

---

## 4. Semantics

Four layers. One design rule governs all of them: **the semantics may only use
operations whose parallel execution is order-free or has a canonical order.**

### 4.1 State: ACSets, taken seriously

Each box's state is an **ACSet** (attributed C-set): a schema of entity tables
(`Person`, `Employer`, `Household`), foreign-key columns
(`employer : Person → Employer`), and typed attribute columns
(`health : Person → {S,I,R}`, `wealth : Person → ℝ`, fixed-size vectors allowed).

- Struct-of-arrays / columnar layout **is the semantics**, not an implementation
  trick beneath it. The "indexed columnar format for performance" and the formal
  data model are the same object.
- Grids for PDE/cellular-automata models are tables whose foreign keys encode the
  lattice; compartmental (SIR-style) models are one-row tables; networks are edge
  tables. One data model, no special cases.
- **An individual is not a system; a population is.** Individuals are rows.
  The surface DSL still reads "an Individual is a system with states and
  reactions" — Poly at the surface, relational at the IR. (See §8 for why the
  pure everything-is-a-wired-system picture was rejected at individual
  granularity.)
- **Parameters are first-class and per-run constant.** The trichotomy: a
  *parameter* is fixed for a run, read-only, and supplied as a vector θ
  alongside the seed — it is the unit of calibration, priors, sweeps, and
  sliders; a *state* evolves during execution; an *input* arrives on a wire.
  The IR declares parameters (name, type, default, optional **prior**
  metadata) and expressions reference them symbolically (`Param`), never as
  inlined literals — running one IR under many θ is the operational shape of
  every inference workflow. Parameters do not violate the no-globals
  principle (§4.4): nothing can write them during execution. The slider
  workflow (§3) edits θ in run configuration; it never mutates the IR or
  live state.

### 4.2 Dynamics: a tick is a bulk relational kernel

Per tick, each box computes `(new state, output tables)` from
`(old state, input tables)`. **Double-buffered, read-old/write-new, no
exceptions** — including within a box. This uniformity is what makes box
boundaries semantically invisible (§4.4).

The kernel language is a deliberately closed fragment:

- **map** over a table — per-entity logic in a first-order, allocation-free
  expression language (the part that compiles to SPMD kernels);
- **filter**; **join on declared keys only**; **group-by/aggregate** where the
  aggregation is a commutative monoid;
- **birth/death** as stream compaction, with entity IDs allocated
  deterministically as a function of `(tick, parent, slot)`;
- **randomness** via counter-based Philox keyed by
  `(seed, tick, rule-id, entity-id)` — a *pure function of coordinates*, no
  stateful streams. Randomness is therefore order-independent by construction;
  reproducibility reduces entirely to floating-point reduction order, which the
  commutative-monoid restriction lets the scheduler canonicalize.

### 4.3 Time and stochastics: hazard rates and racing clocks

Transitions declare **hazard rates** (e.g., `hazard 0.02 / year`), not per-tick
probabilities (`with prob p` remains as sugar, desugaring via
`λ = −ln(1−p)/Δt`). Hazard rates are the native statistical dialect of policy
microsimulation (survival/duration models), and they induce the semantics:

- **Ideal meaning:** a continuous-time Markov chain (CTMC). Every enabled
  transition runs an exponential clock; the earliest firing wins.
- **Executed meaning:** **tau-leaping.** Rates are frozen at tick start; all
  transitions whose sampled firing time lands inside the tick window Δt fire;
  conflicts are resolved by argmin (§5); losers re-race next tick (no
  within-tick cascades). Discretization error is O(Δt²); **Δt is a semantic
  parameter, not just a performance knob**, and is documented as such.
- **Exact path:** sequential Gillespie/discrete-event execution is available as
  a per-box slow path (§6), used both for small subsystems (courts) and as a
  validator for the tau-leaped approximation.
- **Non-exponential durations** (needed for realistic service/processing
  times): (a) phase-type approximations (chained exponential stages — stays
  purely CTMC), and (b) **scheduled clocks**: sample a full duration from any
  distribution at stage entry (Philox at entry coordinates), store the firing
  date as an attribute, fire when reached, **re-check the guard at firing** (a
  case that settles early simply never fires its hearing — cancellation for
  free). Scheduled clocks are a generalized semi-Markov process; easier on the
  runtime than races (no staleness), weaker theory at the edges.

Sequential-random-order updating (NetLogo/Mesa style) is understood as a
discrete-time shadow of independent racing clocks; Sembla treats the CTMC as
ground truth rather than replicating legacy update orders.

**Kurtz's theorem** connects the layers: the mean-field limit of the population
CTMC is an ODE system. "26M racing agents" and "SIR differential equations" are
the same object at two resolutions, and the agent→ODE passage is a
coarse-graining *theorem*, not an analogy.

### 4.4 Composition: an operad with tables on the wires

- Boxes are Moore machines with **table-typed interfaces**:
  `S × Tbl(I) → S × Tbl(O)`, composed by wiring diagrams (operad-style: boxes
  nest within boxes; a composed system is itself a box).
- **Wires carry streams of finite tables** — not just scalars. Interface types
  are relation schemas; each tick a box emits a (possibly empty) message table;
  the receiver joins it against its own state. Individual-granular interaction
  *across* boundaries is expressible without state sharing.
- **Uniform one-tick delay everywhere** — across wires and within boxes alike
  (read-old/write-new). Consequence: **moving a box boundary never changes
  observable semantics.** Refactoring-invariance is a theorem candidate, not a
  README lie.
- Boxes may run **different schedulers on different hardware**: a 26M-person
  population box tau-leaped on GPU, a 30k-case court box run exactly
  (sequential DES) on CPU, an ODE macro block sub-stepping internally (RK4)
  and exposing sampled values per tick. The composition layer is what makes
  heterogeneous-fidelity hybrids principled.
- **No global variables.** Global-looking *mutable* quantities are inputs
  wired in; per-run constants are parameters (§4.1) — read-only, hence not
  globals. The only sanctioned globals are the synchronous tick and θ.

### 4.5 Meaning: the Lean layer

A box denotes a coalgebra (a Poly-flavored lens whose positions are
table-valued); composition is an operad-algebra structure; the ideal semantics
is over ℝ; determinism levels (§5.2) and tau-leaping are *documented deviations*
from the ideal. The IR is a deep embedding in Lean with a meaning function into
this semantics.

### 4.6 Observation: a sink, never a feedback path

What a run *reports* is part of the semantics, and therefore belongs in the IR.
Models declare named **views** (per-tick projections of committed state) and
**summaries** (scalars reduced over a run's views). The runner emits declared
observations generically; it never knows what a model *means*.

The governing invariant:

> Enabling, disabling, filtering, or serializing an observation cannot change
> state, draws, draw coordinates, conflict resolution, or any scheduling
> decision. Observation is a **sink**: there is no path in the IR from a view or
> summary back to a parameter, input, hazard, transition, or wire.

This is not a style rule. It is what makes observation *free* with respect to
the run contract (§5.4): two runs differing only in what they observed are the
same run, and their state hashes must match bitwise. It is also a statable
property of the Lean semantics — the meaning function ignores the observation
layer — and it is cheap to enforce in types rather than by reviewer vigilance.
The same invariant forces the honest converse: a quantity a model wants to *act*
on is a state or an input, and must be declared as one.

**Current status — a known violation.** v0.1's CLI branches on a hard-coded SIR
box name to decide its output columns, and `sembla sweep` refuses models that
are not SIR-shaped. One named example model is wired into the framework's
runner, which contradicts "the IR is the contract" (§2). Declared views and
summaries are the fix, and retiring that branch is their acceptance test.
Sequenced in v0.3 (ROADMAP), because n-box and birth/death models make the
SIR-shaped runner untenable rather than merely embarrassing. Promoted
2026-07-18 to start immediately: the NPE calibration decision (§10.4) makes
declared summaries the conditioning data `x`, so this now gates inference as
well as tidiness, and it is CPU-side work that proceeds alongside the v0.2 GPU
backend.

**Deliberately excluded:** event streams, paged/windowed capture, adaptive
triggers, and external streaming. Views and summaries are the whole construct
until a real model needs more.

---

## 5. Conflicts, determinism, and reproducibility

### 5.1 Conflict resolution: contested resources and racing clocks

Synchronous parallel semantics means multiple transitions can claim the same
entity/slot in one tick (two employers hire the same worker; infection and death
touch the same person). Order-free writes make "last writer wins" unavailable —
by design. Instead:

- Transitions **declare the resources they contest** as part of their signature
  (checked at elaboration time — an interface-typing obligation, and where
  dependent types earn their keep: a transition's claimed resources must cover
  its writes).
- Each contested resource resolves by **argmin over sampled firing times**, with
  a deterministic lexicographic tie-break `(time, rule-id, entity-id)` (floats
  can tie even when reals wouldn't).
- The resolution key is pluggable: **queue disciplines are ordering keys.**
  FIFO = argmin by arrival time; priority = (severity, arrival); random
  service = a Philox draw. Capacity-c resources (c judges, c beds) generalize
  argmin to **top-k selection** — still a commutative merge, still
  deterministic.
- Losers defer to the next tick. The runtime **counts deferred losers per
  contested resource and warns on saturation** — turning the tau-leap
  throughput bias (one event per resource per tick) from a silent error into a
  visible diagnostic.

### 5.2 Determinism levels

Same IR, three schedulers:

| Level | Guarantee | Mechanism | Cost |
|---|---|---|---|
| **A — audit** | Bitwise, same binary + same GPU model | Fixed-shape reduction trees, sorted scatters, Philox-by-coordinate | Moderate |
| **B — portable** | Bitwise across hardware | Software-pinned FP, no FMA/fast-math, fixed order everywhere | High; for published results |
| **C — fast** | Same random draws; FP summation-order jitter only | Atomics allowed | Cheapest |

Because randomness is a pure function of coordinates, *which* random events
happen never varies across levels — only floating-point accumulation order does.

**v0.2 GPU precision is native `f64` through CUDA.** The full-rate H100 gate in
[ADR 0001](docs/decisions/0001-gpu-precision.md) selected Strategy A after three
verified same-machine runs. CUDA had zero reduction error, winner mismatches,
fired mismatches, and unexplained fixed-tree mirror differences while exceeding
the same-machine throughput floor. The CPU `f64` interpreter remains the
semantics oracle and the numeric contract is unchanged.

The production backend is restricted to qualified full-rate NVIDIA hardware and
must preserve fixed-order two-pass reductions and lexicographic winner keys.
This makes Level A plausible on the same pinned binary and GPU model. Level B
remains unproven pending cross-hardware bitwise tests. Runtime manifests must
record CUDA, native `f64`, the exact GPU/driver, determinism level, and fallback
status. Silent fallback to wgpu, double-single, or `f32` is prohibited; any
future reduced or non-NVIDIA path requires its own explicit contract and gate.

### 5.3 Common random numbers: the counterfactual feature

Philox-by-coordinate gives **exact common random numbers across scenarios for
free**: two runs differing only in policy design share identical draws for
identical `(seed, tick, rule, entity)` coordinates. The same simulated person
experiences the same shocks under both court designs / tax schedules —
perfectly paired counterfactuals at individual level, with large variance
reduction. The same mechanism applies across **parameter vectors**: comparing
one model under θ₁ vs θ₂ with a shared seed gives paired sensitivity and
prior-predictive contrasts. For policy comparison work this may be the
headline feature; it is a corollary of the reproducibility design, not an
add-on.

**The principle extends upward.** "Randomness is a pure function of
coordinates" (§4.2) governs draws *within* a run; the same rule governs seeds
*across* runs. When a multi-run experiment (a grid, a scenario set, a
calibration sweep) assigns a seed to each run, that seed derives from a hash of
the run's **canonical semantic coordinate** — the sorted, normalized set of what
actually varies (scenario, θ assignments, replica index) — and never from the
run's positional index in the experiment. The consequence is the one that
matters: adding a case, removing a case, or reordering axis declarations leaves
every other case's seed, draws, and results untouched. An index-derived seed
silently invalidates a whole matrix the moment anyone inserts a row.

Two corollaries follow, and both are load-bearing enough to state now:
declaration order of axes and values must not survive canonicalization (permuted
specs produce byte-identical experiments), and a resumed experiment must be
byte-identical to an uninterrupted one — which is only achievable if run
identity is semantic, so timestamps, attempt counts, host paths, and output
locations can never enter a run's identity or its artifacts.

v0.1's prior-predictive sweep keys θ draws on the draw index, which is *correct
today* — for K i.i.d. prior draws the index genuinely is the coordinate, and
there is no reordering to be invariant under. The rule binds when named axes
arrive (v0.4, ROADMAP), and is recorded here so the sweep is not generalized by
accident into an index-keyed grid.

### 5.4 The run manifest: the contract, recorded

The run contract is **seed + IR + θ + determinism level ⇒ reproducible results**
(§2). A result artifact that does not record the left-hand side does not
*have* the contract — it merely hopes for it. Every run therefore emits a
**manifest** alongside its outputs, recording at minimum:

- IR hash, and the IR/manifest schema versions;
- seed, resolved θ (symbolic names to values), `dt`, tick count;
- determinism level, and — once more than one exists — the **backend that
  actually executed**, its precision representation, and whether it fell back;
- enabled feature flags (§5.5);
- final state hash and output hash;
- component versions.

Three structural rules, each cheap now and expensive to retrofit:

1. **Every hash is stored beside a named algorithm ID.** `ir_hash` travels with
   `ir_hash_algorithm`. The hash function must be able to change without
   silently reinterpreting old artifacts.
2. **Schema versions are explicit and per-concern**, not one global integer.
3. **Optional fields are append-only, and related fields form all-present-or-
   all-absent tuples that readers reject when partial.** Absence then means
   "this run predates the feature" — an honest, checkable statement — rather
   than a guess. A half-written identity tuple must fail loudly, never default.

The manifest is *the* audit surface, and it is deliberately one file, not an
archive format: no captured event streams, no replay bundles, no provenance
database. Sembla records what its own contract claims; it does not grow a
run-management product (§8).

**Why now rather than with the tooling that wants it.** v0.2's differential
testing compares the CPU oracle against the selected CUDA native-`f64` backend.
A result file that cannot name that backend, precision representation, exact
device, and fallback status cannot substantiate the selected contract. The
manifest is trivial to add while one production backend exists and awkward to
retrofit after fallback or additional backends appear.

### 5.5 Extending the semantics: default-off flags, recorded

New semantics land behind **default-off feature flags**, under three rules:

1. **A flag is a runtime option, not a Cargo feature.** Cargo features change
   the compiled surface per build, multiply the CI matrix, and — decisively —
   are invisible to the manifest. A flag must be a value threaded through
   validation and execution, so a run can *record* it.
2. **Every enabled flag appears in the run manifest** (§5.4), sorted and
   deduplicated. A flag changes what a model means; an unrecorded flag breaks
   the run contract, because seed + IR + θ + level would no longer determine
   the result.
3. **No inert syntax.** A construct the frontend accepts is never accepted-and-
   ignored. Either its flag is on and it has full elaboration, validation, and
   runtime meaning, or it is rejected with a diagnostic naming the flag that
   would enable it. Silently ignored syntax is how a semantics starts lying.

This is the mechanism that lets §4's "every construct has a Lean meaning" stay
true while the language grows: a default-off flag is the honest marker for
*meaning is provisional here*, and flag retirement — the flag becomes a no-op,
then is deleted — is the marker for a construct whose meaning has settled.

The policy binds from the first flag (birth/death — deferred to demand in the
2026-07-18 ROADMAP re-cut, but still the first flagged construct whenever it
lands). It is
deliberately a rule and a manifest field, not a subsystem: v0.1 has no flags,
and a flag registry for zero flags would be exactly the over-building this
section exists to prevent.

---

## 6. Worked domain checks

Stress tests the semantics passed during review, with the extensions they
forced (all folded into §4–§5 above):

- **Epidemic ABM** (driving v0.1 case): hazard transitions, employer-mediated
  contact via group-by, racing-clock conflicts.
- **Queueing / courts** (people flowing through court designs): CTMCs are
  queueing theory's native formalism; queue disciplines = conflict ordering
  keys; capacity = top-k; scheduled clocks + guard-recheck for non-exponential
  and calendar-driven durations; the small-scale court box runs the *exact*
  scheduler while the population box runs tau-leaped — the hybrid the operad
  layer exists for. Required diagnostic: contested-resource saturation warning.
- **Compartmental models (SIR)**: one-row tables; also reachable as the Kurtz
  mean-field limit of the agent model.
- **ODE/PDE blocks**: ODE boxes sub-step internally; PDE stencils are joins
  along lattice foreign keys. (Not in v0.1.)

**Known expressiveness cliff** (deliberate exclusions from the fast path):
unbounded match patterns, negative application conditions beyond anti-joins,
recursion within a tick (transitive closure, unbounded market renegotiation) —
approximated across ticks, or opted into a slow path. A model requiring these
in one tick is a design smell to be caught at elaboration.

---

## 7. The optimization story: compiler rewrites = certified equivalences

The pattern, established with one concrete example that is also the first
theorem target:

> `infect`: a person's infection probability depends only on the **count** of
> infectious coworkers. Naive compilation is a self-join on `employer`
> (quadratic in workplace size). The optimized plan — group-by employer,
> aggregate infectious counts, broadcast — is linear, and produces an
> **identical distribution**. This rewrite is an *exact lumping* (the same
> mathematics — lumpability/bisimulation — as macro-level coarse-graining),
> and its correctness is a statable theorem against the Lean semantics.

Planned members of the same family: operator fusion; DBSP-style incremental
recomputation (only touch agents whose inputs changed); agent→compartment
lumping when transition rates factor through a partition; Kurtz-limit
replacement of large sub-populations by ODE blocks. **Every optimization the
backend performs should correspond to an equivalence the theory can state** —
this is the thesis that makes the Lean/semantics investment pay rent.

### Theorem targets (deferred proofs, specified now)

1. Group-by lumping rewrite correctness (§7 example).
2. Refactoring invariance: re-drawing box boundaries preserves semantics
   (consequence of uniform one-tick delay).
3. Composition laws: the operad-algebra axioms for box wiring.
4. Kurtz mean-field limit for the compartmentalizable fragment.
5. (Later, if the differentiable fragment materializes) symbolic gradient
   correctness over ℝ, SciLean-style.

---

## 8. Rejected alternatives (and why)

- **Pure polynomial-functor wiring at individual granularity** ("every person
  is a box with wires"). Rejected: interaction topology is dynamic
  (contacts change, people change employers, are born, die), and wiring
  diagrams fix who-talks-to-whom before the semantics runs. Any fix (router
  systems, mode-dependent interfaces) either reintroduces a global in disguise
  or lives at the research frontier. Poly survives at the **macro** level
  (boxes = populations/modules) and in the **surface syntax**.
- **General graph rewriting** (AlgebraicRewriting/AlgebraicABMs-style DPO on
  ACSets). Adopted for the *data model* (ACSets) and as the conceptual ancestor
  for birth/death/rewiring — but general subgraph pattern matching is the
  anti-GPU workload. Sembla restricts to the relational kernel fragment (§4.2)
  that compiles to columnar kernels.
- **Sequential random-order updating as the native mode** (Mesa/NetLogo
  compatibility). Rejected in favor of CTMC ground truth; sequential updating
  is recoverable as an approximation, and exact DES is available per box.
- **Julia/Python frontend.** Rejected on two grounds that survived adversarial
  review: no infoview-equivalent (cursor-granular, elaborator-driven rendering
  in source files), and no capacity to host the formal semantics that makes
  §7 possible. The costs (toolchain churn, audience, batch latency) are
  accepted and hedged by the frontend-agnostic IR.
- **"Guaranteed gradients" as a v1 requirement.** Deferred: applies only to
  the differentiable fragment; discrete-state transitions have no useful
  gradients without relaxation machinery that is its own research field.
- **An execution-profile matrix** (tree-walked / prepared / specialized /
  generated / hybrid paths, each with capability-dependent fallback). Rejected
  for v1: it is backend proliferation before a normalized kernel IR exists, and
  its real cost is borne by users, who must read manifests to learn which
  semantics they got. Sembla keeps **exactly two execution paths** — the CPU
  oracle and one GPU backend — and pays for the difference with differential
  testing rather than with a compatibility matrix. The one habit worth keeping
  from that design is narrow and already adopted: record in the manifest which
  path ran and whether it fell back (§5.4). Revisit only when a single kernel IR
  makes a third path cheap rather than combinatorial.
- **Units and refinement types in the Rust validator.** Tempting — §4.3 writes
  `hazard 0.02 / year`, `dt` is semantic (§4.3), and a rate/`dt` unit mismatch
  is exactly the error a modeler makes. Rejected *in that location*: it puts the
  check on the wrong side of the one boundary the project is built around. Units
  are a frontend obligation, where Lean's type system can carry dimension in the
  DSL and discharge it before the IR is emitted; replicating a dimensional type
  system in the backend validator would duplicate the frontend's whole reason to
  exist and grow the IR's trusted surface. The door stays open in **Lean** (§3),
  not in `sembla-ir`.

---

## 9. v0.1 definition

**Identity check passed during review:** the one cut the project refuses is
*composition* — confirming this is a semantics project, not a runtime project.
The corresponding trade: **the GPU backend moves out of v0.1**, replaced by the
CPU oracle (needed anyway for differential testing) plus a standalone
performance spike.

### In

- Surface DSL in Lean 4 for systems/states/hazard transitions, elaborating to
  a deep-embedded IR with a defined ℝ-semantics.
- **First-class parameters with declared priors** (§4.1): symbolic `Param`
  references in the IR, per-run θ supplied at the CLI, and a
  **prior-predictive sweep runner** (`sembla sweep`) that samples θ from the
  declared priors via a reserved Philox namespace and runs the same IR per
  draw — reproducible end to end from one seed.
- Rust **CPU reference interpreter** — the executable semantics oracle.
  Level A determinism (bitwise, same binary/machine).
- Hazard-rate transitions, racing-clock (tau-leaped) execution, contested
  resources with argmin resolution and the saturation diagnostic.
- **Composition in minimal viable form: exactly two boxes and one feedback
  wire** — e.g., an SIR population box (~1M synthetic people, static employer
  assignment) wired to a small policy box that reads aggregate infections and
  feeds back a contact-rate modifier. This exercises table-typed ports,
  one-tick delay, traced/feedback structure, and "a composed system is a
  system." Composition is proven in v0.1, generalized later.
- Two **structure widgets**: state-machine diagram; prior-marginal plot.
- **GPU spike (throwaway, 1–2 weeks):** raw kernels only — 26M-row map +
  segmented argmin + Philox draws — to measure ticks/sec and validate the
  performance thesis before the real backend is built.

### Out (deliberately, each deferred not forgotten)

General wiring-diagram language and box nesting UI · GPU backend (v0.2,
differentially tested against the oracle) · ODE/PDE blocks · birth/death ·
courts/queueing extensions (scheduled clocks, top-k capacity) · determinism
Levels B/C · calibration/inference · behavior widgets (slider→simulate→plot) ·
any proofs (theorem *statements* only) · synthetic population realism.

### Success criteria

1. A model written in the Lean DSL compiles to IR and runs end-to-end.
2. Same seed + same IR + same θ ⇒ bitwise-identical results, run after run;
   a changed θ changes results without touching the IR.
3. The two-box feedback loop produces correct, boundary-invariant results
   (verified by merging the boxes by hand and comparing bitwise).
4. The GPU spike reports a credible ticks/sec at 26M rows.
5. The state-diagram widget renders from the elaborated model with no runtime.

---

## 10. Open questions (flagged, not resolved)

1. **Conflict-scope declaration syntax** — how a transition's contested
   resources are written and checked; how coverage of writes is enforced.
2. **Δt discipline** — guidance/diagnostics for choosing tick size per box;
   automatic detection of tau-leap bias beyond the saturation counter.
3. **Level B feasibility** — how expensive portable-bitwise FP really is on
   modern GPUs; whether it's practical or aspirational.
4. **Calibration/inference architecture** — *resolved 2026-07-18*
   (DECISIONS.md §G5): **amortized neural posterior estimation (NPE)**, run as
   an **external Python workflow** (the `sbi` stack) fed by a thin, versioned
   `(θ, x)` export beside the run manifest. Sembla's side stays black-box:
   first-class parameters plus the sweep runner (§9) generate the training
   pairs; nothing reaches into the IR, and the gradient path stays deferred
   (§3). Still open within this choice: the summary-statistic selection
   (declared summaries (§4.6) first; embedding networks over per-tick views
   later), and exactly how the per-draw replica index enters the seed
   coordinate so training pairs carry independent noise (§5.3 — CRN remains
   the default for policy contrasts).
5. **Synthetic population initialization** — *descoped 2026-07-18*. Population
   generation (census/HILDA-style microdata, reweighting, privacy, validation)
   is an external data pipeline's product, consumed through the versioned
   population format the runtime already reads. Historically >50% of the
   effort in policy microsimulation — which is exactly why it is now an
   explicit non-goal with a format contract at the boundary, rather than an
   unowned assumption.
6. **Behavior-widget latency budget** — what interactive loop is achievable
   once the GPU backend exists (scenario caching? surrogate models?). The NPE
   decision (§10.4) adds a third path: an amortized posterior evaluates in
   milliseconds once trained, so posterior-conditioned widgets can query the
   trained flow without re-simulating; live prior-predictive bands still need
   the runtime or a surrogate.
7. **Cross-boundary tick-delay ergonomics** — one-tick message delay is
   uniform and honest, but modelers must be taught to see it.

---

## 11. Key references

- Harry Goldstein, *The Best New Programming Language is a Proof Assistant*
  (DC Systems 006) — Lean-as-PL and widgets motivation.
- Lean 4 widgets / ProofWidgets4 — infoview architecture.
- Spivak & Niu, *Polynomial Functors: A Mathematical Theory of Interaction*;
  Spivak's wiring-diagram operads — macro-level composition semantics.
- Kris Brown et al., Topos Institute: *Agent-Based Modeling via Graph
  Rewriting* (2023); AlgebraicJulia (ACSets, AlgebraicRewriting.jl,
  AlgebraicABMs.jl) — data model and conceptual ancestor of the dynamics.
- DBSP / differential dataflow — incremental relational computation theory.
- Gillespie (SSA), tau-leaping literature — stochastic execution semantics.
- Kurtz — mean-field limits of density-dependent Markov chains.
- Salmon et al., *Parallel Random Numbers: As Easy as 1, 2, 3* — Philox
  counter-based RNG.
- SciLean (Tomáš Skřivan) — verified symbolic differentiation in Lean
  (deferred gradient path).
- Futhark / DEX — the per-entity expression-language compilation model.
