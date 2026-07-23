# Sembla Lean frontend

This Lean 4 package contains Sembla's pure deep IR, the public command-style
modeling language, a supported compatibility syntax, structure widgets, proofs,
and eight Lean-authored canonical models. It deliberately does not depend on
mathlib. `lean-toolchain` pins Lean 4.13.0, so an `elan` installation selects the
same compiler automatically.

## Setup and build

Install [elan](https://github.com/leanprover/elan), then run from the repository
root:

```sh
cd frontend && lake build
```

Lake resolves Lean's standard library plus pinned ProofWidgets4, the package's
only direct external dependency. ProofWidgets inherits Batteries transitively;
both revisions are recorded in `lake-manifest.json`.

## Public command surface

Human-authored models use `sembla_model`. The command header defines an ordinary
namespace-respecting `Sembla.IR.Model` constant. `(dt := ...)` is mandatory;
`(name := ...)` is optional and sets the exact runtime model name.

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

    view infectious := count Person where health = I
    view total_risk := sum Person using risk
    view minimum_visits := min Person using visits
    view active_risk_max := max Person where health = I using risk

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
  summary peak_I := max population.infectious
  summary peak_tick := argmaxₜ population.infectious

end Example
```

There are no list brackets, separator commas, or required empty category
blocks. Model declarations and box declarations may be interleaved. Collection
is multi-pass, so parameters, systems used as Ref targets, ports, and views may
be referenced before their declarations wherever the semantic kernel permits
it. Every emitted IR category is a stable partition of source declarations:
relative textual order within params, boxes, systems, transitions, fields,
effects, views, wires, and summaries is preserved.

### Names, parameters, and expressions

Without a model-name override, the Lean declaration identifier is converted by
the frozen snake-case derivation (`SirWorkplace` becomes `sir_workplace`). A
system identifier uses the same derivation for its table name (`Person` becomes
`person`). Use `(name := "...")` after the model or system identifier whenever
the wire-format name must differ:

```lean
sembla_model observations (dt := 1.0) where
  box population where
    system Person (name := "Person") (rows := 4) where
      status : {active, inactive}
```

Greek identifiers use documented transliterations, so `β`, `γ`, `σ`, and
`«λ_parent»` produce `beta`, `gamma`, `sigma`, and `lambda_parent`. Derivation
collisions are errors rather than name-dependent resolution.

A parameter declaration binds its identifier throughout the model. References
are bare names, not substituted defaults:

```lean
param β : ℝ := 0.8 ~ LogNormal (-0.2231435513142097) 0.25
param γ : ℝ := 0.1
```

The demographic-slot surface additions also accept integer parameters with
integer-literal defaults, including negative literals:

```lean
param retirement_months : Int := 780
param offset : Int := -3
```

`Int` parameters lower to the existing integer IR parameter type and can be
used anywhere the scalar expression typechecker accepts `Int`, including guards
and integer `set` effects. Priors remain real-valued: attaching any prior to an
`Int` parameter is rejected. Defaults and optional `LogNormal` priors on real
parameters remain first-class IR metadata. The surface accepts `ℝ`, mathematical
multiplication `·`, conjunction `∧`, inequality `≠`, and less-than-or-equal `≤`.
Its ASCII operators are `*`, `/`,
`+`, `-`, `=`, `<`, `>`, and `&&`; there are deliberately no ASCII `!=` or
`<=` forms. Expressions also support numeric arithmetic, enum comparisons, `inputSum`, and the
restricted aggregate forms described below. Real values are stored as exact
coefficient/exponent `Scientific` data, preserving supported finite
`f64`-range decimals.

### Systems, attributes, and forward references

Every system requires `(rows := <natural-number literal>)`. This is an IR
`Table.sizeHint`, not runtime population initialization. A nonempty system uses
an indented `where` block; an empty system omits it. Attribute types are inferred
from exactly these forms:

```lean
state : {Open, Restricted} -- enum/state
risk : ℝ                    -- real
visits : Int                -- integer
employer : Employer         -- Ref to a collected system
```

Enum variants and attributes retain textual order. Ref targets must be systems
in the same box, but can be declared later because collection precedes
resolution.

### Reaction arrows and general transitions

Use a reaction arrow when a transition is exactly one enum equality guard and
one write to that enum attribute:

```lean
infect on Person : health: S →[β · freq (health = I) over employer] I
recover on Person : health: I →[γ] R
```

`on Person` may be omitted only when the compatible system is unique. The state
attribute may be omitted only when the selected system has exactly one enum
attribute; a system with multiple enum attributes always requires explicit
`health:` syntax, even if only one contains both endpoint variants. Explicit
`on Person` and `health:` are the stable disambiguation forms; ambiguous
inference is rejected rather than chosen by iteration order.

Use the general form for extra guards, multiple effects, or controller rules:

```lean
transition restrict on Controller where
  guard mode = Open ∧ inputSum activity field infected > 100
  hazard 1e300
  set mode := Restricted
  set modifier := 0.4
```

A general transition requires exactly one `guard`, exactly one `hazard`, and at
least one ordered `set`. As a demographic-slot surface addition, numeric effects
accept the existing row-local scalar expression fragment, so old-snapshot
updates such as `set age_months := age_months + 1` are valid. The expression
must have exactly the destination attribute's type; there is no additional
coercion. Enum effects remain variant literals, Ref writes still require
unsupported resource claims, and aggregate forms such as `countBy`, `freq`, and
`inputSum` are rejected in effect expressions.

### Keyed frequency

`freq (predicate) over key` is the supported frequency-shaped aggregate. It
means the exact legacy `countBy key (predicate) / sizeBy key` plan and does not
introduce a general query language. It requires:

- a transition, output, or view context with a selected source system;
- `key` to be a Ref attribute of that system; and
- a Boolean row-local predicate over the selected system.

Input aggregates, relational aggregates, nested `freq`, non-Ref keys, and
predicates that leave the row-local fragment are rejected with positioned
diagnostics. Keyed comprehensions with row binders (option C(i)) remain deferred
until a real model requires them.

### Inputs, outputs, views, wires, and summaries

Inputs declare ordered schemas with the same enum/real/integer/Ref type family:

```lean
input activity where
  infected : Int
  total_risk : ℝ
  mode : {Open, Restricted}
  subject : Person
```

`inputSum port field column` remains limited to numeric schema fields. Outputs
simultaneously declare an ordered schema and builders:

```lean
output activity from Person where
  infected : Int := count where health = I
  total_risk : ℝ := sum (risk)
```

Count builders have a filter and no value; sum builders have a value. Views use
`count`, `sum`, `min`, or `max`; count has no `using`, while valued reductions
require it. An optional `where` filter remains row-local:

```lean
view infectious := count Person where health = I
view total_risk := sum Person using risk
view minimum_visits := min Person using visits
view active_risk_max := max Person where health = I using risk
```

Wires use four endpoint identifiers and ASCII `->`. Schemas must match and a
destination may be delivered to only once:

```lean
wire population activity -> policy activity
```

Model-level summaries fold `box.view` streams with exactly `sum`, `min`, `max`,
`last`, or `argmaxₜ`; `argmaxₜ` returns the earliest tick attaining the maximum.
Views are observation sinks and cannot feed transitions except through explicit
output/input ports and one-tick-delayed wires.

## Compatibility and machine-writer paths

`model%` remains a supported low-level compatibility and semantic-kernel form.
It is intentionally retained for old models and the legacy/new syntax-twin
regression suite. Both `model%` and `sembla_model` collect the same surface-model
records and invoke the same uniqueness, scope, expression, schema, ordering,
widget, and IR-emission kernel; neither frontend has separate semantics.

Generated models and low-level tests may construct `Sembla.IR` values directly.
That direct-constructor API is the machine-writer path, not the recommended
human surface. A do-notation builder (option E) is rejected/deferred for human
authoring because it reads as imperative construction rather than mathematics.

Complete ill-formed models under `frontend/Negative/` pin full ordered sets of
positioned errors. Focused positives and syntax twins cover forward references,
name derivation, priors, arrows, frequencies, stable ordering, and command/legacy
byte equality. Run them from the repository root with:

```sh
cd frontend && bash scripts/test-negative.sh
```

## Export, validation, and parity

Lean elaborates, inspects, renders structure widgets, proves specification-level
results, and serializes models. Rust validates whole exported models and
executes them. The Rust runtime remains outside Lean; target 1a is proved at the
specification level while evaluator-level target 1b remains open.

Representative export and validation commands are:

```sh
cd frontend
lake exe sembla-export sir /tmp/sir.json
lake exe sembla-export Sembla.Models.sirPolicy /tmp/sir_policy.json
lake exe sembla-export observations /tmp/observations.json
cd ..
cargo run -p sembla-cli -- validate /tmp/sir.json
cmp examples/sir.json /tmp/sir.json
cargo run -p sembla-cli -- diff-ir examples/sir.json /tmp/sir.json
```

The exporter accepts every concise snake-case and camel-case spelling plus the
existing `Sembla.Models.*` and `Sembla/Models/*` qualified aliases. `diff-ir` is
a useful normalized semantic comparison, but it does not replace literal byte
comparison. The parity script exports all eight canonical models and every
accepted alias, validates both sides, and uses literal `cmp` against checked-in
fixtures before supplemental `diff-ir` checks:

```sh
bash frontend/scripts/check-parity.sh
```

It also runs checked and exported models with fixed seeds, comparing CSV bytes,
summaries, final-state hashes, and output hashes while asserting nontrivial
dynamics and conserved state counts. No fixture regeneration is part of the
workflow. The parity command requires Git plus both pinned Rust and Lean
toolchains. For Rust-only validation without Lake, run:

```sh
./scripts/check-rust.sh
```

The strict complete repository check requires Cargo, Git, and Lake and never
silently skips frontend validation:

```sh
./scripts/check.sh
git diff --check
```

The canonical [local check contract](../docs/ci.md#local-check-contract) also
lists determinism, the reduced NPE smoke test, and manual GPU evidence with the
required environments.

The canonical-model catalog, formulas, run commands, and current limits are in
[`docs/examples/canonical-models.md`](../docs/examples/canonical-models.md).

## Proofs

`Sembla.LumpingProof` proves `groupedCount_eq_naiveCount`, exact agreement of
the grouped and naive coworker-count plans, and `plan_rewrite_congr`, which
transports that equality through any per-row function of the count. This is
theorem target 1a at the specification level, not a theorem about the
deep-embedding evaluator; target 1b remains open.

Run the proof-hygiene guard directly, or use the complete repository check
(which includes it):

```sh
bash frontend/scripts/check-proofs.sh
./scripts/check.sh
```

## Widgets

The frontend pins ProofWidgets4 `v0.0.44` for Lean 4.13. Pure functions in
`Sembla.Widgets` build JSON-encodable props from the already-elaborated `Model`;
`Sembla.WidgetDisplay` turns those props into HTML/SVG and registers infoview
panels. Neither path invokes the Rust runtime or performs simulation.

Widgets default to the restrained `academic` preset. A source file may select
any of the three themes before its model declarations:

```lean
set_option sembla.widget.theme "academic" -- also: "editor" or "notebook"
```

`editor` follows standard VS Code widget chrome; `notebook` is softer and more
rounded. `professional` remains an alias for `academic`. All themes inherit the
active VS Code foreground/background for dark and high-contrast support.

To verify widgets manually:

1. Run `cd frontend && lake build`, open the repository with the VS Code Lean 4
   extension, and open the Lean infoview.
2. In `frontend/Sembla/Models.lean`, place the cursor on the `Person` system
   declaration in `sir`. Expect **State machine — person**, states `S`, `I`,
   `R`, and edges `infect` and `recover`.
3. Place the cursor on the `infect` arrow declaration. Expect its distinct
   transition panel, wrapped `freq` hazard, beta default and LogNormal prior,
   and the aggregate-dependent no-probability-plot explanation.
4. Place the cursor on the `recover` arrow declaration. Expect its distinct
   panel, gamma metadata, prior-density plot, and monotone
   `p(dt) = 1 - exp(-lambda * dt)` chart.
5. In `frontend/Sembla/Demos/Modeling.lean`, place the cursor on the general
   `restrict` transition and confirm it has a distinct panel from the system
   and reaction declarations.
6. Resize the infoview to roughly 280--320 pixels and repeat the checks in dark,
   light, and high-contrast VS Code themes. Headers, state labels, axes, cards,
   series, outlines, and text must remain legible without overlap.

Automated props and rendering-structure assertions live in
`Sembla/WidgetTests.lean` and `Sembla/CommandFrontendTests.lean`. They cover
state graphs, hazards, probability/prior plots, responsive SVG, long labels,
loops and opposing routes, badges and empty states, JSON encoding, and all three
theme presets. Final layout and theme verification remains intentionally manual.
