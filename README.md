# Sembla

Sembla is a simulation framework with a Lean frontend and a deterministic Rust runtime. See [DESIGN.md](DESIGN.md) for the architecture and project scope.

## Build and test

```sh
cargo build --workspace
cargo test --workspace
./scripts/check.sh
```

Print the CLI version or validate an IR document with:

```sh
cargo run -p sembla-cli -- --version
cargo run -p sembla-cli -- validate examples/two_state.json
```

## IR JSON conventions

IR enums use snake-case `kind` tags, declarations retain source order, and the canonical serializer emits compact JSON with one trailing newline. Validation assigns zero-based `rule_id` values to transitions in declaration order across all boxes; IDs are derived metadata on `ValidatedModel` and are not serialized into the wire format. Parameter expressions always retain symbolic names rather than inlining per-run values. Box-local views declare per-tick scalar projections of committed state, and model summaries declare scalar reductions over those view values. Observation is a sink: view and summary names cannot be referenced by expressions, transitions, wires, or ports and therefore cannot affect execution.

## SIR end-to-end example

The flagship [`examples/sir.json`](examples/sir.json) model uses the
frequency-dependent workplace hazard `beta * I_workplace / N_workplace` and
recovery hazard `gamma`. See [`docs/examples/sir.md`](docs/examples/sir.md)
for deterministic population generation, the versioned population format,
CSV runs, parameter and `dt` overrides, and hash-based verification. The
[`examples/sir_policy.json`](examples/sir_policy.json) two-box feedback demo
and common-random-numbers `sembla compare` workflow are documented in
[`docs/examples/sir_policy.md`](docs/examples/sir_policy.md).

## Canonical finite-state examples

Five additional Lean-authored models cover a reversible two-state CTMC, a
radioactive decay chain, SIS with importation, SEIRS with waning immunity, and
mean-field noisy voter dynamics. Each checked-in JSON model validates and runs
from numeric `--population` initialization using deterministic generic
state-count/firing CSV output. See
[`docs/examples/canonical-models.md`](docs/examples/canonical-models.md) for
the formulas, commands, output schema, initialization semantics, and honest
expressiveness limits.

## Workspace layout

- `crates/sembla-ir`: shared simulation IR types.
- `crates/sembla-runtime`: deterministic CPU interpreter.
- `crates/sembla-cli`: `sembla` command-line binary, including normalized `diff-ir`.
- [`frontend/`](frontend/README.md): minimal-dependency Lean DSL, IR exporter, and ProofWidgets structure panels (no mathlib).
