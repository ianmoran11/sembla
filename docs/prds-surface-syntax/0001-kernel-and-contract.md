# PRD 0001: Shared surface kernel and frozen no-drift contract

## Context

Read the folder README first; its constraints bind. The current `model%`
elaborator in `frontend/Sembla/DSL.lean` already performs the right semantic
work in two passes, but collection, validation, IR emission, evaluation for
widgets, and source-anchor attachment are enclosed in one term elaborator.
Options B/A/C and especially the command frontend must not copy that logic.
This PRD is a behavior-preserving prerequisite: extract one reusable semantic
kernel before adding notation.

## Goal

A single collected surface-model representation and a single validation/IR
emission path power the unchanged `model%` syntax and are reusable by later
surface forms, with canonical JSON, diagnostics, source anchors, and ordering
provably unchanged.

## Specification

### 1. Freeze the current contract before refactoring

Before changing `DSL.lean`, run and record in the implementation notes:

```bash
cd frontend
lake build
bash scripts/test-negative.sh
bash scripts/check-parity.sh
```

The parity script's literal `cmp` checks over all eight canonical exports are
the baseline. Do not regenerate `examples/*.json` or weaken `cmp` to normalized
comparison. Record the current `git diff --exit-code -- examples frontend/lake-manifest.json`
result as additional evidence that no fixture/dependency update was needed.

### 2. Extract one collected surface-model value

Refactor `frontend/Sembla/DSL.lean` so that the enclosing parser first produces
one internal value containing, at minimum:

- the model declaration token/name, optional eventual runtime-name metadata,
  and `dt` term/token;
- ordered parameters;
- ordered boxes, each with ordered systems/attributes, inputs, transitions,
  outputs/fields, and views;
- ordered wires; and
- ordered summaries.

Existing `SurfaceParam`, `SurfaceSystem`, `SurfaceTransition`, `SurfaceBox`, and
related structures may be reused or reorganized. Preserve every original
`Syntax` token currently needed by `throwErrorAt` and widget attachment. The
internal names are not public API; the **single representation and source-token
retention are required**.

### 3. Extract one semantic kernel

Move the current pass-two behavior behind one reusable function/path that:

1. validates model-level uniqueness and numeric bounds;
2. resolves parameters, systems, references, attributes, inputs, outputs,
   views, wires, and summaries;
3. performs the existing expression/type/schema/effect checks;
4. emits one `Sembla.IR.Model` term with exactly the existing list order;
5. elaborates/evaluates that term once for widget props; and
6. attaches state/hazard panels to the retained original tokens.

The current `model%` term elaborator becomes a thin adapter: parse/collect its
legacy bracketed syntax, then invoke this kernel. PRDs 0002–0005 must be able to
feed the same path. If the command frontend later needs a separate module,
expose only the minimum dependency-free internal API under `Sembla.DSL`; do not
make IR-building helpers a second public authoring API.

No alternate `Model.mk` emitter, duplicate expression elaborator, or
command-only validation path is acceptable.

### 4. Pin ordering explicitly

Add focused tests that make semantic ordering visible rather than relying only
on large fixtures. The test model must include multiple values in each
applicable category and assert:

- parameter, box, system/table, transition, input, output, view, wire, and
  summary declaration order;
- attribute/schema/output-field/effect order; and
- transition order across boxes, because downstream rule IDs are model-global.

Use closed `#guard`/`example` assertions over the emitted `Model` and an exact
`IR.toJson` string or a manually constructed expected `Model`. Import the new
test module from `frontend/Sembla.lean` so `lake build` runs it.

### 5. Preserve positioned diagnostics

Run the existing negative suite before and after the refactor. Every existing
expected `file:line:column: error: ...` line must remain byte-identical. If
moving code changes a source position, fix token propagation; do not update the
expected location merely to accommodate generated syntax.

## Allowed files

- `frontend/Sembla/DSL.lean`
- a focused test module under `frontend/Sembla/`
- `frontend/Sembla.lean` to register that test
- implementation notes/artifacts created by the managed run

Do not edit canonical models, examples, docs authority, widgets' rendering
code, IR/JSON, Rust, dependencies, or CI workflows in this PRD.

## Non-goals

- Any new syntax from options A–D.
- Canonical model migration.
- New diagnostics or semantic restrictions.
- Public exposure of internal builder structures.
- Performance optimization unrelated to avoiding duplicate elaboration paths.

## Acceptance criteria

1. Legacy `model%` accepts every pre-PRD source unchanged and all existing
   positioned diagnostics are byte-identical.
2. Code inspection shows one collected representation and one semantic
   validation/emission path; `model%` is a thin adapter and no second IR builder
   exists.
3. Focused imported tests pin all relevant declaration/list orders and exact
   emitted IR/JSON for a multi-declaration model.
4. All eight canonical Lean exports remain byte-identical to `examples/*.json`;
   fixed-seed CSV, summary, state-hash, and output-hash parity remains green.
5. `frontend/Sembla/IR.lean`, `frontend/Sembla/Json.lean`, `examples/`, Rust
   crates, `frontend/lakefile.toml`, and `frontend/lake-manifest.json` are
   unchanged.
6. Required checks from the folder README and `git diff --check` pass.
