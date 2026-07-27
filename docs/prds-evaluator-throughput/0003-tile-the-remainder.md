# PRD 0003: Tile the rest of the tick

## Context

Read `docs/prds-evaluator-throughput/README.md` first; its constraints bind,
including the **revised measurement protocol** — wall time is primary for
parallel work, with single-worker figures and CPU efficiency reported
separately.

PRD 0001 tiled part of the tick and parallelised over tiles: wall 7.68 s →
5.47 s (1.40×). PRD 0002 made the tiled portion cheaper: user time 6.66 s →
5.95 s (10.7%) with **wall time flat**.

That pair is the whole argument for this PRD. 0001 declared its scope honestly:

> aggregate/input-free, row-infallible transition guard, hazard, claim-resource
> and claim-key expressions are evaluated per fixed tile … **aggregates, numeric
> views, effects, writes and conflict resolution remain column-wise/sequential**

So the tick is now half tiled and parallel, half column-wise and serial. 0002
then demonstrated the consequence: making the parallel half 10.7% cheaper moved
wall time by 0.19%, because **the critical path is the serial half**. Amdahl,
measured rather than assumed.

Two further signals point the same way. Tiling alone (single worker) gave 1.05×
wall against the 2.2× `rowwise_spike` predicted — expected for a tiled
interpreter, but also consistent with the untiled half still walking the state in
full passes, so the column is read many times per tick regardless. And the
`rowwise_spike` arm that produced 2.2× tiled *everything*, including the band
counters that are still column-wise today.

## Goal

The whole tick evaluates in tiles, so the state is read once per tick rather
than once per operation. Results are unchanged, bit for bit.

## Specification

### 1. Extend tiling to the remainder, in order of measured cost

The untiled work, from the CUDA phase split and the CPU profile: numeric and
`Count` views, effects and writes, then aggregates.

**Take them one at a time and measure between each.** This PRD may land with
only some of them tiled; a partial result that is correct, measured and honestly
scoped is a better outcome than a complete one that is rushed. 0001 set that
precedent and it was the right call.

State in the implementation notes which parts are tiled and which remain, in
the same form 0001 used.

### 2. Views: let the histogram fall out, do not build one

Eighteen of the model's views are disjoint age bands over one column. Tiled,
with a counter per band updated per row, that *is* a histogram — `histogram_spike`
measured 7.1× against the current shape.

Do not implement band recognition as a separate mechanism. It should be a
consequence of tiling the view evaluation, not a special case. If it does not
fall out, say so rather than special-casing it.

Per-tile integer counters combine associatively across tiles and workers, so
this stays bit-identical. `f64` view reductions do **not** — see §3.

### 3. What still stays sequential

- **`f64` reductions.** `eval.rs` fixes ascending row order as the canonical
  Level A reduction order. A per-tile partial sum changes the result. This is
  the easiest way to fail this PRD, and it becomes *more* tempting here than in
  0001 because numeric views are exactly where sums live.
- **Conflict resolution** and its sort.

Integer counts, `min` and `max` may combine per-tile partials. Where used, say
so and justify it.

### 4. Writes need care that reads did not

0001 tiled read-side expression evaluation. Effects and writes mutate state, so
tiling them raises questions reads never did: two tiles must not write the same
cell, and the write order must not affect the result.

`DECISIONS.md` §E3 already makes conflict resolution the arbiter of contested
writes, so the ordering question has an answer — but **state explicitly why
tiled writes cannot race or reorder observably**, rather than relying on tests.
If that argument cannot be made cleanly, leave writes column-wise and say so.
That is an acceptable outcome for this PRD.

### 5. Determinism tests

As 0001: identical output across at least three worker counts including 1, and
three tile sizes, compared bitwise via `to_bits`. Extend the corpus to cover the
newly tiled work — at minimum a numeric view containing an `f64` sum, and a
model exercising effects.

### 6. Measure under the revised protocol

Wall time primary, single-worker figures separate, CPU efficiency reported.

**Report the serial fraction.** The headline number this PRD exists to move is
how much of the tick remains sequential; quote it before and after, however
crudely estimated, and say how it was estimated. If wall time improves while
the serial fraction does not fall, something other than tiling caused it.

## Allowed files

- `crates/sembla-runtime/src/eval.rs`
- `crates/sembla-runtime/src/executor.rs`
- `crates/sembla-runtime/src/state.rs` — only if tiled writes require it
- `crates/sembla-runtime/tests/**`, `crates/sembla-cli/tests/**` (tests only)
- `docs/evidence/**` (new evidence only)
- `docs/prds-evaluator-throughput/README.md` (status notes only)

**If a required gate fails on files outside this list, stop and report it.** Do
not fix it and do not treat it as in scope — see `DECISIONS.md` §M2.

## Non-goals

No scalar broadcast — separate and not subsumed by tiling. No buffer reuse. No
RNG change. No change to `f64` reduction order. No IR, Lean, CUDA, or CLI
changes. No new dependencies.

## Acceptance criteria

1. Additional tick phases evaluate in tiles; the implementation notes state
   exactly which are tiled and which remain column-wise.
2. Determinism tests per §5 pass bitwise across three worker counts and three
   tile sizes, covering the newly tiled work.
3. A test or assertion proves no `f64` reduction was tiled or parallelised in a
   way that changes accumulation order.
4. If writes were tiled, §4's argument is stated; if not, the reason is.
5. **Every golden is byte-identical**, including the manifest and
   `final_state_sha256`.
6. `cargo test --locked` and `scripts/check-rust.sh` green.
7. Before/after under the revised protocol, with single-worker figures, CPU
   efficiency, and the serial fraction before and after.
8. `python3 scripts/check-markdown-links.py` passes.

## Note on expectations

`rowwise_spike` measured 2.2× for full tiling single-threaded and 12.2× with
workers, but that arm is hand-written — a tiled interpreter still pays node
dispatch, amortised rather than removed. 0001 got 1.05× single-threaded from
tiling half the tick. Tiling the rest should do better than that, but expect
well under 2.2×.

**The number to watch is not the speedup, it is whether wall time starts
responding to work done in the parallel half again.** 0002's flat wall time is
the symptom this PRD treats. If, after it lands, a cheaper parallel half still
fails to move wall time, then the remaining serial work is somewhere this PRD
did not look — and that is a more valuable finding than any speedup.
