# PRD 0005: Composition wiring/nesting structure widget

## Context

Read `docs/prds-composition-integration/README.md` first; its constraints
bind — especially decision 7 (structure widgets only).

The Lean infoview is the project's stated differentiator (DESIGN.md §3;
DECISIONS §A1), and hierarchy-aware widgets through source structure were an
explicit goal of the composition architecture. Two structure widgets exist
today — the state-machine diagram and prior-marginal plot — built as pure
props structures (`frontend/Sembla/Widgets.lean`: `StateDiagramProps`,
`HazardPanelProps`), themed HTML rendering (`frontend/Sembla/
WidgetDisplay.lean`), attachment via `Widget.savePanelWidgetInfo`
(`WidgetDisplay.lean:775`), and pure-prop tests (`WidgetTests.lean`).

The composition surface currently skips widgets entirely: `Surface.lean`
elaborates primitive bodies through `elaborateSurfaceModelNoWidgets`
(`Surface.lean:15,502,743`). So placing the cursor on a
`sembla_component`/`sembla_composition` declaration shows nothing — the one
place where a wiring picture is most valuable.

## Goal

A composition structure widget renders the instance/wire/exposure graph of
a component or composition under the cursor, with zero runtime cost, built
in the existing widget architecture; existing widgets and all frozen
contracts are untouched; primitive-body panels are re-attached where the
existing kernel hook allows it cheaply.

## Specification

### 1. Props — pure and testable

In a new `frontend/Sembla/Composition/Widget.lean`, define pure props built
from the already-elaborated composition values (never from JSON, never by
re-elaborating):

```lean
structure CompositionNodeProps where
  instanceName : String        -- display name
  definitionName : String      -- display name of the definition
  kind : String                -- "primitive" | "composite"
  ports : Array (String × String)  -- (port display name, "input"|"output")

structure CompositionWireProps where
  source : String              -- "population.infection_count"
  target : String
  delayTicks : Nat             -- always 1 in V1; render it anyway

structure CompositionExposureProps where
  inner : String
  outer : String

structure CompositionDiagramProps where
  title : String
  nodes : Array CompositionNodeProps
  wires : Array CompositionWireProps
  exposures : Array CompositionExposureProps
  hidden : Array String
  deriving …                    -- match the Json/Repr derivations the
                                -- existing props structures use
```

Builders:

- `diagramPropsOfDefinition : ComponentDefinitionV1 → CompositionDiagramProps`
  — for a composite: its instances/wires/exposures/hidden ports, one level
  deep (children shown as boxes with their boundary ports; **no** recursive
  expansion into grandchildren — the widget shows the authored level, and
  the user clicks into a child definition to see its internals). For a
  primitive: nodes = one box with its ports; the existing state-diagram
  panel covers its internals.
- `diagramPropsOfSource : CompositionSourceV1 → CompositionDiagramProps` —
  the root definition's diagram, title from `displayName`.

Determinism: arrays in author order (source order is meaningful and stable);
display names throughout (this is presentation — stable ids appear only in
tooltips/secondary text if the existing widget style has one).

### 2. Rendering

Extend `WidgetDisplay.lean` (or a sibling module if that file's structure
prefers it — follow its internal organization) with an HTML renderer for
`CompositionDiagramProps` in the same visual family as the state diagram:
boxes for instances (kind-distinguished styling), labeled directed edges
for wires with a `1-tick` delay marker, distinct edge styling for exposures
(zero-delay alias — visually different from wires; this distinction is
load-bearing, DESIGN.md §10.7), hidden ports listed struck-through or in a
"hidden" row. Respect the existing theme mechanism exactly — every theme
the current widgets support must render the new widget (the demos test "all
themes"; copy that convention).

Keep it HTML/CSS in ProofWidgets' existing capabilities. No new JS
components, no external assets, no layout engine — a simple
flex/grid arrangement is sufficient at V1 fixture scale (≤4 instances). If
edge-drawing between boxes is not achievable with the existing widget
toolkit's conventions, render wires as a connection *list* under the box
row rather than importing anything new — visual ambition is explicitly
subordinate to zero-dependency discipline.

### 3. Attachment

In `Surface.lean`, attach the panel at elaboration:

- `sembla_component` (composite): diagram of the definition.
- `sembla_component` (primitive): reuse the existing state-diagram/hazard
  panels for the body **iff** the surface kernel's widget-enabled path can
  be invoked on a component body without behavioral drift (the kernel
  needs a model-shaped value; a primitive component body is box-shaped).
  Investigate first: if `elaborateSurfaceModel` (widget-enabled) has or can
  trivially gain a box-granular hook, use it; if it requires reshaping the
  kernel, attach only a ports/interface summary panel for primitives now
  and record the state-diagram reattachment as deferred-with-reason in the
  implementation notes. Do not fork the kernel to force it (the one-kernel
  rule from the surface-syntax track binds).
- `sembla_composition`: diagram of the root definition, plus the existing
  prior-marginal panel for its `param` declarations if that panel's
  builder accepts the param list without change (same investigate-first
  rule).

Anchor panels to the declaration tokens the commands already retain;
diagnostics anchoring must not change (the negative suite proves it).

### 4. Tests

- **Pure prop tests** in `frontend/Sembla/Composition/WidgetTests.lean`
  (imported from `Sembla.lean`), `#guard`-style like `WidgetTests.lean`:
  props for `EpidemicPolicy` (2 nodes, 2 wires, exposure list matching the
  exposing variant), `TwoRegions` (2 composite nodes, 0 wires),
  `RegionalResponse` (1 node, exposure + hidden entries), and a primitive
  (ports only). Author-order preservation pinned. Use the surface-authored
  values from `SurfaceModels.lean` so the test also guards the
  surface→props path.
- **Rendering tests**: whatever form the existing widget tests use for
  HTML/themes (pure render-to-string guards or snapshot fixtures — copy
  the pattern), covering at least one diagram in every theme.
- **No-drift**: `bash frontend/scripts/test-negative.sh` and the full
  parity script pass unchanged; `sembla_model` widget behavior untouched
  (existing widget tests are the proof).

### 5. Documentation

`docs/guides/composition.md`: add a short "Inspecting compositions" section with a
screenshot-free description of what the panel shows and the wire-vs-exposure
visual distinction (this doubles as the §10.7 tick-delay teaching surface).
Update the demo suite doc-comment only if `Demos/Composition*.lean` already
narrates widgets (read first; keep demos' goldens untouched).

## Allowed files

- `frontend/Sembla/Composition/Widget.lean`, `WidgetTests.lean` (new)
- `frontend/Sembla/Composition/Surface.lean` (attachment only)
- `frontend/Sembla/WidgetDisplay.lean` (additive rendering; existing
  renderers byte-unchanged), `frontend/Sembla/Widgets.lean` (only if a
  shared helper genuinely belongs there)
- `frontend/Sembla.lean` (imports), `docs/guides/composition.md`
- implementation notes/artifacts created by the managed run

## Non-goals

- Behavior widgets, simulation calls, sliders, or latency work (v0.4).
- Recursive multi-level expansion, pan/zoom, graph layout engines, JS/CSS
  dependencies, or new ProofWidgets capabilities.
- Rendering from source maps, plans, or JSON (elaborated values only).
- Any change to existing widget output, the surface kernel's semantics, or
  diagnostics positions.

## Acceptance criteria

1. `cd frontend && lake build` passes; placing the cursor on each fixture
   `sembla_component`/`sembla_composition` in `SurfaceModels.lean` attaches
   a panel (attachment is code-verifiable: the `savePanelWidgetInfo` call
   sites exist for each command form, and the prop builders are exercised
   by tests).
2. Pure prop guards pass for the four listed fixtures with author-order
   pinned; every supported theme renders the diagram in the rendering
   tests.
3. Wires and exposures are visually distinct, and wires carry the delay
   marker — verifiable in the rendered HTML output of the tests.
4. The primitive-body panel decision (reused vs deferred-with-reason) is
   recorded in the implementation notes after actual investigation of the
   kernel hook.
5. Negative suite, parity script, existing widget tests, and
   `./scripts/check.sh` all pass unchanged; no Rust diff; no new
   dependencies.
6. `docs/guides/composition.md` gains the inspection section including the
   wire/exposure distinction; `git diff --check` passes.
