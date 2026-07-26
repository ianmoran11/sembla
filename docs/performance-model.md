# Performance model

What limits Sembla, where the time actually goes, what has been measured, and
what has been ruled out. **Read this before optimising anything**, and update it
when a measurement changes it.

This is a reference, not a plan. Plans live in PRD folders; verdicts live in
`DECISIONS.md`.

## The short version

- The **GPU is not the constraint** and has not been since 2026-07-26. Kernels
  are 0.56% of CUDA wall time.
- The **host evaluator** is what limits both backends. Work there sped up CUDA
  by 5.4× and CPU by 3.3× without a line of GPU code changing, and carried the
  §L4 gate from a 2.56× miss to a 4.207× pass (`DECISIONS.md` §L8).
- Roughly **20× more** is available on the host, measured, from three changes.
- After those, the system becomes **RNG-bound**, not memory-bound. That changes
  which floor applies.

## Physical floors

Two, and knowing which one binds is the whole game.

**Memory.** The demographic state is 48 bytes per slot. A tick that reads it
once and writes it once moves ~2× that. At 5M rows on ~200 GB/s that is
**~2.3 ms/tick**; at 50M, ~23 ms.

**RNG.** A tick draws one Philox value per enabled candidate. Philox-4x32-10 is
ten rounds of two 32×32→64 multiplies and costs **~10 ns/draw** single-threaded,
about 35 cycles. `rng_batch_spike` confirms this is not a branch-prediction
artefact: gathering enabled rows first is 0.9×, four-way unrolling 1.0×. It is
near the scalar limit.

At the benchmark's ~43% enabled fraction, the RNG floor exceeds the memory floor
once the memory traffic is under control. **After the measured optimisations the
workload is RNG-bound**, and quoting the bandwidth floor becomes misleading.

## Where the time goes

### CUDA path, 5M rows over 2 ticks

From `docs/evidence/demographic-bench/hyperstack-l4-20260726T140326Z/profile/`,
via the `--timing-json` instrumentation:

| phase | share |
|---|---:|
| `state_reconstruct` | 45.8% |
| `observe_views` | 20.2% |
| `state_transfer` | 13.9% |
| `readback_control` | 7.8% |
| `report` | 7.4% |
| `other` | 4.3% |
| `kernels` | **0.56%** |

`state_reconstruct` is `unpack_state` + `StateStore::new` rebuilding the whole
host state every tick so host-side observation has something to read. It is host
allocation, **not PCIe** — transfer is a third of its cost. Roughly 80% of CUDA
wall time is "bring state to the host and observe it there".
`docs/prds-cuda-host-path/` addresses the reconstruction.

### CPU evaluator, 5M rows, representative tick shape

From `rowwise_spike`, 18 view bands + 5 guards + 1 racing clock:

| shape | ms | × above memory floor |
|---|---:|---:|
| column-wise (current) | 126.60 | 106 |
| row-wise, 1 thread | 58.13 | 48 |
| row-wise, threaded | 10.42 | 9 |
| + guarded `ln` | 6.21 | 5 |

Residual at 6.21 ms: Philox 3.60, band histogram 1.66, reading 0.41, guards 0.15.

## Measured levers

Each was measured as a hand-written arm asserted to produce identical results
against the real code. Spikes are runnable:

```sh
cargo run --release -p sembla-runtime --example threading_spike
```

| lever | measured | spike |
|---|---:|---|
| threading, element-wise | 5.3–5.6× on 10 cores | `threading_spike` |
| guarded racing clock | 12.6× on the draw path | `ln_threshold_spike` |
| whole-tick tiling | 2.2×; 5.9× within one expression | `rowwise_spike`, `fusion_spike` |
| histogram recognition | 7.1× on the view bands | `histogram_spike` |
| buffer reuse | 4.04× on page churn | `alloc_spike` |
| leaf-column borrow | 2.6× | `fusion_spike` |
| scalar broadcast | 2.1× | `fusion_spike` |

Tiling subsumes leaf-column borrow and histogram recognition — both become
properties of the tiled shape. Scalar broadcast is *not* subsumed: a constant
should not be materialised into a tile buffer either.

## Ruled out — do not re-scope these

| idea | measured | why it fails |
|---|---:|---|
| bitset masks | 1.0× | a `Vec<bool>` mask is 4.8 MiB at 5M rows and already in cache; packing cost cancels the traffic saving |
| narrowed column storage | i32 1.3×, i16 0.8×, u8 0.9× | widening on load costs more than the traffic saved, and narrow types defeat vectorisation |
| further CUDA kernel work | — | kernels are 0.56% of wall time (`DECISIONS.md` §L7, §L8) |

The first two together correct an earlier framing: **once fused, the evaluator
is a few × off bandwidth, not ~90×.** The large gap belongs to the intermediates,
not to the representation, so representation changes cannot close it.

## Deferred: the RNG

`rng_variants_spike` measures counter-based alternatives:

| variant | ns/draw | speedup | uniformity | avalanche | serial corr |
|---|---:|---:|---|---:|---:|
| philox-10 (current) | 7.93 | 1.00× | pass | 32.0 | +0.0014 |
| philox-7 | 5.09 | 1.56× | pass | 32.0 | +0.0011 |
| philox-4 (control) | 2.86 | 2.77× | pass | 32.0 | **−0.0551** |
| mix64 | 0.87 | 9.08× | pass | 32.0 | −0.0012 |

**This is not a reproducibility trade.** §E1's guarantee comes from the
counter-based construction — a draw being a pure function of
`(seed, tick, rule_id, entity_id, draw_idx)` — not from Philox. Any counter
mixer keeps every determinism property at every §E2 level. What changes is the
*stream* and its statistical quality.

Three reasons it is deferred: it regenerates every golden and frozen vector in
the project; §E2's levels cannot express it, since they keep the same draws and
vary only FP accumulation, so it needs its own recorded decision; and the screen
above is three homemade tests over one varied coordinate — `tick`, `rule`, and
cross-rule correlation are untested, and those are where counter mixers fail.

Note for whoever takes it: **1.56× and 9.08× cost the same** in the only
expensive respect, since both regenerate everything. Taking the small win first
means paying that cost twice.

## Work queue

Which PRD to run, in order. Two folders are active and their items interleave,
so run them from here rather than folder by folder.

| # | PRD | GPU? | why here |
|---|---|---|---|
| 1 | `prds-evaluator-throughput/0001` threading | no | largest single factor; structurally independent, so nothing later can invalidate it, and it multiplies everything that follows |
| 2 | `prds-evaluator-throughput/0002` guarded `ln` | no | independent of the evaluator's shape; self-contained correctness argument |
| 3 | `prds-cuda-host-path/0001` reuse the state buffer | local only | largest single CUDA-path item at 45.8%; its *local* criteria need no GPU |
| 4 | **one GPU session** | yes | verify 3's hardware criteria and re-measure the CUDA phase split, in a single trip |
| 5 | re-scope | no | write `prds-evaluator-throughput/0003+` and `prds-cuda-host-path/0002+` from the measurements 1–4 produce |

**Why batch the GPU work into step 4.** A Hyperstack session costs money, needs
you at the keyboard, and every session so far has produced at least one
surprise. Steps 1–3 are free and local. Doing them first also makes step 4 more
informative, because threading changes the CUDA phase shares — `observe_views`
runs on the host in both backends — so the split measured after is the one worth
scoping from.

**Why not run the folders separately.** `prds-cuda-host-path/0001` touches
`state.rs`, and so will the buffer-reuse PRD in `prds-evaluator-throughput`.
Interleaving them in this order keeps that overlap to one PRD at a time.

**Deliberately unwritten**, and to be scoped at step 5 rather than now: scalar
broadcast, whole-tick tiling, buffer reuse, and whatever the CUDA phase split
ranks next. Each is re-scoped from a fresh measurement because removing one cost
re-ranks everything behind it.

**Not queued**: the RNG change (deferred, see above) and device-side observation
(a §K6/§L5 semantic decision, needing its own folder).

## Methodology, learned the hard way

**Measure directly; do not scope from profile shares.** See `DECISIONS.md` §M1.

**Profiles understate allocation cost.** First-touch page-fault time is
attributed to the code writing the memory, not to `malloc`. The allocator
symbols in a profile are the visible tip; `alloc_spike` measures 4.04× available
from reuse where the symbols suggested ~14%.

**Sample share is not time.** PRD 0004 of `prds-host-evaluator-performance`
removed a provable 8 MB-per-operand copy, the allocator symbols duly dropped
~20%, and the measured runtime did not move at all.

**Include a control.** `rng_variants_spike`'s deliberately weakened philox-4
failed the serial-correlation screen by ~40×, which is the only reason to
believe the screen had power to reject anything.

**A ratio is not a goal.** §L4 asks whether the GPU beats the CPU by 3×.
Optimising the CPU *lowers* that ratio while improving the product. The gate has
served its purpose; do not steer work by it (`DECISIONS.md` §L8).

## Measurement protocol

The fixed case, reporting rules, and contention handling are specified in
`docs/prds-host-evaluator-performance/README.md` and apply to any host
measurement. Briefly: five runs each side, fastest uncontended run as the
headline, user time primary, per-run contention flagged at
`wall − (user + sys) > 0.5 s`, measurement in-run.

Two traps found in practice. **Quiescence cannot be an acceptance criterion** —
a PRD executed by the runner cannot require the runner's absence, and one that
did stalled a run at five attempts. And **Spotlight indexing a multi-gigabyte
`target/` directory** was the likeliest cause of three contended sessions; a
`.metadata_never_index` marker fixes it.

## Open questions

- Does the CUDA `state_reconstruct` cost survive buffer reuse, or is it
  intrinsic to rebuilding a `StateStore`?
- What is the band histogram's 1.66 ms actually made of? Suspected signed
  integer division, worse in the real system where `band_width` is a runtime
  value rather than a constant.
- Would SIMD Philox pay? It is bit-identical by construction — integer
  arithmetic — but needs platform intrinsics. Perhaps 2–4× on 62% of the
  residual.
- Is device-side observation worth its §K6/§L5 decision? It would remove ~80% of
  CUDA wall time, but the reconstruct fix may capture most of that far more
  cheaply.
