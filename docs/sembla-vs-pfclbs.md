---
title: Sembla compared with PFCLBS
tags:
  - sembla
  - architecture
  - simulation
  - comparison
created: 2026-07-14
---

# Sembla compared with PFCLBS

> [!note] Scope
> This is a comparison of the two repositories as they exist in the working trees on **2026-07-14**, not only of their design aspirations. The sibling `../PFCLBS` repository calls its implementation the **Semantic Kernel Simulation System (SKS)** and exposes an `sks` binary. Both repositories are under active development, and PFCLBS had uncommitted work when inspected.

## Executive summary

Sembla and PFCLBS are close relatives in intent. Both try to put a typed, deterministic semantic layer between a model author and a high-performance simulation runtime. Both use synchronous commit boundaries, column-oriented storage, counter-based randomness, compositional wiring, validation before execution, and reproducibility as part of the model contract rather than as an afterthought.

Their main difference is where each project places its strongest abstraction:

- **Sembla is relational and proof-oriented.** Its semantic state is an ACSet-style collection of typed tables, attributes, and references. Models are authored in actual Lean 4, exported to JSON, and executed by Rust. Its long-term wager is that schema-aware composition and Lean-checked transformations can justify generated SoA kernels.
- **PFCLBS is language-, compiler-, and execution-oriented.** Its semantic state is built from typed systems, components, scalar or indexed instances, paths, updates, transitions, races, reducers, and wires. Models use a custom Lean-inspired `.sks` language. It already has a much broader parser-to-runtime toolchain, replay/calibration/UI facilities, and several specialized execution/code-generation paths.

As a result, **PFCLBS is currently much broader and more mature as an executable simulation platform**, while **Sembla has the cleaner route to a relational semantics and genuine proof-assistant integration**. The latter is architectural potential, not a claim that Sembla already has a certified compiler: its current Lean layer provides the DSL, IR construction, export, tests, and ProofWidgets, but not yet the planned body of semantic proofs or certified kernel rewrites.

There is no controlled cross-repository benchmark in this note. PFCLBS has substantially more performance infrastructure and benchmark evidence, but that alone does not establish a universal speed ratio for equivalent models.

## At a glance

| Dimension | Sembla | PFCLBS / SKS |
|---|---|---|
| Primary goal | Lean-authored, Rust-executed stochastic relational simulation | General typed semantic-kernel simulation system with a broad runtime/tooling stack |
| Authoring language | Actual Lean 4 DSL/macros | Custom Lean-inspired `.sks` syntax |
| Authoring pipeline | Lean elaboration → canonical JSON IR → Rust validation/runtime | Parser → source AST → resolved/validated IR → runtime plan → selected backend |
| Semantic state | ACSet-style entity tables, typed attributes, and reference columns | Typed system state, components, scalar/indexed instances, and path-addressed values |
| Physical state | Deterministic SoA/columnar table storage | Flattened scalar/indexed columnar storage with explicit storage and backend manifests |
| Dynamics | Guarded row-local hazard transitions and effects | Deterministic updates, transitions, probability/rate races, reducers, and broader expression forms |
| Time | Fixed-`dt`, snapshot-isolated tau-leaping; ideal CTMC meaning is distinguished from execution | Step-based execution with explicit scheduling/temporal forms and multiple race modes |
| Composition | Boxes with typed table-valued inputs/outputs and delayed wires; operadic design | Systems/components/interfaces, exposed ports, instances, keyed and staged wires, execution graphs |
| Randomness | Philox keyed by stable simulation coordinates; CRN comparison support | Semantic draw-site keys plus Philox, draw manifests, archive/replay verification, backend conformance |
| Validation | Rust IR validator plus Lean construction/parity tests | Dedicated syntax, IR, and validation crates with units, refinements, invariants, effects, and extensive diagnostics |
| Execution today | Deterministic CPU interpreter; GPU work is a standalone spike | Interpreter and prepared/specialized CPU paths, generated/hybrid infrastructure, SIMD work, and bounded WGSL/WebGPU kernel families with fallback |
| Calibration/tooling | Parameter priors, overrides, SIR-focused sweeps, CLI CSV, ProofWidgets | Rich parameters, calibration export, posterior import, experiments, replay, UI backend, browser UI, and many artifacts |
| Current model breadth | Focused stochastic finite-state and policy microsimulation examples | Broad deterministic/stochastic examples including epidemiology, queues, population models, CA/lattices, flocking, and calibration |
| Formal status | Real Lean frontend; planned semantics/proofs are not yet completed | No proof-assistant kernel; assurance is primarily validation, differential tests, goldens, and replay evidence |

## Important similarities

### 1. A typed IR is the narrow waist

Neither project treats model source as an executable script that directly mutates arbitrary runtime objects. Both lower models into a typed intermediate representation and validate that representation before execution. This separation supports stable identities, diagnostics, alternate backends, canonical artifacts, and differential testing.

The difference is not whether an IR exists, but what it considers primitive: Sembla starts from schemas and tables; PFCLBS starts from systems, values, operations, and indexed instances.

### 2. Snapshot/commit semantics

Both avoid order-dependent “last writer wins” execution. A step reads from a committed snapshot, computes candidates or updates, resolves conflicts under deterministic rules, and commits at a defined boundary. This makes physical loop order less semantically visible and creates a sound basis for vectorization or parallel execution.

### 3. Column-oriented execution

Both recognize that large indexed populations should execute over homogeneous columns rather than object graphs:

- Sembla stores each table attribute or reference as a deterministic column.
- PFCLBS flattens indexed system state into typed storage columns and records storage/backend compatibility in runtime manifests.

This gives both projects a plausible path to SIMD and GPU kernels. In Sembla, however, tables and columns are also the semantic ontology; in PFCLBS they are more clearly a lowering of the typed system model.

### 4. Order-independent randomness

Both use counter-based Philox randomness instead of a mutable RNG stream. A draw is addressed by semantic coordinates, so loop reordering need not change which random number belongs to which model event.

Sembla currently emphasizes stable `(tick, rule, entity, draw)`-style coordinates and common-random-number comparisons. PFCLBS goes further operationally with named draw sites, semantic hashes, draw manifests, replay bundles, verifier paths, and CPU/GPU conformance evidence.

### 5. Composition is semantic, not merely software modularity

Both projects model open systems with typed interfaces and explicit wiring. Both also draw on polynomial/open-system ideas rather than treating composition as an untyped callback graph.

- In Sembla, boxes own relational state and wires carry table-shaped observations between box boundaries.
- In PFCLBS, systems can contain components, expose interface paths, instantiate scalar or indexed systems, and communicate through ordinary, keyed, or staged wires.

### 6. Reproducibility and evidence are first-class

Canonical serialization, stable identities, deterministic output, negative fixtures, golden files, differential checks, and runtime metadata are visible in both repositories. PFCLBS has a much larger evidence surface; Sembla's smaller suite is easier to audit end to end.

## Fundamental differences

### 1. Actual Lean versus Lean-inspired syntax

This is the sharpest distinction.

Sembla models are Lean values constructed by Lean macros. Lean performs parsing and elaboration, the project can show structure through ProofWidgets, and future semantic definitions and theorems can live in the same language as the model frontend. This avoids building a standalone parser and gives Sembla a genuine route to machine-checked transformation correctness.

PFCLBS deliberately uses syntax that resembles Lean but is not Lean. That makes the language easier to tailor to simulation users and allows a conventional Rust toolchain, but PFCLBS must implement and maintain its own parser, source spans, name resolution, type checker, fix-it diagnostics, formatter conventions, and semantic-versioning rules. Its assurance comes from executable validation and testing rather than a proof kernel.

The trade-off is practical:

- Sembla gains elaboration and proof potential but requires a Lean/Rust two-stage workflow and exporter parity checks.
- PFCLBS gains control over user syntax and deployment but pays a large language-engineering cost.

### 2. Relational schemas versus hierarchical system state

Sembla's state model is explicitly ACSet-like. Different entity kinds are separate tables; typed references are foreign keys; attributes are columns; grouped aggregates can follow references. This naturally represents people, employers, households, networks, and other heterogeneous relational structures without making every individual a separately wired system.

PFCLBS represents state through system types, component fields, paths, and fixed indexed domains. This is natural for scalar dynamical systems, product systems, arrays of agents, vectors, lattices, and compiler specialization. Keyed wires and grouped reducers recover many relational operations, but relationships are not organized around a first-class general schema-and-schema-map layer.

Consequences:

- Sembla has the more uniform ontology for heterogeneous relational microsimulation and future schema transformations.
- PFCLBS has the more developed ontology for general typed numerical state and hierarchical composition.
- Sembla does **not** yet implement general ACSet schema morphisms, data migrations, or arbitrary relational query planning, so this advantage is still partly prospective.

### 3. A narrower stochastic core versus a broader dynamics language

Sembla's current runtime is centered on guarded, row-local transitions with hazards and effects. Rates are interpreted through fixed-`dt`, snapshot-isolated tau-leaping. This is a small, explicit semantic core that fits finite-state CTMC-like microsimulation well.

PFCLBS supports a broader mixture of deterministic updates, definitions, reducers, probability races, rate/hazard races, schedules, temporal reads, and richer numeric expressions. Its demonstration suite spans both agent and aggregate formulations, queues, cellular automata, lattices, and vector/spatial models.

The narrow Sembla core is easier to explain and eventually formalize. PFCLBS's broader core is more useful today, but it creates more interactions among timing, scheduling, randomness, replay identity, storage, and backend support.

### 4. Different composition maturity

Sembla's box/table/wire design is conceptually strong, but the implemented wire layer is intentionally basic. Current examples demonstrate delayed table-valued feedback, while keyed/grouped interfaces, duplicate/missing-key policies, richer schedules, and schema transformations remain future work.

PFCLBS already contains interfaces, component composition, exposed paths, indexed instances, keyed wiring, grouped reducers, staging, invariants, and extensive validation around these features. It therefore has the current advantage for complex executable compositions.

Sembla's potential advantage is that composition can eventually be stated as operations on relational schemas and given a Lean meaning. PFCLBS's advantage is that more of its composition design is already represented in parser, IR, validator, runtime, tests, and examples.

### 5. Different reproducibility products

Sembla currently offers deterministic state layout/hashes, deterministic CSV runs, stable rule identities, seeded sweeps, and common-random-number model or parameter comparisons.

PFCLBS treats replay as a larger product surface: semantic source/IR identity, runtime-plan and backend identity, draw metadata, captured views, archives, verification, old-archive policies, and UI/backend replay contracts. This is substantially more mature for audit trails and long-lived run provenance.

Sembla's CRN workflow is nevertheless unusually direct for counterfactual policy work: matching semantic coordinates across two arms receive matching draws without sharing a mutable RNG state.

### 6. Different optimization maturity

Sembla currently executes through a deterministic Rust CPU interpreter. Its design describes generated SoA kernels and certified rewrites, and its GPU PRD is a useful throughput spike, but that GPU code is deliberately not part of the production runtime.

PFCLBS has a much larger performance implementation:

- runtime plans and execution profiles;
- tree-walked, prepared, specialized, generated, and hybrid execution infrastructure;
- dense/indexed vector kernels and architecture-specific SIMD evidence;
- explicit storage/backend manifests and fallback policy;
- isolated WGSL generation, no-device validation, and optional WebGPU execution for supported kernel patterns;
- differential and replay gates intended to keep accelerated paths semantically aligned.

This does **not** mean every PFCLBS model runs on every accelerated backend. Its GPU and vector support is capability- and pattern-dependent, and fallback behavior is part of the contract. The default workspace even excludes the separate `sks_webgpu_backend` crate. PFCLBS's advantage is a real, tested specialization framework; its disadvantage is a larger compatibility matrix and more backend-conditioned behavior.

### 7. Different product breadth

PFCLBS already includes or exposes substantial work for:

- custom syntax and diagnostics;
- units, refinements, invariants, and richer carriers;
- runtime profiles and manifests;
- replay archives and verification;
- calibration-schema export and posterior import;
- experiment matrices;
- a UI backend and browser UI;
- a large demonstration and benchmark-artifact corpus;
- feature-gated bounded lifecycle operations such as deterministic slot reuse for spawn/retire.

Sembla is intentionally smaller. It has a Lean frontend, JSON exporter, ProofWidgets, Rust validator/runtime/CLI, focused examples, prior metadata, parameter overrides, a SIR-oriented sweep/compare workflow, and generic finite-state CSV output. It currently lacks changing table row counts, general multi-row atomic effects, keyed wires, general replay archives, production CPU/GPU code generation, and broad calibration/UI infrastructure.

## Sembla: advantages and disadvantages

### Advantages

1. **A real proof-assistant frontend.** Model construction, future semantics, and future correctness theorems can share Lean's type theory and elaborator.
2. **A strong relational state ontology.** Tables, attributes, and references are a natural fit for heterogeneous populations, networks, and administrative microsimulation.
3. **Clear stochastic honesty.** The documentation distinguishes ideal CTMC meaning from fixed-`dt`, snapshot-isolated tau-leaping and treats `dt` as semantic.
4. **Small conceptual surface.** Three Rust crates plus a focused Lean frontend make the full pipeline comparatively easy to inspect.
5. **Readable interchange boundary.** Canonical JSON makes the Lean/Rust seam explicit and testable.
6. **Direct policy-comparison support.** Stable Philox coordinates make paired, common-random-number comparisons a natural operation.
7. **Good local authoring feedback.** ProofWidgets can explain model structure where the model is written rather than only after compilation.

### Disadvantages

1. **Formal promise exceeds formal implementation.** There is not yet a completed meaning function, proof library, or certified rewrite/code-generation pipeline commensurate with the design document.
2. **Limited dynamics.** Current execution is strongest for fixed-size, finite-state, row-local stochastic transitions, not general ODE/PDE, spatial, queueing, or multi-entity atomic dynamics.
3. **Limited composition runtime.** Table-valued wires exist, but keyed/grouped wiring and richer scheduling semantics do not yet.
4. **No production accelerated backend.** The GPU work is a spike; generated CPU/GPU kernels remain future architecture.
5. **Narrow lifecycle and initialization.** Tables have fixed row counts, numeric initialization is constrained, and one effect cannot atomically update arbitrary multiple rows.
6. **Narrower tooling.** Replay archives, provenance manifests, experiments, generic calibration, and a standalone runtime UI are much less developed.
7. **Two-language operational cost.** Contributors and CI must keep Lean construction, exported JSON, Rust validation, and checked-in artifacts in parity.

## PFCLBS: advantages and disadvantages

### Advantages

1. **Much greater implementation breadth.** The repository has dedicated crates for syntax, IR, validation, runtime, replay, calibration, CPU code generation, GPU code generation, CLI, UI backend, and examples.
2. **A mature compiler-shaped pipeline.** Source spans, diagnostics, resolved/validated forms, runtime planning, capability decisions, and backend fallback are explicit.
3. **Broader model expressiveness.** Deterministic and stochastic models, scalar and indexed systems, components, vectors, reducers, keyed wires, invariants, schedules, lattices, and feature-gated lifecycle operations are represented in code and examples.
4. **Stronger run provenance and replay.** Semantic identities, archives, draw manifests, captured outputs, and replay verification form a coherent audit surface.
5. **More advanced performance engineering.** Specialized CPU/GPU paths, manifests, differential tests, and benchmark artifacts exist now rather than only in a roadmap.
6. **More complete user-facing workflow.** CLI commands, experiments, calibration export/import, UI backend, and browser panels cover more of a simulation project's life cycle.
7. **Large validation corpus.** The many positive, negative, golden, differential, and evidence tests expose subtle semantic and backend cases.

### Disadvantages

1. **High system complexity.** The semantic surface, crate graph, profile matrix, feature flags, generated artifacts, and backend policies impose a substantial maintenance and learning burden.
2. **No machine-checked semantic foundation.** Lean-like syntax should not be confused with Lean proofs; transformation correctness rests on implementation discipline and tests.
3. **A bespoke language is expensive.** PFCLBS owns parsing, elaboration-like behavior, diagnostics, syntax evolution, and tooling that Sembla delegates partly to Lean.
4. **Less direct relational ontology.** Hierarchical/indexed system state is excellent for many numerical models but less canonical than ACSet schemas for heterogeneous foreign-key-rich data.
5. **Backend support is conditional.** Accelerated paths cover selected operation/storage shapes and may reject or fall back; users must understand manifests to know what actually ran.
6. **Documentation is difficult to enter.** The repository has no top-level `README.md` in the inspected working tree, while authoritative material is distributed among `archive/`, milestone plans, PRDs, evidence directories, demonstration notes, and crate tests.
7. **Breadth can obscure the semantic core.** It is harder to tell which features are stable defaults, feature-gated extensions, evidence-only spikes, or future designs without tracing code and acceptance artifacts.

## Architectural assessment

The projects are not simply two implementations of the same language. They embody different optimization wagers:

- **Sembla:** make relational structure and semantics primary, then derive efficient kernels and prove selected rewrites.
- **PFCLBS:** build a rich typed language and runtime-plan pipeline, then preserve semantics across increasingly specialized execution tiers through identities, manifests, tests, and replay.

For a project whose central problem is **heterogeneous relational microsimulation with a credible path to formal semantics**, Sembla is the more distinctive foundation. For a project that must **run a broad range of typed dynamical models with replay, UI, calibration, and optimized backends now**, PFCLBS is currently the stronger platform.

The strongest long-term design would not blindly merge the repositories. A better strategy is to let each inform the other:

### What Sembla should borrow from PFCLBS

1. Versioned semantic identities and replay manifests.
2. A visible validated-IR → runtime-plan boundary with capability and fallback reporting.
3. Keyed wires, grouped reducers, and explicit missing/duplicate-key semantics.
4. Richer units, refinements, invariants, and diagnostics.
5. Differential gates for every optimized backend.
6. Execution profiles that separate assurance work from hot-path work without changing semantics.
7. Bounded lifecycle semantics with stable generation identities, if changing populations become a priority.

### What Sembla should avoid copying too early

1. A second custom source parser—the actual Lean frontend is one of Sembla's main differentiators.
2. Backend proliferation before a small normalized kernel IR exists.
3. Feature breadth that outruns the ability to give each construct a precise Lean meaning.
4. A large profile/feature matrix before ordinary users can see exactly which semantics and backend they selected.

### What PFCLBS could borrow from Sembla

1. An explicit ACSet/schema layer for relational entities and foreign keys.
2. An optional actual-Lean frontend for model elaboration and selected certified transformations.
3. A sharper distinction between ideal stochastic meaning and each executed approximation.
4. A smaller top-level architectural guide separating stable implementation, feature-gated implementation, experiments, and roadmap.
5. Schema transformations as named semantic operations rather than only path/index rewrites.

## Practical recommendation

- Choose **Sembla** when Lean authoring, relational schemas, inspectable stochastic semantics, and future proofs are requirements—and the current fixed-size finite-state scope is acceptable.
- Choose **PFCLBS** when model breadth, runtime features, replay/provenance, calibration, UI, and existing optimization work matter more than proof-assistant integration.
- Use **PFCLBS as an implementation reference for Sembla**, especially for replay identity, runtime planning, backend manifests, keyed composition, validation diagnostics, and differential performance gates.
- Do not claim Sembla's planned formal or generated-kernel advantages until there are theorem-bearing Lean semantics and production code-generation paths to demonstrate them.

## Evidence map

### Sembla

- `README.md` — implemented workflow, examples, and workspace layout.
- `DESIGN.md` — ACSet state, bulk tick semantics, hazard races, composition, determinism, and intended Lean meaning.
- `frontend/README.md` — actual Lean authoring/export workflow and current boundaries.
- `frontend/Sembla/DSL.lean`, `IR.lean`, `Json.lean`, and `Models.lean` — Lean DSL and model construction.
- `crates/sembla-ir/` — serialized IR and validation.
- `crates/sembla-runtime/` — columnar state, Philox, expression evaluation, and tick execution.
- `crates/sembla-cli/` — validation, run, compare, sweep, and CSV behavior.
- `docs/examples/` — executable SIR, policy-feedback, and canonical finite-state examples.
- `docs/prds/0012-gpu-spike.md` — deliberately isolated GPU experiment rather than a production backend.

### PFCLBS / SKS

Paths below are relative to the Sembla repository root.

- `../PFCLBS/Cargo.toml` — current multi-crate workspace and the excluded WebGPU backend boundary.
- `../PFCLBS/archive/01-semantic-core-and-ir.md` — intended semantic core and IR layers.
- `../PFCLBS/archive/02-surface-syntax.md` — Lean-inspired custom language and system model.
- `../PFCLBS/archive/08-runtime-performance.md` — runtime, storage, execution profiles, and backend design.
- `../PFCLBS/crates/sks_syntax/`, `sks_ir/`, and `sks_validate/` — implemented language and validation pipeline.
- `../PFCLBS/crates/sks_runtime/` — runtime plans, storage, execution profiles, and specialized paths.
- `../PFCLBS/crates/sks_replay/` — replay archive and verification surface.
- `../PFCLBS/crates/sks_calibration/` — calibration schema/export support.
- `../PFCLBS/crates/sks_codegen/` and `sks_gpu_codegen/` — generated CPU and WGSL kernel infrastructure.
- `../PFCLBS/apps/sks-ui/README.md` and `../PFCLBS/crates/sks_ui_backend/` — browser UI and backend contract.
- `../PFCLBS/demonstration_models/` — broad executable model and evidence corpus.
- `../PFCLBS/docs/milestone-implementation-plans/feature-flag-convention.md` — default-off extension policy.
- `../PFCLBS/docs/milestone-implementation-plans/er12b-lifecycle-design.md` — bounded spawn/retire lifecycle semantics and implementation split.
