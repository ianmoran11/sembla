# PRD 0001: Evaluate element-wise columns in parallel

## Context

Read `docs/prds-evaluator-throughput/README.md` first; its constraints bind,
especially **Determinism under parallelism**.

The CPU oracle is single-threaded. `eval_expr` walks an expression tree and each
node produces a full-length column on one core, on a machine with ten.

This is not an oversight the design forbids — `DESIGN.md` §4.2 defines the closed
kernel fragment as "operations whose parallel execution is order-free or has a
canonical order", and §E1 makes every draw a pure function of
`(seed, tick, rule_id, entity_id, draw_idx)`. The GPU backend exploits both. The
CPU oracle never has.

Measured by `crates/sembla-runtime/examples/threading_spike.rs` at 5M rows on
ten cores, with output asserted equal to serial — bitwise for the racing clock:

| workload | 1 thread | 2 | 4 | 8 | 10 |
|---|---:|---:|---:|---:|---:|
| guard (memory-bound) | 4.15 ms | 1.75 | 1.10 | 0.90 | **0.78** |
| racing clock (compute-bound) | 98.47 ms | 49.88 | 25.36 | 19.24 | **17.58** |

5.3× and 5.6×. The machine was running other work, and the Hyperstack host has
28 vCPUs, so this likely understates it.

## Goal

Element-wise column evaluation uses the available cores. Results are unchanged,
bit for bit, for any thread count.

## Specification

### 1. Parallelise element-wise operations only

In scope: the per-row `map` bodies that produce a column from one or more
columns of the same length — arithmetic, comparison, equality, ordering, logical
ops, `SelfAttr` reads, `EnumIs`, and the racing-clock loop in
`executor.rs` around line 901.

Out of scope, and explicitly left sequential:

- **`f64` reductions.** `eval.rs`'s module doc fixes ascending row order as the
  canonical Level A reduction order; a tree or per-thread reduction changes the
  sum. This is the single easiest way to fail this PRD.
- **Conflict resolution** (`resolve_claims`) and the sort it depends on.
- **Integer/bool reductions** are *permitted* to combine per-thread partials,
  because integer addition, `min` and `max` are associative — but only where the
  PRD implementing them says so. Not this one.

### 2. Partition by row index, never by thread count

**This is the criterion the PRD turns on.** Chunk boundaries must be a pure
function of the row count and a fixed chunk size. They must not depend on
`available_parallelism()`, on how many threads actually start, or on which
finishes first.

Element-wise work makes this easy — row `i`'s output depends only on row `i`'s
inputs — but the *code* must make it structurally impossible to get wrong, not
merely correct today. A reviewer should be able to see that no result can differ
across thread counts without running anything.

### 3. Threshold small work

A parallel region costs tens of microseconds to set up. The corpus and the test
fixtures are small; at a few thousand rows, spawning would be slower than not.

Introduce a row-count threshold below which evaluation stays on the calling
thread. Pick it by measurement, not by taste, and record the measurement. The
threshold changes only *how* the work runs and so cannot change results — state
that explicitly, because it is what makes an otherwise arbitrary constant safe.

### 4. Concurrency budget

Use `std::thread::scope`. No new dependency — the README forbids it and the
spike shows none is needed.

Do not spawn per node without bounding it: a tick evaluates ~127 nodes, and at
small row counts that is 127 spawn/join cycles. §3's threshold handles the
common case; if per-node spawning proves too costly at realistic sizes, say so
in the implementation notes rather than working around it. The tiling PRD later
in this folder is where one parallel region per tick becomes natural.

### 5. Determinism tests, not just correctness tests

Add tests that assert **identical output across at least three thread counts,
including 1**, on:

- a `Real` arithmetic chain, compared bitwise via `to_bits`, not `==`;
- a racing-clock evaluation, compared bitwise;
- an expression containing an `f64` reduction, proving the reduction did not
  get parallelised.

A test that only checks the parallel path against an expected value is
insufficient: it would pass while being nondeterministic.

### 6. Measure under the README protocol

Five runs each side, fastest uncontended run as the headline, user time primary,
contention flagged per run, in-run. **Additionally report single-thread wall and
user time**, so a loss of serial efficiency cannot hide behind core count.

Report the thread count used and `available_parallelism()` on the measuring
host.

## Allowed files

- `crates/sembla-runtime/src/eval.rs`
- `crates/sembla-runtime/src/executor.rs`
- `crates/sembla-runtime/tests/**`, `crates/sembla-cli/tests/**` (tests only)
- `docs/evidence/**` (new evidence only)
- `docs/prds-evaluator-throughput/README.md` (status notes only)

## Non-goals

No change to evaluation semantics, operand order, or which operations happen. No
tiling or fusion — that is a later PRD and this one must not pre-empt its shape.
No parallel reductions. No parallel conflict resolution. No RNG change. No IR,
Lean, CUDA, or CLI changes. No new dependencies.

## Acceptance criteria

1. Element-wise column evaluation runs on multiple threads above the §3
   threshold; a test demonstrates the threshold is honoured in both directions.
2. **Every golden is byte-identical**: `examples/**`, all CSV and hash goldens,
   the frozen demographic state fixture, the run manifest including
   `final_state_sha256`, and the tracked CUDA differential evidence.
   `git diff --stat` shows none of them.
3. Determinism tests per §5 pass, comparing `f64` results bitwise, across at
   least three thread counts including 1.
4. A test or grep-based assertion proves no `f64` reduction was parallelised.
5. `cargo test --locked` and `scripts/check-rust.sh` green, with unchanged
   negative-suite expectations.
6. Before/after under the README protocol, plus single-thread figures and the
   host's core count.
7. `python3 scripts/check-markdown-links.py` passes.

## Note on expectations

The spike measured 5.3–5.6× on isolated element-wise work. **The end-to-end
figure will be lower**, because a tick also does conflict resolution, reductions,
CSV assembly and state commit, none of which this PRD touches. Amdahl applies
and the sequential remainder is not small.

A result of 2–3× end-to-end would be a good outcome. A result near 1× means the
parallel regions are too small or too frequent, which is a finding that shapes
the tiling PRD — one parallel region per tick over row ranges, rather than one
per node.

The reason this PRD is first is not that it is the largest available change; it
is that it is **structurally independent**. Tiling, scalar broadcast and buffer
reuse all change what the evaluator does. Threading changes only how many cores
do it, so nothing later can invalidate it and it multiplies everything that
follows.
