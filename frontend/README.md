# Sembla Lean frontend

This Lean 4 package contains Sembla's pure deep IR, a small surface DSL, and
the two v0.1 example models. It deliberately does not depend on mathlib.
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
    outputs []]
  wires []
```

The elaborator first collects the actual model parameters, boxes, systems,
ports, transitions, outputs, and wires. It then resolves references and emits
the deep IR. A transition's attributes come only from its selected `on`
system, parameter scope comes only from the enclosing model, and input scope
comes only from the enclosing box. Reference targets come only from systems
in that same box; collection happens before resolution, so forward references
such as `Person` referring to the later `Employer` declaration work.

There are no caller-supplied attribute, parameter, input, or reference-target
allow-lists. Ports, output builders, and wires use the same enclosing syntax:

```lean
inputs [input restriction_modifier {modifier_offset : Real}]
outputs [output infection_count {infected : Int} from Person fields [
  field infected := count where health = I]]
wires [wire population infection_count -> policy infection_count]
```

Expressions support numeric arithmetic, enum guards, symbolic
`parameter name`, `countBy`/`sizeBy`, and `inputSum port field column` for the
two fixtures. Output builders and wire endpoints/schemas are checked against
the same collected declarations. Failures use `throwErrorAt` on the original
surface token. Complete ill-formed models under `Negative/` exercise unknown
attributes, non-Boolean guards, unknown reference targets, undeclared
parameters, unknown inputs, effects, systems, invalid numeric typing, enum
schemas, and Rust representation bounds. `Positive/` covers a forward
reference, a priorless parameter, and canonical output-field ordering. Run all of them with
`bash scripts/test-negative.sh`.

`Sembla.Models` authors both fixtures as complete model blocks. Parameter
references become `Expr.param` nodes; defaults and optional priors remain in
the model's first-class `params` block and are never substituted into hazards.
Real values are stored as exact coefficient/exponent `Scientific` data, so JSON
serialization preserves supported finite `f64`-range decimals rather than
relying on `Float.toString` precision.

## Export and parity

From `frontend/`:

```sh
lake exe sembla-export sir /tmp/sir.json
lake exe sembla-export Sembla.Models.sirPolicy /tmp/sir_policy.json
cd ..
cargo run -p sembla-cli -- validate /tmp/sir.json
cargo run -p sembla-cli -- diff-ir examples/sir.json /tmp/sir.json
cargo run -p sembla-cli -- diff-ir examples/sir_policy.json /tmp/sir_policy.json
```

`diff-ir` parses and validates both documents and compares their canonical
Rust serialization, so whitespace is irrelevant. Module/model spellings
`Sembla.Models.sir`, `Sembla/Models/sir`, `sirPolicy`, and `sir_policy` are
also accepted by the exporter.

For the complete build, negative elaboration, export, validation, parity, and
fixed-seed execution-hash check, run:

```sh
bash frontend/scripts/check-parity.sh
```

That script synthesizes one population and runs both the checked-in and
exported SIR model with seed 55 for 20 ticks, asserting identical result and
final-state hashes and identical CSV bytes. The repository `scripts/check.sh`
runs the same workflow whenever `lake` is available and prints a skip warning
on Rust-only hosts.

## Widgets

The frontend pins ProofWidgets4 `v0.0.44` (the Lean 4.13 release). The two
structure widgets consume only the already-elaborated `Model`: pure functions
in `Sembla.Widgets` build JSON-encodable props, while
`Sembla.WidgetDisplay` only turns those props into HTML/SVG and registers the
infoview panels. Neither path invokes the Rust runtime or performs simulation.

To verify the widgets manually:

1. Run `cd frontend && lake build`, then open the repository in VS Code with
   the Lean 4 extension and open the Lean infoview.
2. Open `frontend/Sembla/Models.lean` and put the cursor directly on `Person`
   in line 15 (`system Person ...`). Expect a **State diagram — person** SVG
   with exactly the nodes `S`, `I`, `R` and labelled arrows `infect: S → I`
   and `recover: I → R`; each arrow label also contains its hazard expression.
3. Put the cursor on `infect` in line 21. Expect the same state diagram plus a
   **Hazard — infect** panel showing `health = S`, the aggregate-dependent
   hazard, beta's default, and an inline LogNormal prior-density curve. The
   panel must state that the per-tick probability plot is unavailable because
   the hazard depends on row state or aggregates.
4. Put the cursor on `recover` in line 25. Expect the state diagram plus a
   **Hazard — recover** panel showing `health = I`, hazard `gamma`, gamma's
   default and LogNormal prior density, and an inline monotone
   `p(dt) = 1 - exp(-lambda * dt)` curve beginning at `(0, 0)`.

The automated data-level assertions are in `Sembla/WidgetTests.lean`; they
check exact graph props, JSON encoding, probability monotonicity, three
closed-form LogNormal density samples, the aggregate no-plot explanation,
and the no-prior case. Pixel/layout verification is intentionally manual.
