# Host evaluator performance PRDs

Ordered PRD set addressing host-side execution cost, which the 2026-07-26
profiling established is what actually limits this system. Run from the Sembla
repository with:

```text
/piprd run docs/prds-host-evaluator-performance
```

`README.md` is ignored by `/piprd run`. Every numbered PRD must read this file
first; the constraints below are binding. When a PRD conflicts with this README,
this README wins.

## Why this folder exists

`prds-cuda-validation-parallelism` fixed a quadratic in the CUDA conflict
resolver — a 33.0× improvement with byte-identical results — and the §L4 gate
still missed at 2.56×. Profiling then showed why
(`docs/evidence/cuda-l4-20260726/`): at 5M rows over 2 ticks, **all GPU kernels
together account for 9.1 ms of 10,620 ms of wall time — 0.09%**. The GPU is not
the constraint and further kernel work cannot pay.

A host profile (`docs/evidence/host-profile-20260726/`) shows where the time
goes. 93% of samples are in tick execution, and everything routes through
`sembla_runtime::eval::eval_expr`, a recursive tree-walking interpreter. Two
patterns recur, both visible in source:

**Column references are resolved by string comparison per row.**
`eval.rs:1196` and its siblings do
`.map(|row| snapshot.real(table.box_name(), table.table_name(), name, row))`.
Each call runs `find_table` (string scan over boxes, then tables) and then
`columns.iter().find(|c| c.name() == column_name)` before indexing a contiguous
`Vec`. At 1M rows that is a million redundant name lookups per column read.

**Every expression node allocates a full-length vector.** `ValueColumn` is
`Real(Vec<f64>)` / `Int(Vec<i64>)` / …, so `a + b * c` at 1M rows allocates
three million-element vectors per tick.

`observe_views` is the largest single branch at 33% of samples and runs on the
host in **both** backends, which bounds how far ahead of CPU the GPU can get and
is consistent with the measured 2.56× ceiling.

## Binding constraints

- **Results must not change.** This is the CPU oracle — DESIGN.md §8 makes it
  ground truth for the CUDA differential harness and DECISIONS.md §E2's
  determinism levels are defined against it. Every golden under `examples/**`,
  every CSV and hash golden, and the tracked CUDA differential evidence must be
  **byte-identical**. A moved golden is a failed PRD, not a new baseline.
- **No IR, Lean, or semantic change.** These PRDs change how the evaluator
  reaches values, never what it computes, accepts, or reports.
- **Error behaviour is observable.** Diagnostics are part of the contract. A
  refactor that changes *when* an error is raised — including on empty tables,
  where a per-row loop currently never executes — is a semantic change and must
  be handled deliberately rather than discovered.

## Profile before scoping (binding, inherited)

The CUDA folder learned this expensively: two PRDs there were scoped from source
structure and both picked the wrong target. **No PRD in this folder may be
scoped without a profile**, and the profile must be re-taken after each landed
change, because removing one cost re-ranks everything behind it.

Host profiling is free and local — `sample` on macOS, `perf` on Linux — so this
costs minutes, not rented GPU hours.

## PRDs

- `0001-resolve-column-references-once` — hoist column resolution out of the row
  loop. Small, bit-identical by construction, and targets a pattern the profile
  shows is hot throughout.

Later PRDs are **deliberately not written yet.** The candidates are buffer reuse
or evaluator fusion for the per-node allocation, and whatever remains in
`observe_views`. Both are re-scoped from a fresh profile after 0001 lands,
because some of their apparent cost may be 0001's.

## Measurement protocol

Fixed case for before/after comparison, on one machine in one session:

```text
model:  fixtures/demographic/benchmark/demographic_slots.no-grouped.json
scale:  1,000,000 slots      ticks: 24      seed: 9009
areas:  4    present fraction: 0.8    streams: birth:600,overseas:250,internal:150
backend: cpu
```

Three runs, median reported, with the `sample` profile committed alongside.
Baseline on Apple M2 Pro: **49.5 s wall, 46.8 s user** (single run,
`docs/evidence/host-profile-20260726/`).

## Status notes

### 2026-07-26 — PRD 0001 local implementation

`Snapshot::resolve_column` now resolves table and column names once, and the
host evaluator matches Real, Int, and Enum column types before indexing their
contiguous values by row. The existing per-row accessors remain available for
other callers.

The implementation selected §3's default **preserve** route. Every affected
path returns its existing empty result before resolving a column when
`row_count == 0`; missing-column and wrong-type errors therefore remain lazy on
empty tables, while non-empty tables retain the existing error types and exact
messages. No `DECISIONS.md` change is needed because evaluator semantics did not
change.

The fixed one-million-slot case measured 49.66 s median wall before and 15.62 s
after over three runs each on the same Apple M2 Pro in one session. All outputs
were byte-identical. Raw measurements, binary and input hashes, and the
post-change `sample` profile are under
`docs/evidence/host-evaluator-resolve-once-20260726/`.
