# PRD 0004: Stop copying owned Real columns in numeric coercion

## Context

Read `docs/prds-host-evaluator-performance/README.md` first; its constraints
bind, including the **revised reporting rules** — five runs, minimum reported,
user time primary, quiesced machine.

PRDs 0001–0003 took the fixed case from 49.5 s / 46.8 s to a best uncontended
8.17 s / 6.19 s, roughly 6× wall and 7.5× user, with every CSV and manifest
digest unchanged. The profile that remains
(`docs/evidence/host-evaluator-hash-on-demand-20260726/post-change-full-duration.sample.txt`,
6471 main-execution samples) has no single dominant entry. Grouping the
allocation-related symbols — `from_iter_in_place` 519, the three `from_iter`
variants 611 combined, `nanov2_free` 259, `madvise` 418, malloc/free/`from_elem`
~340 — comes to roughly **2150 samples, about a third**. That grouping is a
judgement call, not one measured branch, and should be treated as indicative.

Underneath it is a floor of real compute: `execute_tick` 927, `log` 589,
`draw_u32x4` 530 — about 32% that no refactor removes.

Within the allocation traffic, one source is redundant by inspection.
`eval.rs:1414`:

```rust
fn numeric_as_real(column: &InternalColumn) -> Result<Vec<f64>, EvalError> {
    match column {
        InternalColumn::Real(values) => Ok(values.clone()),
```

Every caller owns the column it passes and drops it immediately afterwards, so
the `Real` arm allocates a fresh million-element buffer and memcpys 8 MB into
it for no reason. There are four call sites, and each converts **both**
operands:

| Site | Context | Result |
|---|---|---|
| `eval.rs:1256–1257` | `eval_arithmetic`, Real or Div path | Real |
| `eval.rs:1344–1346` | equality comparison | Bool |
| `eval.rs:1399–1400` | ordering comparison | Bool |

The arithmetic site partly recovers: because the clone is owned, Rust's
in-place collect specialisation reuses that buffer for the result — which is
why `from_iter_in_place` is the largest allocation symbol. The copy is still
pure waste. The two comparison sites recover nothing: they clone two `Vec<f64>`
and then collect into a fresh `Vec<bool>`, so both copies are discarded.

## Goal

No owned `Real` column is copied in order to be read as `Real`. Nothing about
what is computed, accepted, or reported changes.

## Specification

### 1. Take the column by value

Change `numeric_as_real` to consume its argument and **move** the `Vec<f64>`
out of the `Real` arm rather than cloning it. The `Int` arm still allocates —
`i64` and `f64` are different types and the conversion is a real computation —
and its `as f64` cast must be preserved exactly.

If any caller outside these four sites needs a borrowing form, keep one
alongside rather than making callers clone to satisfy the new signature. That
would reintroduce the copy this PRD removes.

### 2. Convert the four call sites

`eval.rs` 1256, 1257, 1344, 1346, 1399, 1400. Each already owns its operand
from `eval_expr`; pass ownership through.

Note the `Int`/`Int` fast paths at 1339 and 1386 must stay: they avoid the
coercion entirely and are already allocation-light.

### 3. Preserve evaluation semantics exactly

The module doc at `eval.rs:1` states that expressions evaluate in syntax-tree
order without reassociation, and that Real arithmetic uses ordinary IEEE-754
`f64` semantics — division by zero yields infinity or NaN rather than an error.

This change must not reorder operations, fuse them, or alter operand
evaluation order. `lhs` is evaluated before `rhs` today and must remain so.
NaN and infinity propagation, and the integer overflow diagnostics with their
embedded row indices, are unchanged.

Bit-identity here is structural rather than argued: the same values are read in
the same order and combined by the same operations. The only difference is
which buffer holds them.

### 4. Measure under the revised protocol

Five runs each side, minimum reported as the headline with median alongside,
user time primary, every run's `wall − (user + sys)` recorded and any run over
0.5 s marked contended. **Do not measure while the PRD runner or another agent
session is active on the same host** — that is the identified cause of the
contention in 0002 and 0003. Commit a full-duration `sample` profile of the
post-change build.

## Allowed files

- `crates/sembla-runtime/src/eval.rs`
- `crates/sembla-runtime/tests/**`, `crates/sembla-cli/tests/**` (tests only)
- `docs/evidence/**` (new profile evidence only)
- `docs/prds-host-evaluator-performance/README.md` (status notes only)

## Non-goals

**No buffer pooling or arena.** That is the obvious larger follow-up and it is
deliberately deferred to 0005, for a specific reason: pooling built around the
redundant clone would cement the waste into the pool's sizing and lifetime
assumptions. Remove the copy first, re-profile, then decide what pooling is
actually worth.

**No scalar broadcast.** `Expr::Real`, `Expr::Int`, and `Expr::Bool` each
allocate and fill a full-length vector for a literal (`eval.rs` 729–731,
759–762), and a scalar variant on `InternalColumn` would avoid it. That is a
representational change touching every match arm and every consumer, with a
much wider blast radius than this PRD. It is a 0005 candidate, not part of 0004.

**No change to `ValueColumn`**, whose four-variant public contract is
documented as frozen at `eval.rs:27`. No evaluator fusion. No IR, Lean, CUDA,
or CLI changes. No new dependencies.

## Acceptance criteria

1. `numeric_as_real`'s `Real` arm performs no copy; a test or grep-based
   assertion covers it so it cannot regress.
2. **Every golden is byte-identical**: `examples/**`, all CSV and hash goldens,
   the frozen demographic state fixture, the run manifest fields including
   `final_state_sha256`, and the tracked CUDA differential evidence.
   `git diff --stat` shows none of them.
3. `cargo test --locked` and `scripts/check-rust.sh` green, with unchanged
   negative-suite expectations — including the integer-overflow diagnostics and
   their row indices.
4. Five runs each side under the revised protocol, minimum and median both
   reported, user time named as load-bearing, contention flagged per run.
5. A full-duration post-change `sample` profile is committed.
6. `python3 scripts/check-markdown-links.py` passes.

## Note on expectations

This is the smallest PRD in the folder and may well be the smallest win. The
arithmetic site already recovers part of the copy through in-place collect, so
the clear gain is at the two comparison sites and in reduced page churn — the
`madvise` traffic at 418 samples is the allocator returning large buffers to
the OS and faulting them back, which is exactly what fewer large short-lived
allocations reduces.

Consistent with 0001–0003, the case does not rest on a predicted number: this
is a copy whose result is discarded, and not making it cannot change a result.
A small measured gain is a fine outcome. **A negligible one is also
informative** — it would mean the remaining allocation cost is intrinsic to
one-vector-per-node evaluation, which makes pooling or fusion the only
remaining lever and tells 0005 exactly what it is choosing between.
