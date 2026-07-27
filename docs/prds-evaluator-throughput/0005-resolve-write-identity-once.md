# PRD 0005: Resolve write identity once per transition, not once per write

## Context

Read `docs/prds-evaluator-throughput/README.md` first; its constraints bind,
including the revised measurement protocol — wall time primary for parallel
work, single-worker figures and CPU efficiency reported separately.

`docs/evidence/host-profile-20260727/` re-profiled the binding case after PRDs
0001–0004. The tick is ~69% serial, and the serial remainder is the write path.
It divides into four costs, none dominant, of which **three share one cause**:

| | samples of 3,421 | |
|---|---:|---|
| `stage_box` allocation and `String::clone` | ~660 | this PRD |
| `detect_double_writes` → `merge_sort` | 261 | this PRD |
| write application → `locate_writable_cell` → `memcmp` | ~244 | this PRD |
| effect evaluation over full columns | 365 | PRD 0006 |

**The cause: the write path carries and re-resolves identity per write, when
identity is per transition and per effect.** All three follow from that.

This work is on the **serial main thread**, so unlike PRDs 0002 and 0004 — which
reduced user time while wall time stayed flat — removing it should move wall
time directly.

## Specification

### 1. Drop the owned `transition_name` from `PendingWrite`

```rust
struct PendingWrite {
    box_index, table_index, attr_index, row,
    value: PendingValue,
    rule_id: u32,
    transition_name: String,   // remove
}
```

It is cloned once per write at `executor.rs:2018`. The benchmark makes roughly
**800,000 writes per tick** — `age_monthly` alone accounts for 772,794 — so a
24-tick run performs about **19 million String allocations**.

Its only consumers are `executor.rs:2421` and `2423`, in the `DoubleWrite`
error. That error already carries `first_rule_id` and `second_rule_id`, and the
three sibling fields beside it — `box_name`, `table`, `attr` — are already
looked up from the model by index at the point of failure. Derive the transition
name the same way.

The field costs 19 million clones to serve an error that fires at most once and
then terminates the tick.

Removing it also shrinks the record, which reduces the `Vec` growth, `memmove`
and drop traffic that surround it.

### 2. Replace the double-write sort with a linear check

`detect_double_writes` sorts all ~800,000 write indices per tick by
`(box, table, attr, row)`, then inspects adjacent pairs. Each comparison
indirects into `pending`, so it is cache-hostile as well as O(n log n).

A per-cell first-writer map or a per-column bitmap gives the same guarantee in
one pass.

**The diagnostic must not change, and this is the criterion §2 turns on.**
`sort_by_key` is stable, so among writes to the same cell the reported pair is
the two *earliest in push order*. Any replacement must report the identical pair
for the identical input — not merely *a* colliding pair. With three or more
writes to one cell the difference is observable.

Add a test with three writes to a single cell asserting the reported pair is
unchanged.

### 3. Resolve the destination column once per effect

`WriteBuffer::set_int` and its siblings call `locate_writable_cell`, which scans
box names, then table names, then column names, by string comparison — per
write.

This is precisely the pattern PRDs 0001 and 0002 of
`prds-host-evaluator-performance` removed from the **read** path, for a combined
~4×. The destination is already known per `(transition, effect)` at staging
time; resolve it there and carry the resolved handle.

Prefer extending the existing resolve-once machinery in `state.rs` over adding a
parallel mechanism.

### 4. Preserve error behaviour exactly

Every diagnostic keeps its type, message text and the order in which it is
raised. That includes:

- `DoubleWrite`, per §2, including which pair is named;
- out-of-range `Ref` and enum-variant checks on write;
- row-bounds errors.

A faster write path that validates less would pass every golden, because the
goldens contain valid states, and fail later in the oracle's own state where
nothing can catch it. Where §3 resolves a column early, the validation that
`locate_writable_cell` performed must still happen, with the same messages.

### 5. Measure under the revised protocol

Wall time primary, single-worker figures separate, CPU efficiency reported.

Report the **estimated serial fraction** before and after by the same Amdahl
inversion PRD 0003 used, so the series is comparable. That is the number this
PRD exists to move; a wall-time gain with an unchanged serial fraction means
something else caused it.

## Allowed files

- `crates/sembla-runtime/src/executor.rs`
- `crates/sembla-runtime/src/state.rs`
- `crates/sembla-runtime/tests/**`, `crates/sembla-cli/tests/**` (tests only)
- `docs/evidence/**` (new evidence only)
- `docs/prds-evaluator-throughput/README.md` (status notes only)

**If a required gate fails on files outside this list, stop and report it** —
`DECISIONS.md` §M2.

## Non-goals

No gather evaluation of effect values — that is PRD 0006 and the two must not be
combined, because their measurements would then be inseparable. No change to
conflict resolution or §E3's ordering: the winners and their order are inputs to
this PRD, not subjects of it. No change to which writes happen. No tiling of the
write path — PRD 0003 declined that for a recorded reason that still holds. No
RNG change. No IR, Lean, CUDA, or CLI changes. No new dependencies.

## Acceptance criteria

1. `PendingWrite` carries no owned `String`; the `DoubleWrite` error still names
   the transition, derived from `rule_id`.
2. `detect_double_writes` performs no sort; a test with three writes to one cell
   asserts the reported pair is unchanged.
3. The destination column is resolved once per `(transition, effect)`, not per
   write; a test or grep-based assertion prevents regression.
4. All write-path validation is preserved with identical messages — covering at
   minimum an out-of-range `Ref`, an out-of-range enum variant, and a row-bounds
   failure.
5. **Every golden is byte-identical**, including the manifest and
   `final_state_sha256`.
6. `cargo test --locked` and `scripts/check-rust.sh` green.
7. Before/after under the revised protocol, with the serial fraction before and
   after.
8. `python3 scripts/check-markdown-links.py` passes.

## Note on expectations

The three costs total roughly 1,165 of 3,421 main-thread samples, about 34% of
the tick. Do not expect 34%: `stage_box`'s allocation share includes work this
PRD does not remove, and the profile attributes worker samples separately.

The distinguishing feature of this PRD is that its work is **serial**. PRDs 0002
and 0004 each removed real work and moved wall time by under 1%, because they
acted on the parallel side. If this one also leaves wall time flat, that is a
significant finding — it would mean the serial fraction is dominated by
something the profile has not yet named, and that is worth more than the
speedup.
