# PRD 0004: Columnar state store

## Context

Sembla state is an ACSet taken seriously (`DESIGN.md` §4.1): entity tables
stored struct-of-arrays, typed attribute columns, Ref columns as row indices.
Execution is double-buffered read-old/write-new (§4.2) — the uniformity that
makes box boundaries semantically invisible. This PRD builds the store; no
transition execution yet.

## Goal

A `state` module in `sembla-runtime` that instantiates a store from a
validated `sembla_ir::Model`'s schema, supports double-buffered reads/writes,
and produces a canonical SHA-256 state hash used by every later determinism
test.

## Specification

- Column types mirroring `Attr`: `Vec<f64>` (Real), `Vec<i64>` (Int),
  `Vec<u16>` (Enum, variant index), `Vec<u32>` (Ref, row index into target
  table).
- Fixed population per table in v0.1 (no birth/death — `DESIGN.md` §9):
  tables are created at a given size and never grow or shrink.
- Double buffering: `Snapshot` (read-only view of tick-start state) and
  `WriteBuffer` (accumulates effects; starts as a copy-on-write or eager copy
  of the snapshot — implementer's choice, but reads NEVER see same-tick
  writes). `commit()` swaps buffers.
- Deterministic layout: tables ordered by IR declaration order; rows by
  index; no hash-map iteration anywhere near data or hashing
  (`docs/prds/README.md` conventions).
- Canonical state hash: SHA-256 over a defined byte serialization — table
  name bytes, then each column in attr declaration order with an explicit
  little-endian encoding per type. Document the exact layout in rustdoc; it
  is frozen by a golden test.
- Population initialization API: build a store from `(Model, per-table sizes,
  per-column initial data)`. Validation: Ref columns must be in-bounds; enum
  indices in-range.

## Non-goals

Transition execution, aggregates/joins (PRD 0005), persistence to disk,
growable tables.

## Acceptance criteria

1. `cargo test --workspace` passes.
2. Instantiating a store from `examples/two_state.json` with 100 rows
   succeeds; out-of-bounds Ref or enum initial data is rejected with an error
   naming table/column/row.
3. Double-buffering test: write attr values through `WriteBuffer`, assert the
   `Snapshot` still reads old values, `commit()`, assert new values visible.
4. Golden hash test: a store built from `two_state.json` with fixed,
   hardcoded initial data has a hardcoded expected SHA-256; the test fails if
   the canonical serialization ever changes silently.
5. Hash sensitivity test: changing any single cell changes the hash;
   rebuilding identical state twice yields identical hashes.
6. Rustdoc on the hash function documents the byte layout exactly.
