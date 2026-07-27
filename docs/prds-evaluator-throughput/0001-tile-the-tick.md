# PRD 0001: Evaluate a tick in row tiles, and parallelise over them

## Context

Read `docs/prds-evaluator-throughput/README.md` first; its constraints bind,
especially **Determinism under parallelism**.

**This PRD replaces an earlier `0001` that scoped threading on its own.** That
attempt was implemented and measured, and the measurement is why this one exists
(`docs/evidence/evaluator-parallel-element-wise-20260727/`). Read that evidence
before starting: it is the reason for this PRD's shape.

### What the previous attempt established

Parallelism pays richly in isolation, measured on a 10-core M2 Pro:

| rows | guard, 1→10 workers | racing clock, 1→10 workers |
|---:|---:|---:|
| 262,144 | 2.4× | 5.3× |
| 1,048,576 | **4.5×** | **5.6×** |

But applied **per expression node** inside the real evaluator it does not pay.
A tick evaluates roughly 127 nodes, so per-node parallel regions mean ~127
spawn/join cycles per tick. The implementer measured that at 1M and 2M rows the
regions "cost more than they save", set the threshold at 5M so nothing
regressed, and the binding case stayed serial at **1.00×**.

That is a correct result, not a failed one. The isolated speedup is real; the
granularity was wrong.

### Why tiling fixes it

The current evaluator walks the `age` column roughly forty times per tick —
twice for each of eighteen view bands, plus once per guard — because each
operation is its own pass. `rowwise_spike` measured the alternative at 5M rows
on a representative tick shape:

| shape | ms | speedup |
|---|---:|---:|
| column-wise (current) | 126.60 | 1.0× |
| row-wise, 1 thread | 58.13 | 2.2× |
| row-wise, threaded | 10.42 | 12.2× |

**Tiling and parallelism are the same PRD because they only work together.**
Tiling gives one natural parallel region per tick over row ranges instead of 127
over columns, which is the granularity the previous attempt lacked. And tiling
alone is worth 2.2× before any threading.

The sizing works: with ~1024 rows per tile, all intermediates for a whole tick —
18 band counters plus 5 guards — come to roughly 23 KB and stay in L1, so
dispatch is amortised over the tile and intermediates never reach memory.

## Goal

A tick evaluates in row tiles, with intermediates confined to cache, and tiles
distributed across cores. Results are unchanged, bit for bit, for any tile size
and any worker count.

## Specification

### 1. Tile the tick, not the expression

Restructure evaluation so a tile of rows is carried through the tick's
expressions before moving to the next tile, rather than each expression being
carried through all rows.

This is the structural change and it is large. If a full restructuring is not
achievable, **narrow the scope rather than the rigour**: tile the view-filter
and guard evaluation first, leave aggregates and the write path column-wise, and
say so. A partial tiling that is correct and measured is a better outcome than a
complete one that is rushed.

### 2. Tile size is a tuned constant, not a guess

Choose it by measurement and record the sweep. The target is that a tick's live
intermediates fit in L1 — state the arithmetic for the benchmark model, as the
README does.

Tile boundaries must derive from row index and tile size alone. Changing the
tile size must not change any result; a test must assert that across at least
three sizes.

### 3. Parallelise over tiles

One parallel region per tick, distributing tiles across workers.

**Work partitioning must derive from row index alone** — never from worker
count, scheduling, or which worker finishes first. This is the criterion the
PRD turns on, and it must be structurally impossible to violate, not merely
correct today. A reviewer should see that no result can differ across worker
counts without running anything.

Keep a worker-count override for testing, as the previous attempt did with
`SEMBLA_EVAL_THREADS`.

### 4. What stays sequential

- **`f64` reductions.** `eval.rs` fixes ascending row order as the canonical
  Level A reduction order. A per-tile or per-worker partial sum changes the
  result. This is the easiest way to fail this PRD.
- **Conflict resolution** and the sort it depends on.

Integer counts, `min` and `max` are associative and *may* combine per-tile
partials. Where that is used, say so explicitly and justify it.

### 5. Threshold small work

Below some row count, tiling and spawning cost more than they save — that is the
previous attempt's central finding. Keep a threshold, measure it, and record the
measurement.

But note the previous threshold was 5M *for per-node regions*. Per-tick regions
should pay far lower, and if the measured threshold still lands above the
binding 1M case, that is a signal the tiling is not delivering its 2.2× and
should be investigated before the PRD is called done.

### 6. Determinism tests

Assert **identical output** across at least three worker counts including 1,
**and** across at least three tile sizes:

- a `Real` arithmetic chain, compared bitwise via `to_bits`, not `==`;
- a racing-clock evaluation, compared bitwise;
- an expression containing an `f64` reduction, proving it was not tiled or
  parallelised in a way that changed accumulation order.

A test that only checks output against an expected value is insufficient: it
would pass while being nondeterministic.

### 7. Measure under the README protocol

Five runs each side, fastest uncontended run as the headline, user time primary,
contention flagged per run, in-run. **Report single-worker figures separately**,
so the tiling gain and the parallel gain are distinguishable — that separation
is the main thing the next PRD will be scoped from.

Report tile size, worker count, `available_parallelism()`, and whether the
binding case cleared the threshold.

## Allowed files

- `crates/sembla-runtime/src/eval.rs`
- `crates/sembla-runtime/src/executor.rs`
- `crates/sembla-runtime/tests/**`, `crates/sembla-cli/tests/**` (tests only)
- `docs/evidence/**` (new evidence only)
- `docs/prds-evaluator-throughput/README.md` (status notes only)

**If the quality gate fails on files outside this list, stop and report it.** Do
not fix it and do not treat it as in scope: it means the baseline is broken, and
that is the operator's to resolve. The previous attempt lost five attempts to
exactly this.

## Non-goals

No scalar broadcast — separate, and not subsumed by tiling. No buffer reuse —
later, after tiling changes what would be pooled. No RNG change. No histogram
recognition as a *separate* mechanism: eighteen band counters updated per row
within a tile is the histogram, and it should fall out rather than be built. No
IR, Lean, CUDA, or CLI changes. No new dependencies.

## Acceptance criteria

1. A tick evaluates in row tiles with tile size a recorded, measured constant.
2. Tiles are distributed across workers in one region per tick, with
   partitioning derived from row index alone.
3. Determinism tests per §6 pass, bitwise, across three worker counts and three
   tile sizes.
4. A test or assertion proves no `f64` reduction was tiled or parallelised in a
   way that changes accumulation order.
5. **Every golden is byte-identical**: `examples/**`, all CSV and hash goldens,
   the frozen demographic state fixture, the run manifest including
   `final_state_sha256`, and the tracked CUDA differential evidence.
6. `cargo test --locked` and `scripts/check-rust.sh` green.
7. Before/after under the README protocol, with single-worker figures reported
   separately, plus tile size, worker count and threshold behaviour.
8. `python3 scripts/check-markdown-links.py` passes.

## Note on expectations

`rowwise_spike` measured 2.2× for tiling alone and 12.2× with threading, but
that arm is **hand-written, i.e. compiled**. A tiled *interpreter* still pays
node dispatch, amortised over the tile rather than eliminated. Expect
meaningfully less than 12.2×.

A useful outcome is tiling clearly positive on its own — the 2.2× is the part
that does not depend on core count — with parallelism adding a further multiple
on top. If tiling alone is near 1×, the tiles are probably too large for L1 or
the dispatch is not being amortised, and that is worth diagnosing before adding
workers on top of it.

**Report the two separately even if the combined number is good.** The previous
attempt's whole lesson is that an isolated speedup can fail to survive contact
with the real granularity, and only a separated measurement shows which half
is working.
