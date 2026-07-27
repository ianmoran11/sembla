# Degenerate-hazard fast-path evidence — 2026-07-27

## Scope implemented

Direct Real-literal and Real-parameter hazards retain PRD 0002's guarded racing
clock. Its precomputed filter now also retains the exact
`exp(-(lambda * dt))` threshold. A transition uses the new `AlwaysFires`
strategy only when that threshold is exactly `0.0` and its IR declaration has
no contests. Every contest is conservatively treated as consuming the sampled
time, including key-ordered contests.

The same strategy selection is used by the fixed-tile and whole-column fallback
paths. Contested transitions retain the guarded uniform draw and canonical
`-uniform.ln() / lambda` transform. Row-dependent hazards retain the original
canonical path. No RNG implementation, guarded-filter boundary, tiling,
conflict-resolution, IR, CUDA, or CLI code changed.

## Why skipping the draw is exact

For positive `lambda` and `dt`, the racing-clock test
`-ln(u) / lambda < dt` is equivalent to `u > exp(-(lambda * dt))`. When the
precomputed right-hand side is exactly `0.0`, every sample returned by
`uniform_f64` satisfies the test because that function returns values strictly
inside `(0, 1)`. Excluding zero is load-bearing: if `u == 0` were possible,
`-ln(u)` would be infinite and the candidate would not fire, so the fast path
would be invalid.

The skipped time has no consumer. `Candidate` does not store a race time; the
executor places it only into contest ordering values. Requiring
`transition.contests.is_empty()` derives this fact from the IR and deliberately
rejects even a key-ordered contest rather than relying on the benchmark model's
shape.

Finally, `DECISIONS.md` §E1 makes a draw a pure function of
`(seed, tick, rule_word, entity_id, draw_idx)`. There is no stream state to
advance, so omitting one coordinate cannot perturb any other candidate's draw.
The row-to-`entity_id` conversion still occurs before strategy dispatch, which
preserves `EntityIdOverflow` even when no draw is needed.

## Tests

The runtime tests establish that:

- a `1e300` uncontested transition produces an exact-zero threshold, selects
  `AlwaysFires`, and returns a firing with no sampled time;
- an otherwise identical contested transition selects the guarded strategy and
  retains the exact canonical race-time bits;
- an uncontested transition with a nonzero threshold remains guarded;
- both the guarded rejection path and `AlwaysFires` retain the exact
  `EntityIdOverflow` diagnostic because conversion precedes dispatch;
- PRD 0002's firing-set, race-time-bit, contested-winner, filter-boundary, and
  worker/tile determinism coverage remains green.

## Per-transition `ln` counts

A measurement-only build counted enabled candidates after the positive-hazard
test and counted immediately before each canonical `ln`. It used relaxed atomic
counters because tiled transition tasks run concurrently. The instrumented
binary was built in a separate target directory, was not used for timings, and
was removed before final verification.

| transition | enabled candidates | before `ln` | after `ln` | after fraction |
|---|---:|---:|---:|---:|
| age_monthly | 18,547,052 | 18,547,052 | **0** | **0.000%** |
| clear_event | 279,009 | 279,009 | **0** | **0.000%** |
| birth_activate | 3,721,059 | 91,823 | 91,823 | 2.468% |
| overseas_arrive | 972,512 | 19,280 | 19,280 | 1.982% |
| internal_arrive | 596,491 | 10,560 | 10,560 | 1.770% |
| die_young | 4,700,929 | 4,708 | 4,708 | 0.100% |
| die_adult | 9,081,191 | 27,148 | 27,148 | 0.299% |
| die_old | 4,648,809 | 55,388 | 55,388 | 1.191% |
| emigrate | 18,430,929 | 36,962 | 36,962 | 0.201% |
| internal_depart | 18,430,929 | 45,819 | 45,819 | 0.249% |
| **total** | **79,408,910** | **19,117,749** | **291,688** | **0.367%** |

The path removes 18,826,061 calls, or **98.47% of PRD 0002's surviving `ln`
work**. Every non-degenerate transition's count is unchanged.

## Measurement protocol

Wall time is primary. The frozen case is the no-grouped demographic model with
1,000,000 slots, 24 ticks, seed 9009, four areas, present fraction 0.8, streams
`birth:600,overseas:250,internal:150`, and the CPU backend. The host was an Apple
M2 Pro with 10 logical cores and 16 GiB RAM. Every after mode has five in-run
measurements; single-worker runs set `SEMBLA_EVAL_THREADS=1`.

The exact PRD 0004 baseline is PRD 0003's after binary and five-run measurement.
Before editing, commit `74cdb63` was rebuilt and matched the recorded binary
SHA-256 `0cc340f1db993f064c2220716486d2749c18a1999e5deee8378ba8521593e103`.
This reuses the already frozen measurements without synthesizing a different
baseline.

A run is contended when `wall - (user + sys) > 0.5 s`; none was contended.

| Mode | Fastest wall/user/sys s | Median wall/user/sys s |
|---|---:|---:|
| Before, default workers | **4.81 / 6.10 / 1.20** | 5.10 / 6.21 / 1.37 |
| After, default workers | **4.90 / 5.87 / 1.28** | 4.97 / 5.88 / 1.35 |
| Before, one worker | **6.69 / 5.51 / 1.16** | 6.71 / 5.51 / 1.17 |
| After, one worker | **6.28 / 5.21 / 1.05** | 6.52 / 5.29 / 1.18 |

Fastest default wall changed from 4.81 to 4.90 seconds, a 1.87% regression
within the run spread; median wall improved 2.55%. Headline user time improved
3.77%. The single-worker headline improved 6.13% in wall time and 5.44% in user
time, which directly shows the removed scalar work without attributing scheduler
noise to the fast path.

CPU efficiency is `(user + sys) / (wall * 10 logical cores)` on the fastest
uncontended default run: **15.18% before** and **14.59% after**. The lower ratio
is consistent with less CPU work while headline wall remains effectively flat,
matching the PRD's expectation that the remaining critical path dominates.

## Identity and gates

All 20 before/after default/single outputs remain byte-identical:

- primary CSV: `eb6d095740127bbf41576d6b05f1470656dbb5f85372ef2ff5f1751576303e37`;
- summaries CSV: `329bc9e17af3032a81d4dd60263cd70c26fd734e33fce7cfcb8da66558bca6d3`;
- manifest: `dbeaa57719ef88945ac46336ad8033c5cdac91d8cd5b8bb21679437e0122aa1f`;
- `final_state_sha256`:
  `2d509ead9aa506e71be155faaa5608542f7ca32cee203ee42b0d3179d670020c`.

The measurement-only run produced the same three artifact hashes. The final
workspace passed `cargo test --locked`, `scripts/check-rust.sh`,
`cargo fmt --all -- --check`, `git diff --check`, JSON validation, and
`python3 scripts/check-markdown-links.py`.

Machine-readable runs, counts, hashes, CPU efficiency, and instrumentation notes
are in [`measurements.json`](measurements.json).
