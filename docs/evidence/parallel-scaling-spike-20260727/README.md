# Parallel scaling spike — 2026-07-27

## Why this spike exists

`docs/evidence/host-profile-20260727/` recorded `__ulock_wait` at 347 samples —
worker threads idle at the scoped-thread join. PRDs 0005 and 0006 then landed,
and a fresh profile shows that number **completely unchanged at 348**, while
every cost those PRDs targeted fell.

`DECISIONS.md` §M1 forbids scoping a PRD from a profile share, and the cause of
the idle was unknown: load imbalance, barrier overhead, or an uneven final tile
were all plausible. This spike measures it instead of guessing.

## What PRDs 0005 and 0006 actually removed

Same case, same inputs, before and after:

| symbol | before | after | Δ |
|---|---:|---:|---:|
| `core::slice::sort::merge_sort` | 261 | 5 | **−256** |
| `_platform_memcmp` | 238 | 39 | **−199** |
| `state::locate_writable_cell` | 135 | 0 | **−135** |
| allocator (`free`, `madvise`, `Vec`) | ~800 | ~450 | ~−350 |
| `__ulock_wait` | 347 | 348 | **+1** |

Wall time 5.88 s → 5.46 s. Both PRDs did exactly what they claimed. The join
idle is untouched by either, which is what made it worth measuring separately.

## Scaling measurement

Binding case, `SEMBLA_EVAL_THREADS` swept, three runs each, fastest reported:

| workers | wall (s) | user (s) | speedup | efficiency |
|---:|---:|---:|---:|---:|
| 1 | 6.60 | 5.32 | 1.00× | 100% |
| 2 | 5.53 | 5.32 | 1.19× | 60% |
| 4 | 4.57 | 5.23 | 1.44× | 36% |
| **6** | **4.34** | 5.29 | **1.52×** | 25% |
| 8 | 4.42 | 5.56 | 1.49× | 19% |
| 10 | 4.35 | 5.68 | 1.52× | 15% |

**Scaling saturates at six workers.** Beyond that, wall time is flat and user
time rises from 5.29 s to 5.68 s — cores are burned for nothing.

## Diagnosis

**The join idle is not a defect. It is Amdahl's law.**

Inverting the measured speedup gives a serial fraction of **0.621**, close to the
0.687 that PRD 0006's evidence derived independently. A serial fraction of ~0.65
caps speedup at roughly 1.5× regardless of core count, and 1.52× is what was
measured.

Workers wait at the barrier because **there is little parallel work left
relative to the serial part** — not because the tiles are unbalanced and not
because the barrier is slow. Rebalancing tiles or coarsening tasks would change
nothing.

## What the serial part is

Of 3,917 main-thread samples against 2,632 worker samples, inside
`execute_tick`'s 3,263:

| | samples |
|---|---:|
| `execute_tick_state` | 1,439 |
| `observe_tick` → `observe_views` | 148 |
| `state_hash` + `state_artifact_hash` (SHA-256) | 236 |

`execute_tick_state` — candidate staging, conflict resolution, write
application — remains the serial core.

## Consequences

1. **No further parallel-side work can pay.** Tile tuning, worker tuning and
   rebalancing are all closed off by this measurement.
2. **The default worker budget is wrong.** It is `available_parallelism()`, or
   10 on this host, where 6 is as fast and uses less CPU. Lowering it costs
   nothing and returns four cores to the machine.
3. **Only serial reduction moves wall time.** That is now the single lever, and
   it explains why PRD 0005 removed ~590 samples of confirmed serial work for a
   3% wall gain: the serial part is large, so removing a piece of it moves the
   total only a little.

## Method note

The first attempt at this measurement was wrong and is not reported above. It
aggregated run times with a string `min()`, so `"10.69"` sorted before `"4.35"`
and the ten-worker row appeared to show a catastrophic regression. The numbers
here use numeric aggregation. Recorded because a plausible-looking artefact of a
measurement script is exactly the failure §M1 exists to guard against.
