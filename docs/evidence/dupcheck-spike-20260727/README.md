# Double-write detection spike — 2026-07-27

## Finding

**PRD 0005 made `detect_double_writes` about 12× slower.** It replaced a sort
with a `HashMap`, and on realistically-shaped input the map costs 26.6 ms where
the sort cost 2.2 ms. A bitmap — which PRD 0005's own §2 suggested before the
implementation chose the map — costs 1.5 ms.

| method | ms | vs hashmap |
|---|---:|---:|
| sort (pre-0005) | 2.16 | 12.3× |
| hashmap (post-0005) | 26.57 | 1.00× |
| **bitmap (proposed)** | **1.48** | **18.0×** |

800,000 writes over 1,000,000 rows and 10 destination columns. All three arms
assert the same duplicate/no-duplicate answer.

## How this was found

`host-profile-20260727` and a fresh profile at HEAD bracket PRDs 0005 and 0006.
Every cost those PRDs targeted fell:

| symbol | before | after | Δ |
|---|---:|---:|---:|
| `merge_sort` | 261 | 5 | −256 |
| `_platform_memcmp` | 238 | 39 | −199 |
| `state::locate_writable_cell` | 135 | 0 | −135 |

But hashing appeared where the sort had been:

| symbol | samples |
|---|---:|
| `core::hash::sip::Hasher` | 450 |
| `core::hash::BuildHasher::hash_one` | 306 |
| `hashbrown::HashMap::insert` | 106 |
| **total** | **862** |

862 against the sort's 261. PRD 0005 removed roughly 590 samples of confirmed
work and added back roughly 600, which is why it returned only 3% of wall time
despite doing exactly what it said.

## Why

Rust's default `HashMap` uses SipHash-1-3 — a keyed, DoS-resistant hash — for a
key of four small integers, hashed 800,000 times per tick. The strength is
irrelevant here: the keys are internal indices, not attacker-controlled input.

## Two measurement errors, both recorded deliberately

**The data shape had to be right, and twice it was not.**

The first attempt scrambled row order (`(i * 7919) % rows`). That is merge
sort's worst case, and it reported the sort at 109 ms — five times its real
cost — making the `HashMap` look like an improvement. The executor pushes writes
per transition in ascending row order, so the array arrives as a small number of
ascending runs, which Rust's merge sort exploits.

The second attempt fixed the ordering but seeded duplicates by accident, because
`attr = t % columns` collides when transitions exceed columns. Every arm
short-circuits on the first duplicate, so all three timed partial passes.

Both are recorded because a spike with the wrong data shape produces a confident
wrong answer, which is exactly what `DECISIONS.md` §M1 exists to guard against.
The scaling spike the same day had a third error of the same family — a string
`min()` over run times.

## Caveat on absolute numbers

The spike's key is `(u16, u16, u32)`; the product's is wider, and the product's
sort carries more indirection. Scaled to 24 ticks the spike gives ~50 ms for the
sort against the profile's ~261, and ~580 ms for the map against ~862. The
direction and the ratio hold; the absolute values do not transfer.

## Consequence

`prds-evaluator-throughput/0007` replaces the map with a bitmap. That recovers
the regression and improves on the original sort as well.

Note that the bitmap detects a collision but does not name the colliding pair.
That is acceptable: the pair is needed only when raising `DoubleWrite`, which
terminates the tick, so a linear scan on that path costs nothing in the common
case.

Scratch memory is one bit per cell over the destination columns actually
written — 1.2 MiB at 1M rows and 10 columns, reused across ticks. It should be
sized to the destinations in use, not to every column in the model.
