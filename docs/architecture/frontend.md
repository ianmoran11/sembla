# Frontend boundary

## Responsibility

`frontend/` owns model authoring, static checking, composition source/linking, structure widgets and the developing Lean semantics. It produces serialized contracts; it does not execute Rust or select an execution backend.

## Conceptual layers

| Layer | Main modules | Hidden complexity |
| --- | --- | --- |
| Deep representation | `Sembla.IR`, `Sembla.Hash` | Exact model vocabulary and stable hashing |
| Contract encoding | `Sembla.Json`, `Sembla.Plan*`, `Sembla.ParameterTable` | Byte-compatible JSON and table decoding |
| Checked semantics | `Sembla.Semantics.*`, `Sembla.Frontend.Builders.*` | Sound/complete static checkers and typed builders |
| Authoring | `Sembla.DSL`, `Sembla.Composition.Surface` | Syntax, elaboration and source-position diagnostics |
| Composition | `Sembla.Composition.Source`, `Link`, `Bundle`, `Spec*` | Recursive source structure lowered to one canonical flat plan |
| Presentation | `Sembla.Widgets`, `WidgetDisplay`, `Composition.Widget` | ProofWidgets structure views; no simulation |
| Models | `Sembla.Models`, `Demos`, `Tutorial` | Authored examples and domain models |

## Public build surfaces

- `Sembla` is the production library imported by `Main.lean` and `LinkMain.lean`.
- `SemblaTests` is a separate default Lake target containing compile-time tests and model validation gates.
- `sembla-export` writes model, direct-plan or composition-source JSON.
- `sembla-link` performs the canonical Lean linker and bundle construction.

This split keeps tests, negative fixtures and validation-only modules out of executable import closures without weakening the default `lake build` contract.

## Dependency rules

`frontend/scripts/check-imports.py` enforces:

- production modules never import test modules;
- IR/plan/encoding contracts do not depend on the DSL, models or widgets;
- foundational semantics does not depend on composition, except the explicit `Semantics.Raw` source/source-map classification edge; and
- composition core does not depend on its surface, widgets or fixtures.

Byte parity with Rust remains a conformance test rather than a shared implementation: `frontend/scripts/check-parity.sh` exports golden models/sources/plans, validates them in Rust and compares exact bytes and selected executions.

## Change locality

- Add authoring syntax in the DSL/surface layer, then lower to existing IR where possible.
- Add a semantic construct only with IR representation, validation, CPU behavior, CUDA behavior or explicit rejection, fixtures, manifest treatment and docs.
- Change serialized field spelling only as a protocol change with updated version/compatibility decisions and parity fixtures.
- Keep widget rendering dependent on elaborated structure, not on runtime execution.
