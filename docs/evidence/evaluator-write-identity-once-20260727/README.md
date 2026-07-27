# Write-identity resolve-once evidence — 2026-07-27

## Scope implemented

The staged write record now carries only a destination slot, row, typed value,
and `rule_id`. It no longer owns a transition-name `String` or repeats model
box/table/attribute indices. A small per-effect destination table stores model
identity and the single `ResolvedWriteColumn` lookup result. Every winner write
for that effect copies only its slot.

`ResolvedWriteColumn` extends the read path's resolve-once design in `state.rs`.
It stores stable table/column declaration indices. `StateStore` keeps the same
schema and declaration order in its current and prepared buffers, so a handle
resolved against the tick-start snapshot addresses the same destination in the
prepared buffer. Valid writes use resolved setters and perform no per-write
table or column name scan.

No gather evaluation, conflict ordering, tiling, RNG, IR, CUDA, CLI, or
dependency behavior changed.

## Double-write equivalence

The old stable sort ordered writes by `(box, table, attr, row)`. Its first
failure was therefore the lexicographically first duplicated cell; stability
made the named writers the first two writes to that cell in push order.

The replacement makes one expected-linear pass with a `HashMap`. The map stores
the first push index for every model-identity `(box, table, attr, row)` cell. A
separate candidate retains the lexicographically smallest duplicated cell. It
is set only for the first duplicate of that cell, so the reported pair remains
the first two push-order writes. Hash-map iteration order is never observed.
Different per-effect destination slots that name the same physical cell still
collide through that model identity.

Transition names are not staged. On the error-only path, each name is derived
from the write's dense `rule_id` through `ValidatedModel::transitions()` and the
validated box/transition indices. Box, table, and attribute names are likewise
looked up from the model. The `DoubleWrite` variant and display text are
unchanged.

## Validation and error order

Each effect value is evaluated before its destination is resolved. The lookup
`Result` is captured once rather than propagated: later effects and transitions
finish staging, `DoubleWrite` detection runs, and the write buffer is prepared
before an error is published at that destination's first pending write. This
matches the original per-write lookup's observable precedence while retaining
the existing unknown-table and unknown-column messages.

Resolved application performs row bounds before type validation, as the old
`locate_writable_cell` path did. Enum variant and Ref target-row checks then run
before mutation with their original cell-qualified messages. A failed
application still discards the prepared buffer.

The precedence corpus proves that:

- `DoubleWrite` beats a captured missing-column error;
- a later effect's integer-overflow error beats an earlier captured destination
  error;
- write-buffer preparation errors beat captured destination errors;
- absent a higher-priority failure, the destination error retains its exact
  type and message and leaves the state reusable;
- out-of-range enum, Ref, and row errors retain their exact messages.

The three-writer fixture still names `first`/rule 0 and `second`/rule 1.
Structural tests pin the String-free staged record, captured rather than early
error publication, the single staging-site resolution, resolved-only setters,
and sort-free linear detector.

## Measurement protocol

Wall time is primary. The frozen case is the no-grouped demographic model with
1,000,000 slots, 24 ticks, seed 9009, four areas, present fraction 0.8, streams
`birth:600,overseas:250,internal:150`, and the CPU backend. The host was an Apple
M2 Pro with 10 logical cores and 16 GiB RAM. Each after mode has five in-run
measurements; single-worker runs set `SEMBLA_EVAL_THREADS=1`.

The exact baseline is PRD 0004's after binary and five-run result. Before
editing, commit `ba41172` rebuilt to SHA-256
`57a7297766debcfc6a88fc4b5743bd18cde90e12207f2c7f61099c652df440d0`,
matching PRD 0004. Commits `25dc293` and `ba41172` changed no runtime source
after the PRD 0004 implementation commit; their non-managed changes are
profiling evidence, the performance model, and PRD documents.

A run is contended when `wall - (user + sys) > 0.5 s`; none was contended.

| Mode | Fastest wall/user/sys s | Median wall/user/sys s |
|---|---:|---:|
| Before, default workers | **4.90 / 5.87 / 1.28** | 4.97 / 5.88 / 1.35 |
| After, default workers | **4.74 / 5.81 / 1.15** | 4.97 / 5.90 / 1.25 |
| Before, one worker | **6.28 / 5.21 / 1.05** | 6.52 / 5.29 / 1.18 |
| After, one worker | **6.30 / 5.20 / 1.07** | 6.45 / 5.24 / 1.12 |

Fastest default wall improved **1.034×**, from 4.90 to 4.74 seconds, a **3.27%
reduction**. Median wall was unchanged at 4.97 seconds. Single-worker headline
wall was effectively flat at 6.28 versus 6.30 seconds, while default headline
user time improved **1.010×**, from 5.87 to 5.81 seconds. This is a measured
partial gain: preserving diagnostic precedence retains a per-effect destination
table and one slot indirection on the hot path.

CPU efficiency is `(user + sys) / (wall * 10 logical cores)` on the fastest
uncontended default run: **14.59% before** and **14.68% after**.

Using PRD 0003's Amdahl inversion,
`S = (1 / (T1 / T10) - 1/10) / (1 - 1/10)`, the estimated serial fraction fell
from **75.58% before** to **72.49% after**. Fastest wall and the serial estimate
move in the intended direction, while the unchanged median records the run
spread rather than overstating the outcome.

## Identity and gates

All 20 inherited-before and new-after outputs are byte-identical:

- primary CSV: `eb6d095740127bbf41576d6b05f1470656dbb5f85372ef2ff5f1751576303e37`;
- summaries CSV: `329bc9e17af3032a81d4dd60263cd70c26fd734e33fce7cfcb8da66558bca6d3`;
- manifest: `dbeaa57719ef88945ac46336ad8033c5cdac91d8cd5b8bb21679437e0122aa1f`;
- stdout: `a034ac5ea499bc5ee82b57c93daf43a8d6580a5d98cf1c0757341aa71fc8154f`;
- `final_state_sha256`:
  `2d509ead9aa506e71be155faaa5608542f7ca32cee203ee42b0d3179d670020c`.

The frozen model/state hashes are
`601766d8c11443cb05da2500b00bb78fade375b8df2d0323bae35b7d8a17a130`
and `896e0062228b74ba24df95e53e28ca368df510f957ed03ef2f49160590a6922b`.
The measured final release binary SHA-256 is
`1f5073e09258a4eb19f579ebf7158b23229046ad8d45e03cc87db47b9005d121`.

The final workspace passed `cargo test --locked`, `scripts/check-rust.sh`,
`cargo fmt --all -- --check`, `git diff --check`, JSON validation, and
`python3 scripts/check-markdown-links.py`. Machine-readable runs, formulas,
hashes, and protocol details are in [`measurements.json`](measurements.json).
