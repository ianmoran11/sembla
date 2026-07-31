# Sembla roadmap

**Status:** Canonical roadmap of record, consolidated 2026-07-31.

[`DESIGN.md`](../DESIGN.md) defines project scope and intended semantics. [`DECISIONS.md`](../DECISIONS.md) records adopted decisions. This document orders current priorities; it does not override either authority.

The previous version-milestone roadmap and two-driver forward roadmap are preserved as historical planning records:

- [Version roadmap, snapshot 2026-07-25](archive/roadmaps/version-roadmap-2026-07-25.md)
- [Two-driver forward roadmap, adopted snapshot 2026-07-25](archive/roadmaps/forward-roadmap-2026-07-25.md)

This consolidated roadmap removes the former situation in which two roadmaps were simultaneously authoritative without authority over one another.

## Current baseline

Sembla currently has:

- a versioned IR and executable-plan contract;
- deterministic CPU execution and native-`f64` CUDA execution;
- Lean authoring, reusable components, canonical linking, bundles, widgets, and parity checks;
- stable draw identities and common-random-number comparisons;
- views, grouped observations, summaries, manifests, and portable state artifacts;
- model execution through `run`, `sweep`, `compare`, `verify-run`, and backend-differential workflows;
- tracked benchmark and conformance evidence;
- a specification-level proof precedent and an open path to full Lean semantics.

The [run queue](prds-run-queue/README.md) is the operational source for pending PRDs. At consolidation time it is empty. Outstanding paid-hardware evidence is documented by the relevant performance track and evidence collectors rather than represented as an active PRD.

## Governing priorities

### 1. Finish complete user workflows before adding semantic breadth

The project has more individually designed capabilities than end-to-end modeller workflows. Prioritise:

- a clear authoring-to-verified-result path;
- readable validation diagnostics;
- maintained examples and component libraries;
- trace and explanation tooling;
- one canonical execution and artifact story.

A construct is not complete until it has a surface syntax or explicit machine-writer boundary, validation, backend behavior, tests, documentation, and manifest treatment.

### 2. Make numerical and model assurance routine

Reproducibility is necessary but not sufficient. Add or maintain workflows for:

- timestep and tau-leap sensitivity;
- contest saturation and capacity diagnostics;
- approximation comparisons;
- CPU/CUDA differential evidence;
- model cards and limitations registers;
- manifest-based reproduction.

Every performance or scientific claim should point to a retained evidence artifact and state its hardware, data, model, commit, and uncertainty.

### 3. Let driver models justify new semantics

The demographic and justice drivers remain useful because they demand different behavior. A runtime primitive should normally be supported by more than one domain or by decisive evidence that an existing representation is inadequate.

Shared demands include:

- identity-preserving transfer;
- external, versioned parameter and rate artifacts;
- capacity and contest semantics;
- deterministic diagnostics and held-out validation.

Driver-specific demands—households, queue disciplines, scheduled events, or multi-row atomic events—remain evidence-gated.

### 4. Prefer one restricted general primitive over several domain-specific ones

If evidence requires relational events, design one constrained mechanism with:

- explicit participants;
- deterministic claims;
- all-or-none commit;
- stable event identity;
- backend lowering or deterministic rejection;
- saturation and failure semantics.

Birth allocation, transfers, matching and household moves should not each invent a separate conflict model.

### 5. Make the Lean semantics investment operational

The next useful formal target is a complete executable, pathwise Lean semantics for validated V1 plans. It should support:

- expression and reference safety;
- deterministic contest and commit semantics;
- observation noninterference;
- wiring and flattening preservation;
- formal schema transport;
- overlay, stratification and lens operations;
- conflict-completion and conservativity statements.

This does not require immediate verification of Rust or CUDA. A Lean semantic oracle plus phase-level differential testing and plan certificates would already make transformations substantially safer.

### 6. Stabilise compatibility before a 1.0 guarantee

Before claiming a stable public contract, define:

- supported IR and plan migration policy;
- artifact retention and historical linker policy;
- numerical determinism guarantees by backend;
- unsupported-combination diagnostics;
- the boundary between normative semantics and implementation evidence.

## Milestone direction

| Milestone | Purpose | Exit direction |
| --- | --- | --- |
| **Current consolidation** | one documentation and workflow story | canonical docs, current examples, explicit history and evidence boundaries |
| **Assurance and authoring** | make existing capabilities easy to use and inspect | better diagnostics, trace/explain, sensitivity and model-card workflows |
| **Driver closure** | credible demographic and justice baselines | versioned external artifacts, held-out checks, quantified approximations |
| **Evidence-gated semantics** | add only primitives required by measured model gaps | complete surface/backend/provenance contract for every construct |
| **Formal semantics** | connect Lean meaning, plans, and transformation proofs | executable V1 semantics and first end-to-end preservation theorems |
| **1.0 contract** | stable reproducible platform | compatibility policy, complete conformance matrix, honest determinism guarantees |

These are dependency directions, not date commitments.

## Work records

Implementation work remains documented in track directories:

- `docs/prds/` contains the original V1 PRDs.
- `docs/prds-*` directories contain focused implementation tracks and their status records.
- `docs/prds-run-queue/` is the single executable queue for pending work.
- `.piprd/` contains managed runner reviews and implementation records; it is workflow state, not reader documentation.

The [documentation index](README.md) separates these records from current guidance. Completed PRDs and evidence remain immutable even when their paths or assumptions are historical.

## Evidence gates

Any future gate should be predeclared before evidence generation and name:

1. baseline and candidate;
2. locked model, data, artifact and commit;
3. metric and quantitative threshold;
4. uncertainty treatment;
5. `pass`, `fail`, and `inconclusive` meanings;
6. the action taken for each outcome;
7. the retained evidence path.

A report written after measurement is evidence, not a predeclared gate.

## Explicit non-goals of this roadmap

- No commitment schedule or duration forecast.
- No automatic promotion of an idea because it appears in an archived design note.
- No claim that reproducibility establishes scientific validity.
- No requirement that proofs block ordinary implementation work.
- No new backend until the existing CPU/CUDA contract and user workflows justify one.
