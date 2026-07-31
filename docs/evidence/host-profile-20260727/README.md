# Host profile after PRDs 0001–0004 — 2026-07-27

Full-process `sample` capture of the binding fixed case at
`cfdcd43`+`74cdb63`+`5b8c96c`, taken to re-scope work after the four
`prds-evaluator-throughput` PRDs landed. The previous CPU profile
(`host-profile-20260726/`, and the one embedded in
`host-evaluator-owned-real-move-20260726/`) predates all of them.

## Case and identity

The binding protocol case, with the same inputs the four PRDs measured against:

```text
model:  fixtures/demographic/benchmark/demographic_slots.no-grouped.json (resized)
scale:  1,000,000 slots     ticks: 24     seed: 9009
areas:  4     present fraction: 0.8     streams: birth:600,overseas:250,internal:150
backend: cpu     workers: default (10)
```

- resized model SHA-256 `601766d8c11443cb…` — matches the protocol record
- initial state SHA-256 `896e0062228b74ba…` — matches the protocol record
- `real 5.88s  user 6.12s  sys 1.40s`
- 1 ms interval, full process lifetime, 4,217 main-thread and 2,618 worker
  samples

## What the tick looks like now

Main thread, from the call tree:

| | samples |
|---|---:|
| `execute_tick` | 3,421 |
| ├ `execute_tick_state` (staging/write path) | 1,806 |
| │  └ `stage_box` branches, combined | ~1,561 |
| │     └ `eval_column` → `eval_expr` | 365 → 195 |
| ├ `execute_tick_state` (conflict resolution) → `merge_sort` | 292 → 261 |
| └ other `execute_tick_state` branches | ~1,020 |

Worker threads account for 2,618 samples — the tiled parallel work from 0001
and 0003.

## Top of stack

| symbol | samples | reading |
|---|---:|---|
| `Vec::from_iter` (5 variants combined) | ~1,346 | allocation |
| `Map::fold` | 449 | |
| `rng::draw_u32x4` | 447 | Philox |
| `_platform_memmove` | 425 | |
| `executor::candidate_race_time` | 415 | |
| `__ulock_wait` | 347 | **workers idle at the join** |
| `sha2::compress256` | 328 | final + results hashing |
| `executor::stage_box` | 283 | |
| `slice::sort::merge_sort` | 261 | conflict resolution |
| `_nanov2_free` / `madvise` | 250 / 250 | allocator returning pages |
| `_platform_memcmp` | 238 | string comparison |
| `state::locate_writable_cell` | 135 | write-path column lookup |
| `eval::eval_prepared_tile` | 127 | tiled evaluator |

## Findings

**`log` is gone from the ranking.** It was 554 samples and the largest
non-allocation item before PRD 0004; the degenerate-hazard fast path removed it.
That change is confirmed working here, independently of its own evidence.

**Write staging is now the dominant serial cost.** `stage_box` and its children
are roughly 1,561 of `execute_tick`'s 3,421 samples — about 46% of the tick, on
the main thread. This is precisely what PRD 0003 declined to tile, for a
recorded and correct reason: effects depend on the declaration-ordered winners
that sequential conflict resolution produces. It is the largest single target
and it needs a design idea, not an optimisation.

**Allocation churn is back at the top.** `from_iter` variants ~1,346, plus
`_nanov2_free` 250, `madvise` 250 and malloc paths ~180 — roughly 1,930 samples
aggregated. `alloc_spike` measured a 4.04× ceiling for buffer reuse on buffers
of this size, and that work has never been scoped. Note `DECISIONS.md` §M1:
profiles *understate* allocation cost, because first-touch page-fault time is
attributed to the consumer rather than to `malloc`, so this is a floor.

**Workers spend 347 samples waiting.** `__ulock_wait` is the scoped-thread join.
That is idle time from load imbalance or tasks too fine to amortise the barrier
— worth checking before adding more parallel work, since it caps what further
tiling can return.

**The write path still resolves columns by string.** `locate_writable_cell` 135
plus much of `_platform_memcmp` 238. This is the exact pattern PRDs 0001 and
0002 of `prds-host-evaluator-performance` removed from the *read* path for a
combined ~4×; it was identified on 2026-07-27 and has never been scoped.

**Philox remains at 447.** Irreducible without SIMD, per `rng_batch_spike`.

## Consequent ranking

1. **Buffer reuse** — largest aggregate, measured ceiling, never scoped.
2. **Write-path column resolution** — small, proven pattern, never scoped.
3. **Write staging / `stage_box`** — largest single item but needs a design
   idea; conflict resolution's sequential winner order is load-bearing.
4. **Join imbalance** — diagnose before extending tiling further.

`docs/performance/model.md` carries the durable version of this.
