# PRD 0002: Resolve reference columns once, not once per row

## Context

Read `docs/prds-host-evaluator-performance/README.md` first; its constraints
bind. Read `0001-resolve-column-references-once.md` too — this PRD is the same
transformation applied to the one attribute type 0001 left out, and its §3
reasoning carries over with a twist.

PRD 0001 hoisted column resolution for `Real`, `Int`, and `Enum`, and measured
3.18× on the fixed case with byte-identical output
(`docs/evidence/host-evaluator-resolve-once-20260726/`). `Ref` was out of scope
and still reads per row (`eval.rs:1220`):

```rust
AttrType::Ref { .. } => (0..row_count)
    .map(|row| snapshot.reference(table.box_name(), table.table_name(), name, row))
```

`Snapshot::reference` (`state.rs:546`) calls `find_cell`, which is `find_table`'s
string scan over boxes and tables followed by
`columns.iter().find(|c| c.name() == column_name)` — the exact pattern 0001
removed everywhere else.

The post-change profile ranks it. In `post-change.sample.txt`, the branch
`execute_tick` → `eval_typed_ref_column` → `eval_expr` → `find_column` accounts
for **1650 of 7591 samples (~22%)**, and top-of-stack shows `_platform_memcmp`
at 760 with `find_column` at 606. Five further per-row `snapshot.reference`
calls sit in the aggregate paths at `eval.rs` 1456, 1473, 1525, 1553, and 1571.

This is the largest remaining evaluator cost that is provably redundant work.

## Goal

No reference column is resolved by name more than once per read. Nothing about
what is computed, accepted, or reported changes.

## Specification

### 1. Extend the resolve-once accessor to `Ref`

Add a `ref_values()` method to `ResolvedColumn` (`state.rs`) alongside
`real_values`, `int_values`, and `enum_values`, returning `&[u32]` and producing
the identical `wrong_column_type(table, column, "Ref")` error on mismatch.

Keep `Snapshot::reference`; it has callers outside `eval.rs` and is not the
subject of this PRD.

### 2. Convert the `SelfAttr` `Ref` arm

At `eval.rs:1220`, follow the shape 0001 established for the other three arms:
a zero-row early return, then resolve once, then index the slice inside the map.

### 3. Convert the five aggregate call sites — and mind the guard

`eval.rs` 1456 and 1473 read the FK inside `for row in 0..query_rows`. Lines
1525, 1553, and 1571 read it inside `if include`, under a filter.

Both are the empty-loop hazard from 0001 §3, and the filtered sites are the
sharper version: today, if the filter selects **no rows**, a missing or
wrongly-typed FK column is never detected. Hoisting resolution makes it eager,
and a model that runs today would begin to fail.

**Default to preserving**, matching 0001: resolve lazily enough that a loop
which executes zero iterations still raises nothing. A clean way is to resolve
on first use; a guard on the loop bound is only sufficient at 1456 and 1473,
because a non-empty loop can still have an all-`false` filter. Whichever route
is taken, state it in the implementation notes and show it covers the filtered
case specifically.

Deviating to eager resolution requires justification, updated negative-suite
expectations, and a `DECISIONS.md` entry — it is a semantic change.

Error messages and types must be identical either way, including the
`aggregate broadcast group {group} is out of bounds` and overflow diagnostics,
which must keep reporting the same group index.

### 4. Extend the regression guard

`crates/sembla-runtime/tests/eval_column_resolution.rs` currently forbids
`snapshot.real(`, `snapshot.int(`, and `snapshot.enum_index(` in `eval.rs`. Add
`snapshot.reference(`. The guard's `resolve_column` occurrence count must be
updated to match the new call sites.

The existing guard is source-text based and the 0001 review flagged that as
weaker than a behavioural fixture. If a behavioural test for the filtered
zero-selection case is practical, add one — it is the case §3 can silently
break, and no source-text assertion can catch a regression in it.

### 5. Measure, locally

Run the README's fixed case three times before and after on the same machine in
one session, take the median, and capture a `sample` profile of the post-change
build. Commit both under `docs/evidence/`.

Capture the profile for the **full duration of the run**, not a fixed 10-second
window. The 0001 evidence used a 10 s sample of a 15.6 s run while the baseline
used 10 s of a 49.5 s run, which makes the two share-of-total figures not
directly comparable. Full-duration captures make the next scoping sound.

## Allowed files

- `crates/sembla-runtime/src/eval.rs`
- `crates/sembla-runtime/src/state.rs`
- `crates/sembla-runtime/tests/**`, `crates/sembla-cli/tests/**` (tests only)
- `docs/evidence/**` (new profile evidence only)
- `docs/prds-host-evaluator-performance/README.md` (status notes only)
- `DECISIONS.md` — **only** if §3 selects the eager-resolution route

## Non-goals

No change to `ValueColumn` or per-node allocation. No evaluator fusion. No work
on the write path — `locate_writable_cell` shows in the profile at 84
top-of-stack samples and plausibly has the same shape, but it is a mutation
path with different aliasing constraints and belongs in its own PRD. Nothing
touching SHA-256 state hashing (see below). No IR, Lean, CUDA, or CLI changes.
No new dependencies.

## Acceptance criteria

1. No `snapshot.reference(...)` call remains in `eval.rs`; the extended guard
   test asserts it.
2. **Every golden is byte-identical**: `examples/**`, all CSV and hash goldens,
   the frozen demographic state fixture, and the tracked CUDA differential
   evidence. `git diff --stat` shows none of them.
3. `cargo test --locked` and `scripts/check-rust.sh` green, including the
   negative suite with unchanged expectations — unless §3 selected the eager
   route, in which case those changes are explicit and justified.
4. §3's choice is recorded in the implementation notes, with the filtered
   zero-selection case addressed by name.
5. Before/after medians over three runs each are committed, with a
   full-duration post-change `sample` profile.
6. `python3 scripts/check-markdown-links.py` passes.

## Note on expectations

The 22% branch figure is a share of a 10-second window, not of total runtime,
and `eval_expr` recurses so tree counts cannot be summed. A speedup smaller
than the share implies is entirely possible.

As with 0001, the case for this PRD does not rest on a predicted number: it is
small, bit-identical by construction, and deletes work that is redundant by
inspection. If the median barely moves, that is a finding — it means the cost
is in allocation, hashing, or elsewhere, and the next PRD is scoped from the
new full-duration profile.

## What this deliberately does not touch

`sha2::compress256` is the single largest top-of-stack entry in the post-change
profile at **1747 of 7591 samples (~23%)** — per-tick `state_hash` over 1M rows.
It is not in scope here and should not be picked up opportunistically.

Those hashes are the reproducibility contract: `DECISIONS.md` §E2 defines
determinism levels against them and §J14 uses them as GPU evidence. Any change
to what is hashed or how changes every recorded digest, which criterion 2
forbids by construction. If it is worth pursuing, the only safe angles are
avoiding hashes nobody consumes or making the same digest cheaper to compute —
and that needs its own PRD, its own argument, and its own §E2 review.
