# PRD 0007: Detect double writes with a bitmap, not a hash map

## Context

Read `docs/prds-evaluator-throughput/README.md` first; its constraints bind.

**This PRD corrects a regression introduced by PRD 0005**, which this folder
approved. That is not a criticism of the implementation: 0005's §2 offered "a
per-cell first-writer map or a per-column bitmap", the implementation chose the
map, and the map satisfied every acceptance criterion including the diagnostic
test. The cost only became visible on the next profile.

`docs/evidence/dupcheck-spike-20260727/` measures the three options on
realistically-shaped input — 800,000 writes, 1,000,000 rows, 10 destination
columns, no duplicates, ascending runs per transition:

| method | ms | vs current |
|---|---:|---:|
| sort (what 0005 removed) | 2.16 | 12.3× |
| **hash map (current)** | **26.57** | 1.00× |
| bitmap (this PRD) | **1.48** | **18.0×** |

The profile agrees. Bracketing 0005 and 0006, `merge_sort` fell 261 → 5 while
SipHash work appeared at 862 samples across `sip::Hasher` (450),
`BuildHasher::hash_one` (306) and `HashMap::insert` (106).

**The cause is the default hasher.** Rust's `HashMap` uses SipHash-1-3, keyed
and DoS-resistant, for a key of four small internal indices hashed 800,000 times
per tick. The resistance buys nothing here — the keys are not
attacker-controlled — and costs roughly 12× the sort it replaced.

## Goal

Double-write detection costs less than the sort that preceded PRD 0005, with the
diagnostic unchanged.

## Specification

### 1. Detect with a bitmap

One bit per `(destination column, row)` over the destination columns actually
written in the tick. Set the bit; if it was already set, a duplicate exists.

No hashing, no ordering, no per-write allocation.

**Size it to the destinations in use, not to every column in the model.** A
model with many columns and few written per tick must not pay for the unwritten
ones. At 1M rows and 10 written columns the scratch is 1.2 MiB.

**Reuse the scratch across ticks.** Reallocating it per tick would reintroduce
the page-churn cost `alloc_spike` measured at 4.04×. Clearing it must also not
cost O(all cells) per tick if only a few are set — either clear only the words
touched, or use a generation counter.

### 2. The diagnostic is unchanged, and this is the criterion

A bitmap detects that a collision happened but not which writes collided. That
is acceptable **only** because the pair is needed solely to raise `DoubleWrite`,
which terminates the tick. On that path, a linear scan to recover the pair costs
nothing in the common case.

The reported pair must remain **the two earliest in push order**, matching both
the pre-0005 stable sort and 0005's map. PRD 0005 added a test with three writes
to one cell asserting exactly this; that test must continue to pass unchanged.

If recovering the pair by linear scan cannot reproduce the same ordering, say so
and stop rather than changing the diagnostic.

### 3. Keep everything else 0005 established

0005 also removed the owned `String` from `PendingWrite` and resolved
destination columns once per effect. Both are confirmed working in the profile —
`locate_writable_cell` 135 → 0, `memcmp` 238 → 39 — and are **not** in scope
here. Do not revert or rework them.

### 4. Measure under the revised protocol

Wall time primary, single-worker figures separate, CPU efficiency, and the
estimated serial fraction before and after.

Also report the hashing symbols from the profile — `sip::Hasher`,
`BuildHasher::hash_one`, `HashMap::insert` — before and after. They should go to
approximately zero. If they do not, the map is still in use somewhere this PRD
did not look.

## Allowed files

- `crates/sembla-runtime/src/executor.rs`
- `crates/sembla-runtime/tests/**` (tests only)
- `docs/evidence/**` (new evidence only)
- `docs/prds-evaluator-throughput/README.md` (status notes only)

**If a required gate fails on files outside this list, stop and report it** —
`DECISIONS.md` §M2.

## Non-goals

No change to which writes happen, to conflict resolution, or to §E3. No revert
of 0005's other two changes. No new dependency — a faster third-party hasher
would also fix this and is **rejected**, because a bitmap is faster still and
the dependency policy is not worth spending here. No tiling of the write path.
No IR, Lean, CUDA, or CLI changes.

## Acceptance criteria

1. `detect_double_writes` performs no hashing and no sort; a grep-based or test
   assertion prevents regression.
2. The scratch is reused across ticks and sized to the destinations actually
   written; a test covers a model where most columns are unwritten.
3. PRD 0005's three-writes-to-one-cell test passes unchanged, naming the same
   pair.
4. **Every golden is byte-identical**, including the manifest and
   `final_state_sha256`.
5. `cargo test --locked` and `scripts/check-rust.sh` green.
6. Before/after under the revised protocol, plus the hashing symbols before and
   after.
7. `python3 scripts/check-markdown-links.py` passes.

## Note on expectations

The spike says 18× on this operation in isolation, and the profile puts the
operation at 862 samples of roughly 6,500. So the ceiling is around 10–12% of
the run, and the realistic gain is below that.

But the parallel-scaling spike established that **only serial reduction moves
wall time**, and this work is serial. Unlike PRDs 0002 and 0004, which acted on
the parallel side and moved wall time by under 1%, this should show up.

If it does not, that is a finding worth more than the change: it would mean the
serial critical path is dominated by something no profile has yet named, and
`execute_tick_state`'s remaining 1,439 samples would be the place to look.

## A note for the folder

Three PRDs in this folder have now had a measured outcome that differed from the
expectation stated in their own scoping: 0001's threading premise, 0004's flat
wall time, and 0005's replacement costing more than what it replaced. In each
case the PRD's own evidence exposed it and the next PRD corrected it.

That is the process working, and it is worth stating plainly so the pattern is
not mistaken for repeated error. The alternative — scoping several PRDs at once
from a single profile — would have compounded all three.
