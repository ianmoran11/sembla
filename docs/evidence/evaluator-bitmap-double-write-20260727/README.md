# Bitmap double-write detection evidence — 2026-07-27

## Scope implemented

`detect_double_writes` now uses one bit per written physical destination-column
row. The detector contains no hash map, hasher, or sort. A thread-local
`DoubleWriteScratch` owns four reusable vectors: the per-effect destination-slot
mapping, the unique written-column descriptors, bitmap words, and touched-word
indices.

At the start of a tick, only words recorded in `touched_words` are zeroed. The
vectors retain capacity across ticks and reserve before the write scan, so the
common scan allocates nothing. Columns are deduplicated by their stable
`(box_index, table_index, attr_index)` identity. Each bitmap segment spans only
to that column's highest row written in the tick; unwritten model columns never
receive a segment. The binding model writes five physical columns, so its
1,000,000-row upper bound is 625,000 bytes (610.4 KiB). The PRD's synthetic ten-
column shape remains 1.2 MiB.

A 128-column regression model writes only one column over 130 rows. Its scratch
has one destination column and three words, not 128 segments. Running two ticks
asserts that the bitmap and touched-word pointers and capacities are unchanged,
proving storage reuse.

PRD 0005's String-free `PendingWrite`, deferred once-per-effect destination
resolution, resolved setters, validation order, and write ordering are unchanged.

## Diagnostic equivalence

The bitmap's common pass records the lexicographically smallest duplicated
`(box, table, attr, row)` cell. It intentionally retains no writer identity. If
a duplicate exists, one terminating-path linear scan selects that cell's first
two writes in push order. This exactly reproduces both the pre-0005 stable sort
and PRD 0005's map.

The existing three-writer regression is unchanged and still names `first`
(rule 0) and `second` (rule 1), never the third writer. Hash-map iteration order
and bitmap layout are not observable.

## Hashing-symbol profile

Both profiles use `/usr/bin/sample` at a 1 ms interval over the full binding
process. The before profile is the tracked PRD 0006 profile at
[`../parallel-scaling-spike-20260727/post-0006.sample.txt`](../parallel-scaling-spike-20260727/post-0006.sample.txt).
The after profile is [`after.sample.txt`](after.sample.txt).

| Top-of-stack symbol | Before | After |
|---|---:|---:|
| `core::hash::sip::Hasher::write` | 450 | **0** |
| `core::hash::BuildHasher::hash_one` | 306 | **0** |
| `hashbrown::map::HashMap::insert` | 106 | **0** |
| **Named hashing total** | **862** | **0** |

The after sample contains no occurrence of any of the three symbols. This
confirms that the map was removed rather than merely shifted elsewhere.

## Measurement protocol

Wall time is primary. The frozen case is the no-grouped demographic model with
1,000,000 slots, 24 ticks, seed 9009, four areas, present fraction 0.8, streams
`birth:600,overseas:250,internal:150`, and the CPU backend. The host was an Apple
M2 Pro with 10 logical cores and 16 GiB RAM. Each mode has five in-run
measurements; single-worker runs set `SEMBLA_EVAL_THREADS=1`. No official run
crossed the contention threshold.

The exact baseline is commit `34a5162`. Runtime source is unchanged from the
approved PRD 0006 implementation commit `6a4601c`; the intervening commits add
only scaling and duplicate-check spike evidence. Before editing, the rebuilt
binary still matched PRD 0006 SHA-256
`b7d07ab2902064b2af9f7679608cd2424d06a7f67c748654bd2dd6ca0a0ff37a`.

| Mode | Fastest wall/user/sys s | Median wall/user/sys s |
|---|---:|---:|
| Before, default workers | **4.25 / 5.62 / 0.93** | 4.30 / 5.64 / 0.96 |
| After, default workers | **3.32 / 4.72 / 0.89** | 3.37 / 4.72 / 0.92 |
| Before, one worker | **5.92 / 5.05 / 0.85** | 6.43 / 5.20 / 1.06 |
| After, one worker | **4.78 / 4.08 / 0.69** | 5.05 / 4.15 / 0.87 |

Fastest default wall improved **1.280×**, from 4.25 to 3.32 seconds, a
**21.88% reduction**. Median wall improved **1.276×**, from 4.30 to 3.37
seconds. Single-worker headline wall improved **1.238×**, from 5.92 to 4.78
seconds; default headline user time improved **1.191×**, from 5.62 to 4.72
seconds.

CPU efficiency is `(user + sys) / (wall * 10 logical cores)` on the fastest
default run: **15.41% before** and **16.90% after**.

Using PRD 0003's Amdahl inversion,
`S = (1 / (T1 / T10) - 1/10) / (1 - 1/10)`, the estimated serial fraction fell
from **68.66% before** to **66.06% after**. Primary wall time, user time, and the
serial estimate all move in the intended direction.

## Identity and gates

All 20 before/after outputs are byte-identical:

- primary CSV: `eb6d095740127bbf41576d6b05f1470656dbb5f85372ef2ff5f1751576303e37`;
- summaries CSV: `329bc9e17af3032a81d4dd60263cd70c26fd734e33fce7cfcb8da66558bca6d3`;
- manifest: `dbeaa57719ef88945ac46336ad8033c5cdac91d8cd5b8bb21679437e0122aa1f`;
- stdout: `a034ac5ea499bc5ee82b57c93daf43a8d6580a5d98cf1c0757341aa71fc8154f`;
- `final_state_sha256`:
  `2d509ead9aa506e71be155faaa5608542f7ca32cee203ee42b0d3179d670020c`.

The frozen model/state hashes are
`601766d8c11443cb05da2500b00bb78fade375b8df2d0323bae35b7d8a17a130`
and `896e0062228b74ba24df95e53e28ca368df510f957ed03ef2f49160590a6922b`.
The final release binary SHA-256 is
`ac5e54c60348e38af3a96acff5cf3e4d289db20d43dc6c52586dc06ee7506838`.

The final workspace passed `cargo test --locked`, `scripts/check-rust.sh`,
`cargo fmt --all -- --check`, `git diff --check`, JSON/arithmetic validation,
and `python3 scripts/check-markdown-links.py`. Machine-readable runs, formulas,
hashes, symbol counts, and protocol details are in
[`measurements.json`](measurements.json).
