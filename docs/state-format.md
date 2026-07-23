# Sembla state artifacts

`sembla.state/v1` is the generic, model-shape-free initial-state artifact used
by `sembla run`, `sweep`, `compare`, `verify-run`, and `diff-backends`.
Artifacts contain typed table columns only. They contain no seed, tick,
parameters, execution identity, model hash, checkpoint cursor, or other
execution metadata.

## Frozen binary format (`sembla.state/v1`)

1. **Magic:** the 12 bytes `SEMBLA_STATE`.
2. **Header length:** little-endian `u32`.
3. **Header:** canonical JSON (`sembla.canonical-json/v1` rules — sorted
   keys, compact, no trailing newline) with exactly:
   `{"schema_version": "sembla.state/v1", "tables": [...]}` where each
   table entry is `{"box": ..., "table": ..., "row_count": N, "columns":
   [{"name": ..., "type": "real"|"int"|"enum"|"ref", "variant_count": N?,
   "ref_target": {"box":...,"table":...}?}]}` — `variant_count` present iff
   enum, `ref_target` present iff ref (all-present-or-absent per type).
   Tables and columns appear in **model declaration order**.
4. **Column blobs:** immediately after the header, in header order, raw
   little-endian arrays matching the runtime's `ColumnData` exactly:
   `real` = `f64`, `int` = `i64`, `enum` = `u16` variant indices in the
   model's declaration order, `ref` = `u32` row indices. There is no padding,
   alignment, compression, or trailing data.

## Exact validation

Loading never repairs, defaults, truncates, or reorders an artifact. The reader
and model matcher reject each of the following with deterministic errors naming
the offending table or column where applicable:

- wrong magic, unsupported `schema_version`, malformed or non-canonical header;
- duplicate, missing, extra, or declaration-reordered tables and columns;
- table or column type mismatches;
- metadata that does not provide `variant_count` exactly for enum columns or
  `ref_target` exactly for ref columns;
- an enum count different from the model declaration, or an enum value greater
  than or equal to that count;
- a ref target different from the attribute's box-qualified target, or a ref
  value outside the target table's row range;
- a truncated blob, arithmetic overflow, or trailing bytes.

For artifact-loaded runs, every artifact `row_count` must equal the
corresponding model's declared `rows :=` value (the IR `size_hint`) exactly.
This ends the size-hint-only behavior for this input path. Numeric
`--population N` and legacy `SEMBLA_POP` inputs retain their existing behavior.

## Hash record

The state-artifact hash uses SHA-256 with domain
`sembla.state-artifact/v1` over the exact file bytes through the shared
Sembla domain-digest operation. Print it with:

```text
sembla state-hash initial.state
state sha256 sembla.state-artifact/v1 <64-lowercase-hex-digest>
```

The artifact itself remains execution-metadata-free. Run manifests record state
links separately with optional `initial_state` and `exported_state` tuples. Each
tuple contains `format: "sembla.state/v1"` and the complete
`sembla.state-artifact/v1` hash record; each tuple is either wholly present or
absent.

## Loading

Every command that accepts `--population` dispatches by content, not extension:

```text
--population 1000                 # existing numeric initialization
--population population.bin       # frozen SEMBLA_POP compatibility path
--population initial.state        # generic SEMBLA_STATE path
```

A state artifact is validated only after the model or executable plan has
produced a `ValidatedModel`. Consequently the same loader applies to legacy
model JSON and plan envelopes without depending on their identity scheme.
See [Composition](composition.md) for plan-envelope execution and model
composition.

## Chained runs

`sembla run --export-state final.state` writes every table from the final
committed state, after the final tick barrier. Export uses the same bytes and
validation rules described above. An existing export path is rejected rather
than overwritten: state artifacts are chain links, so silently replacing one
would invalidate the recorded chain. Exporting does not change `results.csv`,
the final-state hash, or the observation/output hashes.

A later run loads that artifact through `--population`. With output manifests,
the first run's `exported_state.hash` and the second run's
`initial_state.hash` are identical, while each manifest records its own seed,
tick count, and resolved parameters. For example:

```sh
sembla run model.json --population initial.state --seed 101 --ticks 12 \
  --params year-1.json --out year-1.csv --export-state year-1.state
sembla run model.json --population year-1.state --seed 202 --ticks 12 \
  --params year-2.json --out year-2.csv --export-state year-2.state
```

`year-1.csv.manifest.json` records `exported_state` for `year-1.state`.
`year-2.csv.manifest.json` records the same hash under `initial_state`, its own
resolved θ from `year-2.json`, and a new `exported_state` for `year-2.state`.
Numeric and legacy `SEMBLA_POP` inputs do not produce `initial_state` tuples.

This is chaining, not checkpoint/restart. A pair of 12-tick runs is **not**
bitwise-equivalent to one continuous 24-tick run, even when both windows use
the same seed and θ. Tick coordinates restart at zero in the second run, so
its counter-based random draws intentionally differ. Manifests describe the
two honest run identities rather than claiming calendar or RNG continuity.

## Fixture regeneration

Binary fixtures under `fixtures/state/` are byte-frozen. Their deterministic
Rust regeneration test is ignored by default and must be invoked explicitly:

```text
cargo test --locked -p sembla-runtime --test state_artifact \
  regenerate_state_fixtures -- --ignored --exact
```

Normal tests regenerate the expected bytes in temporary files, compare every
checked-in fixture, validate and load both valid artifacts, and assert the
specific failure for every invalid artifact.
