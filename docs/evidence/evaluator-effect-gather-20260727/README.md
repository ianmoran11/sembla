# Effect winner-row gather evidence — 2026-07-27

## Scope implemented

Effect values now use an explicit ascending-row gather when the expression is
structurally row-local and row-infallible. `eval_gather` and
`eval_typed_ref_gather` prepare the same `PreparedNode` tree used by the tiled
whole-row evaluator. The node evaluator accepts either a contiguous range or an
explicit row list, so arithmetic, coercion, comparison, literal, parameter, and
state-column semantics have one implementation.

`stage_box` preserves candidate and write order. Winner indices are filtered from
the declaration- and row-ordered candidate vector; the lazily materialized row
list therefore stays strictly ascending. Gathered columns are indexed by winner
position, while fallback columns retain absolute-row indexing. No candidate,
claim, conflict, write-identity, destination, or application behavior changed.

## Preserve route and structural eligibility

PRD 0006 §4's **preserve** route was selected. No `DECISIONS.md` change is
needed because diagnostics and semantics remain unchanged.

The existing PRD 0003 `expr_is_row_infallible` predicate is the eligibility
authority. `Expr::Agg` and `Expr::Input` always fall back. Checked integer
`Add`, `Sub`, and `Mul` fall back; the same nodes are gatherable only when type
inference proves their result is Real. Recognized literals, parameters,
self-attributes, Real arithmetic/division, comparisons, boolean operators, and
`EnumIs` are row-local and row-infallible.

Enum literals are validated while the expression is prepared, independently of
which rows are requested. Enum and Ref state columns are validated when the
state is constructed or written; their effect-write range checks still execute
for every staged winner exactly as before. Full-column evaluation did not apply
write validation to non-winners, so gathering these values removes no error.

A two-row regression has one winning row and an overflowing checked-Int effect
at the non-winning row. The tick still reports exactly `integer arithmetic
overflow at row 1`, proving that row-fallible effects retain full-column error
timing. Separate tests pin aggregate/input fallback and bitwise equality between
full and gathered Real, Int, Enum, and Ref values.

## Per-transition evaluated versus used values

Counts are exact for the frozen 24-tick output. A transition with no winners is
not staged. For every active transition, the old path evaluated `1,000,000 ×
effect_count` scalar values. The new path evaluates `winner_count` values for
each gatherable effect and 1,000,000 values for each fallback effect. “Used” is
`winner_count × effect_count`. The fired counts come from the byte-identical
benchmark CSV; eligibility comes from the IR predicate, not transition names.

| Transition | Gather/full effects | Fired | Before evaluated | After evaluated | Used |
|---|---:|---:|---:|---:|---:|
| `age_monthly` | 0 / 1 | 18,547,052 | 24,000,000 | 24,000,000 | 18,547,052 |
| `clear_event` | 1 / 0 | 279,009 | 23,000,000 | 279,009 | 279,009 |
| `birth_activate` | 4 / 1 | 91,823 | 120,000,000 | 24,367,292 | 459,115 |
| `overseas_arrive` | 4 / 1 | 19,280 | 120,000,000 | 24,077,120 | 96,400 |
| `internal_arrive` | 4 / 1 | 10,560 | 120,000,000 | 24,042,240 | 52,800 |
| `die_young` | 3 / 0 | 4,698 | 72,000,000 | 14,094 | 14,094 |
| `die_adult` | 3 / 0 | 27,098 | 72,000,000 | 81,294 | 81,294 |
| `die_old` | 3 / 0 | 55,252 | 72,000,000 | 165,756 | 165,756 |
| `emigrate` | 3 / 0 | 36,843 | 72,000,000 | 110,529 | 110,529 |
| `internal_depart` | 3 / 0 | 45,677 | 72,000,000 | 137,031 | 137,031 |
| **Total** | **28 / 4** | — | **767,000,000** | **97,274,365** | **19,943,080** |

The implementation eliminates 669,725,635 scalar effect evaluations, **87.31%**
of the old total. The 77,331,285 after-values that are not used are explained by
the four checked-Int fallback effects, principally `age_monthly`.

## Measurement protocol

Wall time is primary. The frozen case is the no-grouped demographic model with
1,000,000 slots, 24 ticks, seed 9009, four areas, present fraction 0.8, streams
`birth:600,overseas:250,internal:150`, and the CPU backend. The host was an Apple
M2 Pro with 10 logical cores and 16 GiB RAM. Each mode has five in-run
measurements; single-worker runs set `SEMBLA_EVAL_THREADS=1`. The exact baseline
is approved PRD 0005 commit `7c2d503` and binary SHA-256
`1f5073e09258a4eb19f579ebf7158b23229046ad8d45e03cc87db47b9005d121`.

A preliminary after session was discarded before publication because an
external Chromium workload overlapped it. The official groups below were
collected after that workload ended. No official run crossed the contention
threshold.

| Mode | Fastest wall/user/sys s | Median wall/user/sys s |
|---|---:|---:|
| Before, default workers | **4.74 / 5.81 / 1.15** | 4.97 / 5.90 / 1.25 |
| After, default workers | **4.25 / 5.62 / 0.93** | 4.30 / 5.64 / 0.96 |
| Before, one worker | **6.30 / 5.20 / 1.07** | 6.45 / 5.24 / 1.12 |
| After, one worker | **5.92 / 5.05 / 0.85** | 6.43 / 5.20 / 1.06 |

Fastest default wall improved **1.115×**, from 4.74 to 4.25 seconds, a
**10.34% reduction**. Median wall improved **1.156×**, from 4.97 to 4.30
seconds. Single-worker headline wall improved **1.064×**, from 6.30 to 5.92
seconds; default headline user time improved **1.034×**, from 5.81 to 5.62
seconds.

CPU efficiency is `(user + sys) / (wall * 10 logical cores)` on the fastest
uncontended default run: **14.68% before** and **15.41% after**.

Using PRD 0003's Amdahl inversion,
`S = (1 / (T1 / T10) - 1/10) / (1 - 1/10)`, the estimated serial fraction fell
from **72.49% before** to **68.66% after**. Both primary wall time and the
serial-fraction estimate move in the intended direction.

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
`b7d07ab2902064b2af9f7679608cd2424d06a7f67c748654bd2dd6ca0a0ff37a`.

The final workspace passed `cargo test --locked`, `scripts/check-rust.sh`,
`cargo fmt --all -- --check`, `git diff --check`, JSON/arithmetic validation,
and `python3 scripts/check-markdown-links.py`. Machine-readable runs, formulas,
hashes, eligibility counts, and protocol details are in
[`measurements.json`](measurements.json).
