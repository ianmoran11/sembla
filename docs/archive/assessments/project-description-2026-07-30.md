# Sembla: Project Description

> **Archived snapshot — 2026-07-30.** This comprehensive description records the project at a named commit. The maintained summary is [`../../overview.md`](../../overview.md).

**Status:** Descriptive summary. Written 29 July 2026 and revised 30 July 2026, against branch `main` at commit `133f87b`.

**Language:** This document uses ASD-STE100 Simplified Technical English. The sentences are short. The voice is active. Each technical name has one meaning. Section 11 gives the technical names.

**Purpose:** This document tells you what Sembla is, what its models mean, how you write a model, how the software runs a model, and what comes next. It does not change any rule of the project.

---

## The short version

Sembla is a simulation framework for large stochastic systems. It has a Lean 4 frontend and a deterministic Rust runtime. Its driving use case is public-policy microsimulation.

One rule controls the project:

> **seed + IR + parameter vector + determinism level = reproducible results.**

The framework exists because a large policy simulation is hard to trust. Two runs can differ. Two machines can differ. Moving a piece of the program can change the answer. Sembla removes all three by construction, not by care. Randomness is a pure function of coordinates, so evaluation order cannot change which events occur. Every operation is order-free or has one canonical order, so parallel execution and bitwise replication become the same problem. Every box reads old state and writes new state, so a box boundary is invisible to the meaning of a model.

**What works today.**

| Part | State |
| --- | --- |
| Lean surface language, composition, linking, bundles | Built |
| Deterministic CPU interpreter, the semantics oracle | Built |
| CUDA backend with native 64-bit floating point | Built |
| `run`, `sweep`, `compare`, `verify-run`, `diff-backends` | Built |
| Views, grouped views, summaries, run manifests | Built. Grouped views run on the CPU only |
| Determinism Level A, same binary and same machine | Built and tested by repetition |
| Structure widgets in the Lean infoview | Built |
| One proof: the grouped-count rewrite | Built, at the level of the specification |

**What is designed and not built.** Birth and death, ODE blocks, scheduled clocks, queue disciplines, lookup and rate tables, categorical draws, a per-box clock, atomic multi-row events, calibration by neural posterior estimation, and behaviour widgets. Each has a design and a named trigger. None is scheduled without its trigger.

**The honest caveat.** Sembla can tell you what a model means and how a run was produced. It cannot tell you that the model is a good description of the world. The demographic driver model is a software fixture today: nothing in it is empirically initialised, fitted, or validated. A reproducible result is not a valid result. Section 8 gives this in full.

**Where to go next.** Section 2 gives the semantics. Section 3 gives the language. Section 6 walks one model from a Lean file to a verified result. Section 8 gives the limitations. If you read one section, read section 8.

## Contents

- [The short version](#the-short-version)
- [How to read the project documents](#how-to-read-the-project-documents)
- [1. Philosophy and rationale](#1-philosophy-and-rationale)
    - [1.1 The problem](#11-the-problem)
    - [1.2 The run contract](#12-the-run-contract)
    - [1.3 The four commitments](#13-the-four-commitments)
    - [1.4 The one-sentence thesis](#14-the-one-sentence-thesis)
    - [1.5 Why the project uses Lean 4](#15-why-the-project-uses-lean-4)
    - [1.6 What Sembla is not](#16-what-sembla-is-not)
    - [1.7 The driving use case](#17-the-driving-use-case)
- [2. The semantics of the simulation framework](#2-the-semantics-of-the-simulation-framework)
    - [2.1 The governing rule](#21-the-governing-rule)
    - [2.2 State: the ACSet](#22-state-the-acset)
    - [2.3 The three kinds of quantity](#23-the-three-kinds-of-quantity)
    - [2.4 What one tick does](#24-what-one-tick-does)
    - [2.5 Time and hazard rates](#25-time-and-hazard-rates)
    - [2.6 Randomness](#26-randomness)
    - [2.7 Conflicts](#27-conflicts)
    - [2.8 Composition](#28-composition)
    - [2.9 Observation](#29-observation)
    - [2.10 Determinism levels](#210-determinism-levels)
    - [2.11 Identity](#211-identity)
    - [2.12 The expressiveness cliff](#212-the-expressiveness-cliff)
    - [2.13 The whole picture](#213-the-whole-picture)
- [3. The Lean 4 syntax and the frontend features](#3-the-lean-4-syntax-and-the-frontend-features)
    - [3.1 What the frontend is](#31-what-the-frontend-is)
    - [3.2 The model command](#32-the-model-command)
    - [3.3 Names](#33-names)
    - [3.4 Parameters](#34-parameters)
    - [3.5 Systems and attributes](#35-systems-and-attributes)
    - [3.6 Transitions](#36-transitions)
    - [3.7 Aggregates](#37-aggregates)
    - [3.8 Ports, views, wires, and summaries](#38-ports-views-wires-and-summaries)
    - [3.9 The composition commands](#39-the-composition-commands)
    - [3.10 The structure widgets](#310-the-structure-widgets)
    - [3.11 The frontend tools](#311-the-frontend-tools)
    - [3.12 The tests](#312-the-tests)
    - [3.13 The proofs](#313-the-proofs)
- [4. Seeing a model: the widgets and the visual guide](#4-seeing-a-model-the-widgets-and-the-visual-guide)
    - [4.1 Why the pictures are free](#41-why-the-pictures-are-free)
    - [4.2 The three panels](#42-the-three-panels)
    - [4.3 Themes](#43-themes)
    - [4.4 What the tests cover](#44-what-the-tests-cover)
    - [4.5 The visual guide](#45-the-visual-guide)
    - [4.6 What is missing](#46-what-is-missing)
- [5. The backend implementation](#5-the-backend-implementation)
    - [5.1 The workspace](#51-the-workspace)
    - [5.2 The IR and the validator](#52-the-ir-and-the-validator)
    - [5.3 The state store](#53-the-state-store)
    - [5.4 The CPU oracle](#54-the-cpu-oracle)
    - [5.5 The tick pipeline](#55-the-tick-pipeline)
    - [5.6 The CUDA backend](#56-the-cuda-backend)
    - [5.7 The command-line interface](#57-the-command-line-interface)
    - [5.8 The run manifest](#58-the-run-manifest)
    - [5.9 The measured performance state](#59-the-measured-performance-state)
    - [5.10 The process machinery](#510-the-process-machinery)
- [6. From a Lean file to a verified result](#6-from-a-lean-file-to-a-verified-result)
    - [6.1 The six stages](#61-the-six-stages)
    - [6.2 Which input form to use](#62-which-input-form-to-use)
    - [6.3 The other workflows](#63-the-other-workflows)
    - [6.4 What each stage guarantees](#64-what-each-stage-guarantees)
- [7. How the project knows a result is correct](#7-how-the-project-knows-a-result-is-correct)
    - [7.1 The oracle defines the answer](#71-the-oracle-defines-the-answer)
    - [7.2 The nine layers](#72-the-nine-layers)
    - [7.3 One proof](#73-one-proof)
    - [7.4 What the checks do not cover](#74-what-the-checks-do-not-cover)
    - [7.5 The method that produced the numbers](#75-the-method-that-produced-the-numbers)
- [8. Limitations, and how to read a result](#8-limitations-and-how-to-read-a-result)
    - [8.1 The distinction that matters most](#81-the-distinction-that-matters-most)
    - [8.2 The demographic model is a fixture](#82-the-demographic-model-is-a-fixture)
    - [8.3 What the framework itself cannot express today](#83-what-the-framework-itself-cannot-express-today)
    - [8.4 The open-risk register](#84-the-open-risk-register)
    - [8.5 How to state a result honestly](#85-how-to-state-a-result-honestly)
- [9. The second driver: the justice pipeline](#9-the-second-driver-the-justice-pipeline)
    - [9.1 Why a second driver exists](#91-why-a-second-driver-exists)
    - [9.2 The model shape](#92-the-model-shape)
    - [9.3 What Sembla already has for it](#93-what-sembla-already-has-for-it)
    - [9.4 Where the two drivers agree and disagree](#94-where-the-two-drivers-agree-and-disagree)
    - [9.5 The staged plan](#95-the-staged-plan)
    - [9.6 Two findings already recorded](#96-two-findings-already-recorded)
- [10. Future features](#10-future-features)
    - [10.1 How the project chooses work](#101-how-the-project-chooses-work)
    - [10.2 Performance work, next](#102-performance-work-next)
    - [10.3 Semantics that are specified but not built](#103-semantics-that-are-specified-but-not-built)
    - [10.4 Inference and interactive widgets](#104-inference-and-interactive-widgets)
    - [10.5 Empirical data work](#105-empirical-data-work)
    - [10.6 Assurance and diagnostics](#106-assurance-and-diagnostics)
    - [10.7 Authoring and consolidation](#107-authoring-and-consolidation)
    - [10.8 The proof track](#108-the-proof-track)
- [11. Technical names used in this document](#11-technical-names-used-in-this-document)

---

## How to read the project documents

The repository has many documents. Some of them are authorities. Some of them are records. Use this order.

| Document | What it is |
| --- | --- |
| `DESIGN.md` | The design authority. It gives the semantics and the scope. |
| `DECISIONS.md` | The decision record. Sections A to M. Each entry gives a decision, the alternatives, and the reason. |
| `docs/archive/roadmaps/version-roadmap-2026-07-25.md` | The version milestones v0.2 to v1.0. |
| `docs/archive/roadmaps/forward-roadmap-2026-07-25.md` | A second roadmap of record. It gives the two driver models and the staged work tracks. |
| `docs/prds-*/` | The work specifications. One folder for each track. Each folder has a `README.md` index. |
| `docs/evidence/` | The measured results. Each directory holds the data for one measurement. |
| `docs/decisions/0001-gpu-precision.md` | The architecture decision record for GPU precision. |

If two documents disagree, `DESIGN.md` and `DECISIONS.md` win. The two roadmaps do not have authority over each other. An amendment must be explicit.

---

## 1. Philosophy and rationale

### 1.1 The problem

A large policy simulation is difficult to trust. Two runs of the same model can give different results. Two machines can give different results. A small change to the program structure can change the answer. The user cannot see which of these things occurred.

Sembla makes the answer a property of the meaning of the model. The result does not depend on the order of work inside the machine.

### 1.2 The run contract

One rule controls the project:

> **seed + IR + parameter vector + determinism level = reproducible results.**

The IR is the wire format for a model. The parameter vector holds one value for each declared parameter. The determinism level tells you which guarantee applies. If you keep these four things, you get the same result again.

### 1.3 The four commitments

`DESIGN.md` gives four commitments in priority order.

1. **Composition has a real meaning.** You build a model from parts. The parts nest, connect, and multiply. A change to the part boundaries does not change the result.
2. **Reproducibility is a semantic property.** It is not a run option. The project gives different levels of guarantee. Each level trades strictness against speed.
3. **The semantics is GPU-shaped by construction.** Sembla permits only operations with an order-free result, or with one canonical order. Parallel execution and bitwise replication are then the same problem.
4. **The frontend is the formal home of the semantics.** Lean 4 holds the meaning of the language. An optimisation in the backend is then a theorem that a person can state.

### 1.4 The one-sentence thesis

Sembla is a synchronous relational machine.

- The state is a typed columnar database.
- A timestep is a query.
- Composition connects boxes. The boxes send tables to each other.
- Randomness is a pure function of coordinates.

### 1.5 Why the project uses Lean 4

There are two reasons, in priority order.

First, the Lean infoview shows interactive output for the code under the cursor. Sembla uses this to draw structure widgets: state diagrams, wire diagrams, and prior plots. No other environment gives this at the level of a source line.

Second, Lean holds the meaning of the language as a mathematical object. A rewrite in the compiler becomes a theorem. The project pays the specification cost now, because you cannot add it later.

The limits are known and recorded. A theorem is a statement about the real-number semantics. A theorem stops at the IR boundary. The Rust code and the GPU code are trusted, not proved. Floating-point execution is not covered.

### 1.6 What Sembla is not

The project refused these things. A proposal to add one of them re-opens a decision.

- A proof-verified compiler.
- A differentiable simulator.
- A general discrete-event engine.
- A custom parser for the surface language. Lean is the frontend.
- A browser user interface. The Lean infoview is the interface.
- A matrix of execution profiles. There are two paths only: the CPU oracle and one GPU backend.
- A unit system in the Rust validator. Units belong in Lean.
- A run-management product. There are no replay archives, no event streams, and no provenance database.
- A generator for synthetic populations. An external pipeline makes the population. Sembla reads it through a versioned format.

### 1.7 The driving use case

The driving use case is public-policy microsimulation. Two driver models steer the work.

- **Driver A, demographic population change.** A fixed pool of slots. Births, deaths, and migration over a national geography. Monthly ticks and chained annual windows.
- **Driver B, the justice pipeline.** Offending, courts, and corrections as a network of queues with limited capacity. Section 9 gives it in full. Nothing of it is built.

Two drivers are necessary. A single driver can bend the language to one domain. A demand from one driver alone gives a provisional specification only.

---

## 2. The semantics of the simulation framework

### 2.1 The governing rule

Sembla permits only operations with an order-free result, or with one canonical order. This rule controls every layer below. Section 2.13 draws the whole picture in one diagram.

### 2.2 State: the ACSet

#### Two readings of one object

The state of a box is an **ACSet**. The full name is *attributed C-set*. An ACSet has two readings. The two readings describe the same object.

The **mathematical reading.** A schema is a small category. Its objects are the entity types. Its arrows are the references between those types. A state is a functor from the schema to sets. The functor gives a finite set of rows to each object. It gives a function between row sets to each arrow. An attribute is a function from a row set to a value type.

The **engineering reading.** A schema is a set of table definitions. A state is a typed columnar database. A reference is a foreign-key column. An attribute is an ordinary typed column.

Sembla treats these two readings as one object. This is the pivotal convergence of the design. The formal object and the fast representation are not two things, so the meaning of a model and the execution of that model cannot drift apart.

#### The column types

A table has typed columns. There are four column types.

| Type | Meaning |
| --- | --- |
| `Real` | A 64-bit floating-point number. |
| `Int` | A 64-bit integer. |
| `Enum` | One name from a declared list. |
| `Ref` | A row index into a named table in the same box. |

#### The five properties that do work

The ACSet is not decoration. Five of its properties carry load.

1. **A reference is a total function, not a pointer.** The column `employer : Person -> Employer` gives one employer row to every person row. The loader compares each value against the row count of the target table. It rejects the state before the run starts if one value is out of range. A dangling reference is therefore impossible. An object graph of pointers gives no such guarantee.
2. **The layout is the semantics.** The columnar layout is not a trick below the semantics. It is one identity step from the mathematics. Parallel execution is therefore structural. Nobody added it later.
3. **One data model covers every shape.** A compartment model is a table with one row. A network is a table of edges. A lattice is a table with reference columns for the neighbours. A population is a table of people with a reference to an employer. There are no special cases.
4. **A join is composition of arrows.** The form `freq (health = I) over employer` follows the `employer` arrow from the row, collects the rows with the same value, filters them, and counts them. The validator permits a join only when both columns are references to the same table. Joins on declared keys only keep the cost linear. This restriction is also the reason that the lumping rewrite is a theorem that a person can state.
5. **The state has a canonical byte form.** A state is a finite functor with a declared order. It therefore has one encoding: the table name, the row count, the column count, and then each column with a type tag and fixed-width values. A real number contributes its bits. This canonical form gives the SHA-256 state hash. The whole reproducibility contract rests on that hash.

One substance runs through the design. The state is tables. An aggregate is a table. A wire message is a table. A port schema is a relation schema. There is one data model to compile, and one to prove things about.

#### What Sembla does not take from ACSets

The same literature gives general graph rewriting. AlgebraicRewriting and AlgebraicABMs apply double-pushout rewriting to ACSets. Sembla takes the data model and refuses the rewriting. General subgraph pattern matching is the worst workload for a GPU. Sembla keeps the restricted relational fragment of section 2.4 instead.

Three more limits apply today.

- **The schema is static.** The tables, the columns, and the reference targets are fixed before the run. A row can change its values. The shape cannot change.
- **The row count is fixed.** The runtime does not make or delete rows. The demographic driver uses a fixed pool of slots for this reason. Birth and death by stream compaction stay in the design, but nobody has built them.
- **An individual is a row, not a system.** The wiring picture survives at the level of a population and in the surface syntax. It does not survive at the level of one person. Interaction inside a population is a query over tables, not a message across an interface.

### 2.3 The three kinds of quantity

Sembla keeps three kinds of quantity apart.

- A **parameter** is constant for one run. It is read-only. It is the unit of calibration, priors, and sweeps. An expression refers to a parameter by name. The runtime never writes the value into the IR.
- A **state** column changes during the run.
- An **input** arrives on a wire from another box.

There are no global variables. The tick and the parameter vector are the only shared things.

### 2.4 What one tick does

Each box reads the old state and the input tables. It writes the new state and the output tables. The rule is *read-old, write-new*, with no exception. This rule applies inside a box and across a wire.

The result is a uniform one-tick delay. A box boundary is therefore invisible to the meaning of the model. You can move a boundary and get the same numbers.

#### What the delay costs

The delay is real. A signal that goes through a wire arrives one tick later. A feedback loop with two wires reacts two ticks after its cause. The committed demonstration model shows this. The population box and the policy box give identical columns at tick 0 and at tick 1. The first difference appears at tick 2, after both wires carry the change.

Three facts limit the cost.

- **The delay counts wires, not levels.** An exposure adds no delay. See section 2.8. If you put a box inside three composites, you add no tick.
- **The tick length sets the delay in model time.** One tick is `dt` of model time. If you make `dt` smaller, the lag gets smaller. The discretisation error falls with the square of `dt`. The run costs more machine time.
- **A merge does not remove the delay.** If you write the two boxes as one box, the read-old and write-new rule still applies inside that box. This is the reason that the merged model and the wired model give the same bytes.

#### How other methods compare

| Method | Sees writes inside one step | Depends on the order of work | Parallel and deterministic |
| --- | --- | --- | --- |
| Sembla: synchronous, read-old and write-new | No | No | Yes |
| Sequential random-order update, for example NetLogo or Mesa | Yes | Yes | No |
| Exact discrete-event, for example Gillespie or a DES engine | Yes, one event at a time | No, the method is exact | No, the method is sequential |
| Co-simulation of coupled modules, for example FMI or DEVS | No, at a coupling boundary | No | Yes |

A sequential random-order update lets an agent read the writes of the agents before it. This removes the delay. It adds a hidden assumption: the activation order. That order is usually random, so the modeller must make the order part of the model. Sembla treats this method as a discrete-time shadow of independent racing clocks. It uses the CTMC as the ground truth instead.

Exact discrete-event execution has no delay and no discretisation error. It runs one event at a time. It cannot run 26 million rows. Sembla keeps it in the design as a slow path for one box. It is not built yet.

A co-simulation framework accepts the same one-step delay at each coupling boundary. Sembla makes the same choice. Sembla then makes the delay uniform, so the rule also applies inside a box. That uniformity is what gives boundary invariance.

#### The true limitation

The delay is not the limitation. The ban on cascades inside one tick is the limitation. You cannot say "A fires, then B reads the result of A, then B fires, all at the same instant". A model that needs this is on the wrong side of the expressiveness cliff. See section 2.12. You must put the chain across more than one tick, or make `dt` small enough that the difference does not change the answer to your question.

`DESIGN.md` keeps the ergonomics of the delay as an open question. The delay is honest, but a modeller must learn to see it.

The kernel language is a closed fragment:

- map over a table, with row-local expressions;
- filter;
- join on declared keys only;
- group and aggregate, where the aggregation is a commutative monoid;
- segmented argmin;
- random draws by coordinate.

### 2.5 Time and hazard rates

A transition declares a hazard rate. It does not declare a per-tick probability. A probability is sugar. The frontend converts it with `lambda = -ln(1 - p) / dt`.

The ideal meaning is a continuous-time Markov chain. Each permitted transition runs an exponential clock. The first clock to fire wins.

The executed meaning is tau-leaping. The runtime freezes the rates at the start of the tick. A transition fires if its sampled time falls inside the tick window `dt`. There are no cascades inside a tick. A loser races again in the next tick.

`dt` is a semantic parameter. It is not only a speed control. The discretisation error is of order `dt` squared.

#### What a hazard rate can read

A hazard is an expression with the type `Real`. The expression language is first-order. It has no recursion and no user functions. A hazard can read four things.

1. **A parameter.** The reference is by name. The run supplies the value. The IR never holds the value.
2. **An attribute of the row itself.** This includes a `Real` column that came in with the initial state. Loaded columns are the supported method for rate heterogeneity by age, by sex, and by area.
3. **A keyed aggregate over a table in the same box.** The operations are `count` and `sum`. The join uses one declared `Ref` key. The filter is row-local. For example, `freq (health = I) over employer` gives the share of infected coworkers. For a whole-population aggregate, use a key that every row shares. The noisy-voter model does this with one community key.
4. **An aggregate over an input port.** For example, `inputSum activity field infected` reads the table that a wire delivered. This is the only way to read a quantity from another box. It therefore carries the one-tick delay.

You can combine these four things with `+`, `-`, `*`, `/`, and comparisons.

A hazard cannot read these things:

- **The tick or the clock.** There is no time expression. `Expr::Tick` is a named deferred construct. To change a rate between years, run chained annual windows with a different parameter vector for each year.
- **A rate table indexed by a value.** Lookup tables and time-indexed rates are future work. Each one needs an accepted amendment to a decision.
- **Another row directly.** You reach other rows only through a keyed aggregate. There is no row index and no unbounded join.
- **A transcendental function.** The expression language has arithmetic and comparisons only. There is no `exp`, no `ln`, and no power.
- **A nested aggregate, or a key that is not a reference.** The validator rejects each of these with a positioned message.
- **A random draw of a category.** Categorical draws are deferred.

Two runtime rules complete the picture. The runtime computes each rate from the committed state of the previous tick. A rate of zero or less gives an infinite firing time, so that transition never fires.

#### One clock for the whole model

A model has one `dt`. Every box uses it. A box cannot declare its own tick length, and a box cannot tick faster or slower than another box.

The rule is enforced, not merely advised. `dt` is one field on the model, not a field on a box. A component never declares `dt`; only the root composition declares the outer `dt`. The plan validator then requires **exactly one scheduler domain**. Its identity must be `domain:global`. Its algorithm must be `tau_leap`. Its leaves must equal the sorted list of every box in the model. A plan that declares two domains is rejected with a message. Nothing is accepted and then ignored.

The `--dt` option on the command line overrides the value for a legacy model. It never applies to a plan. A plan carries the `dt` that the linker recorded.

#### How to model a slow process today

A slow process does not need a slow clock. It needs a small hazard rate. This is the point that a new modeller gets wrong most often.

The tick is the resolution of the approximation. The hazard is the speed of the process. A death rate of 0.008 each year and an infection rate of 40 each year can sit in the same box under the same `dt`. The runtime races both clocks in every tick. The slow one almost never wins.

Three rules follow.

- **Choose the tick length for the fastest process in the model.** The discretisation error grows with the square of `dt`, and the largest hazard binds. A rate that is large against `1/dt` fires nearly every tick, and its true behaviour is then lost.
- **Use chained windows for a rate that changes between periods.** A run exports its final state. The next run reads that state and takes a new parameter vector. This is how the demographic driver varies annual rates across a decade of monthly ticks.
- **Use a guard for a process that is not always active.** A transition with a false guard costs no draw.

#### What one clock costs

Every box pays the tick rate of the fastest box. Consider the planned justice model. A court box may need a resolution of one day. A population box is content with one month. Under one clock, the population box must also run daily. It then does thirty times the work for no gain in accuracy.

This is a cost in machine time, not a cost in correctness. It becomes a real limit only when the two parts of a model differ by a large factor in their natural time scale.

#### What would unlock different clocks

Heterogeneous scheduler domains are designed and deferred. Phase 8 of the composition architecture names the five parts: a scheduler declaration on each leaf; a common outer boundary between domains; stable draw and event coordinates inside each domain; a rule for a transition family that spans two domains; and the complete domain plan in the run manifest. Its exit criterion is one sentence: no domain observes the internal substeps of another domain.

Two things gate the work.

First, evidence. The two drivers disagree. The demographic driver finds monthly ticks adequate. The justice driver wants dated events, such as a hearing or a release. The two-driver rule therefore applies. The project must first measure a staged-tick baseline against analytical queue cases and an independent discrete-event run on the same inputs. Only a measured distortion admits event-driven scheduling.

Second, a loss that the project states in advance. Boundary invariance today is bitwise: a wired model and its hand-merged equivalent give identical bytes. That test needs one clock. Once two domains run at different rates, a single exact run validates a wired hybrid only up to the coupling error between the domains. The check then becomes a convergence check, not a bitwise check. Section 2.4 rests on the bitwise form, so different clocks weaken a load-bearing guarantee. The project treats that as a price to be justified by evidence, not as a detail.

### 2.6 Randomness

Every draw is a pure function of five coordinates: the seed, the tick, the rule word, the entity ID, and the draw index. The generator is Philox4x32-10. There is no mutable stream and no state.

Two consequences follow.

- The evaluation order cannot change which random events occur.
- Two runs with the same coordinates get the same draws. This gives common random numbers at no cost. The same simulated person gets the same shocks under two different policies.

The rule also applies one level up. When an experiment gives a seed to each run, that seed comes from a hash of the semantic coordinate of the run. It never comes from the position of the run in a list. If you add a case to an experiment, no other case changes.

### 2.7 Conflicts

Two transitions can claim the same row in one tick. Sembla does not permit "the last writer wins", because that answer depends on order.

- A transition declares the resources that it claims.
- Each resource resolves by argmin over the sampled firing times.
- A tie breaks by the lexicographic key: time, then rule ID, then entity ID.
- A loser waits for the next tick.
- The runtime counts the deferred losers for each resource table. It prints a saturation warning when the deferred count is more than 10 per cent of the fired count.

The surface language exposes one ordering: `race_time`. Keyed orderings and queue disciplines are deferred.

### 2.8 Composition

A box is a Moore machine with table-typed ports. A wire carries a stream of tables. A composed system is itself a box.

There are three connection forms. They differ in one thing: what they cost at run time.

| Form | Makes a mailbox | Adds delay | Adds state |
| --- | --- | --- | --- |
| Wire | Yes, exactly one | One tick | Yes. The mailbox holds a table. |
| Exposure | No | None | No |
| Hide | No | None | No |

A **wire** is a mailbox. The source box writes an output table. The mailbox holds that table. The target box reads it as an input table in the next tick. A wire therefore adds semantic state and exactly one tick of delay. The linker gives each wire one mailbox with a stable identity. The two port schemas must match. A destination port can receive from one wire only.

An **exposure** is an alias. It gives a port of a child a name at the boundary of the parent. It makes no mailbox and adds no delay. It moves no data and changes no state. Nesting is therefore free: a box inside three composites runs at the speed of the same box at the top. An exposure can also give the port a new name. A new name changes the label only. It does not change the stable identity.

A **hide** removes a port of a direct child from the public interface of the parent. It deletes no state. The port continues to work inside the composite. A box outside cannot see it.

One rule of visibility controls all three forms. A port of a child is private outside its immediate parent. A parent can wire, expose, hide, or rename a port of its direct child. A parent must not reach through a child to a port of a grandchild that the child did not expose.

```lean
sembla_component EpidemicPolicy where
  instance population := Population
  instance policy := Policy
  wire count_to_policy : population.infection_count -> policy.infection_count
  wire modifier_to_population : policy.restriction_modifier -> population.restriction_modifier
  expose infection_count : population.infection_count as infection_count
```

The two wires make the feedback loop. Each wire carries one tick of delay, so the loop closes after two ticks. The exposure lets the parent of this component read the infection count with no delay and no copy.

The structure widget teaches the difference. A solid row is a wire and shows the mark `1-tick delay`. A dashed row is an exposure and shows the mark `zero-delay alias`. A hidden port appears struck through.

These three meanings are frozen. You cannot change one without a new version of the source schema and a new version of the linker semantics.

The design intends that each box can one day use a different scheduler, so that a large population box and a small exact box run together. This is a stated reason for the composition layer. It is **not** what the software does today: V1 puts every box in one scheduler domain under one algorithm and one `dt`. Section 2.5 gives the rule, the cost, and the work that would lift it.

### 2.9 Observation

Observation is a sink. There is no path from an observation back to a parameter, an input, a hazard, a transition, or a wire.

There are three observation forms.

- A **view** is a scalar projection of the committed state, for each tick. The reductions are `count`, `sum`, `min`, and `max`.
- A **grouped view** is a table of counts by one to four keys. A key is an `Enum` column, a `Ref` column, or an `Int` column in bands.
- A **summary** is a scalar reduction over the views of a run. The reductions are `sum`, `min`, `max`, `last`, and `argmax` over ticks.

The invariant is checkable: if you add, remove, or disable an observation, the state hash does not change.

### 2.10 Determinism levels

| Level | Guarantee | State today |
| --- | --- | --- |
| A, audit | Bitwise, for the same binary and the same GPU model. | The only level the software runs. |
| B, portable | Bitwise across different hardware. | Designed. Never tested across two machines. |
| C, fast | The same draws. The floating-point sum order can change. | Designed. Not built. |

Because randomness is a function of coordinates, the *events* never change between levels. Only the floating-point accumulation order can change.

Be exact about the present state. The command line has no option to choose a level. `determinism_level` is a constant with the value `A` in the manifest writer, and every run records it. The claim is therefore an assertion by construction: the CPU path holds a fixed reduction order, and the CUDA path holds fixed-order two-pass reductions and lexicographic winner keys. A repeated-run byte comparison tests it. No test compares two different machines, so Level B stays a design, not a result.

The GPU path uses native 64-bit floating point through CUDA. The project prohibits a silent fall-back to a lower precision.

### 2.11 Identity

Sembla gives each declaration a stable identity. The identity does not use position.

- A slug is `[a-z][a-z0-9_]*`.
- A declaration identity is `kind:slug`, for `model`, `def`, `inst`, `port`, `wire`, or `expose`.
- An occurrence identity is `occ:` and the chain of instance slugs from the root.
- A transition occurrence adds `#` and the transition name.

A display name is provenance only. If you rename a label or reorder independent declarations, the identity and the canonical bytes do not change. If you move a declaration across a boundary, the occurrence chain changes. The random draws then change too. Such an edit is a migration.

### 2.12 The expressiveness cliff

Sembla excludes these things from the fast path on purpose:

- unbounded match patterns;
- negative application conditions beyond anti-joins;
- recursion inside one tick.

A model that needs one of these inside a tick has a design fault. The elaborator must catch it.

### 2.13 The whole picture

The diagram below draws one model with two boxes. It puts every part of sections 2.2 to 2.12 in one place.

<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 1160 660" width="100%" role="img" aria-labelledby="anatomy-title anatomy-desc" style="max-width:100%;height:auto">
  <title id="anatomy-title">The anatomy of a Sembla model</title>
  <desc id="anatomy-desc">A model holds parameters and a fixed step. It contains two boxes. Each box holds tables, transitions, ports, and views. References join tables inside one box with no delay. Wires carry tables between boxes with one tick of delay. Views and summaries drain into an observation sink that cannot feed back.</desc>

  <defs>
    <marker id="wire-arrow" viewBox="0 0 10 10" refX="9" refY="5" markerWidth="7" markerHeight="7" orient="auto-start-reverse">
      <path d="M 0 0 L 10 5 L 0 10 z" fill="#4f7cac"/>
    </marker>
    <marker id="ref-arrow" viewBox="0 0 10 10" refX="9" refY="5" markerWidth="6" markerHeight="6" orient="auto-start-reverse">
      <path d="M 0 0 L 10 5 L 0 10 z" fill="#64748b"/>
    </marker>
    <marker id="sink-arrow" viewBox="0 0 10 10" refX="9" refY="5" markerWidth="6" markerHeight="6" orient="auto-start-reverse">
      <path d="M 0 0 L 10 5 L 0 10 z" fill="#a855f7"/>
    </marker>
  </defs>

  <rect x="0" y="0" width="1160" height="660" fill="var(--background-primary,#ffffff)"/>

  <!-- MODEL frame -->
  <rect x="20" y="20" width="1120" height="580" rx="18" fill="none" stroke="var(--background-modifier-border,#94a3b8)" stroke-width="2" stroke-dasharray="9 7"/>
  <text x="46" y="54" font-family="Inter, system-ui, -apple-system, sans-serif" font-size="20" font-weight="700" fill="var(--text-normal,#17202a)">MODEL</text>
  <text x="128" y="54" font-family="Inter, system-ui, -apple-system, sans-serif" font-size="14" fill="var(--text-muted,#64748b)">parameters &#952; &#183; one fixed step dt for every box &#183; determinism level</text>

  <!-- ================= BOX A ================= -->
  <rect x="48" y="78" width="440" height="400" rx="14" fill="var(--background-secondary,#f8fafc)" stroke="var(--color-blue,#3b82f6)" stroke-width="2.5"/>
  <path d="M 48 92 a 14 14 0 0 1 14 -14 h 412 a 14 14 0 0 1 14 14 v 30 h -440 z" fill="var(--color-blue,#3b82f6)" fill-opacity="0.14"/>
  <text x="72" y="108" font-family="Inter, system-ui, -apple-system, sans-serif" font-size="18" font-weight="700" fill="var(--text-normal,#17202a)">BOX population</text>
  <text x="248" y="108" font-family="Inter, system-ui, -apple-system, sans-serif" font-size="12" fill="var(--text-muted,#64748b)">Moore machine &#183; own state</text>

  <!-- table person -->
  <rect x="72" y="136" width="196" height="142" rx="10" fill="var(--background-primary,#ffffff)" stroke="var(--color-green,#22c55e)" stroke-width="2"/>
  <text x="86" y="160" font-family="Inter, system-ui, sans-serif" font-size="15" font-weight="700" fill="var(--text-normal,#17202a)">TABLE person</text>
  <text x="86" y="180" font-family="ui-monospace, Menlo, monospace" font-size="11" fill="var(--text-muted,#64748b)">rows: individuals</text>
  <line x1="82" y1="192" x2="258" y2="192" stroke="var(--background-modifier-border,#cbd5e1)" stroke-width="1"/>
  <text x="86" y="211" font-family="ui-monospace, Menlo, monospace" font-size="11" fill="var(--text-normal,#17202a)">health : Enum {S,I,R}</text>
  <text x="86" y="231" font-family="ui-monospace, Menlo, monospace" font-size="11" fill="var(--text-normal,#17202a)">risk : Real</text>
  <text x="86" y="251" font-family="ui-monospace, Menlo, monospace" font-size="11" fill="var(--text-normal,#17202a)">age_months : Int</text>
  <text x="86" y="271" font-family="ui-monospace, Menlo, monospace" font-size="11" fill="var(--color-orange,#b45309)">employer : Ref</text>

  <!-- table employer -->
  <rect x="72" y="316" width="196" height="60" rx="10" fill="var(--background-primary,#ffffff)" stroke="var(--color-green,#22c55e)" stroke-width="2"/>
  <text x="86" y="340" font-family="Inter, system-ui, sans-serif" font-size="15" font-weight="700" fill="var(--text-normal,#17202a)">TABLE employer</text>
  <text x="86" y="360" font-family="ui-monospace, Menlo, monospace" font-size="11" fill="var(--text-muted,#64748b)">rows: workplaces</text>

  <!-- Ref arrow -->
  <line x1="128" y1="278" x2="128" y2="314" stroke="var(--text-muted,#64748b)" stroke-width="2" stroke-dasharray="4 4" marker-end="url(#ref-arrow)"/>
  <text x="140" y="294" font-family="Inter, system-ui, sans-serif" font-size="11" font-weight="600" fill="var(--text-muted,#64748b)">Ref &#183; total function</text>
  <text x="140" y="308" font-family="Inter, system-ui, sans-serif" font-size="11" fill="var(--text-muted,#64748b)">one box &#183; no delay</text>

  <!-- transition infect -->
  <rect x="286" y="136" width="180" height="112" rx="10" fill="var(--background-primary,#ffffff)" stroke="var(--color-purple,#a855f7)" stroke-width="2"/>
  <text x="300" y="158" font-family="Inter, system-ui, sans-serif" font-size="14" font-weight="700" fill="var(--text-normal,#17202a)">TRANSITION infect</text>
  <text x="300" y="178" font-family="ui-monospace, Menlo, monospace" font-size="10" fill="var(--text-muted,#64748b)">guard  health = S</text>
  <text x="300" y="196" font-family="ui-monospace, Menlo, monospace" font-size="10" fill="var(--text-muted,#64748b)">hazard &#946; &#183; freq(health=I)</text>
  <text x="300" y="212" font-family="ui-monospace, Menlo, monospace" font-size="10" fill="var(--text-muted,#64748b)">          over employer</text>
  <text x="300" y="230" font-family="ui-monospace, Menlo, monospace" font-size="10" fill="var(--text-muted,#64748b)">set    health := I</text>

  <!-- transition recover -->
  <rect x="286" y="264" width="180" height="76" rx="10" fill="var(--background-primary,#ffffff)" stroke="var(--color-purple,#a855f7)" stroke-width="2"/>
  <text x="300" y="286" font-family="Inter, system-ui, sans-serif" font-size="14" font-weight="700" fill="var(--text-normal,#17202a)">TRANSITION recover</text>
  <text x="300" y="306" font-family="ui-monospace, Menlo, monospace" font-size="10" fill="var(--text-muted,#64748b)">guard  health = I</text>
  <text x="300" y="324" font-family="ui-monospace, Menlo, monospace" font-size="10" fill="var(--text-muted,#64748b)">hazard &#947;</text>

  <!-- A ports -->
  <rect x="286" y="356" width="180" height="32" rx="16" fill="var(--color-blue,#3b82f6)" fill-opacity="0.16" stroke="var(--interactive-accent,#4f7cac)" stroke-width="2"/>
  <text x="300" y="377" font-family="ui-monospace, Menlo, monospace" font-size="11" font-weight="700" fill="var(--text-normal,#17202a)">OUT activity</text>
  <rect x="286" y="404" width="180" height="32" rx="16" fill="var(--color-blue,#3b82f6)" fill-opacity="0.16" stroke="var(--interactive-accent,#4f7cac)" stroke-width="2"/>
  <text x="300" y="425" font-family="ui-monospace, Menlo, monospace" font-size="11" font-weight="700" fill="var(--text-normal,#17202a)">IN restriction</text>

  <!-- A view -->
  <rect x="72" y="404" width="196" height="32" rx="8" fill="var(--color-purple,#a855f7)" fill-opacity="0.12" stroke="var(--color-purple,#a855f7)" stroke-width="1.5" stroke-dasharray="5 4"/>
  <text x="86" y="425" font-family="ui-monospace, Menlo, monospace" font-size="11" fill="var(--text-normal,#17202a)">view infectious</text>

  <!-- ================= BOX B ================= -->
  <rect x="672" y="78" width="420" height="400" rx="14" fill="var(--background-secondary,#f8fafc)" stroke="var(--color-orange,#f59e0b)" stroke-width="2.5"/>
  <path d="M 672 92 a 14 14 0 0 1 14 -14 h 392 a 14 14 0 0 1 14 14 v 30 h -420 z" fill="var(--color-orange,#f59e0b)" fill-opacity="0.16"/>
  <text x="696" y="108" font-family="Inter, system-ui, -apple-system, sans-serif" font-size="18" font-weight="700" fill="var(--text-normal,#17202a)">BOX policy</text>
  <text x="836" y="108" font-family="Inter, system-ui, -apple-system, sans-serif" font-size="12" fill="var(--text-muted,#64748b)">same domain &#183; same dt</text>

  <!-- table controller -->
  <rect x="696" y="136" width="372" height="100" rx="10" fill="var(--background-primary,#ffffff)" stroke="var(--color-green,#22c55e)" stroke-width="2"/>
  <text x="710" y="160" font-family="Inter, system-ui, sans-serif" font-size="15" font-weight="700" fill="var(--text-normal,#17202a)">TABLE controller</text>
  <text x="880" y="160" font-family="ui-monospace, Menlo, monospace" font-size="11" fill="var(--text-muted,#64748b)">rows: 1 (a compartment)</text>
  <line x1="706" y1="172" x2="1058" y2="172" stroke="var(--background-modifier-border,#cbd5e1)" stroke-width="1"/>
  <text x="710" y="192" font-family="ui-monospace, Menlo, monospace" font-size="11" fill="var(--text-normal,#17202a)">mode : Enum {Open, Restricted}</text>
  <text x="710" y="214" font-family="ui-monospace, Menlo, monospace" font-size="11" fill="var(--text-normal,#17202a)">modifier : Real</text>

  <!-- transition restrict -->
  <rect x="696" y="252" width="372" height="86" rx="10" fill="var(--background-primary,#ffffff)" stroke="var(--color-purple,#a855f7)" stroke-width="2"/>
  <text x="710" y="274" font-family="Inter, system-ui, sans-serif" font-size="14" font-weight="700" fill="var(--text-normal,#17202a)">TRANSITION restrict</text>
  <text x="710" y="296" font-family="ui-monospace, Menlo, monospace" font-size="10" fill="var(--text-muted,#64748b)">guard  mode = Open &#8743; inputSum activity.infected &gt; 100</text>
  <text x="710" y="314" font-family="ui-monospace, Menlo, monospace" font-size="10" fill="var(--text-muted,#64748b)">set    mode := Restricted</text>
  <text x="710" y="330" font-family="ui-monospace, Menlo, monospace" font-size="10" fill="var(--text-muted,#64748b)">set    modifier := 0.4</text>

  <!-- B ports -->
  <rect x="696" y="356" width="180" height="32" rx="16" fill="var(--color-orange,#f59e0b)" fill-opacity="0.18" stroke="var(--interactive-accent,#4f7cac)" stroke-width="2"/>
  <text x="710" y="377" font-family="ui-monospace, Menlo, monospace" font-size="11" font-weight="700" fill="var(--text-normal,#17202a)">IN activity</text>
  <rect x="696" y="404" width="180" height="32" rx="16" fill="var(--color-orange,#f59e0b)" fill-opacity="0.18" stroke="var(--interactive-accent,#4f7cac)" stroke-width="2"/>
  <text x="710" y="425" font-family="ui-monospace, Menlo, monospace" font-size="11" font-weight="700" fill="var(--text-normal,#17202a)">OUT restriction</text>

  <!-- B view -->
  <rect x="896" y="404" width="172" height="32" rx="8" fill="var(--color-purple,#a855f7)" fill-opacity="0.12" stroke="var(--color-purple,#a855f7)" stroke-width="1.5" stroke-dasharray="5 4"/>
  <text x="910" y="425" font-family="ui-monospace, Menlo, monospace" font-size="11" fill="var(--text-normal,#17202a)">view restricted</text>

  <!-- ================= WIRES ================= -->
  <line x1="470" y1="372" x2="692" y2="372" stroke="var(--interactive-accent,#4f7cac)" stroke-width="3" marker-end="url(#wire-arrow)"/>
  <text x="581" y="363" text-anchor="middle" font-family="Inter, system-ui, sans-serif" font-size="12" font-weight="700" fill="var(--interactive-accent,#4f7cac)">wire &#183; 1 tick</text>

  <line x1="692" y1="420" x2="470" y2="420" stroke="var(--interactive-accent,#4f7cac)" stroke-width="3" marker-end="url(#wire-arrow)"/>
  <text x="581" y="444" text-anchor="middle" font-family="Inter, system-ui, sans-serif" font-size="12" font-weight="700" fill="var(--interactive-accent,#4f7cac)">wire &#183; 1 tick</text>

  <text x="581" y="240" text-anchor="middle" font-family="Inter, system-ui, sans-serif" font-size="13" font-weight="700" fill="var(--text-normal,#17202a)">a wire carries</text>
  <text x="581" y="258" text-anchor="middle" font-family="Inter, system-ui, sans-serif" font-size="13" font-weight="700" fill="var(--text-normal,#17202a)">a finite TABLE</text>
  <text x="581" y="280" text-anchor="middle" font-family="Inter, system-ui, sans-serif" font-size="11" fill="var(--text-muted,#64748b)">a mailbox holds it</text>
  <text x="581" y="296" text-anchor="middle" font-family="Inter, system-ui, sans-serif" font-size="11" fill="var(--text-muted,#64748b)">for one tick</text>

  <!-- ================= OBSERVATION ================= -->
  <rect x="48" y="516" width="1044" height="64" rx="12" fill="var(--color-purple,#a855f7)" fill-opacity="0.08" stroke="var(--color-purple,#a855f7)" stroke-width="2" stroke-dasharray="7 5"/>
  <text x="72" y="542" font-family="Inter, system-ui, sans-serif" font-size="15" font-weight="700" fill="var(--text-normal,#17202a)">OBSERVATION &#8212; a sink</text>
  <text x="72" y="564" font-family="ui-monospace, Menlo, monospace" font-size="11" fill="var(--text-muted,#64748b)">views per tick &#8594; summaries per run &#8594; results.csv &#183; grouped csv &#183; run manifest &#183; state hash</text>
  <text x="1068" y="552" text-anchor="end" font-family="Inter, system-ui, sans-serif" font-size="12" font-weight="700" fill="var(--color-purple,#a855f7)">no path back</text>

  <line x1="170" y1="440" x2="170" y2="512" stroke="var(--color-purple,#a855f7)" stroke-width="2" stroke-dasharray="5 4" marker-end="url(#sink-arrow)"/>
  <line x1="982" y1="440" x2="982" y2="512" stroke="var(--color-purple,#a855f7)" stroke-width="2" stroke-dasharray="5 4" marker-end="url(#sink-arrow)"/>

  <!-- ================= LEGEND ================= -->
  <line x1="52" y1="622" x2="96" y2="622" stroke="var(--interactive-accent,#4f7cac)" stroke-width="3" marker-end="url(#wire-arrow)"/>
  <text x="106" y="626" font-family="Inter, system-ui, sans-serif" font-size="12" fill="var(--text-normal,#17202a)">wire: a table between boxes, one tick of delay</text>

  <line x1="452" y1="622" x2="496" y2="622" stroke="var(--text-muted,#64748b)" stroke-width="2" stroke-dasharray="4 4" marker-end="url(#ref-arrow)"/>
  <text x="506" y="626" font-family="Inter, system-ui, sans-serif" font-size="12" fill="var(--text-normal,#17202a)">Ref: a join inside one box, no delay</text>

  <line x1="812" y1="622" x2="856" y2="622" stroke="var(--color-purple,#a855f7)" stroke-width="2" stroke-dasharray="5 4" marker-end="url(#sink-arrow)"/>
  <text x="866" y="626" font-family="Inter, system-ui, sans-serif" font-size="12" fill="var(--text-normal,#17202a)">observation: a sink, never feedback</text>
</svg>

**Figure 1. One model, two boxes, and the three kinds of connection.**

Read the diagram in four steps.

1. **The model owns the constants.** The parameters, the step `dt`, and the determinism level sit at the top. None of them changes during a run.
2. **A box owns its tables.** Each table has typed columns. The column `employer : Ref` is a total function from a person row to an employer row. The dotted arrow shows it. This join stays inside one box and costs no delay.
3. **A wire crosses a boundary.** The `activity` port emits a finite table. A mailbox holds that table for one tick. The `policy` box reads it on the next tick. The second wire carries the answer back, so the loop closes two ticks after its cause.
4. **Observation drains downward.** The views leave the model and reach the output files. No arrow comes back.

Three colours carry meaning. Green marks state. Purple marks a rule or an observation. Blue and amber mark the two boxes and their ports.

The diagram leaves out three things on purpose. It does not draw the tick pipeline of section 5.5. It does not draw a contest between two transitions for one resource. It does not draw nesting: each box here is a leaf, and a composite box would hold more boxes joined by the same three forms.

---

## 3. The Lean 4 syntax and the frontend features

### 3.1 What the frontend is

The frontend is a Lean 4 package in `frontend/`. It pins Lean 4.13.0. It does not use mathlib. Its only external dependency is ProofWidgets4 v0.0.44. It holds 473 Lean files: the deep IR, the surface language, the widgets, the proofs, the linker, and a large body of positive and negative tests.

The frontend elaborates, inspects, draws widgets, proves specification results, and writes JSON. It does not run a simulation.

### 3.2 The model command

A human writes a model with `sembla_model`. The command makes an ordinary Lean constant.

```lean
sembla_model WorkplacePolicy
    (name := "workplace_policy")
    (dt := 0.25) where
  param β : ℝ := 0.8 ~ LogNormal (-0.2231435513142097) 0.25
  param γ : ℝ := 0.1

  box population where
    system Person (rows := 1_000) where
      health : {S, I, R}
      risk : ℝ
      employer : Employer
    system Employer (rows := 50)

    infect on Person : health: S →[β · freq (health = I) over employer] I
    recover on Person : health: I →[γ] R

    view infectious := count Person where health = I
```

`(dt := ...)` is mandatory. `(name := ...)` is optional. There are no list brackets, no separator commas, and no empty category blocks.

Collection runs in more than one pass. You can therefore refer to a parameter, a system, a port, or a view before its declaration. The emitted IR keeps the textual order inside each category.

### 3.3 Names

Without an override, Lean converts the identifier by a frozen rule. `SirWorkplace` becomes `sir_workplace`. `Person` becomes `person`. Greek letters use documented transliterations: `β` becomes `beta`. A collision between two derived names is an error. The frontend does not choose a winner.

### 3.4 Parameters

A parameter declaration binds its name for the whole model.

```lean
param β : ℝ := 0.8 ~ LogNormal (-0.2231435513142097) 0.25
param retirement_months : Int := 780
```

A real parameter can carry a prior. The families are `Normal`, `LogNormal`, and `Uniform`. An integer parameter cannot carry a prior. The frontend rejects it. A real value is stored as exact coefficient and exponent data.

### 3.5 Systems and attributes

Each system needs `(rows := N)`. This is a size hint in the IR. It is not the run population.

The frontend infers each attribute type from its form:

```lean
state : {Open, Restricted}   -- an Enum
risk : ℝ                     -- a Real
visits : Int                 -- an Int
employer : Employer          -- a Ref to a system in the same box
```

### 3.6 Transitions

There are two forms.

Use the **reaction arrow** when the transition has one enum guard and one write to that same attribute:

```lean
infect on Person : health: S →[β · freq (health = I) over employer] I
```

You can omit `on Person` only when one system is possible. You can omit `health:` only when the system has one enum attribute. An ambiguous case is an error. The frontend does not choose by iteration order.

Use the **general form** for more guards, more effects, or a controller rule:

```lean
transition depart on Slot where
  guard occupancy = present
  hazard departure_hazard
  contest slot_resource by race_time
  set occupancy := vacant
  set age_months := age_months + 1
```

A general transition needs exactly one `guard`, exactly one `hazard`, and one or more ordered `set` effects. A numeric effect accepts the row-local scalar expressions. An enum effect accepts a variant name only. A `Ref` write is rejected. An aggregate in an effect is rejected.

`race_time` is the only ordering. The reaction arrow cannot carry a contest.

### 3.7 Aggregates

`freq (predicate) over key` is the supported frequency form. It means the plan `countBy key (predicate) / sizeBy key`. It needs a `Ref` key on the selected system and a row-local Boolean predicate. Nested `freq`, non-`Ref` keys, and input aggregates are rejected with positioned messages.

`inputSum port field column` reads a numeric field of an input port.

### 3.8 Ports, views, wires, and summaries

```lean
input activity where
  infected : Int

output activity from Person where
  infected : Int := count where health = I
  total_risk : ℝ := sum (risk)

view infectious := count Person where health = I
view active_risk_max := max Person where health = I using risk

grouped view population_cells :=
  count PersonSlot by sex, area, band age_months 60 where occupancy = present

wire population activity -> policy activity
summary peak_I := max population.infectious
summary peak_tick := argmaxₜ population.infectious
```

A wire needs matched schemas. A destination port can receive from one wire only. A count view has no `using` clause. A valued view needs one. An `Int` group key needs a positive band width. An `Enum` key or a `Ref` key must not use `band`.

Grouped observation is default-off. You must give `--enable grouped-observations` to run a model that declares one.

### 3.9 The composition commands

```lean
sembla_component EpidemicPolicy where
  instance population := Population (beta := beta, gamma := gamma)
  instance policy := Policy
  wire count_to_policy : population.infection_count -> policy.infection_count
  wire modifier_to_population : policy.restriction_modifier -> population.restriction_modifier

sembla_composition epidemicPolicyModel
    (name := "epidemic_policy") (dt := 0.25) where
  param beta : ℝ := 0.3
  root EpidemicPolicy
  summary infected_peak := max population.I
```

A primitive component uses the same declarations as a model. It does not declare `dt`. `requires` names the model parameters that it uses. A composite component makes instances and connects, exposes, or hides their ports. A binding maps a component requirement to a root parameter. A literal is not accepted.

A `sembla_composition` gives one root instance, an exact lowercase name, the outer `dt`, the root parameters, and the summaries.

### 3.10 The structure widgets

The frontend draws three infoview panels from the elaborated model: a state-machine diagram for a system, a hazard panel for a transition, and a structure panel for a component or a composition. No panel runs a simulation. Section 4 describes all three, together with the themes and the visual guide.

### 3.11 The frontend tools

| Command | What it does |
| --- | --- |
| `lake exe sembla-export <name> <file>` | Writes a model or a composition source as canonical JSON. |
| `lake exe sembla-link <source> --plan <file>` | Links a composition source into one executable plan. |
| `lake exe sembla-link <source> --bundle <dir>` | Writes the frozen four-file bundle. |
| `bash frontend/scripts/check-parity.sh` | Exports all eight canonical models, validates both sides, and compares the bytes with `cmp`. |
| `bash frontend/scripts/check-proofs.sh` | Runs the proof-hygiene guard. |

A bundle holds `composition-source.json`, `executable-plan.json`, `link-report.json`, and `bundle-manifest.json`. The linker rejects a destination directory that is not empty.

### 3.12 The tests

`frontend/Negative/` holds complete ill-formed models. Each file pins the full ordered set of positioned errors. `frontend/Positive/` holds focused correct cases.

A syntax twin proves that `model%` and `sembla_model` give the same bytes. `model%` stays as the compatibility form and the semantic kernel. Both surfaces call one kernel. Neither has separate semantics.

### 3.13 The proofs

`frontend/Sembla/LumpingProof.lean` proves `groupedCount_eq_naiveCount`. The grouped coworker-count plan and the naive plan agree exactly. `plan_rewrite_congr` carries that equality through any per-row function of the count.

This is theorem target 1a at the level of the specification. Target 1b binds the theorem to the deep-embedding evaluator. Target 1b is open.

---

## 4. Seeing a model: the widgets and the visual guide

Sembla gives two ways to look at a model without a run. The widgets are a feature of the frontend. They draw the model under the cursor. The visual guide is a document. It draws the language itself.

### 4.1 Why the pictures are free

A widget builds its picture from the elaborated model. It never calls the Rust runtime. It never runs a simulation. Every public builder in `Sembla/Widgets.lean` is a total function from the deep IR to JSON props, with no input and output operations. `Sembla/WidgetDisplay.lean` turns those props into HTML and SVG and registers the panel with the infoview.

The cost at run time is therefore zero. A widget cannot change a model and cannot change a result. This is the rule of section 2.9 one level up: to look at a thing does not change the thing.

### 4.2 The three panels

Put the cursor on a declaration. The infoview opens the panel for that declaration.

| Panel | Opens on | Shows |
| --- | --- | --- |
| State machine | A `system` declaration | Each state as a node, and each transition as an edge with its hazard. |
| Hazard | A transition declaration | The guard, the hazard, each parameter with its default, the prior density, and the firing-probability curve. |
| Composition structure | A `sembla_component` or `sembla_composition` declaration | The instance boxes with their ports, the wires, the exposures, and the hidden ports. |

**The state-machine panel** reads one table of one box. It gives one node for each variant of the enum attribute. It gives one edge for each transition between two variants. Each edge carries the printed hazard expression.

**The hazard panel** belongs to one transition, not to a system. It prints the guard and the hazard. For each parameter in that hazard, it gives the default value, and a density curve when the model declared a prior. It then plots `p(dt) = 1 - exp(-lambda * dt)` against `dt`. That curve is the honest statement of the tau-leap approximation for the transition.

The panel does not always give the curve. When the hazard holds an aggregate, the rate depends on the state of other rows, and one curve cannot describe it. The panel then gives the reason in words instead of a picture that misleads.

**The composition panel** shows the authored level only. Each instance box says whether its definition is primitive or composite, and lists its boundary ports. A child stays collapsed. To inspect a child, move to the declaration of that child.

The connection rows teach the delay discipline of section 2.8. A solid row is a wire and carries the mark `1-tick delay`. A dashed row is an exposure and carries the mark `zero-delay alias`. A hidden port appears struck through in its own row. The props hold the delay as a number, so the picture cannot disagree with the plan.

### 4.3 Themes

Widgets use the `academic` preset by default. A source file can select a theme before its model declarations:

```lean
set_option sembla.widget.theme "academic"
```

The three themes are `academic`, `editor`, and `notebook`. `editor` follows the standard widget chrome of VS Code. `notebook` is softer and more rounded. `professional` is a second name for `academic`. Every theme takes the foreground and background colours of the editor, so dark themes and high-contrast themes stay legible.

### 4.4 What the tests cover

`Sembla/WidgetTests.lean` and `Sembla/CommandFrontendTests.lean` assert the props and the structure of the rendering. They cover state graphs, hazards, probability and prior plots, responsive SVG, long labels, loops and opposing routes, badges, empty states, JSON encoding, and all three themes.

The final layout and the colours stay a manual check. `frontend/README.md` gives a procedure of six steps. The last step repeats the checks at a width of 280 to 320 pixels, in dark, light, and high-contrast themes.

### 4.5 The visual guide

`docs/guides/visual-guide.md` has a different job. A widget draws *your* model. The visual guide draws *the language*. Its diagrams are inline SVG. They render in the Obsidian reading view and follow the light or dark theme of Obsidian.

The guide has six parts.

1. **The abstract anatomy.** One diagram of a model: the parameters and `dt` at the top, two boxes below, the tables and transitions inside each box, and the wires between the boxes.
2. **A reversible two-state chain.** The smallest complete model.
3. **SIS with a grouped aggregate.** A reference column and a keyed count in one picture.
4. **A two-box SIR and policy feedback.** The delayed loop.
5. **The example catalogue.** A table of the checked-in models with their boxes, tables, internal aggregates, and wires.
6. **The expressiveness boundary.** A list of what the diagrams leave out on purpose.

The guide makes one distinction that a new modeller gets wrong. **A reference is not a wire.** A reference joins two tables inside one box and adds no delay. A wire carries a finite table between two boxes and adds one tick of delay. The two look similar on a whiteboard and behave differently in a run.

The guide also names a difference in words. The Lean surface says `system`. The IR and the Rust runtime say `table`. The two names mean the same collection of rows.

### 4.6 What is missing

There is no behaviour widget. A behaviour widget moves a slider, runs the model, and plots the result. `DESIGN.md` separates structure widgets from behaviour widgets for one reason: a structure widget costs nothing, and a behaviour widget needs the runtime to answer inside an interactive time budget. Section 10.4 gives the two paths that can make it possible.

A person draws the visual guide by hand. Nobody generates it from the models. Its example catalogue can therefore fall behind the repository. The capability matrix of section 10.6 has the same weakness, and its answer is the correct one here too: generate the table from the test suite.

---

## 5. The backend implementation

### 5.1 The workspace

The Rust workspace holds four crates. The source is about 30,200 lines. The tests bring the total to about 50,000 lines.

| Crate | Job |
| --- | --- |
| `sembla-ir` | The versioned model and plan types, the canonical serializer, the stable identities, and the validator. |
| `sembla-runtime` | The deterministic CPU interpreter, the state store, the evaluator, Philox, priors, populations, and state artifacts. |
| `sembla-cuda` | CUDA kernel generation and the optional NVRTC execution backend. |
| `sembla-cli` | The `sembla` command. |

### 5.2 The IR and the validator

The wire format uses snake-case `kind` tags. Declarations keep their source order. The serializer writes compact JSON with one trailing newline.

The validator assigns a zero-based `rule_id` to each transition, in declaration order, across all boxes. The ID is derived data. It is not in the wire format.

An input document has one of three origins.

| Origin | Meaning |
| --- | --- |
| `legacy` | An unversioned model document. It keeps the frozen dense positional identity. |
| `direct_stable` | A versioned plan exported from flat IR. It has stable identities but no linked source. |
| `linked` | A versioned plan from a composition source. It holds the source hash, the linker descriptor, the source map, and the full identity map. |

### 5.3 The state store

The state store is columnar and fixed in size. It uses read-old and write-new buffers. It computes a canonical SHA-256 hash of the committed state.

A run can export the final state as a `sembla.state/v1` artifact. A later run can read that artifact as its start state. This gives chained annual windows. The measured artifact size is 48 bytes for each slot.

### 5.4 The CPU oracle

The CPU interpreter is the executable semantics oracle. Two rules make it the reference.

- It evaluates an expression tree in syntax order. It does not reassociate.
- An aggregate sum makes one pass over the target table in ascending row order. That order is the canonical Level A reduction order.

The evaluator tiles the tick over row ranges and uses more than one worker thread. It does this only above a measured work threshold of 1,500,000 node-rows. The tile size comes from a 32 KiB cache budget and the live set of the model. The threshold and the budget do not come from the machine, so the partition stays comparable between hosts.

Measurement closed one direction of work. Parallel scaling saturates at six workers. The serial fraction is about 0.62. This caps the speed-up near 1.5 times at any core count. Only a reduction of serial work moves the wall time.

### 5.5 The tick pipeline

Each tick runs three phases.

1. **Execute.** Evaluate the guards and the hazards. Take the racing-clock draws. Collect the claims. Resolve each contested resource by argmin. Evaluate the effects at the winner rows. Commit the new state.
2. **Observe.** Evaluate the views and the grouped views over the committed state.
3. **Report.** Build the tick report, the fired counts, and the deferred counts.

`--timing-json` writes the duration of each phase.

### 5.6 The CUDA backend

The CUDA path generates CUDA C source from the validated IR. It compiles that source with NVRTC when the run starts. It uses native 64-bit floating point. The state stays on the device between ticks.

Two rules protect Level A: fixed-order two-pass reductions, and lexicographic winner keys.

The backend can also observe on the device. It does this only when it recognises every declared view as a filtered `Count`, or as a filtered `Min` or `Max` over an infallible row-local `Int` expression. The gate is all-or-nothing for the run. Any other view forces a full state download to the host. `Sum` over `Real` stays on the host, because the canonical order must not change. Real extrema stay on the host, because NaN behaviour differs.

CUDA support is behind a build feature. The default build needs no CUDA toolkit.

### 5.7 The command-line interface

| Command | Job |
| --- | --- |
| `validate` | Validates a model or a plan. |
| `run` | Runs a model or a plan and writes CSV, a manifest, and an optional state export. |
| `sweep` | Draws parameter vectors from the declared priors, or reads them from a file, and runs each draw. |
| `compare` | Runs two plans, or one plan with two parameter files, under one seed. |
| `verify-run` | Reproduces a run from its manifest and compares the hashes. |
| `diff-backends` | Runs the CPU path and the CUDA path and compares the results. |
| `diff-ir` | Compares two IR documents after normalisation. |
| `bundle-verify` | Checks every version, byte, hash, and membership rule of a bundle. |
| `plan-hash`, `state-hash` | Print the canonical hashes. |
| `synth-pop`, `synth-state` | Make deterministic test populations and state artifacts. |

### 5.8 The run manifest

Every run writes a manifest beside its output. The manifest records the run contract: the IR or plan hash with its algorithm name, the seed, the resolved parameters, `dt`, the tick count, the determinism level, the backend that ran, the precision, the fall-back status, the enabled features, the final state hash, the output hash, and the component versions.

Three rules protect the manifest.

- Each hash sits beside a named algorithm ID.
- Schema versions are explicit and per concern. There is no single global integer.
- Related fields form all-present or all-absent groups. A reader rejects a partial group. Absence then means "this run is older than the feature".

### 5.9 The measured performance state

This part of the project moved often. The numbers below are the current record.

| Measurement | Result | Where |
| --- | --- | --- |
| SIR, 26M rows, H100, native `f64` | About 1,380 ticks each second. Zero reduction error. | ADR 0001 |
| Demographic model, 10M slots, first CUDA measurement | CUDA was 12.3 times slower than the CPU on the same host. | §L1 |
| Cause, first diagnosis | Serial validation kernels. This diagnosis was wrong. Parallel validation changed nothing. | §L6 |
| Cause, correct diagnosis | Conflict resolution was quadratic in rows. The fitted exponent was 1.96. | §L6 |
| After segmented argmin and the host work | CUDA median 14.6 s against CPU 50.3 s, at 10M slots and 24 ticks. | §L13 |
| Kernel share of CUDA wall time | About 1 per cent. | §L9 |
| Sweep with a retained backend, CUDA at 1M | 28.68 s down to 11.63 s. | §L13 |

Three lessons are recorded, and they matter more than the numbers.

- **The GPU is not the constraint.** The host path is. Kernels have been about 1 per cent of CUDA wall time through three sessions.
- **A ratio gate is a bad gate.** The old §L4 gate compared CUDA against CPU. It gave three different verdicts while the true answer did not change, because host work made the slower arm faster. The project retired it. Absolute wall time and per-phase timing replace it.
- **Do not extrapolate a decomposition.** Two projections failed for the same reason. Both split a measurement into parts and carried a part to a different scale as if it were fixed. Measure the thing.

### 5.10 The process machinery

The repository runs its work through numbered specifications. Each `docs/prds-*/` folder is one track. Each specification has mechanical acceptance criteria and an allowed-file list. A script checks the allowed-file list before a run starts. `docs/prds-run-queue/` gathers pending specifications from several folders under sortable names, so one command runs them in order. Only one specification is active at a time.

`docs/evidence/` holds the measured results. A gate must publish its threshold in a commit before anybody generates the evidence. The git history then proves that nobody chose the threshold after the result.

---

## 6. From a Lean file to a verified result

The earlier sections describe each part on its own. This section puts them in order. It is the same pipeline for every model.

```text
author in Lean  ->  export source  ->  link  ->  validate  ->  run  ->  verify
    .lean            .source.json    .plan.json    ok        .csv    ok
                                                            + manifest
```

### 6.1 The six stages

**1. Author.** Write `sembla_component` definitions and one `sembla_composition` root. The Lean elaborator checks the names, the types, the scopes, the port schemas, and the ordering. An error stops you here, with a position in your file.

**2. Export.** `lake exe sembla-export --source <name> <file>` writes the composition source as canonical JSON. The bytes use `sembla.composition-source/v1`. Arrays keep the order you wrote. The Rust runtime never executes a source.

**3. Link.** `lake exe sembla-link <source> --plan <file>` produces one executable plan. The linker expands the instances, allocates one mailbox for each wire, resolves the exposures to aliases, and assigns every stable identity. `--bundle <dir>` instead writes the frozen four-file directory with its hashes.

**4. Validate.** `sembla validate <plan>` checks the whole document: the schema versions, the types of every expression, the join rules, the claim coverage, and the single scheduler domain. Validation is separate from execution on purpose. You can validate a plan on a machine that will never run it.

**5. Run.** `sembla run <plan> --population N --seed S --ticks K --out results.csv`. The run writes the CSV, any grouped CSV files, and a manifest beside them. `--export-state` writes the final state for a later window.

**6. Verify.** `sembla verify-run <manifest> <plan> --population N` reads the manifest, reproduces the run from it alone, and compares the hashes. This is the run contract of section 1.2, executed.

### 6.2 Which input form to use

Three forms reach the runtime. Section 5.2 gives their origins. The practical rule is short.

| You have | Use | You get |
| --- | --- | --- |
| A model authored today | A linked plan | Stable identities, provenance, and every command |
| Flat IR with no composition | A direct-stable plan | Stable identities, no linked source |
| An old model document | The legacy path | Frozen positional identity, fewer commands |

New work should produce a linked plan. The legacy path stays because the golden fixtures depend on it. Two arms of a `compare` must both be plans or both be legacy: the two identity schemes cannot form a meaningful pairing, so a mixed pair is rejected before execution.

### 6.3 The other workflows

The same plan feeds four more commands.

- **A sweep.** `sembla sweep <plan> --draws K --seed S ...` samples each parameter from its declared prior through a reserved draw namespace, then runs the model once for each draw. `--noise independent` gives each draw its own seed for training pairs. The default keeps common random numbers for policy contrasts. `--export-pairs` writes the parameter and summary pairs that an external calibration pipeline reads.
- **A comparison.** `sembla compare` takes two plans, or one plan with two parameter files, under one seed. Because identity is content-addressed, a component shared by both arms keeps the same identity, the same rule word, and therefore the same draws. A shared part of the model is then *exactly* equal between the arms, not merely close.
- **A backend difference.** `sembla diff-backends` runs the CPU path and the CUDA path on the same inputs and compares the outputs.
- **A chained window.** Export the final state, then start the next run from it with a new parameter vector. Section 5.3 gives the format. A chain of windows is not the same object as one continuous run, and section 8 says why.

### 6.4 What each stage guarantees

Each stage rejects a different class of error. A later stage never repairs an earlier one.

| Stage | Catches |
| --- | --- |
| Elaboration | Unknown names, wrong types, ambiguous inference, bad ordering |
| Linking | Unresolved ports, duplicate wires, reaching through an unexposed child |
| Validation | Schema, expression types, join rules, claim coverage, feature flags |
| Run | Out-of-range references in the initial state, saturation warnings |
| Verify | Any disagreement between a recorded result and a fresh one |

---

## 7. How the project knows a result is correct

Sembla holds nine layers of checking. No single layer is the argument. Together they are.

### 7.1 The oracle defines the answer

The CPU interpreter is the reference. When the CPU path and any other path disagree, the CPU path is right by definition. This is why it evaluates in syntax order and sums in ascending row order, and why nobody may make it faster by reassociating arithmetic.

### 7.2 The nine layers

| Layer | What it proves | Where |
| --- | --- | --- |
| Negative tests | An ill-formed model gives the exact ordered set of positioned errors | `frontend/Negative/` |
| Positive tests | A correct construct elaborates to the intended IR | `frontend/Positive/` |
| Syntax twins | Two surfaces give byte-identical IR, so neither has its own semantics | Frontend tests |
| Parity | Lean's export equals the checked-in fixture, byte for byte, by `cmp` | `check-parity.sh` |
| Golden fixtures | The IR wire format cannot change by accident | `examples/`, `fixtures/` |
| Law tests | Renaming and permuting declarations give identical canonical bytes | Composition tests |
| Boundary invariance | A wired model and its hand-merged twin give identical state hashes every tick | Composition test |
| Determinism | Repeating a run gives identical bytes | `scripts/` and CI |
| Differential | The CUDA path equals the CPU oracle on a corpus of models | `diff-backends` |

Two more checks sit above these. `bundle-verify` re-checks every version, byte, hash, and membership rule of a bundle. `verify-run` reproduces a run from its manifest alone.

### 7.3 One proof

`groupedCount_eq_naiveCount` proves that the grouped coworker-count plan and the naive plan agree exactly, and `plan_rewrite_congr` carries that equality through any per-row function of the count. This is the flagship optimisation of section 1.3, stated and proved as a theorem.

It is one theorem at the level of the specification. It does not yet bind to the deep-embedding evaluator. That is the open target.

### 7.4 What the checks do not cover

State this plainly, because the project does.

- **The Rust code and the CUDA code are trusted, not proved.** A theorem is about the real-number semantics and stops at the IR boundary.
- **Floating-point execution is not covered by any theorem.** Differential testing, not proof, is what stands behind it.
- **Level B is unproven.** No test has compared two different machines.
- **Reproducibility is not validity.** Every layer above shows that the software computes what the model says. No layer shows that the model is a good description of the world. Section 8 owns that distinction.

### 7.5 The method that produced the numbers

The performance work of section 5.9 taught the project three rules, and it now applies them.

- **Measure the thing. Do not subtract for it.** Two projections failed because each split a measurement into parts and carried one part to a new scale as if it were fixed.
- **A gate must be absolute, not a ratio.** A ratio between two moving implementations inverts its verdict when the slower one improves.
- **Publish the threshold before the evidence.** A gate must land in a commit before anybody generates its result. The git history then proves nobody chose the threshold after seeing the answer.

---

## 8. Limitations, and how to read a result

### 8.1 The distinction that matters most

Sembla can tell you two things with confidence: **what a model means**, and **how a run was produced**. It cannot tell you that the model is a good description of the world.

A reproducible result is not a valid result. A run that repeats byte for byte, verifies against its manifest, and agrees across two backends can still rest on rates that nobody fitted and a structure that nobody validated. The framework guarantees the first property. Only empirical work gives the second.

This is not a hypothetical caution. It describes the present state of the driver model.

### 8.2 The demographic model is a fixture

The demographic model is an executable accounting and software-validation fixture. It is not a calibrated population model. Its own limitations register lists fifteen items. These carry the most weight.

- **A slot is not permanently a person.** The pool is fixed. A slot is occupied or vacant. Identity does not persist through the pool in the way a person's identity persists in life.
- **A birth is not a fertility process.** The hazard is a rate for each eligible vacant slot. It is not a rate for each woman. You must not read it as a fertility rate without an explicit scaling derivation.
- **Internal migration does not move the same person.** It is not identity-preserving. National balance holds only in expectation. The residual is always reported and never silently reconciled.
- **There are no origin-destination matrices.** Entry characteristics are preclassified, not sampled.
- **Capacity, the one-tick entrant lockout, and simplified rates are approximations.** They are documented and measured, and they are model trade-offs, not defects of the framework.
- **A chain of windows is not one continuous stochastic history.**
- **Nothing is empirically initialised, fitted, or validated.** The parameters and the priors are test values.

A benchmark result says nothing about scientific validity. A model that runs 10 million slots quickly is a statement about the software.

### 8.3 What the framework itself cannot express today

Section 2.12 gives the deliberate exclusions. These are the practical ones.

- The row count is fixed, and the schema is static.
- A hazard cannot read the clock, a lookup table, or another row directly.
- There is one `dt` and one scheduler domain for the whole model.
- Grouped observation runs on the CPU only.
- A transition writes one row. There is no atomic multi-row event.
- There is no exact discrete-event path, and no ODE block.

Each of these has a design and a named trigger. Section 10.3 lists them.

### 8.4 The open-risk register

| Risk | Present state |
| --- | --- |
| The demographic model is not calibrated. | It is a software fixture. Reproducibility does not prove empirical validity. |
| Two execution paths. | The legacy path and the plan path differ in capability. No document says which one is canonical for new work. A compatibility policy written over an unresolved fork would freeze two contracts instead of one. |
| The ageing cost share is about 40 per cent. | The decision record set a 10 per cent threshold. Five readings are near 40 per cent. Each verdict notes it and moves on. It is the oldest un-actioned measurement in the record. |
| Grouped observation is CPU-only. | The driver model uses grouped views for its validation outputs. |
| Level B determinism is not proved. | No test has compared two different machines. |
| One clock for every box. | A model whose parts differ greatly in time scale pays the rate of its fastest part. Section 2.5 gives the cost. |
| Lean toolchain risk. | Widget API churn is an accepted risk. The frontend-agnostic IR is the hedge. |
| The visual guide is hand-drawn. | Its example catalogue can fall behind the repository. |

### 8.5 How to state a result honestly

When you report a number from Sembla, say four things.

1. **The contract.** The seed, the plan hash, the parameters, `dt`, and the tick count. The manifest holds all of them.
2. **The backend and the precision.** The manifest holds these too.
3. **The approximation.** The value of `dt`, and any sensitivity check you ran against it.
4. **The provenance of the inputs.** Where the rates and the initial state came from, and whether anybody fitted them.

A result without the fourth item is a statement about software behaviour. It is not a statement about a population.

---

## 9. The second driver: the justice pipeline

### 9.1 Why a second driver exists

Sembla is steered by real models, not by a feature list. One model is not enough. A single driver bends a language toward one domain, and the framework then acquires conveniences that look like general primitives.

The rule is therefore explicit. A demand that appears in one driver gives a provisional specification only. A demand that appears in both drivers is corroborated and can proceed.

The demographic model is the first driver. The justice pipeline is the second. It was chosen because it is policy-relevant in its own right and because it is structurally unlike the demographic model.

**Nothing of this model is built.** Its design and data audit has not run. This section describes a plan and the reasoning behind it, not software.

### 9.2 The model shape

An offending, courts, and corrections pipeline, modelled as a network of queues with limited capacity.

- **Stations.** Charges laid. Remand and bail. Court queues by court and matter type. Sentencing. Custodial and community corrections. Release.
- **Flows.** Lodgements feed the court queues. Courts dispose of matters at a rate limited by judicial sitting capacity. Sentencing splits custodial from community outcomes. A custodial term occupies corrections capacity for a nearly fixed service time. Release returns a person to the community, and a recidivism hazard routes a share back to offending.
- **Policy levers.** Court capacity and listing rules. Sentence lengths. Diversion programs. Parole rates. Prison bed numbers.
- **Outputs that matter.** Court backlog and waiting times. Remand and sentenced populations against capacity. Time in the system. The response of all three to a change in capacity, sentencing, or diversion.

### 9.3 What Sembla already has for it

Three parts of the existing semantics were designed with queueing in mind.

- **A queue discipline is an ordering key.** A free server is a contested resource. First-in-first-out is argmin by arrival time. Priority is argmin by severity and then arrival. Random service is argmin by a Philox draw. The conflict mechanism of section 2.7 *is* the queueing engine.
- **A capacity of several servers is a top-k selection.** Several judges or several beds generalise argmin to the best `k` rows. This is still a commutative merge, and still deterministic.
- **The saturation diagnostic already exists.** The runtime counts deferred losers for each resource and warns. In a busy queue that count is the signal that the tick approximation is biting.

One part is designed and not built: a small court box running exactly while a large population box runs approximately. Section 2.5 explains why. That hybrid needs different clocks for different boxes, and V1 has one clock.

### 9.4 Where the two drivers agree and disagree

This table is the reason the pair is useful.

| Question | Demographic driver | Justice driver | Consequence |
| --- | --- | --- | --- |
| Identity-preserving transfer | Migration as an area change | Court to corrections | Two unrelated domains want it. Strong evidence for a general primitive |
| Scheduler | Monthly ticks adequate | Dated events, such as a hearing | Measure a staged-tick baseline first. Event scheduling only on measured distortion |
| Service-time law | Memoryless hazards | Nearly fixed delays, such as a sentence | Phase-type staging or fixed delays |
| Capacity | A storage ceiling | Contention, with blocking | One claim contract must express both |
| Queue discipline | Not exercised | First-in-first-out, priorities, batch service | Extends the contest model |
| Feedback | Weak | Strong, through recidivism | Composition wires with cycles |
| Observations | Grouped marginals | Waiting-time and time-in-system distributions | Tests the observation contract |
| Households | Needed, and gated | Not needed | The gate stays the demographic model's burden |

Convergent demand can proceed. Divergent demand is exactly where arbitration bites: no primitive lands on one driver's word alone.

### 9.5 The staged plan

| Stage | What it does | Conditional |
| --- | --- | --- |
| Design and data audit | Station structure, data sources, validation targets, and a capability audit | No. It runs first |
| Tick-based baseline | A method-of-stages approximation on the existing runtime, plus comparators | No |
| Scheduler decision report | The gate. Measures the distortion of the baseline | No |
| Server pools | Acquire and release under Level A | Only if its gate line passes |
| Blocking and backpressure | Remand grows when beds are full | Only if its gate line passes |
| Queue disciplines | First-in-first-out, priorities, batch service | Only if its gate line passes |
| Validation and model card | Held-out validation against published series | No. It runs whatever the gates decided |

The comparators matter more than they look. Observed justice statistics alone cannot separate scheduler distortion from calibration error and structural error. The baseline stage therefore builds two independent yardsticks: analytical queueing cases with known answers, and an independent discrete-event run on identical inputs. The gate then measures one thing in isolation.

Each of the three conditional contracts is marked provisional and single-driver. To promote one, the project must name an independent case from a third domain that reuses the same contract.

### 9.6 Two findings already recorded

The review of this plan produced two corrections worth keeping, because both are the kind of error that is expensive later.

**The observation gap was overstated.** The plan first claimed that grouped marginals cannot express waiting times, and pointed at new observation machinery to fix it. That was too strong. A grouped view takes banded keys, and an effect can write an arithmetic expression. A queue-entry-tick column, plus a derived column for ticks waited, plus a banded grouped view over it, gives a waiting-time distribution as counts for each bucket, with no new semantics. The genuine gaps are narrower: **exact quantiles** rather than binned ones, and **per-entity event streams**. The audit must build the cheap construction and show it insufficient before proposing anything new.

**The validation targets may not be published.** The plan assumes waiting-time and time-served *distributions*. Published court and corrections statistics are usually aggregate: caseloads by court level and matter type, with duration given as a median and quartiles, and time served often only as a mean by offence category. If that is what the audit finds, the tail tolerances of the gate have no observable tail to be declared against. The audit must therefore state which targets survive on aggregate quantiles alone and which are dropped. Finding this out at the validation stage would invalidate a gate that had already admitted or rejected three contracts.

---

## 10. Future features

### 10.1 How the project chooses work

Two rules control the order.

- **Evidence comes before its gate.** The work that produces the evidence for a gate must run before the gate.
- **Two drivers arbitrate.** A demand from one driver alone gives a provisional specification. A demand from both drivers is corroborated.

A new specification that adds semantics must also declare two things: a story for each backend, and a story for the surface language. "Not mentioned" is not permitted. A construct that the IR can reach, but the author cannot write, is not shipped.

### 10.2 Performance work, next

- Reduce the winner and deferred arrays on the device. This moves about 200 MB for each tick at 5M rows to produce a few diagnostic integers.
- Run several retained backends at the same time. A spike for this is in progress.
- Answer the open CPU question at 10M slots. The leading hypothesis is NUMA first-touch placement. The collector now runs the compared arms next to each other and offers an interleaved memory option.

### 10.3 Semantics that are specified but not built

Each item below has a design and a named trigger. None of them is scheduled without its trigger.

- **Birth and death** as stream compaction. Entity IDs come from `(tick, parent, slot)`. This was the first flagged construct. The demographic driver chose the fixed-slot architecture instead, so the trigger did not fire.
- **ODE and macro blocks.** A box sub-steps internally and shows sampled values for each tick. This is the entry point for the Kurtz mean-field limit.
- **Scheduled clocks and phase-type stages** for durations that are not exponential. A scheduled clock samples a full duration at stage entry and re-checks the guard when it fires.
- **Top-k capacity and queue disciplines.** A queue discipline is an ordering key. A capacity of *c* generalises argmin to top-*k*.
- **Lookup and rate tables.** This needs an accepted amendment to §K5.
- **Time-indexed rates.** This needs the same amendment.
- **Categorical draws** at fixed coordinates. This needs an accepted amendment to §K9.
- **One restricted atomic-event mechanism.** It must give explicit participants, deterministic claiming of a row or a vacancy, combined claims, all-or-none commit, stable event identity, and defined behaviour at saturation. Births into vacant slots, matching, transfers, and household moves must all reuse this one mechanism.
- **Generation-safe references.** A `Ref` carries or validates a generation.
- **Household and kinship tables**, with synchronised household migration.
- **CUDA support for grouped observations.** Grouped observation is CPU-only today.

### 10.4 Inference and interactive widgets

The calibration method is amortized neural posterior estimation. The workflow runs outside Sembla, in an external Python pipeline. Sembla writes one thin, versioned export of parameter and summary pairs beside the run manifest. Nothing reaches into the IR.

Behaviour widgets need two latency paths. A trained flow answers in milliseconds and needs no simulation. A live prior-predictive band needs the runtime.

When named axes arrive, a run seed must come from a hash of its canonical semantic coordinate. Permuting the axis declarations must give byte-identical results.

### 10.5 Empirical data work

The demographic model is a software fixture today. It is not a calibrated population model. Nothing in it is initialised, fitted, or validated against observed data.

The empirical track adds observed stocks by age, sex, and area; mortality, fertility, and migration rates; a reconciled origin-destination matrix; and generated entrant records.

The boundary is fixed. The external pipeline owns the estimation, the transformation, and the scientific validation. Sembla owns the schemas, the validators, the provenance manifests, the adapters, and the reference fixtures. Every artifact enters through a versioned, hash-audited file.

Two contracts inside that track are load-bearing. Published rates are annual, and the model runs monthly ticks, so one document must own the conversion. Small-area death counts are sparse, so area mortality is an estimation problem, not a join. The estimation method must appear as provenance.

### 10.6 Assurance and diagnostics

- Automatic rejection of parameter sets that saturate a resource.
- Sensitivity reports for `dt` and for tau-leap convergence.
- Sensitivity reports for the one-tick lockout and for the initial slot allocation.
- A validation harness and a generated model card.
- A capability matrix generated from the conformance test suite. A hand-written matrix repeats the problem that it replaces.

### 10.7 Authoring and consolidation

- A validation error contract. An error must name the violated rule in the words of the user.
- Trace and explain tooling. It must show which rule fired and which claim won. A test must prove that it does not change the state hash.
- A component library and one polished example for each semantic family.
- A convergence policy for the legacy path and the plan path.
- A compatibility policy for the IR and the CLI, before any v1 guarantee.
- A time-boxed report on Level B determinism, written honestly either way.

### 10.8 The proof track

The proof track runs beside the milestones. It is never on the critical path. The order is cheapest first.

1. Lumping rewrite correctness. Target 1a is proved. Target 1b is open.
2. Refactoring invariance. Byte-level law tests exist for renaming and for permutation. The universal static-preservation statement is the next tractable proof.
3. The composition laws.
4. The Kurtz mean-field limit.
5. Symbolic gradient correctness, only if the differentiable fragment appears.

---

## 11. Technical names used in this document

| Name | Meaning in this project |
| --- | --- |
| ACSet | Attributed C-set. A schema of tables, reference columns, and typed attribute columns. It reads as a category-theory object and as a columnar database at the same time. |
| arrow | A reference from one table to another, in the mathematical reading of a schema. |
| functor | The map that gives rows to each table of a schema and a function to each arrow. A state is one functor. |
| schema | The fixed shape of the state: the tables, the columns, and the reference targets. |
| total function | A function with a result for every row. Each reference column is one. |
| argmin | The selection of the item with the smallest key. |
| box | One system with state, ports, transitions, and observations. |
| bundle | A frozen directory of four files that holds one linked plan and its provenance. |
| CTMC | Continuous-time Markov chain. |
| determinism level | The named strength of the reproducibility guarantee. Levels A, B, and C. |
| `dt` | The tick length. It is a semantic parameter. |
| hazard rate | The instantaneous rate of a transition. |
| IR | Intermediate representation. The versioned wire format for a model. |
| manifest | The sidecar file that records the run contract. |
| NVRTC | The NVIDIA run-time compiler for CUDA source. |
| oracle | The CPU interpreter. It defines the correct answer. |
| Philox | The counter-based random number generator. |
| plan | An executable model with stable identities. |
| prior | The declared distribution of a parameter. |
| slot | One row of a fixed pool. It can be occupied or vacant. |
| summary | A scalar reduction over the views of one run. |
| tau-leaping | The executed approximation of the CTMC. Rates freeze at the start of a tick. |
| tick | One synchronous timestep. |
| view | A scalar projection of the committed state, for each tick. |
| wire | A connection between two ports. It adds a one-tick delay. |
