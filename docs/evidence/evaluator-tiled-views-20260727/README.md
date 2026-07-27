# Tiled-view evidence — 2026-07-27

## Scope implemented

PRD 0003 lands the first measured remainder phase: committed-state views.
`Count` filters and row-local numeric-view filters/value expressions now use
fixed `(table, tile_start, tile_end)` tasks. A post-commit scoped region assigns
those complete tasks to workers; task boundaries depend only on stable model
indices, row count, and tile size. Count tiles return associative integer
partials instead of retaining filter columns. Numeric tiles return row-local
values, then join before reduction.

The following remain column-wise or sequential:

- every aggregate/input-dependent or row-fallible view expression;
- grouped views;
- effect-column evaluation, write staging, and write application;
- conflict resolution and its sort;
- aggregate construction.

Writes were deliberately not tiled. Effect values depend on winners produced by
sequential conflict resolution. The existing path evaluates effect columns from
the immutable tick-start snapshot, stages writes in declaration/winner order,
checks all target cells for double writes, and applies through one write buffer.
Keeping that path avoids shared-cell races and preserves double-write and
expression-error precedence. This is the explicitly permitted partial outcome.

## Structural identity argument

Workers evaluate only row-local expressions. They never reduce `f64` values.
After all view tasks join, numeric tile outputs are sorted by absolute start row.
The Real reducer visits tiles in that order and visits rows in vector order,
performing the unchanged `result + value`, `total_cmp` minimum, or `total_cmp`
maximum operation once per selected row. This is exactly the former ascending
row order, including signed zero and NaN behavior. No floating-point partial sum
exists.

Count partials contain only selected-row cardinalities. Their combination is
integer addition over disjoint fixed tiles, so worker assignment and completion
order cannot affect the result. Numeric integer reductions are also retained in
ascending row order so checked-overflow behavior is unchanged.

If a view cannot be prepared without changing observable error precedence, it
uses the original whole-column evaluator. View results and stored errors remain
in declaration order even though eligible tile computations are independent.

## Determinism coverage

The runtime matrix compares the tiled result against the whole-column fallback
at workers 1, 2, and 4 and tile sizes 257, 1,024, and 4,093. Its model includes:

- a filtered Real arithmetic expression reduced with `Sum`, compared by
  `to_bits` through `ObservationValue` and explicitly as raw sum bits;
- a transition that wins contested rows and writes a Real attribute;
- the complete post-tick Real state compared by `to_bits`;
- the prior Real racing-clock and claim-key corpus.

A source-shape assertion pins the Real view reducer to nested ascending tile/row
loops and rejects parallel iterators or a `sum::<f64>` reduction.

## Measurement protocol

PRD 0003's revised rule makes wall time primary. Measurements used the frozen
no-grouped demographic case: 1,000,000 slots, 24 ticks, seed 9009, four areas,
present fraction 0.8, streams `birth:600,overseas:250,internal:150`, and CPU
backend. The host was an Apple M2 Pro with 10 physical/logical cores, 16 GiB
RAM, and `available_parallelism() = 10`.

Each mode has five in-run measurements:

```sh
/usr/bin/time -l -o <time-file> <binary> run <resized-no-grouped-model> \
  --seed 9009 --population <shared-1m-state> --backend cpu --ticks 24 \
  --out <run-output.csv>
```

Single-worker modes set `SEMBLA_EVAL_THREADS=1`. A run is contended when
`wall - (user + sys) > 0.5 s`; no headline run was contended.

| Mode | Fastest wall/user/sys s | Median wall/user/sys s |
|---|---:|---:|
| Before, default workers | **5.23 / 5.93 / 1.15** | 5.51 / 5.94 / 1.36 |
| Before, one worker | **6.81 / 5.50 / 1.27** | 6.95 / 5.56 / 1.30 |
| After, default workers | **4.81 / 6.10 / 1.20** | 5.10 / 6.21 / 1.37 |
| After, one worker | **6.69 / 5.51 / 1.15** | 6.71 / 5.51 / 1.17 |

Headline default-worker wall time improves **1.087× (8.03%)**; median wall time
improves 1.080×. Single-worker wall improves 1.018× while user time changes by
+0.18%, so the parallel result is not hiding a serial-efficiency loss.

CPU efficiency is reported as `(user + sys) / (wall * 10 logical cores)` on the
headline default runs: **13.54% before** and **15.18% after**.

## Serial-fraction estimate

The coarse whole-run estimate uses the fastest uncontended single/default wall
ratio and inverts Amdahl's law for ten workers:

```text
f = (1 / speedup - 1 / 10) / (1 - 1 / 10)
```

The estimate falls from **74.22% before** (`6.81 / 5.23 = 1.302×`) to **68.78%
after** (`6.69 / 4.81 = 1.391×`). The estimate includes startup, output, state
commit, and every remaining sequential phase, so it is directional rather than
a phase timer. Its movement agrees with the implementation: wall time improves
and the modeled serial fraction falls after view work moves into fixed tasks.

## Measured rejected iteration

The first implementation retained every Count filter tile until workers joined.
Five default-worker runs regressed fastest wall from 5.23 s to 5.59 s and user
from 5.93 s to 6.46 s. It was rejected. Returning associative Count partials
removed that retained-buffer cost and produced the final figures above. Effects,
writes, and aggregates were not attempted after the view phase because the PRD
prefers a correct, measured partial landing over rushing the next phase.

## Acceptance gates

The final workspace passed:

- `cargo test --locked`;
- `scripts/check-rust.sh` (after a focused retry confirmed the known transient
  `state_artifact` failure and the complete gate rerun passed);
- `python3 scripts/check-markdown-links.py` (120 links);
- `cargo fmt --all -- --check`;
- `git diff --check`;
- JSON validation for `measurements.json`.

No tracked file under `examples/**`, `fixtures/**`, `frontend/Fixtures/**`, the
CUDA fixture corpus, or tracked CUDA evidence changed.

## Identity

All 20 official before/after default/single runs are byte-identical:

- primary CSV: `eb6d095740127bbf41576d6b05f1470656dbb5f85372ef2ff5f1751576303e37`;
- summaries CSV: `329bc9e17af3032a81d4dd60263cd70c26fd734e33fce7cfcb8da66558bca6d3`;
- manifest: `dbeaa57719ef88945ac46336ad8033c5cdac91d8cd5b8bb21679437e0122aa1f`;
- `final_state_sha256`:
  `2d509ead9aa506e71be155faaa5608542f7ca32cee203ee42b0d3179d670020c`.

The machine-readable file records all runs, contention flags, medians, binary
and input hashes, CPU efficiency, serial-fraction calculations, and the rejected
iteration: [`measurements.json`](measurements.json).
