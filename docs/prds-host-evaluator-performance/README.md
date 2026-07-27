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

**Closed 2026-07-27.** All four PRDs landed; the fixed case went from 49.5 s to
~7.6 s and the §L4 gate passed at 4.207× (`DECISIONS.md` §L8). Follow-on work is
in `docs/prds-evaluator-throughput/`, and the durable findings — including this
folder's measurement protocol and the lessons behind it — are consolidated in
[`docs/performance-model.md`](../performance-model.md).

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
- `0003-compute-per-tick-hashes-only-when-consumed` — per-tick `state_hash` was
  31.4% of the full-duration profile and, outside the CPU-vs-CUDA differential,
  nothing read the result. **Landed 2026-07-26: 1.35× user time, outputs and
  manifests byte-identical**
  (`docs/evidence/host-evaluator-hash-on-demand-20260726/`). Wall-clock median
  went backwards under machine contention; the uncontended run measured
  8.17 s wall / 6.19 s user. This session prompted the revised reporting rules
  below.
- `0004-stop-copying-owned-real-columns` — `numeric_as_real` previously cloned
  a million-element buffer to read an owned `Real` column, at three call sites
  that each convert both operands. **Implemented 2026-07-26: outputs and
  manifests remained byte-identical; the user-time headline was a permitted
  negligible 0.51% regression**
  (`docs/evidence/host-evaluator-owned-real-move-20260726/`).

Later PRDs remain **deliberately unwritten.** After 0003 the profile has no
dominant entry: allocation-related symbols aggregate to roughly a third,
real compute (`execute_tick`, `log`, `draw_u32x4`) to another third and sets a
floor, and the rest is spread thin. 0004 removes the one provably redundant
copy inside the allocation traffic. The two larger candidates — buffer pooling
and a scalar broadcast representation for literals — are named in 0004's
non-goals and get scoped from the profile 0004 produces, because removing the
copy changes what pooling would be sized for.

## Measurement protocol

Fixed case for before/after comparison, on one machine in one session:

```text
model:  fixtures/demographic/benchmark/demographic_slots.no-grouped.json
scale:  1,000,000 slots      ticks: 24      seed: 9009
areas:  4    present fraction: 0.8    streams: birth:600,overseas:250,internal:150
backend: cpu
```

### Reporting rules (binding, revised 2026-07-26 after PRDs 0001–0003)

Three sessions in a row were contended, twice badly enough to distort the
headline: 0002's median wall was inflated by a run at 23.92 s wall against
14.20 s user, overstating a real ~1.4× as 1.62×; 0003's median wall went
*backwards* while user time improved 1.35×. The noise floor is now comparable
to the effect sizes being measured, so the protocol is tightened:

- **Five runs each side, not three.** Record every run individually.
- **Report the minimum, not the median.** For a deterministic single-threaded
  workload on a quiet machine, the fastest run is the one least polluted by
  scheduling; the median rewards nothing and absorbs outliers badly at n=3.
  Report the median too, but the minimum is the headline.
- **Serial work: user time is primary**, wall time secondary. Quote both; where
  they disagree, user time decides and the discrepancy is explained.
- **Parallel work: wall time is primary** — see the note below. Quote both,
  plus the CPU-efficiency ratio, and report single-worker figures separately.
- **Flag contention per run (binding).** Compute `wall − (user + sys)`; any run
  exceeding **0.5 s** is marked `contended: true` in `measurements.json`, as
  0003's evidence already does. **This test is only valid for single-threaded
  runs.** With N workers, `user` can legitimately approach `N × wall`, so the
  quantity goes large and negative and means nothing. For parallel runs compare
  wall against the fastest observed wall instead, and say which test was used.
- **Report the fastest uncontended run on each side (binding).** A comparison
  is reportable once **at least one uncontended run exists on each side**. If
  five runs on a side are all contended, run more; if they stay contended,
  record that fact and report user time only, saying so explicitly.
- **Prefer a quiet machine (advisory, not a gate).** Measure when little else
  is running. This is guidance the operator can act on, **not an acceptance
  criterion** — see the note below.
- Continue recording binary and input SHA-256 digests, and commit a
  full-duration `sample` profile of the post-change build.

### Why parallel work is judged on wall time (added 2026-07-27)

The "user time decides" rule was written when everything here was
single-threaded, where user ≈ wall and user time is the less noisy of the two.
**Under parallelism it inverts, and applying it unchanged would reject every
parallel change on principle.**

Total CPU consumed necessarily *rises* with worker count — spawn and join,
per-worker setup, cache and memory-controller contention. Wall time is the thing
that improves and the thing a user experiences. `prds-evaluator-throughput/0001`
measured wall 7.68 s → 5.47 s (1.40×) while user time went 6.00 s → 6.66 s
(+11%). By the old rule that is a regression; it is plainly an improvement.

So: **wall time is primary whenever worker count exceeds one.** Three guards,
because wall time alone is easy to abuse:

- **Report single-worker figures separately.** A change that is only fast
  because it uses ten cores is a different thing from one that is fast, and the
  separation is what later PRDs are scoped from.
- **Report the CPU-efficiency ratio** (`user_after / user_before`). Burning ten
  cores for 5% of wall should be visible as a bad trade even when the headline
  improves.
- **A wall-time win with a large user-time loss needs justifying**, not just
  reporting. On a shared or batch host, aggregate CPU is a real cost.

`prds-evaluator-throughput/0002` is the mirror image and shows why both metrics
must be quoted: user time fell 10.7% while wall time stayed flat, because after
0001 the critical path is the *untiled serial remainder*. Neither metric alone
would have told that story.

### Why quiescence is advisory (added 2026-07-26 after PRD 0004 stalled)

The first version of these rules made quiescence binding: *"do not measure
while the PRD runner or another agent session is active on the same host."*
That is **unsatisfiable by construction** — the runner cannot stand itself down
to satisfy an acceptance criterion in a PRD it is executing. PRD 0004's
implementation was complete and correct on attempt 1 and still exhausted all
five attempts, because each review found an evidence gate that no in-run action
could ever close.

**No PRD in this folder may make its acceptance depend on the absence of the
process evaluating it.** Noise is handled statistically — five runs, per-run
contention flags, fastest uncontended run — not by demanding conditions the
runner cannot create.

The contention also turned out not to be the runner. With the runner stopped,
load average was 9.84, dominated by `mds_stores` at 166% CPU: Spotlight was
indexing a 3.1 GB `target/` directory that had no `.metadata_never_index`
marker, so every build fed it gigabytes of fresh object files. The marker has
since been added. That is the likeliest cause of the contended runs in 0002,
0003, and 0004.

### Reference points

- Original baseline, Apple M2 Pro: **49.5 s wall, 46.8 s user** (single run,
  `docs/evidence/host-profile-20260726/`).
- After 0001–0003, best uncontended run: **8.17 s wall, 6.19 s user**
  (`docs/evidence/host-evaluator-hash-on-demand-20260726/`). Cumulative ≈ 6×
  wall, 7.5× user, with every CSV and manifest digest unchanged throughout.

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
