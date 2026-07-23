# Sembla

[![CI](https://github.com/ianmoran11/sembla/actions/workflows/ci.yml/badge.svg)](https://github.com/ianmoran11/sembla/actions/workflows/ci.yml)

CI builds, lints, and tests Rust and Lean, runs the reduced Python NPE smoke test on relevant changes, and byte-compares repeated CPU runs and sweeps for determinism.

Sembla is a simulation framework with a Lean frontend and a deterministic Rust runtime. Its composition pipeline authors reusable components in Lean, links canonical sources into stable-identity executable plans, and verifies portable artifact bundles. See [DESIGN.md](DESIGN.md) for the architecture and project scope.

## Build and test

Run the fast Rust-only contract without requiring Lean:

```sh
./scripts/check-rust.sh
```

Run the strict complete repository contract when both pinned toolchains are
available:

```sh
./scripts/check.sh
```

The complete command requires Cargo, Git, and Lake; it runs Rust validation,
Lean proof hygiene, and frontend parity without silently skipping a missing
tool. The canonical [check matrix](docs/ci.md#local-check-contract) also lists
the determinism check, reduced NPE smoke test, and manual GPU evidence command
with their environment requirements.

Print the CLI version or validate an IR document with:

```sh
cargo run -p sembla-cli -- --version
cargo run -p sembla-cli -- validate examples/two_state.json
```

After authoring a `sembla_composition`, export its source, link it, and run the
standalone plan with three commands:

```sh
(cd frontend && lake exe sembla-export --source surface_epidemic_policy /tmp/epidemic_policy.source.json)
(cd frontend && lake exe sembla-link /tmp/epidemic_policy.source.json --plan /tmp/epidemic_policy.plan.json)
cargo run -p sembla-cli -- run /tmp/epidemic_policy.plan.json --population 1000 --seed 55 --ticks 40
```

See [`docs/composition.md`](docs/composition.md) for component syntax, bundles,
identity and provenance rules, validation, and run verification. The
[`composition showcase`](docs/examples/composition-showcase.md) provides four
runnable models covering counterfactuals, policy fan-out, surveillance, and
deep regional nesting.

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

## Contributing and security

See [CONTRIBUTING.md](CONTRIBUTING.md) for pinned toolchains, canonical checks,
frozen-artifact rules, and managed PRD workflow expectations. Report suspected
vulnerabilities through the private process in [SECURITY.md](SECURITY.md).

## License

Licensed under either of [Apache License, Version 2.0](LICENSE-APACHE) or
[MIT license](LICENSE-MIT), at your option.

## Workspace layout

- `crates/sembla-cli`: `sembla` command-line validation, execution, sweep,
  comparison, verification, and backend-differential workflows.
- `crates/sembla-cuda`: CUDA kernel generation and an optional NVRTC execution
  backend for native-`f64` CPU/GPU differential testing.
- `crates/sembla-ir`: versioned simulation and plan types, stable identities,
  canonical serialization, and semantic validation.
- `crates/sembla-runtime`: deterministic CPU simulation, synthetic population,
  prior-sampling, and local Philox RNG implementation.
- [`frontend/`](frontend/README.md): minimal-dependency Lean DSL, IR exporter,
  linker, and ProofWidgets structure panels (no mathlib).
