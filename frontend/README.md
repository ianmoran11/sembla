# Sembla Lean frontend

This dependency-free Lean 4 package contains Sembla's pure deep IR, a small
surface DSL, and the two v0.1 example models. It deliberately does not depend
on mathlib. `lean-toolchain` pins Lean 4.13.0, so an `elan` installation selects
the same compiler automatically.

## Setup and build

Install [elan](https://github.com/leanprover/elan), then run:

```sh
cd frontend
lake build
```

Lake resolves only Lean's standard library; the package has no external Lake
dependencies.

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
