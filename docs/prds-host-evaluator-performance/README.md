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
  shows is hot throughout. **Landed 2026-07-26: 3.18× wall on the fixed case,
  output byte-identical** (`docs/evidence/host-evaluator-resolve-once-20260726/`).
- `0002-resolve-reference-columns-once` — the same transformation for the `Ref`
  attribute type, which 0001 left out. **Landed 2026-07-26: ~1.4× on the fixed
  case, output byte-identical**
  (`docs/evidence/host-evaluator-reference-resolve-once-20260726/`). The
  evidence reports 1.62× wall, but one baseline run was contended (23.92 s wall
  against 14.20 s user) and inflated the median; user time, 13.05 s → 9.21 s,
  is the load-bearing figure. Cumulative with 0001: 49.7 s → 10.9 s.
- `0003-compute-per-tick-hashes-only-when-consumed` — per-tick `state_hash` is
  31.4% of the full-duration profile and, outside the CPU-vs-CUDA differential,
  nothing reads the result. Scoped from 0002's full-duration capture.

Later PRDs remain **deliberately unwritten.** 0002's capture is the first that
covers a whole process, so its shares are the first that can be trusted. Under
the hashing, the remainder is real compute (`log`, `draw_u32x4`) plus
allocation traffic spread thin across `from_iter`, `nanov2_free`, and
`madvise`, with the write path (`locate_writable_cell`) small but present. None
of those is an obvious next target until 0003 lands and the profile is retaken.

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

### 2026-07-26 — PRD 0002 local implementation

`ResolvedColumn::ref_values` now exposes the contiguous `Ref` values with the
same wrong-type diagnostic as `Snapshot::reference`. The `SelfAttr` Ref path
and all five aggregate reads resolve the column at most once before indexing
it by row; `Snapshot::reference` remains available to callers outside the host
evaluator.

The implementation selected §3's default **preserve** route. `SelfAttr` and
aggregate broadcast return or skip resolution when their row range is empty.
The three filtered aggregate paths resolve on the first selected row, so the
**filtered zero-selection case** remains lazy even when the target table has
rows but every filter result is `false`. A behavioural regression test covers
Count, integer Sum, and real Sum with an intentionally wrong-typed target FK;
all three still succeed when the filter selects no rows. Missing-column and
wrong-type errors therefore retain their prior timing, types, and messages, and
no `DECISIONS.md` change is needed.

The fixed one-million-slot case measured 17.64 s median wall before and 10.90 s
after over three runs each on the same Apple M2 Pro in one session, a 1.62×
improvement. All measured and profiled outputs were byte-identical. Raw
measurements, binary and input hashes, and the full-process-lifetime
post-change `sample` profile are under
`docs/evidence/host-evaluator-reference-resolve-once-20260726/`.

### 2026-07-26 — PRD 0003 local implementation

The host run path now reuses `sembla_cuda::HashMode`: plain runs, sweeps,
comparisons, and manifest verification request `FinalOnly`, while the
CPU-vs-CUDA differential explicitly requests `EveryTick` from both backends.
Per-tick hashes are represented as `Option<Vec<[u8; 32]>>`, so disabled hashing
cannot become an empty sequence that passes the differential vacuously. The
differential rejects absent sequences as an internal invariant violation,
checks lengths before elements, and retains first-divergent-tick diagnostics;
unit tests cover each case with deliberately divergent sequences.

The CLI CUDA tick path passes the requested mode into `CudaBackend` and uses the
same mode to gate hashing of its downloaded host-state mirror. These are not
currently two separate per-tick hashes: `run_tick_observed` downloads state but
does not invoke `CudaBackend`'s own hash path, so the host-mirror
`StateStore::state_hash` is the one per-tick hash performed by this CLI path.
The frozen §L4 gate was not rerun or amended; only its future protocol cost
profile has changed.

The fixed case measured 11.09 s median wall / 9.30 s median user before and
12.15 s wall / 6.88 s user after. After runs 1 and 2 were contended, with wall
time exceeding user plus system time by 3.62 s and 2.73 s, so user time is the
load-bearing comparison: **1.35× faster, a 26.02% reduction**. All primary
CSVs, summary CSVs, run manifests, and emitted final-state hashes were
byte-identical. The full-duration profile contains no per-tick `state_hash`
branch; its only `StateStore::state_hash` call is the retained final hash under
`execution_hashes`. Measurements and profile are under
`docs/evidence/host-evaluator-hash-on-demand-20260726/`.
