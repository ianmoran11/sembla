# Sembla project overview

**Status:** Maintained descriptive overview. Normative semantics live in [`DESIGN.md`](../DESIGN.md); adopted amendments live in [`DECISIONS.md`](../DECISIONS.md).

Sembla is a semantics-first simulation framework for large stochastic systems. Models are authored through a Lean 4 frontend, exported as a versioned intermediate representation or composition source, linked into canonical plans, and executed by deterministic Rust CPU or CUDA backends.

The central run contract is:

> **seed + model or plan + parameter vector + execution contract ⇒ reproducible result**

Reproducibility does not establish empirical validity. Sembla can record what a model means and how a result was produced; validation against the world remains a scientific responsibility.

## What is implemented

- A Lean 4 surface language and direct deep-IR construction path.
- Typed table schemas, row-local expressions, hazard transitions, aggregates, references, and contested resources.
- Reusable components, nesting, exposure, delayed wiring, canonical linking, and artifact bundles.
- A deterministic CPU interpreter and a native-`f64` CUDA backend.
- Counter-addressed Philox randomness and common-random-number comparison.
- `run`, `sweep`, `compare`, `diff-backends`, bundle verification, and run-manifest verification workflows.
- Scalar views, grouped observations, summaries, portable state artifacts, and chained runs.
- Structure widgets in the Lean infoview.
- A specification-level proof of the grouped-count rewrite and stated composition-preservation obligations.

The [roadmap](ROADMAP.md) identifies current priorities. Historical plans and assessments are retained under the [archive](archive/README.md).

## Semantic shape

### State

Each box owns typed columnar tables. Rows represent entities or resources; attributes are Real, Int, Enum, or references to rows in another table in the same box. The semantic state and the runtime struct-of-arrays layout intentionally have the same shape.

### One tick

Every tick reads a frozen snapshot, evaluates guards and hazards, samples candidate firing times from stable coordinates, resolves resource contests canonically, stages effects, rejects double writes, commits simultaneously, and evaluates observation sinks.

This read-old/write-new discipline makes work partitioning and evaluation order semantically invisible where the declared operations are order-free or canonically ordered.

### Randomness

Random draws are addressed by stable coordinates rather than consumed from mutable streams. A rule's occurrence identity, tick, entity row and draw index determine its entropy. This supports deterministic replay and common-random-number counterfactuals.

### Composition

Boxes retain private state and exchange values through one-tick-delayed mailboxes. Lean-authored component hierarchies compile to the same flat executable model shape used by direct IR. Source maps and identity maps preserve authored structure for provenance and draw coordinates.

The maintained workflow is documented in the [composition guide](guides/composition.md). The deeper algebraic account is in [the model algebra](design/model-algebra.md).

### Observation

Views and summaries are sinks. They are evaluated from committed state and cannot feed transitions, consume random draws, affect conflicts, or change scheduling. Grouped observations are currently CPU-only.

## Repository architecture

| Area | Responsibility |
| --- | --- |
| `frontend/` | Lean DSL, IR construction, linker, canonical plan export, widgets, and proofs |
| `crates/sembla-ir` | versioned Rust IR and plan types, stable identities, canonical serialization, validation |
| `crates/sembla-runtime` | state store, expression evaluation, CPU tick execution, synthetic state generation, Philox |
| `crates/sembla-cuda` | CUDA lowering and native execution path |
| `crates/sembla-cli` | validation, execution, sweeps, comparison, verification, and backend differential workflows |
| `calibration/` | external calibration and NPE workflow material |
| `docs/evidence/` | immutable benchmark and conformance evidence |
| `docs/prds*` | implementation specifications and completion records |

## Normal workflows

### Build and check

```sh
./scripts/check-rust.sh
./scripts/check.sh
```

The complete contract requires the pinned Rust and Lean toolchains. See [CI and local checks](contributing/ci.md).

### Run a direct model

```sh
cargo run -p sembla-cli -- validate examples/two_state.json
cargo run -p sembla-cli -- run examples/two_state.json \
  --population 1000 --seed 55 --ticks 40
```

### Export, link, and run a composition

```sh
(cd frontend && lake exe sembla-export --source surface_epidemic_policy \
  /tmp/epidemic_policy.source.json)
(cd frontend && lake exe sembla-link /tmp/epidemic_policy.source.json \
  --plan /tmp/epidemic_policy.plan.json)
cargo run -p sembla-cli -- run /tmp/epidemic_policy.plan.json \
  --population 1000 --seed 55 --ticks 40
```

### Continue from a state artifact

Use `--export-state` to write the final committed tables and provide the resulting artifact as a later run's `--population` input. See [state artifacts](guides/state-format.md) for schema and manifest rules.

## Important boundaries

Sembla currently has several deliberate limits:

- one model-level timestep and one V1 scheduler domain;
- fixed rows during a run rather than general birth/death graph rewriting;
- row-local effects on the transition's source table;
- restricted declared-key aggregates rather than a general relational query language;
- one-tick-delayed communication between boxes;
- no proof that the Rust or CUDA implementations refine an ideal Lean semantics;
- reproducibility guarantees that are narrower than scientific validation.

Proposed features in historical roadmaps or design discussions are not implemented merely because they were described. Check the current [roadmap](ROADMAP.md), [decision record](../DECISIONS.md), and executable validation behavior.

## Documentation map

- [Documentation home](README.md)
- [Lean frontend](../frontend/README.md)
- [Examples](examples/README.md)
- [Models](models/README.md)
- [Performance](performance/README.md)
- [Design notes](design/README.md)
- [Roadmap](ROADMAP.md)
- [Archive](archive/README.md)
