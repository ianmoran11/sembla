---
title: Sembla User Guide
created: 2026-08-12
tags:
  - Sembla
  - user-guide
  - simulation
  - lean
  - rust
source_commit: 12af5523031889a6fd7050c2186358db0d880bff
---

# Sembla User Guide

> [!abstract] What this guide is
> A task-oriented walkthrough of Sembla for people who want to author, run, calibrate, and verify models. It covers the whole pipeline end to end and links out to the reference documents rather than restating them.
>
> **Status:** descriptive. Normative semantics live in [`DESIGN.md`](../DESIGN.md); adopted amendments live in [`DECISIONS.md`](../DECISIONS.md). Where this guide and those documents disagree, they win.

## Contents

1. [[#1. What Sembla is]]
2. [[#2. The mental model]]
3. [[#3. Install and build]]
4. [[#4. Quick start]]
5. [[#5. Populations and initial state]]
6. [[#6. Authoring models in Lean]]
7. [[#7. The mathematical surface]]
8. [[#8. Export and validate]]
9. [[#9. Composition]]
10. [[#10. Command reference]]
11. [[#11. Output artifacts]]
12. [[#12. Calibration workflow]]
13. [[#13. Reproducibility and verification]]
14. [[#14. Backends]]
15. [[#15. Widgets in the Lean infoview]]
16. [[#16. Proofs]]
17. [[#17. Limits and boundaries]]
18. [[#18. Troubleshooting]]
19. [[#19. Where to go next]]
20. [[#20. Glossary]]

---

## 1. What Sembla is

Sembla is a semantics-first simulation framework for large stochastic systems. You author a model in a Lean 4 frontend, export it as a versioned intermediate representation (IR) or composition source, link it into a canonical plan, and execute it on a deterministic Rust CPU backend or a CUDA backend.

The central promise is a single contract:

> **seed + model or plan + parameter vector + execution contract ⇒ reproducible result**

> [!warning] Reproducibility is not validity
> Sembla can record exactly what a model means and how a result was produced. It cannot tell you whether the model describes the world. Validation against reality remains a scientific responsibility that lives outside the tool.

### Who this is for

| You are | Read | Then |
| --- | --- | --- |
| Evaluating Sembla | [[#2. The mental model]], [[#4. Quick start]] | [Project overview](overview.md) |
| Authoring a model | [[#6. Authoring models in Lean]], [[#7. The mathematical surface]] | [Lean frontend](https://github.com/ianmoran11/sembla-lean/blob/main/README.md) |
| Running experiments | [[#10. Command reference]], [[#12. Calibration workflow]] | [SIR example](examples/sir.md) |
| Building reusable components | [[#9. Composition]] | [Composition guide](guides/composition.md) |
| Auditing a result | [[#13. Reproducibility and verification]] | [State artifacts](guides/state-format.md) |

---

## 2. The mental model

Five ideas explain almost everything about how Sembla behaves. Internalise these and the rest of the system stops surprising you.

### 2.1 State is typed columnar tables inside boxes

A model is a set of **boxes**. Each box owns typed **tables** (declared as `system`s). Rows are entities or resources. Attributes are `Real`, `Int`, `Enum`, or a `Ref` to a row in another table *in the same box*.

The semantic state and the runtime struct-of-arrays layout deliberately have the same shape, so there is no hidden translation layer between what a model means and how it executes.

### 2.2 One tick is read-old, write-new

Every tick runs this sequence:

1. read a frozen snapshot of committed state;
2. evaluate guards and hazards against that snapshot;
3. sample candidate firing times from stable coordinates;
4. resolve resource contests canonically;
5. stage effects;
6. reject double writes;
7. commit simultaneously at the tick barrier;
8. evaluate observation sinks.

> [!important] The consequence that matters
> A transition **cannot** see another transition's newly written state within the same tick. Rates are frozen at tick start; there are no within-tick cascades. This is what makes work partitioning and evaluation order semantically invisible where declared operations are order-free or canonically ordered.

### 2.3 Randomness is addressed, not consumed

Random draws are not pulled from a mutable stream. They are **addressed by stable coordinates**: a rule's occurrence identity, the tick, the entity row, and a draw index together determine its entropy (via counter-addressed Philox).

Two payoffs follow directly:

- **Deterministic replay.** The same coordinates always yield the same draw.
- **Common random numbers (CRN).** Two model variants that share a component keep identical draws for that component, so a counterfactual contrast is *exactly* attributable rather than statistically noisy.

### 2.4 Boxes communicate with a one-tick delay

Boxes keep private state and exchange values through **mailboxes**. An ordinary wire always adds exactly one tick:

$$A \xrightarrow{\text{wire}} B \qquad\Longrightarrow\qquad u_B(t+1) = y_A(t).$$

This is explicit semantic state, not an implementation latency. Putting two things in the same box does not create same-tick causality either, because every box is double-buffered: $(x_{t+1}, y_t) = F(x_t, u_t)$.

### 2.5 Observation is a sink

Views, grouped views, and summaries are evaluated from committed state *after* the tick. They **cannot** feed transitions, consume random draws, affect conflict resolution, or change scheduling. The only way observation influences dynamics is explicitly, through an output port, a wire, and its one-tick delay.

---

## 3. Install and build

### 3.1 Prerequisites

| Tool | Purpose | Notes |
| --- | --- | --- |
| Rust / Cargo | runtime, IR, CLI | pinned by `rust-toolchain` |
| [elan](https://github.com/leanprover/elan) | Lean toolchain manager | `sembla-lean/lean-toolchain` pins Lean 4.13.0, selected automatically |
| Git | check scripts and compatibility | required by the strict checks |
| CUDA + NVRTC | optional GPU backend | only for `--backend cuda` and GPU evidence |
| Python | reduced NPE smoke test, import checks | optional |

### 3.2 Build

```sh
# Rust side
cargo build --release

# Lean side, from a sibling frontend checkout
(cd ../sembla-lean && lake build)
```

`lake` resolves pinned Mathlib and ProofWidgets4 dependencies; the complete
transitive revisions are recorded in `sembla-lean/lake-manifest.json`.

The Lean build produces two libraries. `Sembla` is the production import surface used by `sembla-export` and `sembla-link`. `SemblaTests` holds the compile-time test and model-validation corpus, kept out of executable import closures.

### 3.3 Check contracts

```sh
./scripts/check-rust.sh   # fast, Rust only, no Lean required
./scripts/check.sh        # strict complete repository contract
```

The backend strict check requires Cargo, Git, and Python. Lean build, proof
hygiene, and backend compatibility run independently in `sembla-lean`. The
canonical matrix — including determinism, reduced NPE smoke, and manual GPU
evidence — is in [CI and local checks](contributing/ci.md#local-check-contract).

```sh
cargo run -p sembla-cli -- --version
```

---

## 4. Quick start

### 4.1 Validate and run a shipped model

```sh
cargo run -p sembla-cli -- validate examples/two_state.json
cargo run -p sembla-cli -- run examples/two_state.json \
  --population 1000 --seed 55 --ticks 40 --out results.csv
```

That single `run` writes three files and prints three hashes:

| File | Content |
| --- | --- |
| `results.csv` | per-tick observation columns |
| `results.csv.summaries.csv` | `name,value` rows in declaration order |
| `results.csv.manifest.json` | the canonical reproducibility record |

Stdout ends with SHA-256 digests of the exact result bytes, the final columnar state, and the observation summary bytes.

### 4.2 The flagship end-to-end example

The workplace SIR model uses a frequency-dependent hazard `beta * I_workplace / N_workplace` and a recovery hazard `gamma`.

```sh
# 1. deterministic synthetic population
cargo run --release -p sembla-cli -- synth-pop \
  --persons 1000000 --employers 50000 --initial-infected 100 \
  --seed 2025 --out pop.bin

# 2. run with explicit parameters
printf '{"beta":0.8,"gamma":0.1}\n' > params.json
cargo run --release -p sembla-cli -- run examples/sir.json \
  --population pop.bin --seed 99 --ticks 100 --dt 0.25 \
  --params params.json --out results.csv

# 3. verify the recorded run reproduces
cargo run --release -p sembla-cli -- verify-run \
  results.csv.manifest.json examples/sir.json --population pop.bin
```

Full walkthrough: [workplace SIR guide](examples/sir.md).

### 4.3 Prove determinism to yourself

```sh
cargo run --release -p sembla-cli -- run examples/sir.json --population pop.bin \
  --seed 99 --ticks 100 --params params.json --out first.csv  > first.hashes
cargo run --release -p sembla-cli -- run examples/sir.json --population pop.bin \
  --seed 99 --ticks 100 --params params.json --out second.csv > second.hashes
cmp first.csv second.csv
cmp first.hashes second.hashes
```

Both commands must print identical hashes and produce byte-identical CSV, summaries, and manifest sidecars. Change `--seed` or any value in `params.json` and the result and final-state hashes change.

### 4.4 Export, link, and run a composition

```sh
(cd ../sembla-lean && lake exe sembla-export --source surface_epidemic_policy \
  /tmp/epidemic_policy.source.json)
(cd ../sembla-lean && lake exe sembla-link /tmp/epidemic_policy.source.json \
  --plan /tmp/epidemic_policy.plan.json)
cargo run -p sembla-cli -- run /tmp/epidemic_policy.plan.json \
  --population 1000 --seed 55 --ticks 40
```

---

## 5. Populations and initial state

Every command taking `--population` dispatches **by content, not by file extension**:

```text
--population 1000              # numeric initialization
--population population.bin    # frozen SEMBLA_POP compatibility path
--population initial.state     # generic SEMBLA_STATE artifact
```

### 5.1 Numeric initialization

`--population N` gives every declared table `N` rows. Every enum column starts at variant index zero, real and integer columns start at zero, and reference columns point to row zero of their target table.

> [!note] This is a real modelling decision, not a default to ignore
> Homogeneous initialization means the CTMC starts entirely in `A`, the decay chain entirely in `Parent`, SIS and SEIRS entirely in `S`. Canonical models carry positive `import_rate` / `mutation_rate` defaults precisely so the interesting states stay reachable — those defaults are part of the stated model, not hidden initial-condition machinery.

Note also that a system's `(rows := N)` declaration is an IR `Table.sizeHint`, **not** runtime population initialization.

### 5.2 `synth-pop` — the frozen SIR population

```sh
cargo run --release -p sembla-cli -- synth-pop \
  --persons 1000000 --employers 50000 --initial-infected 100 \
  --seed 2025 --out pop.bin
```

`pop.bin` is portable and versioned little-endian: 12-byte `SEMBLA_POP\0\0` magic, `u32` version, `u64` person and employer counts, then all `u16` health indices and all `u32` employer references in person-row order. The loader rejects wrong magic or version, truncation, trailing data, invalid health indices, and out-of-range employer references.

Generation uses only Philox coordinates: rule ID `0xffff_ff00` for workplace assignment, `0xffff_ff01` for the initial-infection Fisher–Yates permutation. Workplace assignment is `floor(E * U^2)` — a documented power-law-ish bucketing that is deterministic, not a demographic claim.

### 5.3 `synth-state` — generic benchmark state

```sh
cargo run -p sembla-cli -- synth-state --model model-or-plan.json \
  --slots N --areas K --present-fraction F \
  --streams birth:B,overseas:O,internal:I --seed S --out state.artifact
```

Benchmark and test tooling for the documented demographic column roles. It also emits `state.artifact.model.json`.

### 5.4 State artifacts (`sembla.state/v1`)

Written by `run --export-state`, loadable by any later `--population`.

The frozen binary layout is:

1. **Magic** — the 12 bytes `SEMBLA_STATE`.
2. **Header length** — little-endian `u32`.
3. **Header** — canonical JSON (`sembla.canonical-json/v1`: sorted keys, compact, no trailing newline) of exactly `{"schema_version": "sembla.state/v1", "tables": [...]}`. Each table entry carries `box`, `table`, `row_count`, and `columns`, where a column is `{"name", "type": "real"|"int"|"enum"|"ref", "variant_count"?, "ref_target"?}`. `variant_count` is present iff enum; `ref_target` is present iff ref. Tables and columns appear in **model declaration order**.
4. **Column blobs** — immediately after the header, in header order, raw little-endian arrays matching the runtime's `ColumnData` exactly: `real` = `f64`, `int` = `i64`, `enum` = `u16` variant indices in declaration order, `ref` = `u32` row indices. No padding, alignment, compression, or trailing data.

Hash it with:

```sh
sembla state-hash initial.state
# state sha256 sembla.state-artifact/v1 <64-lowercase-hex-digest>
```

The artifact itself is execution-metadata-free. Run manifests record links separately via optional `initial_state` and `exported_state` tuples, each wholly present or wholly absent.

### 5.5 Chained runs

```sh
sembla run model.json --population initial.state --seed 101 --ticks 12 \
  --params year-1.json --out year-1.csv --export-state year-1.state
sembla run model.json --population year-1.state --seed 202 --ticks 12 \
  --params year-2.json --out year-2.csv --export-state year-2.state
```

`year-1.csv.manifest.json` records `exported_state` for `year-1.state`; `year-2.csv.manifest.json` records the same hash under `initial_state`, plus its own seed, ticks, and resolved θ.

An existing export path is **rejected rather than overwritten** — state artifacts are chain links, and silently replacing one would invalidate the recorded chain.

> [!warning] Chaining is not checkpoint/restart
> Two 12-tick runs are **not** bitwise-equivalent to one continuous 24-tick run, even with the same seed and θ. Tick coordinates restart at zero in the second run, so its counter-based draws intentionally differ. The manifests describe two honest run identities; they do not claim calendar or RNG continuity.

Details: [state artifacts](guides/state-format.md).

---

## 6. Authoring models in Lean

Human-authored models use the `sembla_model` command. It defines an ordinary namespace-respecting `Sembla.IR.Model` constant. `(dt := ...)` is mandatory; `(name := ...)` is optional and sets the exact runtime model name.

### 6.1 A complete model

```lean
namespace Example
open Sembla.IR Sembla.DSL

sembla_model WorkplacePolicy
    (name := "workplace_policy")
    (dt := 0.25) where
  param β : ℝ := 0.8 ~ LogNormal (-0.2231435513142097) 0.25
  param γ : ℝ := 0.1

  box population where
    system Person (rows := 1_000) where
      health : {S, I, R}
      risk : ℝ
      visits : Int
      employer : Employer
    system Employer (rows := 50)

    input restriction where
      modifier : ℝ

    infect on Person : health: S →[
      β · freq (health = I) over employer ·
        (1.0 + inputSum restriction field modifier)
    ] I
    recover on Person : health: I →[γ] R

    output activity from Person where
      infected : Int := count where health = I
      total_risk : ℝ := sum (risk)

    views Person where
      infectious := count where health = I
      total_risk := sum risk
      minimum_visits := min visits
      active_risk_max := max risk where health = I

  box policy where
    system Controller (rows := 1) where
      mode : {Open, Restricted}
      modifier : ℝ

    input activity where
      infected : Int
      total_risk : ℝ

    transition restrict on Controller where
      guard mode = Open ∧ inputSum activity field infected > 100
      hazard 1e300
      set mode := Restricted
      set modifier := 0.4

    output restriction from Controller where
      modifier : ℝ := sum (modifier - 1.0)

  wire population activity -> policy activity
  wire policy restriction -> population restriction

  summaries population where
    peak_I := max infectious
    peak_tick := argmaxₜ infectious

end Example
```

> [!tip] Syntax ergonomics
> There are no list brackets, separator commas, or required empty category blocks. Model and box declarations may be interleaved. Collection is multi-pass, so parameters, `Ref` target systems, ports, and views may be referenced **before** they are declared, wherever the semantic kernel permits it.
>
> Every emitted IR category is a stable partition of source declarations: relative textual order is preserved within params, boxes, systems, transitions, fields, effects, views, wires, and summaries.

### 6.2 Names

Without a `(name := ...)` override, the Lean identifier is converted by the frozen snake-case derivation: `SirWorkplace` → `sir_workplace`, system `Person` → table `person`. Greek identifiers use documented transliterations — `β`, `γ`, `σ`, `«λ_parent»` become `beta`, `gamma`, `sigma`, `lambda_parent`.

Derivation collisions are **errors**, never resolved by iteration order. Use `(name := "...")` whenever the wire-format name must differ.

### 6.3 Parameters

```lean
param β : ℝ := 0.8 ~ LogNormal (-0.2231435513142097) 0.25
param γ : ℝ := 0.1
param retirement_months : Int := 780
param offset : Int := -3
```

A parameter binds its identifier throughout the model; references are bare names, not substituted defaults. Defaults and optional `Normal` / `LogNormal` priors on real parameters are first-class IR metadata.

> [!warning] Integer parameters cannot carry priors
> `Int` parameters lower to the integer IR parameter type and work anywhere the scalar typechecker accepts `Int`, including guards and integer `set` effects. Priors remain real-valued: attaching any prior to an `Int` parameter is rejected.

Real values are stored as exact coefficient/exponent `Scientific` data, preserving supported finite `f64`-range decimals.

### 6.4 Expressions

| Category | Accepted |
| --- | --- |
| Mathematical | `·` (multiply), `∧`, `¬`, `≠`, `≤`, `≥` |
| ASCII | `*`, `/`, `+`, `-`, `=`, `<`, `>`, `&&` |
| Deliberately absent | ASCII `!=`, `<=`, `>=` |
| Also available | numeric arithmetic, enum comparisons, `inputSum`, restricted aggregates |

### 6.5 Systems and attributes

Every system requires `(rows := <natural-number literal>)`. A nonempty system uses an indented `where` block; an empty system omits it. Attribute types are inferred from exactly these forms:

```lean
state : {Open, Restricted}  -- enum
risk : ℝ                     -- real
visits : Int                 -- integer
employer : Employer          -- Ref to a collected system
```

Enum variants and attributes retain textual order. `Ref` targets must be systems in the same box, but may be declared later.

### 6.6 Transitions

**Reaction arrows** cover the common case — exactly one enum equality guard and one write to that enum attribute:

```lean
infect on Person : health: S →[β · freq (health = I) over employer] I
recover on Person : health: I →[γ] R
```

`on Person` may be omitted only when the compatible system is unique. The state attribute may be omitted only when the selected system has exactly one enum attribute. Explicit `on Person` and `health:` are the stable disambiguation forms; ambiguous inference is rejected rather than guessed.

**General transitions** cover extra guards, multiple effects, and controller rules:

```lean
transition restrict on Controller where
  guard mode = Open ∧ inputSum activity field infected > 100
  hazard 1e300
  set mode := Restricted
  set modifier := 0.4
```

A general transition requires exactly one `guard`, exactly one `hazard`, and at least one ordered `set`. Numeric effects accept the row-local scalar expression fragment, so old-snapshot updates like `set age_months := age_months + 1` are valid. The expression must have exactly the destination attribute's type — there is no coercion. Enum effects are variant literals; `Ref` writes are rejected; `countBy`, `freq`, and `inputSum` are rejected in effect expressions.

### 6.7 Race-time contests

A general transition may claim one or more `Ref`-valued row resources in declaration order:

```lean
transition depart on Slot where
  guard occupancy = present
  hazard departure_hazard
  contest slot_resource by race_time
  set occupancy := vacant
  set cause := departure
```

`race_time` is the only exposed ordering; keyed orderings and queue disciplines are deferred (`DECISIONS.md` §K7). Duplicate claims are rejected, reaction-arrow sugar cannot carry contests, and this does not permit `Ref` writes. The runtime owns argmin conflict resolution, deferred-loser reporting, and the double-write defense.

### 6.8 Keyed frequency

```lean
freq (predicate) over key
```

This is the supported frequency-shaped aggregate. It means exactly the `countBy key (predicate) / sizeBy key` plan and introduces no general query language. It requires a transition/output/view context with a selected source system, a `key` that is a `Ref` attribute of that system, and a Boolean row-local predicate.

Input aggregates, relational aggregates, nested `freq`, non-`Ref` keys, and predicates leaving the row-local fragment are all rejected with positioned diagnostics.

### 6.9 Inputs, outputs, views, wires, summaries

```lean
input activity where
  infected : Int
  total_risk : ℝ
  mode : {Open, Restricted}
  subject : Person

output activity from Person where
  infected : Int := count where health = I
  total_risk : ℝ := sum (risk)

views Person where
  infectious := count where health = I
  total_risk := sum risk
  minimum_visits := min visits
  active_risk_max := max risk where health = I

wire population activity -> policy activity

summaries population where
  final_infectious := last infectious
  peak_infectious := max infectious
  peak_tick := argmaxₜ infectious
```

- `inputSum port field column` is limited to numeric schema fields.
- Outputs simultaneously declare an ordered schema and its builders. Count builders have a filter and no value; sum builders have a value.
- Views use `count`, `sum`, `min`, or `max`. Numeric reductions take their value expression directly; an optional `where` filter stays row-local.
- Wires use four endpoint identifiers and ASCII `->`. Schemas must match, and a destination may be delivered to only once.
- Summaries fold view streams with exactly `sum`, `min`, `max`, `last`, or `argmaxₜ`. `argmaxₜ` returns the **earliest** tick attaining the maximum.

The single-line `view ...`, `grouped view ...`, and `summary name := reduce box.view` forms remain supported for interleaving declarations from different tables.

### 6.10 Grouped observations

Adding a `by` clause turns a view entry into a grouped count view, keyed by one to four Enum, `Ref`, or banded `Int` keys:

```lean
views PersonSlot where
  population := count where occupancy = present
  population_cells := count
    where occupancy = present
    by sex, area, band(age_months, 60)
```

Int keys require a positive literal band width; Enum and `Ref` keys must not use `band`. Filters are row-local count predicates.

> [!warning] Grouped observation is default-off and CPU-only
> Authoring always elaborates the complete declaration — there is no runtime feature context at author time. **Execution** requires the repeatable flag `--enable grouped-observations` on `sembla run` or `sembla sweep` (`DECISIONS.md` §K6), and V1 executes grouped observations on CPU only.

Each declaration writes `<out-stem>.grouped.<view>.csv` with header `tick,<key1>,…,<keyN>,count`, non-empty groups only, numeric key-tuple ordering before Enum names are rendered.

### 6.11 Indexed families

Finite compile-time domains expand a demographic table without adding a tensor node to the IR:

```lean
index age := 0 .. 120
index sex := {male, female}

param β[age, sex] : ℝ where
  [0, male] := 0.10
  [0, female] := 0.09
  -- every remaining cell is required

infect[age, sex] on Person : health: S →[
  β[age, sex] · freq (health = I) over employer
] I
```

The selected system must have same-named compatible attributes. Expansion is canonical and produces ordinary names such as `beta_0_male` and `infect_0_male`. Large complete tables may use strict source-relative CSV or JSON declarations, each with a **mandatory exact-byte SHA-256 pin** — see [indexed parameter families](guides/indexed-parameter-families.md).

### 6.12 The compatibility path

`model%` is a supported low-level compatibility and semantic-kernel form, retained for old models and the legacy/new syntax-twin regression suite. Both `model%` and `sembla_model` collect the same surface-model records and invoke the same kernel; neither frontend has separate semantics.

Generated models and low-level tests may construct `Sembla.IR` values directly. That is the **machine-writer path**, not the recommended human surface.

---

## 7. The mathematical surface

Everything in this section is **checked static sugar**. It expands to the same scalar IR — no runtime tensor, domain, alias, or relation node is introduced. Full reference: [mathematical model surface](guides/mathematical-model-surface.md).

### 7.1 Named finite domains

```lean
domain Area := {nsw, vic, qld}
domain ExactAge := 0 .. 120

system Person (rows := 1_000) where
  area : Area       -- lowers to enum {nsw, vic, qld}
  age : ExactAge    -- lowers to Int
```

> [!warning] A named range is not a runtime constraint
> `ExactAge := 0 .. 120` is a finite compile-time domain, but the attribute still lowers to ordinary `Int`. Rows outside the declared range are **not** rejected by the runtime.

Enum order and integer order are semantic expansion order. Domain and system names may not be ambiguous.

### 7.2 Projected partitions

A partition names contiguous half-open intervals of one specific `Int` attribute:

```lean
partition AgeBand projects population.Person.age_months where
  «00_04» := 0 ..< 60
  «05_09» := 60 ..< 120
  «10_plus» := 120 ..
```

Rules: the target must have the exact `box.System.attribute` form and resolve to an `Int` attribute; bounds are natural-number literals; each bounded interval's upper bound exceeds its lower; each later lower bound equals the preceding upper bound; and the final cell — and only the final cell — is open-ended. The first lower bound need not be zero, so values below it match no cell.

Labels starting with digits use Lean escaped identifiers (`«00_04»`); the generated suffix is still `00_04`.

`AgeBand(band)` in a `from` list lowers to `age_months >= lower` and, for bounded cells, `age_months < upper`. A projected partition is a predicate over an integer column, not an enum column, and cannot appear after `become`.

### 7.3 Parameter families with call notation

```lean
param mortality (area : Area, band : AgeBand) : ℝ where
  [nsw, «00_04»] := 0.0001 ~ LogNormal (-9.210340371976184) 0.5
  [nsw, «05_09»] := 0.0    ~ Normal 0.0 0.00001
  -- every remaining Cartesian cell exactly once

hazard mortality(area, band)
```

This emits scalar parameters like `mortality_nsw_00_04`; a call resolves statically to the scalar for the current bound values. Cells may mix `Normal` and `LogNormal`. Legacy `param mortality[area, band]` lowers identically, but the parenthesized form names each binder's domain explicitly, preventing accidental reuse of a same-spelled binder from an incompatible domain.

### 7.4 Finite expression functions

```lean
domain Level := 0 .. 1

function Identity (level : Level) : Int where
  [0] := level
  [1] := level
```

At least one typed argument is required, argument names must be unique, and the table must contain every Cartesian cell exactly once. Each cell elaborates with its formal arguments bound to that cell's domain values.

> [!note] These are inline tables, not functions
> No recursion, no function-to-function calls, no aggregates, no row-local attribute context. Calls like `Identity(level)` or `Identity(1)` accept only bound values or domain literals and are replaced by the selected scalar expression.

### 7.5 State aliases

```lean
state Present on Person where
  occupancy := present

state InArea (region : Area) on Person where
  area := region

state Eligible on Person where
  match age_months ≥ 18 * 12
```

An assignment atom (`attribute := value`) has two roles: in `from` it lowers to an equality predicate; after `become` it lowers to an ordered `setAttr` effect. A `match` atom contributes an arbitrary row-local Boolean predicate and is valid **only** in `from`. Assignments need the attribute's exact type; `Ref`-valued attributes cannot be matched by assignment. Aggregates are rejected in all alias bodies. Alias names are box-local and never emitted into the IR.

### 7.6 Relations

```lean
relation move (origin : Area, destination : Area) on Person
    subject to origin ≠ destination where
  from Present, InArea(origin)
  hazard movement(origin) * Pull(destination)
  claim slot_resource by race_time
  set previous_area := origin
  become InArea(destination)
  set event := moved
```

A relation requires at least one binder, exactly one `from` list, exactly one hazard, and at least one effect. `subject to` accepts comma-separated binder equality/inequality constraints (`=`, `≠`) over the same domain; these filter the finite expansion at compile time and **do not become runtime guards**.

The `from` applications *are* the runtime guard: each alias expands in atom order and applications are conjoined in written order. Effects and claims preserve body order independently — each `set` emits one effect, `become` emits the selected alias's assignment atoms in alias order.

> [!warning] Binders do not add implicit guards
> Relation binders do not silently add same-named attribute guards. Express those restrictions explicitly with aliases or a projected partition.

---

## 8. Export and validate

Lean elaborates, inspects, renders widgets, proves specification-level results, and serialises models. Rust validates whole exported models and executes them.

```sh
(cd ../sembla-lean
lake exe sembla-export sir /tmp/sir.json
lake exe sembla-export Sembla.Models.sirPolicy /tmp/sir_policy.json
lake exe sembla-export observations /tmp/observations.json)
cargo run -p sembla-cli -- validate /tmp/sir.json
cmp examples/sir.json /tmp/sir.json
cargo run -p sembla-cli -- diff-ir examples/sir.json /tmp/sir.json
```

The exporter accepts concise snake-case and camel-case spellings plus `Sembla.Models.*` and `Sembla/Models/*` qualified aliases.

> [!warning] `diff-ir` does not replace `cmp`
> `diff-ir` is a useful *normalized semantic* comparison. Byte-level parity still requires literal comparison.

### Parity

```sh
../sembla-lean/scripts/check-backend-compat.sh "$(pwd)"
```

This exports all eight canonical models and every accepted alias, validates both sides, and uses literal `cmp` against checked-in fixtures before supplemental `diff-ir` checks. It then runs checked and exported models with fixed seeds, comparing CSV bytes, summaries, final-state hashes, and output hashes, while asserting nontrivial dynamics and conserved state counts. **No fixture regeneration is part of the workflow.**

### Negative tests

Complete ill-formed models under `Negative/` in `sembla-lean` pin full ordered
sets of positioned errors:

```sh
(cd ../sembla-lean && bash scripts/test-negative.sh)
```

---

## 9. Composition

Composition lets you author reusable components once and assemble them into models. It adds **no second runtime semantics** — the linker emits the same flat model shape consumed by existing validation and execution.

### 9.1 Author components

`sembla_component` defines an ordinary Lean constant. A primitive component uses the same `system`, `input`, transition, `output`, and `view` declarations as `sembla_model`; `requires` names the model-level parameters its body uses. **Components do not declare `dt`.**

A composite component instantiates component constants and explicitly wires, exposes, or hides ports:

```lean
sembla_component EpidemicPolicy where
  instance population := Population (
    beta := beta,
    gamma := gamma)
  instance policy := Policy
  wire count_to_policy : population.infection_count -> policy.infection_count
  wire modifier_to_population : policy.restriction_modifier -> population.restriction_modifier
```

Bindings always map a component requirement to a root model parameter. An omitted binding is the explicit same-named binding; literals are not accepted. Wire and exposure declarations require stable labels; `hide` does not.

### 9.2 The composition root

```lean
sembla_composition epidemicPolicyModel
    (name := "epidemic_policy") (dt := 0.25) where
  param beta : ℝ := 0.3
  param gamma : ℝ := 0.1
  root EpidemicPolicy
  summary infected_peak := max population.I
```

Exactly one root instance, an exact lowercase slug name, the outer `dt`, root parameters, and optional summaries.

### 9.3 Link

Single-file mode writes an independently runnable plan, plus an optional non-semantic link report:

```sh
(cd ../sembla-lean
lake exe sembla-link /tmp/epidemic_policy.source.json \
  --plan /tmp/epidemic_policy.plan.json \
  --report /tmp/epidemic_policy.link-report.json)
```

Bundle mode writes the frozen four-file layout into a new or empty directory:

```sh
(cd ../sembla-lean && lake exe sembla-link /tmp/epidemic_policy.source.json \
  --bundle /tmp/epidemic_policy.bundle)
```

```text
composition-source.json
executable-plan.json
link-report.json
bundle-manifest.json
```

A non-empty destination is rejected rather than overwritten. The bundle manifest records source, semantic-plan, envelope-plan, and bundle-root SHA-256 records. A plan never embeds hashes of itself; a linked plan *does* embed its source-hash provenance. Copying `executable-plan.json` out of the bundle does not make it incomplete.

```sh
cargo run -p sembla-cli -- bundle-verify build/epidemic_policy.bundle
```

This verifies all versions, canonical bytes, hashes, bundle membership, plan validity, and source–plan agreement.

> [!warning] Keep your artifacts
> Relinking old sources and automating historical-linker retention are future work. When a historical result must stay reproducible, keep the original plan and bundle.

### 9.4 Stable identities

V1 identities use this grammar:

| Kind | Form |
| --- | --- |
| slug | `[a-z][a-z0-9_]*` |
| declaration | `<kind>:<slug>` for `model`, `def`, `inst`, `port`, `wire`, `expose` |
| occurrence | `occ:` + slash-joined instance-ID slugs from the root |
| transition occurrence | `<occurrence>#<transition-name>` |
| wire occurrence | `<owner-occurrence>#wire:<wire-slug>` |
| mailbox | `mbox:<wire-occ>\|<source-occ>.<port>\|<target-occ>.<port>` |
| plan leaf | slash-joined occurrence chain without the `occ:` prefix |

Display names are provenance only. **Identity-preserving:** renaming a display label, reordering independent declarations, permuting source definitions. **Identity-changing:** altering a stable component/instance/port/wire/exposure/transition ID, or moving a declaration across a composite boundary — that changes its occurrence chain and therefore its transition, wire, mailbox, and Philox draw identities, even when the visible scientific structure looks similar.

### 9.5 Origins and provenance

| Origin | Meaning |
| --- | --- |
| `legacy` | unversioned model JSON; no plan envelope; frozen dense positional identity |
| `direct_stable` | versioned plan exported directly from flat IR; stable identity map, no linked-source tuple |
| `linked` | versioned plan from composition source; embeds source hash, linker descriptor, source map, complete execution identity map |

A plan run records the complete plan tuple:

```json
"plan": {
  "plan_schema": "sembla.executable-plan/v1",
  "identity_scheme": "sembla.identity/stable-v1",
  "origin": "linked",
  "plan_semantic_hash": {
    "algorithm": "sha256",
    "domain": "sembla.plan-core/v1",
    "digest": "…"
  },
  "enabled_features": []
}
```

Linked runs additionally record `linked_source` with a `sembla.source-artifact/v1` hash and `"linker_semantics": "sembla.linker/v1"`. That `source_hash` must equal both the plan's embedded provenance and the bundle source record. These are all-present-or-absent contracts, not optional hints.

Four runnable models covering counterfactuals, policy fan-out, surveillance, and deep regional nesting are in the [composition showcase](examples/composition-showcase.md).

---

## 10. Command reference

All commands run as `cargo run -p sembla-cli -- <command>` or, after `cargo build --release`, as `./target/release/sembla <command>`.

### 10.1 Inspection and integrity

| Command | Purpose |
| --- | --- |
| `sembla --version` | print CLI version |
| `sembla validate <model-or-plan.json>` | semantic validation of a model or plan |
| `sembla plan-hash <plan-envelope.json>` | canonical plan hash |
| `sembla state-hash <file.state>` | `sembla.state-artifact/v1` hash |
| `sembla bundle-verify <bundle-dir>` | verify a linked bundle end to end |
| `sembla diff-ir <a.json> <b.json>` | normalized semantic IR comparison |

### 10.2 Generation

```sh
sembla synth-pop --persons N --employers E --initial-infected I --seed S --out pop.bin

sembla synth-state --model model-or-plan.json --slots N --areas K \
  --present-fraction F --streams birth:B,overseas:O,internal:I \
  --seed S --out state.artifact
```

### 10.3 `run`

```sh
sembla run <model-or-plan.json> --seed N --ticks K --population N|pop.bin|file.state
  [--backend cpu|cuda]
  [--out results.csv]
  [--export-state final.state]
  [--dt D]
  [--params file.json]
  [--timing-json timing.json]
  [--enable grouped-observations]
```

| Flag | Notes |
| --- | --- |
| `--population` | numeric, `SEMBLA_POP`, or state artifact; dispatched by content |
| `--params` | JSON object of overrides; unknown names and wrong JSON types are errors that name the parameter |
| `--dt` | overrides the model timestep and **changes the effective IR hash** |
| `--export-state` | refuses to overwrite an existing path |
| `--enable` | repeatable; `grouped-observations` is CPU-only |

### 10.4 `sweep`

```sh
sembla sweep <model-or-plan.json> --population N|pop.bin|file.state --seed S
  (--draws K | --theta-file file.json) --ticks T --out dir
  [--backend cpu|cuda]
  [--draw-workers N]
  [--noise crn|independent]
  [--params file.json]
  [--export-pairs pairs.csv]
  [--timing-json timing.json]
  [--enable grouped-observations]
```

`--draws` and `--theta-file` are mutually exclusive: the file's entry count *is* the draw count.

### 10.5 `compare`

Two forms — two plans, or one plan with two parameter files. Both arms share the same seed.

```sh
sembla compare <a.json> <b.json> --population pop.bin|file.state \
  --seed N --ticks K --out compare.csv [--backend cpu|cuda]

sembla compare <model-or-plan.json> --population pop.bin|file.state \
  --seed N --ticks K --params-a a.json --params-b b.json --out compare.csv \
  [--backend cpu|cuda] [--enable grouped-observations]
```

> [!warning] Legacy/plan pairs are rejected
> Legacy models may compare with legacy models, but a legacy/plan pair is rejected before execution, because positional and stable identities cannot form a meaningful CRN contrast.

### 10.6 `verify-run`

```sh
sembla verify-run <manifest.json> <model-or-plan.json> \
  --population N|pop.bin|file.state [--params file.json] [--draw K]
```

Re-executes and checks recorded hashes. `--draw K` selects a single sweep draw.

### 10.7 `diff-backends`

```sh
sembla diff-backends <model-or-plan.json> --population N|pop.bin|file.state \
  --seed N --ticks K [--dt D] [--params file.json] [--enable grouped-observations]

sembla diff-backends --all-examples [--population N] [--seed N] [--ticks K] [--dt D]
sembla diff-backends --all-plan-fixtures [--population N] [--seed N] [--ticks K]
```

---

## 11. Output artifacts

### 11.1 Results CSV — two schemas

**Models that declare views** emit their view columns. The SIR model's three filtered count views produce:

```text
tick,S,I,R,fired_infect,fired_recover,deferred_total
```

**Models with no declared views** use the model-agnostic generic schema. Two canonical comment headers come first:

```text
# params={...}
# dt=...
```

Then columns in deterministic declaration order:

1. `tick`;
2. one `count:<box>.<table>.<attribute>=<variant>` column for every variant of every enum attribute, iterating boxes, tables, attributes, and variants in source order;
3. one `fired:<box>.<transition>` column for every transition in model-global rule-ID order, **including transitions that fired zero times**;
4. `deferred_total`.

For example:

```text
tick,count:chain.particle.phase=A,count:chain.particle.phase=B,fired:chain.move_ab,fired:chain.move_ba,deferred_total
```

Counts are observed after each completed tick; for each enum attribute, its variant columns sum to that table's row count. Generated header fields are CSV-escaped.

> [!note] There are no model-specific output branches
> The CLI contains no model-name or SIR-shape special case. Both schemas fall out generically from what the model declares.

### 11.2 `<out>.summaries.csv`

```text
name,value
peak_I,...
peak_tick,...
```

Declaration order. For views-free, summary-free models this file contains only the header — and the observation hash of those exact bytes is still printed and recorded.

### 11.3 `<out>.manifest.json`

Canonical compact JSON, sorted keys, one trailing newline. It records: schema versions; the effective canonical-IR hash (including `--dt`); model name; seed; ticks; `dt`; determinism level `A`; sorted resolved θ; the population basename (or numeric specification) and input hash; backend/precision/fallback identity; enabled flags; result, final-state, and observation hashes; hash algorithm IDs (`sha256`); and workspace component versions.

> [!important] Deliberate omissions
> The manifest contains **no timestamp, host, or absolute path**. That is what makes byte-identical manifests a meaningful equality check.

### 11.4 Grouped observation CSVs

`<out-stem>.grouped.<view>.csv`, header `tick,<key1>,…,<keyN>,count`, non-empty groups only, numeric key-tuple ordering before Enum names are rendered.

### 11.5 Sweep directory

| File | Content |
| --- | --- |
| `manifest.csv` | tabular θ report per draw — a *report*, not a contract |
| `draw_<k>.csv` | one standard result per draw |
| `summary.csv` | nearest-index 5/25/50/75/95 percentiles for every reported per-tick column |
| `run-manifest.json` | the canonical reproducibility contract |

`run-manifest.json` stores shared model, population, seed, tick, backend, schema, and component fields once, then one `executions` entry per draw with `k`, its actual simulation seed, sorted resolved θ, results hash, and final-state hash. It also records `noise_mode` and the all-or-nothing `theta_source` kind/hash/algorithm tuple.

> [!note] The two manifest names are intentionally distinct
> `manifest.csv` is only a tabular parameter report. `run-manifest.json` is the reproducibility contract.

### 11.6 `pairs.csv` and `comparison.csv.manifest.json`

See [[#12. Calibration workflow]] and [[#13. Reproducibility and verification]].

---

## 12. Calibration workflow

### 12.1 Prior-predictive sweep

```sh
cargo run --release -p sembla-cli -- sweep examples/sir.json \
  --population pop.bin --seed 99 --draws 20 --ticks 50 \
  --noise independent --out sweep/
```

### 12.2 Choosing a noise mode

| Mode | Behaviour | Use for |
| --- | --- | --- |
| `crn` (default) | every θ draw reuses the master simulation seed | paired policy or sensitivity contrasts |
| `independent` | derives a stable simulation seed from master seed + replica index, without changing θ | NPE training data |

> [!warning] CRN is wrong for training pairs
> One shared noise realization teaches an artificially deterministic θ→x mapping and produces an overconfident learned posterior (`DECISIONS.md` §G5). `--export-pairs` under CRN is allowed for diagnostics but emits a warning, and the NPE pipeline rejects CRN-mode training input.

### 12.3 Pinning parameters

```sh
printf '{"gamma":0.1}\n' > pinned.json
cargo run --release -p sembla-cli -- sweep examples/sir.json \
  --population pop.bin --seed 99 --draws 20 --ticks 50 \
  --params pinned.json --out sweep-pinned/
```

Pinned values are marked in the manifest header and are not sampled.

### 12.4 External proposals

```sh
printf '[{"beta":0.7,"gamma":0.12},{"beta":0.8,"gamma":0.1}]\n' > theta.json
cargo run --release -p sembla-cli -- sweep examples/sir.json \
  --population pop.bin --seed 99 --theta-file theta.json --ticks 50 \
  --noise independent --out sweep-proposals/
```

Every entry must provide every prior-bearing parameter.

### 12.5 Exporting `(θ, x)` training pairs

```sh
cargo run --release -p sembla-cli -- sweep examples/sir.json \
  --population pop.bin --seed 99 --draws 5000 --ticks 50 \
  --noise independent --out sweep-training/ \
  --export-pairs pairs.csv
```

`pairs.csv` has one row per draw: column `k`, then parameters sorted by name, then summaries in model declaration order. **Only declared summaries become `x`** — per-tick view series are not included.

The canonical `pairs.csv.meta.json` sidecar binds the bytes to the effective IR, master seed, noise mode, θ source, draw/tick/`dt` settings, determinism level, ordered columns, component versions, and `pairs_sha256`.

### 12.6 Draw-coordinate stability

This is what lets you extend a sweep without invalidating earlier work:

- **θ draws** reserve `rule_id = 0xffffffff`, use the draw index as `tick`, and the parameter's declaration index as `entity_id`. Extending K never changes an earlier θ draw.
- **Independent simulation seeds** reserve `rule_id = 0xfffffffe`, use the replica index as `tick`, and set `entity_id = draw_idx = 0`; Philox lane 0 supplies the low 32 bits and lane 1 the high 32 bits of the derived `u64`. Extending K never changes an earlier replica seed or result.

Normal priors use the frozen cosine branch of Box–Muller; LogNormal draws are the exponential of that Normal draw. Since 2026-07-19 their transcendental operations use an exactly pinned pure-Rust `libm`, so θ draws are platform-independent.

---

## 13. Reproducibility and verification

### 13.1 The three hashes

Every `run` prints SHA-256 digests of the exact result bytes, the final columnar state, and the observation summary bytes. Two runs with the same seed, IR, and resolved θ must print all three identically and produce byte-identical CSV, summaries, and manifest sidecars — apart from the explicitly recorded population basename when different population filenames are used.

### 13.2 Verifying a recorded run

```sh
# whole sweep
cargo run --release -p sembla-cli -- verify-run \
  sweep/run-manifest.json examples/sir.json --population pop.bin

# one draw
cargo run --release -p sembla-cli -- verify-run \
  sweep/run-manifest.json examples/sir.json --population pop.bin --draw 3
```

### 13.3 Comparison manifests

`sembla compare --out comparison.csv` writes `comparison.csv.manifest.json`. Its `executions` array holds deterministic `arm_a` and `arm_b` entries, each with its model, effective IR hash, `dt`, resolved θ, results hash, and final-state hash. Population, seed, ticks, backend identity, flags, and component versions stay shared.

### 13.4 Why CRN comparison is principled here

Content-addressed transition identities mean a shared component keeps the same `occ:…#…` identity, rule word, and Philox draws even when the surrounding composed model changes (`DECISIONS.md` §J4, §J14).

Concretely: the population leaf is shared between a standalone plan and an unwired population-plus-policy product, so every population view and firing trajectory in that contrast is **exactly equal**, not merely statistically similar:

```sh
cargo run -p sembla-cli -- compare \
  fixtures/plans/linked/solo_population.plan.json \
  fixtures/plans/linked/independent_epidemic_policy.plan.json \
  --population build/population.bin --seed 55 --ticks 8 \
  --out build/population-noninterference.csv
```

And a wired counterfactual shows the one-tick delay directly — thresholds 500 and 1000 produce identical population columns at ticks 0 and 1, with the first difference at tick 2, after both wires have carried the counterfactual:

```sh
printf '%s\n' '{"restriction_threshold":500}'  > build/policy-a.json
printf '%s\n' '{"restriction_threshold":1000}' > build/policy-b.json
cargo run -p sembla-cli -- compare \
  crates/sembla-cli/tests/fixtures/epidemic_policy_threshold.plan.json \
  --population build/population.bin --seed 55 --ticks 8 \
  --params-a build/policy-a.json --params-b build/policy-b.json \
  --out build/policy-counterfactual.csv
```

---

## 14. Backends

| Backend | Selection | Notes |
| --- | --- | --- |
| CPU | default, `--backend cpu` | the deterministic oracle |
| CUDA | `--backend cuda` | native `f64`, via CUDA lowering and an optional NVRTC execution path |

Plan envelopes run on the CPU oracle and may select CUDA when a qualified device is available. Grouped observations are **CPU-only** in V1.

Check backend agreement with:

```sh
cargo run -p sembla-cli -- diff-backends examples/sir.json \
  --population pop.bin --seed 99 --ticks 50

cargo run -p sembla-cli -- diff-backends --all-examples
cargo run -p sembla-cli -- diff-backends --all-plan-fixtures
```

Performance evidence is under [`docs/performance/`](performance/README.md) and [`docs/evidence/`](evidence/README.md).

---

## 15. Widgets in the Lean infoview

The frontend pins ProofWidgets4 `v0.0.44` for Lean 4.13. Pure functions in `Sembla.Widgets` build JSON-encodable props from the already-elaborated `Model`; `Sembla.WidgetDisplay` renders them as HTML/SVG infoview panels. **Neither path invokes the Rust runtime or performs simulation.**

```lean
set_option sembla.widget.theme "academic"  -- also: "editor" or "notebook"
```

`academic` is the restrained default (`professional` is an alias). `editor` follows standard VS Code widget chrome; `notebook` is softer and more rounded. All themes inherit the active VS Code foreground/background, so dark and high-contrast modes work.

To verify manually: build the frontend, open the repository with the VS Code Lean 4 extension, and place the cursor on a `system` declaration (expect a state-machine panel), on a reaction arrow (expect a transition panel with hazard, defaults, priors, and where applicable a `p(dt) = 1 - exp(-lambda * dt)` chart), and on a general transition (expect a distinct panel). Step-by-step checks are in the [frontend guide](https://github.com/ianmoran11/sembla-lean/blob/main/README.md).

---

## 16. Proofs

`Sembla.LumpingProof` proves `groupedCount_eq_naiveCount` — exact agreement of the grouped and naive coworker-count plans — and `plan_rewrite_congr`, which transports that equality through any per-row function of the count.

> [!warning] What this proof does and does not cover
> This is theorem target **1a at the specification level**, not a theorem about the deep-embedding evaluator. Target 1b remains open. There is no proof that the Rust or CUDA implementations refine an ideal Lean semantics.

`Sembla.Semantics.ProofAudit` provides the deterministic environment inventory used by the hygiene guard: it enumerates every theorem and lemma in the covered module roots and rejects transitive axioms outside `{propext, Classical.choice, Quot.sound}`.

```sh
(cd ../sembla-lean && bash scripts/check-proofs.sh)
```

The syntax-independent frontend boundary is `Sembla.Frontend.Builders.Observation`. `CompleteModelSpec.toRaw` is the sole complete raw assembly path — the command frontend no longer constructs `IR.Box` or `IR.Model` directly. Parser expansion, family/alias/partition lowering, source-token bookkeeping, diagnostic rendering, widget attachment, and composition/wire compatibility checks remain **trusted and regression-tested rather than verified**; the negative harness is the oracle for exact message positions.

---

## 17. Limits and boundaries

> [!important] Read this before designing around a feature
> Proposed features in historical roadmaps or design discussions are **not implemented merely because they were described**. Check the current [roadmap](ROADMAP.md), [`DECISIONS.md`](../DECISIONS.md), and actual validation behaviour.

Deliberate V1 limits:

- one model-level timestep and one V1 scheduler domain;
- fixed rows during a run — no general birth/death graph rewriting;
- row-local effects on the transition's source table;
- restricted declared-key aggregates, not a general relational query language;
- one-tick-delayed communication between boxes;
- `race_time` as the only exposed contest ordering;
- grouped observations CPU-only and default-off;
- no proof that Rust or CUDA refine an ideal Lean semantics;
- reproducibility guarantees narrower than scientific validation.

Additional honest caveats worth repeating: a named range domain is not a runtime constraint; chained runs are not checkpoint/restart; `diff-ir` is not byte equality; and model documentation must distinguish software-validation fixtures from empirically calibrated models.

Scientific limits of the shipped substantive models are documented per model — see [demographic slot model](models/demographic.md) and [Australian population](models/australian-population.md).

---

## 18. Troubleshooting

| Symptom | Likely cause | Fix |
| --- | --- | --- |
| Ambiguous transition rejected | omitted `on System` or `health:` where inference is not unique | write the explicit disambiguation form; Sembla rejects rather than guessing |
| Name derivation collision error | two identifiers derive to the same snake-case name | add `(name := "...")` |
| Prior rejected on a parameter | priors are real-valued only | remove the prior or make the parameter `ℝ` |
| Aggregate rejected in an effect | `countBy`, `freq`, `inputSum` are not allowed in effects | move the aggregate into a hazard, output, or view |
| `freq` rejected | key is not a `Ref` attribute, or the predicate left the row-local fragment | use a `Ref` key and a row-local predicate |
| Grouped view columns missing at run time | execution is default-off | add `--enable grouped-observations` (CPU only) |
| `--export-state` fails | the path already exists | choose a new path; artifacts are chain links and are never overwritten |
| `bundle-verify` rejects a directory | non-empty destination at link time, or a modified member file | relink into a clean directory |
| `compare` rejected before execution | legacy/plan pair | compare like with like |
| Two runs differ | `--seed`, θ, or `--dt` changed | `--dt` changes the effective IR hash; check the manifest diff |
| Chained runs don't match one long run | expected | tick coordinates restart at zero; see [[#5.5 Chained runs]] |
| Unknown parameter error | `--params` name not in the model | the error names the parameter; check spelling and the derived snake-case name |
| Strict check fails on a missing tool | `./scripts/check.sh` never silently skips | install Cargo, Git, and Lake, or use `./scripts/check-rust.sh` |

---

## 19. Where to go next

### Reference documents

- [Documentation home](README.md) — the full index
- [Project overview](overview.md) — what is implemented and where the boundaries are
- [`DESIGN.md`](../DESIGN.md) — normative semantics, architecture, and scope
- [`DECISIONS.md`](../DECISIONS.md) — adopted decisions and rationale
- [Roadmap](ROADMAP.md) — current priorities

### Authoring

- [Lean frontend](https://github.com/ianmoran11/sembla-lean/blob/main/README.md) — the complete DSL, exporter, widgets, proofs
- [Mathematical model surface](guides/mathematical-model-surface.md)
- [Indexed parameter families](guides/indexed-parameter-families.md)
- [Composition](guides/composition.md)
- [State artifacts](guides/state-format.md)
- [Visual guide](guides/visual-guide.md) — diagrams of boxes, tables, wires, dynamics

### Examples and models

- [Examples index](examples/README.md)
- [Workplace SIR](examples/sir.md) · [SIR policy](examples/sir_policy.md)
- [Canonical finite-state models](examples/canonical-models.md)
- [Composition showcase](examples/composition-showcase.md)
- [Demographic slot model](models/demographic.md) · [Australian population](models/australian-population.md)
- [ABS data pipeline](guides/abs-data.md)

### Engineering

- [Architecture atlas](architecture/README.md)
- [CI and local checks](contributing/ci.md)
- [Performance index](performance/README.md) · [Evidence](evidence/README.md)
- [Design notes](design/README.md) · [Archive](archive/README.md)

### Repository layout

| Area | Responsibility |
| --- | --- |
| [`sembla-lean`](https://github.com/ianmoran11/sembla-lean) | Lean DSL, IR construction, linker, canonical plan export, widgets, proofs |
| `crates/sembla-ir` | versioned IR and plan types, stable identities, canonical serialization, validation |
| `crates/sembla-runtime` | backend-neutral state, parameters, observations, synthetic state, Philox |
| `crates/sembla-cpu` | CPU expression evaluation, tick execution, conflict resolution, observation reduction |
| `crates/sembla-cuda` | CUDA lowering and native execution path |
| `crates/sembla-cli` | validation, execution, sweeps, comparison, verification, backend differentials |
| `calibration/` | external calibration and NPE workflow material |
| `docs/evidence/` | immutable benchmark and conformance evidence |
| `docs/prds*` | implementation specifications and completion records |

---

## 20. Glossary

| Term | Meaning |
| --- | --- |
| **Box** | a unit of private state; communicates only through ports and one-tick-delayed wires |
| **System** | a typed table declared inside a box; rows are entities or resources |
| **Attribute** | a typed column: `Real`, `Int`, `Enum`, or `Ref` |
| **Ref** | a reference to a row in another table *in the same box* |
| **Transition** | a guarded, hazard-rated rule that stages row-local effects |
| **Reaction arrow** | sugar for a transition with one enum equality guard and one enum write |
| **Hazard** | the rate expression governing a transition's firing time |
| **Contest** | a race for a `Ref`-valued row resource, resolved by `race_time` |
| **Deferred** | a transition occurrence that lost a contest, reported in `deferred_total` |
| **Port** | a declared `input` or `output` schema on a box |
| **Wire** | a one-tick-delayed mailbox connecting an output port to an input port |
| **View** | a per-tick observation sink over committed state |
| **Grouped view** | a keyed count view (1–4 Enum/`Ref`/banded-`Int` keys), default-off and CPU-only |
| **Summary** | a fold of a view stream via `sum`, `min`, `max`, `last`, or `argmaxₜ` |
| **Sink** | an observation that cannot feed dynamics, consume draws, or affect scheduling |
| **IR** | the versioned intermediate representation Rust validates and executes |
| **Plan** | a linked, canonically identified executable artifact |
| **Bundle** | the frozen four-file linked-plan directory |
| **Origin** | `legacy`, `direct_stable`, or `linked` — how a runnable input was produced |
| **Occurrence** | the slash-joined instance chain identifying a leaf from the root |
| **State artifact** | a portable `sembla.state/v1` file of committed tables |
| **CRN** | common random numbers; shared draws that make contrasts exactly attributable |
| **θ (theta)** | the resolved parameter vector for a run |
| **NPE** | neural posterior estimation, trained on exported `(θ, x)` pairs |
| **Determinism level A** | the recorded determinism class in run manifests |
