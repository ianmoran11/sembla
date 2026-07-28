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

**Superseded twice. Current split, 2026-07-28** (`hyperstack-l4-20260728T072119Z/profile/`,
after `prds-device-observation/0001` and `0002`, §L11), 5M rows over 2 ticks:

| phase | no-grouped | grouped | share (grouped) |
|---|---:|---:|---:|
| `readback_control` | 193.1 | 197.0 | **56.1%** |
| `report` | 141.8 | 119.5 | 34.0% |
| `other` | 21.2 | 21.4 | 6.1% |
| `kernels` | 10.4 | 13.6 | 3.9% |
| `state_transfer` | **0.0** | **0.0** | 0% |
| `state_reconstruct` | **0.0** | **0.0** | 0% |
| `observe_views` | **0.0** | **0.0** | 0% |
| **wall** | **366.5** | **351.4** | |

`readback_control` and `report` together are **90%** of CUDA wall time, and both
serve one thing: moving the `wins` and `deferred` buffers to the host (200 MB
per tick at 5M) and counting them, to produce at most 13 integers of purely
diagnostic output. `readback_control` is the transfer alone; `report` is the
host-side scan. Neither touches simulation state.

Grouped and no-grouped totals are within run-to-run noise, so **device-side
grouped observation is approximately free** — against 1,335 ms of host
`observe_views` for the same work on CPU.

**Extrapolation warning.** Linear scaling from this 5M/2-tick profile predicted
12.7 s for the 10M/24-tick case; the measurement was 14.10 s, 11% low. The two
dominant phases allocate a fresh multi-hundred-megabyte host buffer per tick
(400 MB/tick at 10M), so they scale worse than linearly. **Projections above 10M
are optimistic**, and increasingly so with scale.

**Superseded 2026-07-27** by `hyperstack-l4-20260727T120050Z/profile/`, after
`prds-cuda-host-path/0001`: wall `1674.4ms → 936.1ms` (1.79×),
`state_reconstruct` `766.8 → 220.7ms`, `observe_views` `338.9 → 93.5ms`,
`state_transfer` flat, `kernels` 9.3ms (1.0%). The split below is the
pre-fix baseline the PRD was scoped from.

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

### CPU tick after PRDs 0001–0004 (`host-profile-20260727/`)

Binding 1M case, 24 ticks, 10 workers: `real 5.88s user 6.12s`. Main thread
4,217 samples, workers 2,618.

| | samples | reading |
|---|---:|---|
| `stage_box` and children | ~1,561 | **write staging — 46% of the tick, serial** |
| `Vec::from_iter` + free + `madvise` | ~1,930 | allocation churn, aggregated |
| `draw_u32x4` | 447 | Philox, irreducible without SIMD |
| `__ulock_wait` | 347 | workers idle at the join |
| `sha2::compress256` | 328 | final + results hashing |
| `merge_sort` | 261 | conflict resolution |
| `memcmp` + `locate_writable_cell` | 373 | **write path still resolves columns by string** |

`log` has left the ranking entirely — it was 554 samples and the largest
non-allocation item before PRD 0004 removed it.

Estimated serial fraction, by Amdahl inversion on single-versus-parallel wall
time: **68.8%** after 0003, down from 74.2%. That, not the speedup, is the
number the remaining work has to move.

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
| threading, over tiles | 5.3–5.6× on 10 cores in isolation, but **only at tick granularity** — per node it measured 1.00× | `threading_spike`, `evaluator-parallel-element-wise-20260727` |
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
| 1 | `prds-evaluator-throughput/0001` tile the tick | no | **landed**: wall 7.68 s → 5.47 s (1.40×), user +11% |
| 2 | `prds-evaluator-throughput/0002` guarded `ln` | no | **landed**: user −10.7%, wall flat |
| 2a | `prds-evaluator-throughput/0003` tile the remainder | no | 0002's flat wall time located the critical path in the untiled serial half |
| 2b | `prds-evaluator-throughput/0004` degenerate hazards | no | `age_monthly` holds 97% of the surviving `ln` calls |
| 3 | `prds-cuda-host-path/0001` reuse the state buffer | local only | largest single CUDA-path item at 45.8%; its *local* criteria need no GPU |
| 4 | **one GPU session** | yes | verify 3's hardware criteria and re-measure the CUDA phase split, in a single trip |
| 5 | re-scope | no | **done 2026-07-27**: `docs/evidence/host-profile-20260727/` |
| 6 | `prds-evaluator-throughput/0005` write identity | no | the serial remainder is the write path; three of its four costs share one cause |
| 7 | `prds-evaluator-throughput/0006` effect gathering | no | effect values computed for every row, used for the ~2% that fire |
| 8 | `prds-device-observation/0001` + `0002` | yes | **landed, hardware-verified §L11**: CUDA 26.37 s → 14.10 s (1.87×); `state_transfer`, `state_reconstruct` and `observe_views` all exactly zero |
| 9 | **fix the §L12 validation deadlock** | yes | *do this first.* It blocks the differential corpus, which is the safety net for every further device-side change — including item 10. 23-second reproduction |
| 10 | **device-side `wins`/`deferred` reduction** | yes | now **91%** of CUDA wall time (`readback_control` 53% + `report` 33%), moving and counting 200 MB/tick at 5M to produce ≤13 diagnostic integers |
| 11 | **amortise startup across sweep draws** | no | largest win for batch work: after item 10 a 1M draw is ~2.3 s of which ~2.2 s is JIT and state load, paid once per draw |

**Why 9 before 10.** Item 10 is a device-side change to a reduction, which is
exactly the shape the differential corpus exists to check. Landing it while the
corpus cannot run would mean shipping the riskiest available change with the
safety net down. The deadlock's reproduction is 23 seconds, so the cost of
fixing it first is negligible.

**Why 11 is not last in value.** It is last in dependency order only. For batch
runs — the actual goal — it beats item 10: a hundred 1M draws as separate
processes cost ~230 s after item 10, of which ~220 s is startup repeated a
hundred times; sharing one process makes it ~11 s. The two compound, because
item 10 is what makes the per-draw simulation cost small enough for startup to
dominate.

The write path was earlier described here as needing "a design idea, not an
optimisation". That was wrong, and it was wrong for the reason §M1 warns about:
it reasoned from an aggregate share without decomposing it. Decomposed, the
serial remainder is four ordinary costs, three of which share one cause.
Conflict resolution's ordering — the thing that looked like the obstacle — is
not involved.

**Worker join idle: diagnosed 2026-07-27, and it closes off a whole direction.**
`docs/evidence/parallel-scaling-spike-20260727/` swept worker count on the
binding case:

| workers | 1 | 2 | 4 | 6 | 8 | 10 |
|---|---:|---:|---:|---:|---:|---:|
| wall (s) | 6.60 | 5.53 | 4.57 | **4.34** | 4.42 | 4.35 |
| speedup | 1.00× | 1.19× | 1.44× | **1.52×** | 1.49× | 1.52× |

**Scaling saturates at six workers**, and beyond that user time rises from
5.29 s to 5.68 s for no wall-time gain. The idle is not imbalance and not
barrier overhead — it is Amdahl. The serial fraction inverted from the measured
speedup is 0.621, close to the 0.687 PRD 0006 derived independently, and a
serial fraction near 0.65 caps speedup at about 1.5× at any core count.

Three consequences:

- **No further parallel-side work can pay.** Tile tuning, worker tuning and
  rebalancing are closed off.
- **The default worker budget is wrong** — `available_parallelism()` gives 10
  where 6 is as fast and uses less CPU.
- **Only serial reduction moves wall time**, which is why PRD 0005 removed ~590
  samples of confirmed serial work for a 3% wall gain.

**Order revised 2026-07-27.** Threading was step 1, scoped as a standalone PRD
on the argument that it was structurally independent and multiplicative. It was
implemented and measured, and the measurement disproved that
(`docs/evidence/evaluator-parallel-element-wise-20260727/`): parallelism gives
4.5–5.6× in isolation at 1M rows but **does not pay applied per expression
node**, because a tick evaluates ~127 nodes and the spawn/join cost exceeds the
saving. The implementer set the threshold above the binding case and the
headline was 1.00×.

Threading is therefore *not* independent — it needs one parallel region per tick
over row ranges, which is what tiling creates. The two are now one PRD. Tiling
also stands on its own at 2.2×, so it leads.

The general lesson, which cost an hour to learn: **an isolated speedup can fail
to survive contact with the real granularity.** A spike measures whether a
mechanism is fast; it does not measure whether the surrounding structure lets
you use it.

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

**Device-side observation is now scoped** — `docs/prds-device-observation/`,
with `0001` for ungrouped views and `0002` for grouped. `0002` is the one that
matters: the driver model's calibration and validation outputs *are* grouped
views, and eligibility is all-or-nothing per run, so `0001` alone leaves the
real workload downloading the state anyway.
The 2026-07-27 profile shows 81% of CUDA wall time exists to serve host-side
observation, and every reduction in the benchmark model is `count` or `max` over
`int`, so a device reduction is bit-identical by construction. `Sum` over `Real`
must stay on the host, and eligibility is a property of the whole run: one
host-bound view means the state is downloaded anyway.

**After device observation**: `prds-evaluator-throughput/0008` generalises the
tiling constants. `TICK_TILE_ROWS = 1_024` and `TICK_TILE_THRESHOLD = 1_000_000`
were both measured on `demographic_slots` and neither derives anything from the
model. The tile size assumes this model's ~20 KB live set fits L1; a model with
many more views would spill and tiling would *hurt*. The threshold sits exactly
at the benchmark's row count while `threading_spike` measured parallelism paying
from ~262k rows. 0008 derives both from the IR and adds a second benchmark shape
so the problem cannot recur.

**Not queued**: the RNG change (deferred, see above), and device-side reduction
of the `wins`/`deferred` arrays — worth 22% and needing no semantic decision,
but deliberately separate so its measurement stays interpretable.

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

**A ratio is not a goal, and §L4 is now retired.** It asked whether the GPU
beats the CPU by 3×. Optimising the CPU *lowers* that ratio while improving the
product — and on 2026-07-27 it did exactly that, flipping to **1.914× NOT MET**
while CUDA improved 1.21× and CPU 2.65× (`DECISIONS.md` §L9). A criterion whose
verdict inverts because unrelated work improved is measuring the wrong thing.
Use absolute wall time and the per-phase split instead.

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
