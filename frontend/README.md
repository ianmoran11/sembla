# Sembla Lean frontend

This Lean 4 package contains Sembla's pure deep IR, a small surface DSL, and
eight Lean-authored example models. It deliberately does not depend on mathlib.
`lean-toolchain` pins Lean 4.13.0, so an `elan` installation selects
the same compiler automatically.

## Setup and build

Install [elan](https://github.com/leanprover/elan), then run:

```sh
cd frontend
lake build
```

Lake resolves Lean's standard library plus pinned ProofWidgets4, the package's
only direct external dependency. ProofWidgets inherits Batteries transitively;
both revisions are recorded in `lake-manifest.json`.

## DSL

`Sembla.DSL` provides one enclosing, multi-pass `model%` elaborator that
produces an ordinary `Sembla.IR.Model` value:

```lean
def sir : Model := model% "sir_workplace_frequency_dependent" step(0.25) where
  params [
    param beta : Real := 0.8 prior LogNormal(-0.2231435513142097, 0.25),
    param gamma : Real := 0.1]
  boxes [box sir where
    systems [
      system Person as "person" rows(1000000) where [
        state health : {S, I, R}, ref employer : Employer],
      system Employer as "employer" rows(50000) where []]
    inputs []
    transitions [transition recover on Person where
      guard health = I
      hazard parameter gamma
      set [health := R]]
    outputs []
    views [
      view S from Person where health = S reduce count,
      view I from Person where health = I reduce count,
      view R from Person where health = R reduce count]]
  wires []
  summaries [
    summary peak_I from sir view I reduce max,
    summary peak_tick from sir view I reduce argmax_tick]
```

The elaborator first collects the actual model parameters, boxes, systems,
ports, transitions, outputs, views, wires, and summaries. It then resolves
references and emits
the deep IR. A transition's attributes come only from its selected `on`
system, parameter scope comes only from the enclosing model, and input scope
comes only from the enclosing box. Reference targets come only from systems
in that same box; collection happens before resolution, so forward references
such as `Person` referring to the later `Employer` declaration work.

Views and summaries are observation sinks. Views are declared in their box;
a view names its source system, may add a `where` predicate, and either uses
`reduce count` without a value or `using <expression>` with `sum`, `min`, or
`max`. Model-level summaries name a box and view and fold it with `sum`, `min`,
`max`, `last`, or `argmax_tick`; the last reduction returns the earliest tick
attaining the maximum. Both lists preserve textual declaration order in the
exported IR and reported columns. Unknown systems or attributes, non-Boolean filters, invalid
count/value combinations, and undeclared summary views fail elaboration rather
than becoming inert syntax.

There are no caller-supplied attribute, parameter, input, or reference-target
allow-lists. Ports, output builders, and wires use the same enclosing syntax:

```lean
inputs [input restriction_modifier {modifier_offset : Real}]
outputs [output infection_count {infected : Int} from Person fields [
  field infected := count where health = I]]
wires [wire population infection_count -> policy infection_count]
```

Expressions support numeric arithmetic, enum guards, symbolic
`parameter name`, `countBy`/`sizeBy`, and `inputSum port field column` across
the examples. Output builders and wire endpoints/schemas are checked against
the same collected declarations. Failures use `throwErrorAt` on the original
surface token. Complete ill-formed models under `Negative/` exercise unknown
attributes, non-Boolean guards and view filters, unknown reference targets,
undeclared parameters and summary views, invalid view declarations, unknown
inputs, effects, systems, invalid numeric typing, enum schemas, and Rust
representation bounds. `Positive/` covers a forward reference, a priorless
parameter, canonical output-field ordering, and observation declaration order. Run all of them with
`bash scripts/test-negative.sh`.

`Sembla.Models` authors the SIR fixtures, the all-reduction observation fixture,
and five canonical finite-state examples as complete model blocks. Parameter references become `Expr.param`
nodes; defaults and optional priors remain in
the model's first-class `params` block and are never substituted into hazards.
Real values are stored as exact coefficient/exponent `Scientific` data, so JSON
serialization preserves supported finite `f64`-range decimals rather than
relying on `Float.toString` precision.

## Export and parity

From `frontend/`:

```sh
lake exe sembla-export sir /tmp/sir.json
lake exe sembla-export Sembla.Models.sirPolicy /tmp/sir_policy.json
lake exe sembla-export observations /tmp/observations.json
lake exe sembla-export reversible_ctmc /tmp/reversible_ctmc.json
lake exe sembla-export radioactive_decay_chain /tmp/radioactive_decay_chain.json
lake exe sembla-export sis_importation /tmp/sis_importation.json
lake exe sembla-export seirs_waning /tmp/seirs_waning.json
lake exe sembla-export noisy_voter /tmp/noisy_voter.json
cd ..
cargo run -p sembla-cli -- validate /tmp/sir.json
cargo run -p sembla-cli -- diff-ir examples/sir.json /tmp/sir.json
cargo run -p sembla-cli -- diff-ir examples/sir_policy.json /tmp/sir_policy.json
cargo run -p sembla-cli -- diff-ir examples/observations.json /tmp/observations.json
cargo run -p sembla-cli -- diff-ir examples/reversible_ctmc.json /tmp/reversible_ctmc.json
```

`diff-ir` parses and validates both documents and compares their canonical
Rust serialization, so whitespace is irrelevant. Each model accepts concise
snake-case and camel-case spellings plus `Sembla.Models.*` and
`Sembla/Models/*` qualified spellings. The complete canonical-model catalog,
formulas, run commands, and current limits are in
[`docs/examples/canonical-models.md`](../docs/examples/canonical-models.md).

For the complete build, negative elaboration, export, validation, parity, and
fixed-seed execution-hash check, run:

```sh
bash frontend/scripts/check-parity.sh
```

That script exports, validates, and compares all eight Lean models. It
synthesizes one SIR population for checked/exported SIR and SIR-policy
observation/runtime parity, then runs every canonical checked/exported pair
twice from numeric initialization, asserting identical result/final-state and
observation hashes, identical CSV bytes, nontrivial dynamics, and conserved
enum counts. The repository
`scripts/check.sh` runs the same workflow whenever `lake` is available and
prints a skip warning on Rust-only hosts.

## Widgets

The frontend pins ProofWidgets4 `v0.0.44` (the Lean 4.13 release). The two
structure widgets consume only the already-elaborated `Model`: pure functions
in `Sembla.Widgets` build JSON-encodable props, while
`Sembla.WidgetDisplay` only turns those props into HTML/SVG and registers the
infoview panels. Neither path invokes the Rust runtime or performs simulation.

Widgets default to the restrained `academic` preset: serif display headings,
fine ruled borders, compact corners, and muted research-figure colors that still
inherit the active VS Code foreground/background for dark and high-contrast
support. A source file can select any preset before its model declarations:

```lean
set_option sembla.widget.theme "academic" -- also: "editor" or "notebook"
```

`editor` follows standard VS Code widget chrome and uses stronger chart colors;
`notebook` is softer and more rounded. `professional` is accepted as an alias
for `academic`.

To verify the widgets manually:

1. Run `cd frontend && lake build`, then open the repository in VS Code with
   the Lean 4 extension and open the Lean infoview.
2. Open `frontend/Sembla/Models.lean` and put the cursor directly on `Person`
   in line 15 (`system Person ...`). Expect a **State machine — person** panel
   with a `3 states · 2 transitions` summary, distinct theme-colored nodes
   `S`, `I`, `R`, and short SVG labels `infect` and `recover`. Full rates appear
   in labelled, wrapped transition-detail cards below the graph rather than
   inside the SVG, so aggregate expressions remain readable at narrow infoview
   widths.
3. Put the cursor on `infect` in line 21. Expect the same state-machine panel
   plus a **Transition — infect** panel showing `health = S`, the wrapped
   aggregate-dependent rate, beta's concise default, and an inline LogNormal
   prior-density chart with visible axes and a marked peak. The panel must state
   that the per-tick probability plot is unavailable because the rate depends
   on row state or aggregates.
4. Put the cursor on `recover` in line 25. Expect the state-machine panel plus a
   **Transition — recover** panel showing `health = I`, rate `gamma`, gamma's
   default and LogNormal prior density, and an inline monotone
   `p(dt) = 1 - exp(-lambda * dt)` chart beginning at `(0, 0)`. Parameter and
   probability sections have explicit status/count badges; transitions with no
   referenced parameters or priors show a clear empty state rather than a blank
   area. Axis endpoints and intervals should use rounded values (for example,
   `0`, `0.5`, `1`, `1.5`) while the plotted samples remain unchanged.
5. Resize the infoview to roughly 280--320 pixels and verify that headers wrap,
   state labels and axes remain legible, and no content overlaps. Repeat in a
   dark, light, and high-contrast VS Code theme; cards, state outlines, chart
   series, and text should continue to use the active theme colors.

The automated data-level and rendering-structure assertions are in
`Sembla/WidgetTests.lean`; they check exact graph props, JSON encoding,
probability monotonicity, three closed-form LogNormal density samples, the
aggregate no-plot explanation, the no-prior case, React-compatible style
objects, responsive SVGs, concise chart labels, wrapped hazards, long state
labels, self-loops, opposing transition routes, distinct state colors, chart
peak markers, rounded axis ticks, summary badges, explicit empty states, and all
three theme presets. Theme and final layout verification remains intentionally
manual.
