# PRD 0001: Resolve column references once per column, not once per row

## Context

Read `docs/prds-host-evaluator-performance/README.md` first; its constraints
bind.

`crates/sembla-runtime/src/eval.rs` reads a column like this (line 1196, with
siblings at 1201, 1206, and an enum case near 900):

```rust
.map(|row| snapshot.real(table.box_name(), table.table_name(), name, row))
```

`Snapshot::real` calls `find_cell(state, box_name, table_name, column_name, row)`
(`state.rs:900`), which performs `find_table` — a string scan over boxes then
tables — followed by `table.columns.iter().find(|c| c.name() == column_name)`,
another string scan, before finally evaluating `values[row]` on a contiguous
`Vec`.

The lookup is invariant across the loop. At 1M rows it is performed a million
times to read a million values that are already adjacent in memory. The
2026-07-26 host profile shows `find_cell` throughout the call tree with
`_platform_memcmp` as its child.

## Goal

Column resolution happens once per column read, not once per row. Nothing about
what is computed, accepted, or reported changes.

## Specification

### 1. Add a resolve-once accessor

Add a `Snapshot` method that resolves `(box_name, table_name, column_name)` to a
borrowed `&ColumnState` (plus whatever table context the existing error paths
need) **without** taking a row. Keep the existing per-row accessors: they have
other callers and are not the subject of this PRD.

### 2. Use it at the per-row call sites

At `eval.rs` 1196, 1201, 1206 and the enum case near 900: resolve before the
iteration, match the column type once, then index `values[row]` inside the map.
The type check currently performed per row moves outside the loop with the
resolution.

### 3. Preserve error behaviour exactly — including on empty tables

This is the one place the refactor can change semantics, and it must not.

Today a missing column or wrong column type is detected **inside** the row loop,
so a table with **zero rows never raises it**. Hoisting the resolution would
raise it eagerly. That is an observable change: a model that currently runs
would begin to fail.

Choose deliberately and record the choice in the PRD's implementation notes:

- **Preserve** (default): only resolve when the row range is non-empty, so
  zero-row tables behave exactly as now; or
- **Change**: eager resolution, justified, with the negative-suite expectations
  updated and a `DECISIONS.md` entry — this is a semantic change and needs one.

Error **messages and types** must be identical either way. If any message embeds
the row index, preserve that value.

### 4. Measure, locally

Run the README's fixed case three times before and after on the same machine in
one session, take the median, and capture a `sample` profile of the post-change
build. Commit both under `docs/evidence/`.

## Allowed files

- `crates/sembla-runtime/src/eval.rs`
- `crates/sembla-runtime/src/state.rs`
- `crates/sembla-runtime/tests/**`, `crates/sembla-cli/tests/**` (tests only)
- `docs/evidence/**` (new profile evidence only)
- `docs/prds-host-evaluator-performance/README.md` (status notes only)
- `DECISIONS.md` — **only** if §3 selects the eager-resolution route

## Non-goals

No change to `ValueColumn` or per-node allocation — that is a later PRD, scoped
after re-profiling. No evaluator fusion. No changes to `observe_views` beyond
what it inherits. No IR, Lean, CUDA, or CLI changes. No new dependencies.

## Acceptance criteria

1. No `snapshot.real/int/enum_index(...)` call remains inside a per-row `map` in
   `eval.rs`; a test or grep-based assertion covers this so it cannot regress.
2. **Every golden is byte-identical**: `examples/**`, all CSV and hash goldens,
   the frozen demographic state fixture, and the tracked CUDA differential
   evidence. `git diff --stat` shows none of them.
3. `cargo test --locked` and `scripts/check-rust.sh` green, including the
   negative suite with unchanged expectations — unless §3 selected the eager
   route, in which case the negative suite changes are explicit and justified.
4. §3's choice is recorded in the implementation notes with its reasoning.
5. Before/after medians over three runs each are committed with a post-change
   `sample` profile.
6. `python3 scripts/check-markdown-links.py` passes.

## Note on expectations

The profile shows this pattern is hot, but not how much of the total it
accounts for — `eval_expr` recurses, so tree counts cannot be summed cleanly.
This PRD is worth doing because it is small, safe, and removes work that is
provably redundant. **It is not a prediction of a specific speedup.** If the
median barely moves, that is a finding: it means the cost is in allocation or
elsewhere, and the next PRD is scoped from the new profile rather than from this
one.
