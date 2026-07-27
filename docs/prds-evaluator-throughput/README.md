# Evaluator throughput PRDs

Ordered PRD set taking the host evaluator from roughly 100× above its physical
floor to within a small factor of it. Run from the Sembla repository with:

```text
/piprd run docs/prds-evaluator-throughput
```

`README.md` is ignored by `/piprd run`. Every numbered PRD must read this file
first; the constraints below are binding. When a PRD conflicts with this README,
this README wins.

`docs/performance-model.md` is the durable synthesis of everything below,
plus the CUDA-side picture and the open questions. This folder is the plan;
that document is the reference, and it outlives this folder.

**Where this sits in the overall order:** this folder's 0001 and 0002 are steps
1 and 2 of the work queue in
[`docs/performance-model.md`](../performance-model.md#work-queue), which
interleaves them with `docs/prds-cuda-host-path/`. Run from the queue, not
folder by folder — both folders eventually touch `state.rs`.

## Why this folder exists

`prds-host-evaluator-performance` took the fixed 1M case from 49.5 s to ~7.6 s
and, with no GPU change at all, carried the §L4 gate from a 2.56× miss to a
**4.207× pass** (`DECISIONS.md` §L8). That folder is closed.

This one is scoped from ten measured spikes rather than from a profile, because
PRD 0004 of the previous folder established that profile shares can mislead:
it removed a provable 8 MB-per-operand copy, the allocator symbols duly dropped
~20%, and the runtime did not move at all. Every item below was therefore
measured directly, as a hand-written arm asserted to produce identical results
against the real code.

The spikes live in `crates/sembla-runtime/examples/` and are runnable:

```sh
cargo run --release -p sembla-runtime --example fusion_spike
```

## The evidence

At 5M rows, a representative tick shape (18 view bands, 5 transition guards,
1 racing clock), all arms asserted to produce identical counts:

| shape | ms | speedup | × above memory floor |
|---|---:|---:|---:|
| column-wise (current) | 126.60 | 1.0× | 106 |
| row-wise, 1 thread | 58.13 | 2.2× | 48 |
| row-wise, threaded | 10.42 | 12.2× | 9 |
| + guarded `ln` | 6.21 | **20.4×** | 5 |

Individually measured, each against the real evaluator or the real RNG:

| change | measured | spike |
|---|---:|---|
| tiling + threading over tiles | 2.2× tiled; 12.2× with workers | `rowwise_spike`, `threading_spike` |
| guarded racing clock | 12.6× on the draw path | `ln_threshold_spike` |
| whole-tick tiling | 2.2× alone; 5.9× within an expression | `rowwise_spike`, `fusion_spike` |
| histogram recognition | 7.1× on the view bands | `histogram_spike` |
| buffer reuse | 4.04× on page churn | `alloc_spike` |
| leaf-column borrow | 2.6× | `fusion_spike` |
| scalar broadcast | 2.1× | `fusion_spike` |

### Where the residual goes

After row-wise, threading and the guarded `ln`, the remaining 6.21 ms is:
Philox draws 3.60 ms, band histogram 1.66 ms, reading 0.41 ms, guards 0.15 ms.

**The binding constraint has moved.** This workload is no longer memory-bound
or interpretation-bound; it is RNG-bound, so the 1.20 ms bandwidth floor is not
the floor that applies. Philox costs ~10 ns/draw and `rng_batch_spike` shows
that is not a branch-prediction artefact — gathering the enabled rows first is
0.9× and four-way unrolling is 1.0×. It is near the scalar limit.

### Ruled out — do not re-scope these

- **Bitset masks: 1.0×** (`bitset_spike`). A `Vec<bool>` mask is 4.8 MiB at 5M
  rows and already sits in cache; packing cost cancels the traffic saving.
- **Narrowed column storage: i32 1.3×, i16 0.8×, u8 0.9×** (`narrow_spike`).
  Widening on load costs more than the traffic saved and narrow types defeat
  vectorisation.

Together these correct an earlier framing: once fused, the evaluator is a few ×
off bandwidth, not ~90×, so representation changes cannot close the gap.

## Binding constraints

- **Results must not change.** This is the CPU oracle — `DESIGN.md` §8 makes it
  ground truth for the CUDA differential harness and §E2's determinism levels
  are defined against it. Every golden, every CSV and hash golden, the frozen
  demographic state fixture, the run manifest including `final_state_sha256`,
  and the tracked CUDA differential evidence must be **byte-identical**. A moved
  golden is a failed PRD, not a new baseline.
- **Bit-identity must be argued structurally, not empirically.** "The tests
  passed" is not the standard here. Each PRD must state why its change *cannot*
  alter a result — same values, same operations, same order — because the oracle
  has nothing above it to catch a wrong answer.
- **No new dependencies.** Everything below was measured using only `std`.
- **Error behaviour is observable.** Diagnostics are part of the contract,
  including *when* an error is raised.

## Determinism under parallelism (binding)

Several PRDs here introduce concurrency. `DESIGN.md` §4.2 defines the closed
kernel fragment as "operations whose parallel execution is order-free or has a
canonical order", and §E1 makes randomness coordinate-pure, so this is within
the design rather than against it. But:

- **Work partitioning must derive from row index alone** — never from thread
  count, scheduling, or availability. A result that depends on how many cores
  the machine has is not reproducible.
- **Real reductions stay sequential.** `eval.rs` documents ascending row order
  as the canonical Level A reduction order. Integer counts, min and max are
  associative and may be combined from per-thread partials; `f64` sums may not.
- **Every parallel PRD must include a test asserting identical output across at
  least three thread counts**, including 1.

## Sequencing, and why

1. **Tile the tick** first, with parallelism over tiles folded in. Revised
   2026-07-27: threading was originally scoped standalone and first, and the
   measurement disproved the premise — per-node parallel regions do not pay
   (`docs/evidence/evaluator-parallel-element-wise-20260727/`). Tiling is what
   gives parallelism a granularity that works, and is worth 2.2× by itself.
2. **Guarded `ln`** next: independent of the evaluator's shape, self-contained,
   and it attacks the largest non-allocation item in the profile.
3. **Scalar broadcast**: contained, and *not* subsumed by tiling — a constant
   should not be materialised into a tile buffer either.
4. **Whole-tick tiling**: the structural change. Deliberately later because it
   is the largest, and because it **subsumes leaf-column borrow and histogram
   recognition** — both of those become properties of the tiled shape rather
   than separate work. Do not scope them separately.
5. **Buffer reuse**: after tiling, which changes what would be pooled.

4. **Tile the remainder** (`0003`): 0001 tiled half the tick and 0002 then showed
   the consequence — making the parallel half 10.7% cheaper moved wall time by
   0.19%, because the critical path is the untiled serial half.
5. **The degenerate hazards** (`0004`): 0002's own evidence shows `age_monthly`
   alone holds 97% of the surviving `ln` calls, retained by a choice made before
   that distribution was known.

`0003` and `0004` were scoped from 0001's and 0002's measurements. Anything
after them stays **deliberately unwritten**, per the discipline inherited from
`prds-host-evaluator-performance`.

## Deferred: the RNG decision

`rng_variants_spike` measures counter-based alternatives to Philox-4x32-10:

| variant | ns/draw | speedup | uniformity | avalanche | serial corr |
|---|---:|---:|---|---:|---:|
| philox-10 (current) | 7.93 | 1.00× | pass | 32.0 | +0.0014 |
| philox-7 | 5.09 | 1.56× | pass | 32.0 | +0.0011 |
| philox-4 (control) | 2.86 | 2.77× | pass | 32.0 | **−0.0551** |
| mix64 | 0.87 | 9.08× | pass | 32.0 | −0.0012 |

**This is not a reproducibility trade.** §E1's guarantee comes from the
counter-based construction, not from Philox, so any counter mixer keeps every
determinism property at every §E2 level. What changes is the stream and its
statistical quality.

It is deferred, and not part of this folder, for three reasons. It would
regenerate **every** golden and frozen vector in the project. §E2's levels
cannot express it — they keep the same draws and vary only FP accumulation, so
it needs its own recorded decision. And the quality screen above is three
homemade tests over one varied coordinate; `tick`, `rule`, and cross-rule
correlation are untested, and those are where counter mixers fail.

Note for whoever picks it up: the 1.56× and the 9.08× cost the same in the only
expensive respect, since both regenerate everything. Taking the small win first
means paying that cost twice.

## Measurement protocol

Inherited unchanged from `docs/prds-host-evaluator-performance/README.md`:
the fixed 1M/24-tick/seed-9009 case, five runs each side, fastest uncontended
run as the headline with the median alongside, user time primary, per-run
contention flagged at `wall − (user + sys) > 0.5 s`, and measurement performed
in-run. Quiescence is advisory, not a gate.

Reference: after `prds-host-evaluator-performance`, best uncontended run was
**8.17 s wall / 6.19 s user**
(`docs/evidence/host-evaluator-hash-on-demand-20260726/`).

Parallel PRDs must additionally report single-thread time, so a regression in
serial efficiency cannot hide behind core count.

## PRD 0001 implementation status

Revised PRD 0001 implements the permitted rigorous partial tiling scope.
Aggregate-free, input-reduction-free, row-infallible transition guards, hazards,
claim resources, and claim keys are evaluated in fixed 1,024-row tiles. Racing
clocks and candidate construction share the same task. One tick-level
`std::thread::scope` distributes complete fixed tasks from 1,000,000 rows;
post-commit `Count` views use the same tile shape serially. Aggregates, numeric
views, effects, writes, and conflict resolution retain their canonical
column-wise or sequential paths.

Tile boundaries depend only on row count and tile size. Worker count changes
only task assignment, and candidates merge in transition/tile/row order.
`SEMBLA_EVAL_THREADS` remains the worker override; tile and threshold sweeps use
`SEMBLA_EVAL_TILE_ROWS` and `SEMBLA_EVAL_TILE_THRESHOLD`. Tests cross workers 1,
2, and 4 with tile sizes 257, 1,024, and 4,093, comparing Real chains and racing
clocks by `to_bits`, complete tick reports exactly, and Real aggregate fallback
bits. Prepared and column evaluation share the same checked parameter resolver;
a cross-model wrong-type regression test pins identical diagnostics across the
same worker/tile matrix.

The binding five-run corpus kept every output byte-identical. After rerunning the
revised binary, fastest uncontended user/wall times were 6.00/7.71 seconds
before, 6.10/7.32 with tiled single-worker execution, and 6.66/5.47 with ten
workers. Thus single-worker user time is near-flat while ten workers reduce wall
time 1.41× at the cost of more aggregate CPU. The separated tile sweep,
threshold sweep, every official run, contention flags, live-set arithmetic, and
hashes are under
[`docs/evidence/evaluator-tiled-tick-20260727/`](../evidence/evaluator-tiled-tick-20260727/).

## PRD 0002 implementation status

PRD 0002 adds a conservative prefilter for direct Real-literal and Real-parameter
hazards. Each transition/tick computes `lo = exp(-(lambda * dt)) * (1 - 1e-12)`
once. Enabled candidates below `lo` skip `ln`; all admitted candidates use the
same open uniform, unchanged `-uniform.ln() / lambda` transform, and unchanged
`race_time < dt` comparison. Row-to-`entity_id` conversion remains before the
filter, and row-dependent hazards keep the unfiltered oracle path.

The `1e-12` named margin is about 4,500 binary64 ULPs near the benchmark
thresholds, conservatively covering the documented one-ULP platform difference
and threshold-rounding envelope. Degenerate `1e300` hazards are deliberately not
special-cased: `lo` is zero, so every open uniform is admitted and its exact race
time remains available to any contested model.

Across the eight ordinary demographic transitions, only 0.10%–2.47% of enabled
candidates computed `ln`; the two deliberate `1e300` transitions remained at
100%. Fastest uncontended user/wall time changed from 6.66/5.34 seconds to
5.95/5.35 seconds, a 1.119× user-time speedup with essentially flat headline
wall time, while all outputs and final-state hashes remained byte-identical.
Full five-run measurements, per-transition fractions,
structural argument, tests, and hashes are under
[`docs/evidence/evaluator-guarded-racing-clock-20260727/`](../evidence/evaluator-guarded-racing-clock-20260727/).

## PRD 0003 implementation status

PRD 0003 lands the permitted measured partial scope for committed-state views.
Fixed post-commit tasks now evaluate eligible `Count` filters and row-local
numeric-view filters/value expressions. Count tasks return associative integer
partials. Numeric task outputs join before reduction; every Real value is still
accumulated one row at a time in ascending tile-start and row order, so no
floating-point partial reduction exists.

Aggregate/input-dependent and row-fallible views retain the whole-column path.
Grouped views, effects, writes, aggregate construction, and conflict resolution
remain column-wise or sequential. Writes were not tiled because effect values
depend on declaration-ordered conflict winners; the existing snapshot, staged
write, double-write check, and single-buffer application path preserves both
cell ownership and observable error order without introducing shared mutation.

Tests compare the whole-column fallback with workers 1, 2, and 4 and tile sizes
257, 1,024, and 4,093. The corpus includes a filtered Real `Sum`, raw `to_bits`
comparisons, a contested transition with a Real effect write, the complete
post-tick Real state, racing clocks, and claim keys. A structural assertion pins
Real accumulation to nested ascending tile/row loops.

On the revised wall-primary protocol, fastest uncontended default-worker wall
time improved from 5.23 to 4.81 seconds (1.087×); median wall improved from
5.51 to 5.10 seconds (1.080×). Single-worker wall/user changed from 6.81/5.50
to 6.69/5.51 seconds. Headline CPU efficiency rose from 13.54% to 15.18%, and
the coarse Amdahl serial-fraction estimate fell from 74.22% to 68.78%. All 20
official outputs, summaries, manifests, and final-state hashes are byte-identical.
Full measurements and the structural argument are under
[`docs/evidence/evaluator-tiled-views-20260727/`](../evidence/evaluator-tiled-views-20260727/).

## PRD 0004 implementation status

PRD 0004 adds an `AlwaysFires` strategy alongside PRD 0002's guarded racing
clock. It applies only to a constant hazard whose exact `exp(-(lambda * dt))`
threshold is `0.0` and whose IR declaration has no contests. Every contest is
conservatively treated as a sampled-time consumer, so degenerate contested
transitions still draw and retain the canonical `ln` race-time bits.

The proof depends on three frozen properties. The racing-clock inequality is
`u > exp(-(lambda * dt))`; `uniform_f64` excludes zero, so every open uniform is
strictly above an exact-zero threshold; and §E1's coordinate-pure RNG has no
stream state for a skipped draw to perturb. Entity-id conversion remains before
strategy dispatch, preserving its overflow diagnostic. No RNG, guarded-filter,
tiling, conflict-resolution, IR, CUDA, or CLI behavior changed.

Measured `ln` calls fell from 19,117,749 to 291,688, removing 98.47% of PRD
0002's residue. Fastest default wall changed from 4.81 to 4.90 seconds while
median wall improved from 5.10 to 4.97 seconds; fastest user time improved from
6.10 to 5.87 seconds. Single-worker wall/user improved from 6.69/5.51 to
6.28/5.21 seconds. CPU efficiency changed from 15.18% to 14.59%, consistent
with less CPU work on an effectively flat wall-time critical path. All outputs,
summaries, manifests, and final-state hashes remain byte-identical. Full
measurements, per-transition counts, tests, hashes, and the structural argument
are under
[`docs/evidence/evaluator-degenerate-hazard-fast-path-20260727/`](../evidence/evaluator-degenerate-hazard-fast-path-20260727/).
