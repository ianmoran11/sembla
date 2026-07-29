# PRD 0006 implementation notes — attempt 1

## Feature split and validation

`grouped-observations` is a runtime value (`sembla_ir::FeatureSet`) threaded from
CLI parsing through model validation and CPU execution. It is not global state
or a Cargo feature. The Lean authoring surface has no runtime option context, so
it always elaborates grouped syntax fully; the comment beside
`groupedViewTerm` records this division. Rust rejects a legacy grouped model at
its `$.boxes[*].grouped_views[*]` location unless the runtime set contains the
feature.

Artifact validation and execution are intentionally separate. A direct-stable
plan that accurately lists `grouped-observations` validates with `sembla
validate` without a CLI runtime flag. Running that same plan requires
`--enable grouped-observations`. Plan validation enforces both directions:
grouped views require the feature string, and the string is invalid when no
grouped view exists. Unknown plan/runtime features remain errors. This pins the
PRD's requested decision rather than treating artifact inspection as execution.

## Sink and backend behavior

Grouped counts are evaluated from immutable committed state after each tick.
The evaluator has no RNG, write-buffer, conflict, or scheduler access. Numeric
key tuples are accumulated in `BTreeMap<Vec<i128>, _>` before rendering, so Enum
labels and decimal Ref spellings cannot change sort order. Int bands use
`div_euclid`, defining negative values by floor division toward negative
infinity.

CPU run and sweep write sparse long-form grouped CSV files and manifest SHA-256
records. CUDA rejects before backend construction with
`grouped observations run on the cpu backend only for now`. Compare and
diff-backends reject grouped inputs with the named follow-up diagnostic.
Composition-source JSON recognizes and deterministically rejects a
`grouped_views` primitive field under DECISIONS §K6; linker support was not
added.

## Compatibility notes

Empty Rust/Lean `grouped_views` fields are omitted, and empty manifest
`enabled_features`/`grouped_outputs` fields are omitted, preserving legacy
model, plan, and manifest bytes. The two `crates/sembla-cuda/src/codegen.rs`
changes only initialize the new empty IR field in existing test-only model
literals; CUDA production semantics are unchanged.

## Validation

Passed:

- `cargo test --locked -p sembla-ir --test grouped_validation`
- `cargo test --locked -p sembla-cli --test grouped_observations`
- the ignored exact Lean model/plan fixture-regeneration test
- `cd frontend && lake build Sembla.GroupedObservationTests Sembla.Composition.SourceTests`
- `cd frontend && bash scripts/test-negative.sh`
- `bash frontend/scripts/check-parity.sh`
- `cargo test --locked --workspace`
- `./scripts/check.sh` (documentation, formatting, Clippy, Rust tests, Lean build,
  proof hygiene, negative suite, parity, dependency policy, and lock checks)
