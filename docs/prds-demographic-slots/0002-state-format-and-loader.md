# PRD 0002: `sembla.state/v1` format and the generic initial-state loader

## Context

Read `docs/prds-demographic-slots/README.md` first; its frozen state-format
section is the normative spec for this PRD. DECISIONS §K3 (PRD 0001) binds.

Today, initial state reaches the runtime through exactly two paths, both
inadequate for multi-table models:

- `--population N` → `initialize_population(&model, N)`, which gives every
  table a cardinality derived from one number — wrong for a model with
  independent `PersonSlot`/`Area`/`SlotResource` cardinalities;
- `--population pop.bin` → `SyntheticPopulation::read` +
  `initializers_from_population`, which is hard-coded to SIR shape:
  `health: Vec<u16>`, `employer: Vec<u32>`
  (`crates/sembla-runtime/src/population.rs:24-27`), magic `SEMBLA_POP`.

The runtime itself is already generic: `StateStore::new(model,
initial_tables: Vec<TableInit>)` takes per-table `row_count` and typed
`ColumnData` columns (`state.rs:11-15,56-76`). The missing piece is a
versioned, model-shape-free artifact format and loader that produce those
`TableInit`s — the "versioned population format" DESIGN.md §10.5's descope
has been leaning on without it generically existing. This is the same class
of fix as retiring the SIR-shaped runner branch (DESIGN §4.6): acceptance
means the generic path contains zero model-specific names.

## Goal

A `sembla.state/v1` reader/writer in `sembla-runtime`, wired into every CLI
entry point that accepts `--population`, with exhaustive deterministic
validation, hash records, checked-in fixtures, and byte-frozen legacy
behavior for `SEMBLA_POP` files and numeric populations.

## Specification

### 1. Format implementation — `crates/sembla-runtime/src/state_artifact.rs`

Implement the folder README's frozen format exactly (magic `SEMBLA_STATE`,
LE `u32` header length, canonical-JSON header, raw LE column blobs in
model declaration order). Public API:

```rust
pub struct StateArtifact { /* header + columns, or a streaming reader */ }

pub fn write(path, model: &ValidatedModel, tables: &[TableInit]) -> Result<(), StateArtifactError>;
pub fn read(path) -> Result<StateArtifact, StateArtifactError>;
/// Exact-match validation against the model; returns loader-ready inits.
pub fn to_table_inits(artifact: &StateArtifact, model: &ValidatedModel)
    -> Result<Vec<TableInit>, StateArtifactError>;
pub fn sniff_magic(path) -> Result<StateKind, …>;   // SemblaPop | SemblaState | Unknown
```

The canonical-JSON header must be produced with the same key-sorting and
number rules as `sembla-ir`'s `to_canonical_string` — reuse it (add the
narrow dependency direction that already exists: `sembla-runtime` depends
on `sembla-ir`). `write` must emit deterministic bytes: same model + same
`TableInit`s ⇒ identical file, proven by a test.

Validation in `to_table_inits` (each a distinct deterministic error naming
the offending table/column): magic/version; table-set bijection with the
model by `(box, table)`; `row_count == model-declared rows` for every table
(this is the `rows :=` enforcement from §K3); column-set bijection per
table by name with exact type match; `variant_count` equal to the model
attr's variant count and every stored `u16 < variant_count`; every `ref`
value `< ref_target`'s `row_count` and `ref_target` naming the attr's
declared target table; file length exactly `header_end + Σ blob sizes`.
Never repair, never default, never truncate.

### 2. Hashing

`pub fn state_artifact_hash(path) -> Result<HashRecordV1, …>` — SHA-256
over exact file bytes, domain `sembla.state-artifact/v1`, reusing
`sembla-ir`'s `domain_digest` and `HashRecordV1`. (Manifest wiring is
PRD 0003; this PRD only provides the function plus a CLI convenience:
`sembla state-hash <file.state>` printing
`state sha256 sembla.state-artifact/v1 <digest>`.)

### 3. CLI dispatch

Every `--population <path>` site — `run`, `sweep`, `compare`,
`verify-run`, `diff-backends` (call sites near
`crates/sembla-cli/src/main.rs:794,1151,2393,2595,2791`) — dispatches on
`sniff_magic`:

- `SEMBLA_POP` → the existing SIR path, **byte-identical** (existing
  goldens are the proof; do not touch `initializers_from_population`
  beyond call-site routing);
- `SEMBLA_STATE` → generic path: read, validate against the (already
  validated) model or plan-derived model, build `TableInit`s;
- numeric `--population N` → existing behavior unchanged;
- unknown magic → deterministic error naming both supported formats.

The generic path must work identically for legacy models and plan
envelopes (both produce a `ValidatedModel`; nothing here is
identity-scheme-specific). Update `USAGE`.

### 4. Fixtures and tests

- **Generator convention.** State artifacts are binary; fixtures are
  produced by `#[ignore]`d regeneration tests (the established
  plan-fixture pattern): a regen test builds `TableInit`s in code
  deterministically, writes the artifact, and the normal test compares
  bytes, validates, loads, and runs.
- **Fixtures** under `fixtures/state/`:
  - `two_box_small.state` — for the existing `examples/two_box.json`
    model shape at its declared rows (exercises multi-table with a
    Ref-free schema);
  - `refs_small.state` — a small model fixture (add a test-local model
    JSON under `fixtures/state/models/`) with independent cardinalities
    and a `Ref` column, exercising ref bounds validation;
  - invalid variants under `fixtures/state/invalid/`: wrong magic, wrong
    `schema_version`, missing table, extra table, row-count mismatch with
    declared `rows`, missing column, type mismatch, `variant_count`
    mismatch, out-of-range enum value, out-of-range ref, truncated blob,
    trailing bytes, non-canonical header. One test per fixture asserting
    the specific error.
- **Round-trip:** write → read → `to_table_inits` → write ⇒ identical
  bytes.
- **Determinism:** `sembla run <model> --population refs_small.state …`
  twice ⇒ bitwise-identical CSV and hashes; and a golden run checked in.
- **Legacy freeze:** existing `pop.bin`-based CLI tests and goldens pass
  unchanged; a test asserts a `SEMBLA_POP` file still routes to the
  legacy loader.

### 5. Documentation

New `docs/state-format.md`: the format spec (copy the README's frozen
section as the normative text), validation rules, the `rows :=`
enforcement note, hash record, and the explicit statement that the
artifact carries no execution metadata. Cross-link from
`docs/composition.md`'s running-models section and USAGE.

## Allowed files

- `crates/sembla-runtime/src/state_artifact.rs` (new), `lib.rs`,
  `Cargo.toml` (only if the `sembla-ir` dependency needs adjusting —
  no new external deps)
- `crates/sembla-cli/src/main.rs`, `crates/sembla-cli/tests/**`
- `crates/sembla-runtime/tests/**`
- `fixtures/state/**` (new), `docs/state-format.md` (new), `Cargo.lock`
- implementation notes/artifacts created by the managed run

## Non-goals

- Manifest tuples and `--export-state` (PRD 0003).
- Any change to `SEMBLA_POP` parsing, `synth-pop`, or population
  *generation* (DESIGN §10.5 stands; regen tests are test code, not a
  product feature).
- Compression, streaming/mmap performance work (PRD 0009 measures first),
  or partial/occupied-only artifacts.
- Surface syntax, flags, or model work (later PRDs).

## Acceptance criteria

1. `./scripts/check.sh` passes; every legacy golden, example, and
   `pop.bin` path is byte-unchanged.
2. The format matches the README spec exactly: a hex dump of
   `refs_small.state`'s first bytes shows `SEMBLA_STATE` + LE header
   length + canonical JSON (spot-checked in a test).
3. Every invalid fixture fails with its specific named error; row-count
   mismatch against declared `rows` is among them.
4. Round-trip byte-identity and write-determinism tests pass; the
   state-loaded run golden reproduces bitwise, twice.
5. `sembla state-hash` prints the frozen one-line record format;
   `sembla run` accepts `.state` artifacts for both a legacy model and a
   plan envelope (one test each).
6. The generic path contains no model-, table-, or attribute-name
   literals (review check); `git diff --check` passes; no new external
   dependencies.
