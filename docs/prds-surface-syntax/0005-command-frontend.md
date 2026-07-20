# PRD 0005: Complete command-style `sembla_model` frontend

## Context

Read the folder README first; its constraints bind. PRDs 0001–0004 have made
parameters, transitions, and frequency hazards mathematical while retaining the
legacy bracketed enclosing term. This PRD implements option D: a complete
indentation-structured command that collects every currently supported model
feature and invokes the same PRD 0001 semantic kernel.

This is the high-risk parser PRD. Canonical models are not migrated until PRD
0006; a full command/legacy twin and positioned command diagnostics must be
green first.

## Goal

`sembla_model` defines an ordinary namespace-respecting `Model` constant from
command-style declarations with no list brackets, separator commas, or required
empty categories, while preserving exact IR bytes, order, diagnostics, widgets,
and legacy `model%` compatibility.

## Specification

### 1. Module and single-kernel architecture

Implement the command grammar in `frontend/Sembla/DSL.lean` or, if separation
is necessary, a dependency-free `frontend/Sembla/CommandDSL.lean` imported by
`frontend/Sembla.lean`. A separate parser module is allowed; a separate semantic
builder/type checker is not.

The command elaborator must:

1. collect declarations/tokens into the shared PRD 0001 surface-model value;
2. invoke the shared uniqueness, scope, expression, type, schema, reference,
   ordering, and IR-emission path;
3. evaluate the resulting model for widgets through the same path; and
4. define exactly one ordinary Lean constant at the requested namespace/name.

Do not lower by generating a giant quoted `model%` syntax tree and then lose
source tokens. Do not emit `Model.mk` independently.

### 2. Header and model constant

Accept exactly the header family:

```lean
sembla_model SirWorkplace (dt := 0.25) where
  ...

sembla_model SirWorkplace
    (name := "sir_workplace_frequency_dependent")
    (dt := 0.25) where
  ...
```

`(dt := ...)` is mandatory. `(name := ...)` is optional and, when present,
sets `Model.name` exactly. Without it, derive the model runtime name using the
folder README algorithm. The command defines `SirWorkplace : Sembla.IR.Model`
in the current namespace and respects normal Lean duplicate-declaration rules.
Preserve the declaration/header tokens for errors.

### 3. Model-level declarations

Inside the model block accept interleaved, repeated declarations of:

```lean
param β : ℝ := 0.8 ~ LogNormal (-0.2231435513142097) 0.25
param γ : ℝ := 0.1

box population where
  ...

wire population infection_count -> policy infection_count
summary peak_I := max population.I
summary peak_tick := argmaxₜ population.I
```

Params use PRD 0002's exact forms. Wires keep the existing four endpoint
identifiers and ASCII `->`. Summaries accept exactly `sum`, `min`, `max`,
`last`, and `argmaxₜ` over `box.view`. Unknown boxes/ports/views and duplicate
wire targets use the shared diagnostics and original command tokens.

### 4. Box-level declarations

Inside a box accept interleaved declarations of systems, inputs, arrow/general
transitions, outputs, and views. No `systems []`, `inputs []`, etc. blocks are
present.

#### Systems and attributes

```lean
system Person (rows := 1_000_000) where
  health : {S, I, R}
  employer : Employer

system Employer (name := "employer") (rows := 50_000)
```

`(rows := ...)` is mandatory and `(name := ...)` optional. An empty system
omits `where`. Infer attribute type from the frozen forms:

- `{...}` → enum/state attribute;
- `ℝ` → Real;
- `Int` → Int; and
- another collected system identifier → Ref.

Forward references are valid. Preserve attribute and enum-variant order.
Existing duplicate/empty-enum/ref-target/row-bound diagnostics apply at command
tokens.

#### Inputs

```lean
input restriction_modifier where
  modifier_offset : ℝ
  infected : Int
```

Each indented field becomes one ordered schema attribute using the complete
current `semblaAttr` type family: enum `{...}`, `ℝ`, `Int`, and a collected
system identifier as Ref. Preserve schema/variant order and forward Ref
resolution exactly as legacy inputs do. `inputSum` remains restricted to Real
or Int fields; accepting enum/Ref schema declarations does not add new input
expression operators. Add a legacy/command twin whose input schema contains
all four attribute kinds.

#### Reaction and general transitions

Reuse PRD 0003 arrows unchanged:

```lean
infect on Person : health: S →[β · freq (health = I) over employer] I
```

Add command-style general transitions for rules with extra guards/effects:

```lean
transition restrict on Controller where
  guard mode = Open ∧ inputSum infection_count field infected > 500
  hazard 1e300
  set mode := Restricted
  set modifier := 0.4
```

`guard` and `hazard` occur exactly once. One or more ordered `set` lines are
required. Reuse existing expression/effect typing and the existing diagnostic
that Ref writes need unsupported resource claims. Do not add contests.

#### Outputs

```lean
output infection_count from Person where
  infected : Int := count where health = I
  total_risk : ℝ := sum (risk)
```

Each field simultaneously declares the output schema name/type and existing
`OutputField` builder. Support only current count-with-filter and sum-of-value
builders. Preserve field order and require the declared field type to match the
aggregate result under existing rules.

#### Views

```lean
view S := count Person where health = S
view total_risk := sum Person using risk
view minimum_visits := min Person using visits
view active_risk_max := max Person where health = I using risk
```

Count has no `using`; non-count reductions require `using`. `where` is optional.
Support exactly current view reductions `count|sum|min|max`, current typing, and
source-table/attribute diagnostics. Preserve view order.

### 5. Stable collection and ordering

Collection remains multi-pass. Forward references work regardless of whether a
system/parameter/port/view is textually declared before its use where the
current kernel permits forward resolution. Declarations may be interleaved,
but each emitted IR list is a **stable partition** of source declarations:
relative textual order among params, boxes, systems, transitions, inputs,
outputs, views, wires, summaries, attributes, fields, and effects must be
preserved.

Add a deliberately interleaved positive fixture asserting every resulting list
order. Never sort by name or choose inference candidates by iteration order.

### 6. Full-feature command twin

Add one imported full-feature command model and an equivalent legacy `model%`
twin covering:

- prior and priorless parameters;
- two systems with a forward Ref;
- Real/Int/enum/Ref system attributes and an input schema covering all four;
- inferred or explicit reaction arrows and `freq`;
- a general multi-effect transition;
- two boxes, typed inputs/outputs, and two feedback wires;
- all four view reductions and all five summary reductions; and
- widget state/hazard props.

`Sembla.Demos.Modeling.featureTour` is the target feature set if that module is
present; do not migrate it yet. Assert structural `Model` equality, exact
`IR.toJson` equality, and equal widget props between command and legacy twins.

### 7. Command-specific diagnostics

Add complete negative files and exact expected lines for at least:

- missing/zero/out-of-range `dt`;
- duplicate model/parameter/box/system/table/attribute/port/transition/view/
  summary declarations;
- invalid indentation/misplaced declaration with a deliberate message where
  Lean recovery syntax permits it;
- unknown Ref target, transition system/attribute/parameter/input, output
  source/field, wire endpoint, view source/value, and summary view;
- arrow ambiguities from PRD 0003 in command form;
- input/output wire schema mismatch and duplicate wire destination;
- wrong guard/hazard/effect/output/view types;
- count view/output with a value and non-count view without `using`;
- general transition missing/duplicating guard or hazard, or with no effects;
  and
- unsupported declaration form rather than accepted inert syntax.

Reuse shared semantic diagnostic text where meaning is identical. Positions
must point to command tokens, not quoted kernel syntax.

### 8. Widget cursor anchors

System names and arrow/general transition names must remain distinct infoview
cursor targets. Add data-level tests proving props equal to the legacy twin and
an implementation note recording manual cursor checks in VS Code for one system,
one arrow, and one general transition. Visual redesign is forbidden.

## Allowed files

- `frontend/Sembla/DSL.lean`
- optional new `frontend/Sembla/CommandDSL.lean`
- `frontend/Sembla.lean` for module/test registration
- focused surface/positive/negative/widget-data tests
- `frontend/scripts/test-negative.sh`
- minimal `frontend/README.md` syntax note only if needed to make manual testing
  possible; full docs migration belongs to PRD 0006

## Non-goals

Canonical/public model migration, removal of `model%`, C(i), option E, IR/JSON,
Rust/runtime, new aggregate/effect/claim semantics, autocomplete, formatter,
pretty-printer, or widget styling.

## Acceptance criteria

1. The complete frozen command grammar defines ordinary namespace-respecting
   `Model` constants and supports every current model feature without mandatory
   empty blocks/list punctuation.
2. The command adapter feeds the sole PRD 0001 kernel; no duplicate IR builder,
   type checker, name resolver, schema checker, or widget path exists.
3. Full-feature and interleaved command fixtures equal their legacy twins
   structurally, byte-for-byte in JSON, in every emitted list order, and in
   widget props.
4. Forward references and deterministic arrow/name resolution remain green;
   every listed command negative has an exact positioned diagnostic.
5. `model%`, canonical models, exporter aliases, fixtures, and all existing
   runtime/parity behavior remain unchanged.
6. No IR/JSON/Rust/dependency/workflow changes occur.
7. All required README checks and `git diff --check` pass.
