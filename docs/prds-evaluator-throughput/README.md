# Evaluator throughput PRDs

Ordered PRD set taking the host evaluator from roughly 100× above its physical
floor to within a small factor of it. Run from the Sembla repository with:

```text
/piprd run docs/prds-evaluator-throughput
```

`README.md` is ignored by `/piprd run`. Every numbered PRD must read this file
first; the constraints below are binding. When a PRD conflicts with this README,
this README wins.

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
| threading, element-wise | 5.3–5.6× | `threading_spike` |
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

1. **Threading** first: the largest single factor, structurally independent of
   everything else, bit-identical by construction, and it multiplies whatever
   follows. Nothing later can invalidate it.
2. **Guarded `ln`** next: independent of the evaluator's shape, self-contained,
   and it attacks the largest non-allocation item in the profile.
3. **Scalar broadcast**: contained, and *not* subsumed by tiling — a constant
   should not be materialised into a tile buffer either.
4. **Whole-tick tiling**: the structural change. Deliberately later because it
   is the largest, and because it **subsumes leaf-column borrow and histogram
   recognition** — both of those become properties of the tiled shape rather
   than separate work. Do not scope them separately.
5. **Buffer reuse**: after tiling, which changes what would be pooled.

PRDs after 0002 are **deliberately unwritten**. Each is re-scoped from a fresh
measurement once its predecessor lands, per the discipline inherited from
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
