# Sembla documentation

This is the canonical entry point for Sembla's documentation. Documents are grouped by purpose so that current guidance, design authority, implementation records, and historical evidence are not mistaken for one another.

## Start here

| Reader | First document | Then read |
| --- | --- | --- |
| New user | [Project overview](overview.md) | [Lean frontend](../frontend/README.md), [examples](examples/README.md) |
| Model author | [Lean frontend](../frontend/README.md) | [composition guide](guides/composition.md), [state artifacts](guides/state-format.md) |
| Contributor | [Contributing](../CONTRIBUTING.md) | [CI and local checks](contributing/ci.md), [roadmap](ROADMAP.md) |
| Semantics or architecture reviewer | [Design authority](../DESIGN.md) | [decision record](../DECISIONS.md), [design notes](design/README.md) |
| Performance engineer | [Performance index](performance/README.md) | [measurement evidence](evidence/README.md) |
| Project historian | [Archive](archive/README.md) | [PRD index](prds/README.md), [evidence index](evidence/README.md) |

## Authority and status

Documents have four roles:

1. **Normative** — [`DESIGN.md`](../DESIGN.md) defines intended semantics and scope; [`DECISIONS.md`](../DECISIONS.md) records adopted amendments and decisions. If they conflict, the later explicit decision wins.
2. **Current guidance** — this index, the [overview](overview.md), [roadmap](ROADMAP.md), guides, model documentation, and contributor documentation describe the maintained project.
3. **Implementation records** — PRDs, track READMEs, benchmark reports, and evidence directories record what was proposed, implemented, or measured. They do not override normative documents.
4. **Historical snapshots** — files under [`archive/`](archive/README.md) preserve superseded roadmaps, assessments, comparisons, and design discussions. They are not current guidance.

A current document should say when it is intentionally descriptive rather than normative. A historical document must remain in the archive even when its claims are stale, because its value is provenance.

## Current documentation

### Project and planning

- [Project overview](overview.md) — what Sembla is, what works, and where the boundaries are.
- [Roadmap](ROADMAP.md) — current priorities, active work source, and milestone direction.
- [Design authority](../DESIGN.md) — semantics, architecture, and scope.
- [Decision record](../DECISIONS.md) — adopted decisions and rationale.

### Authoring and execution

- [Lean frontend](../frontend/README.md) — DSL, examples, exporter, widgets, and proofs.
- [Composition guide](guides/composition.md) — components, linking, bundles, identities, and comparison.
- [State artifacts](guides/state-format.md) — portable state files and chained runs.
- [Visual guide](guides/visual-guide.md) — diagrams of boxes, tables, wires, and dynamics.
- [Examples](examples/README.md) — runnable tutorials and canonical models.

### Models

- [Models index](models/README.md)
- [Demographic slot model](models/demographic.md)

### Engineering

- [CI and local checks](contributing/ci.md)
- [Performance index](performance/README.md)
- [Design notes](design/README.md)
- [CUDA crate notes](../crates/sembla-cuda/README.md)

### Records and evidence

- [PRD track index](prds/TRACKS.md) — original V1 PRDs and subsequent focused implementation tracks.
- [Evidence index](evidence/README.md) — immutable benchmark and conformance evidence.
- [Archive](archive/README.md) — superseded prose and decision exploration.

## Documentation policy

- Put user tasks and maintained workflows under `guides/`, `examples/`, `models/`, or `contributing/`.
- Put current technical interpretation under `design/` or `performance/`, while keeping normative decisions in the root authorities.
- Put immutable measurements under `evidence/` and implementation specifications under `prds*`.
- Move superseded narrative documents to `archive/`; do not keep competing current summaries.
- Prefer links to canonical documents over duplicating status prose.
- Do not edit frozen evidence to make it look current. Add a current analysis that links to it.
- Run `python3 scripts/check-markdown-links.py` after moving or renaming documentation.
