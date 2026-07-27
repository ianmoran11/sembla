# PRD 0006: Evaluate effect values at the winner rows, not at every row

## Context

Read `docs/prds-evaluator-throughput/README.md` first; its constraints bind.

`stage_box` evaluates each effect's value as a **full column over every row of
the table** (`executor.rs:1998`), then uses `values[candidate.row]` for the
winners only (`executor.rs:2008`):

```rust
_ => PendingColumn::Value(eval_column(
    value, table.with_expected_attr(attr)?, snapshot, params, &mut cache,
)?),
…
for candidate_index in winner_indices {
    for (attr_index, values) in &effect_columns { … }
}
```

`docs/evidence/host-profile-20260727/` measures this at **365 of 3,421**
main-thread samples, about 11% of the tick, on the serial path.

Fire rates in the benchmark model are 0.1%–2.5% for the stochastic transitions.
So for those, the evaluator computes a million values to use a few thousand.

### Why this is safe to change

All 32 effect value expressions in
`fixtures/demographic/benchmark/demographic_slots.no-grouped.json` are
**row-local**: their node kinds are `self_attr` (13), `enum` (17), `int` (6) and
`add` (4). No aggregates, no input ports.

A row-local expression's value at row *r* depends only on row *r*'s inputs.
Evaluating it at a subset of rows therefore yields **identical values** for
those rows — the same structural argument PRDs 0001–0004 relied on, not an
empirical one.

### The one transition this does not help

`age_monthly` fires for every enabled candidate — 772,794 of roughly 800,000
writes per tick. For it, the full column is nearly all used and gathering saves
almost nothing. The win is concentrated in the other transitions, where the
used fraction is around 2%.

**Do not assume the saving scales with the sample count.** Measure it.

## Goal

Effect values are computed only for rows whose writes are staged. Nothing about
what is computed, accepted, or reported changes.

## Specification

### 1. Add a gather evaluation entry point

Add an evaluator entry point that evaluates an expression at an explicit,
ascending list of row indices and returns values in that order.

It must produce, for each requested row, **the value the full-column path would
have produced for that row**. Prefer sharing the node-level implementation with
the existing path over writing a second evaluator: two implementations of the
same semantics is how the oracle acquires a silent divergence.

### 2. Row-local expressions only, decided structurally

Use the gather path only where the expression is row-local. Determine that from
the IR — a predicate over `Expr` — not from the benchmark model's shape.

Aggregates (`Expr::Agg`) and input ports (`Expr::Input`) depend on rows other
than the one being evaluated, so they **must** fall back to full-column
evaluation. Getting this wrong changes results.

The predicate must be conservative: anything it does not recognise falls back.

### 3. Use it for effect values

At `executor.rs:1998`, gather at the winner rows instead of evaluating the
column, where §2 permits. The winner rows are already known — `resolve_claims`
runs first — and are already ascending, or can be made so at no cost.

The `Ref` path at `executor.rs:1995` deserves the same treatment if it is
row-local; if `eval_typed_ref_column` cannot support gathering cleanly, leave it
and say so.

### 4. Preserve error behaviour, including *when* errors are raised

This is the subtle risk. A full-column evaluation raises an error for a row that
fails, **whether or not that row's write is staged**. A gather at winner rows
would not see that row at all.

So an expression that overflows at some non-winner row currently fails the tick
and would silently succeed after this change. That is an observable semantic
change.

Choose deliberately and record the choice:

- **Preserve** (default): keep full-column evaluation wherever the expression
  can fail per row — integer arithmetic that can overflow, `Ref` bounds, enum
  range — and gather only where it is infallible. This narrows the win but is
  bit-identical, including diagnostics.
- **Change**: gather regardless, accepting that errors on non-staged rows no
  longer fire. This needs a `DECISIONS.md` entry and updated negative tests, and
  it is a semantic change, not an optimisation.

Note that PRD 0003's implementation notes already use the phrase "row-infallible"
for the tiled candidate path, so the codebase has a precedent for this
distinction. Reuse it rather than inventing a second notion.

### 5. Measure under the revised protocol

Wall time primary, single-worker figures separate, CPU efficiency reported, and
the serial fraction before and after.

Additionally report, per transition, **how many effect values were evaluated
versus used** — in the form PRD 0002's evidence used for `ln` counts. That table
is the evidence the change did what it claims.

## Allowed files

- `crates/sembla-runtime/src/eval.rs`
- `crates/sembla-runtime/src/executor.rs`
- `crates/sembla-runtime/tests/**`, `crates/sembla-cli/tests/**` (tests only)
- `docs/evidence/**` (new evidence only)
- `docs/prds-evaluator-throughput/README.md` (status notes only)
- `DECISIONS.md` — **only** if §4 selects the changed-semantics route

**If a required gate fails on files outside this list, stop and report it** —
`DECISIONS.md` §M2.

## Non-goals

No write-path identity work — that is PRD 0005, and combining them would make
the two measurements inseparable. No change to conflict resolution or which
candidates win. No tiling. No RNG change. No IR, Lean, CUDA, or CLI changes. No
new dependencies.

## Acceptance criteria

1. A gather entry point exists and is used for effect values where §2 permits.
2. A test asserts gathered values are bitwise identical to the full-column values
   at the same rows, across `Real`, `Int`, `Enum` and `Ref` effects.
3. A test asserts an aggregate-bearing or input-bearing effect expression falls
   back to full-column evaluation.
4. §4's choice is recorded, with a test covering an error at a **non-winner**
   row demonstrating the chosen behaviour.
5. **Every golden is byte-identical**, including the manifest and
   `final_state_sha256`.
6. `cargo test --locked` and `scripts/check-rust.sh` green, with unchanged
   negative-suite expectations unless §4 selected the changed route.
7. Before/after under the revised protocol, with the serial fraction and the
   per-transition evaluated-versus-used table.
8. `python3 scripts/check-markdown-links.py` passes.

## Note on expectations

365 samples is the ceiling and the realistic gain is well below it, for two
reasons stated above: `age_monthly` uses nearly its whole column, and §4's
default route keeps full-column evaluation wherever an expression can fail per
row — which includes the `add` nodes, since integer arithmetic can overflow.

It is entirely possible this lands at a few percent. That would still be worth
having, and the evaluated-versus-used table would explain exactly why.

If §4's preserve route leaves almost nothing gatherable, **say so and stop**
rather than reaching for the changed-semantics route to rescue the number. The
finding that effect expressions are mostly fallible is more useful than a small
speedup bought with a semantic change.
