# Tiled tick evaluator evidence — 2026-07-27

## Scope implemented

This PRD deliberately takes the rigorous partial-tiling option allowed by
`docs/prds-evaluator-throughput/0001-tile-the-tick.md`:

- transition guards, hazards, claim resources, and claim keys are prepared once,
  then evaluated in fixed row tiles when the whole transition is aggregate-free,
  input-reduction-free, and cannot raise a row-dependent integer-overflow error;
- racing-clock draws and candidate construction happen inside the same tile task;
- eligible post-commit `Count` view filters are tiled on the calling thread;
- aggregate/input transitions, numeric views, effects, writes, and conflict
  resolution retain the previous column-wise or sequential paths.

There is one `std::thread::scope` in production execution. It is opened once by
transition staging for the tick and distributes complete fixed tile tasks.
Post-commit count observation is a second row-wise phase but is serial, because
it observes committed state and cannot share the tick-start snapshot without
changing Moore-machine semantics.

The old per-expression-node parallel regions were removed. Whole-column
expression maps are serial fallbacks. After review found that prepared parameters
had bypassed the column path's declaration-type check, both paths were routed
through one checked resolver and the after/single-worker/tile/threshold
measurements below were rerun with the revised binary. The recorded before runs
and baseline binary are unchanged.

## Structural determinism argument

For table row count `n` and tile size `z`, tile `i` is always
`[i*z, min((i+1)*z, n))`. Neither the worker budget nor scheduling can change
those boundaries. Workers receive complete pre-existing tasks. Results are
restored by task index and candidates are merged in transition, tile, then row
order.

Within a row, the implementation performs the same IEEE-754 operations and uses
the same Philox coordinates as before. Claim expressions remain eager even when
a guard or hazard prevents firing. Any transition containing an aggregate,
input reduction, or checked integer arithmetic falls back as a whole, preserving
observable error precedence. Prepared and column evaluation share one checked
parameter resolver, so both look up the target-model declaration before accepting
a resolved value and produce the same wrong-type diagnostic.

Real aggregate sums, numeric view reductions, summary reductions, effects,
writes, and conflict sorting/resolution are outside the parallel region. In
particular, the canonical ascending-row `f64` aggregate loop in `eval.rs` is
unchanged.

Tests exercise worker counts 1, 2, and 4 crossed with tile sizes 257, 1,024, and
4,093. They compare:

- a Real arithmetic chain through `f64::to_bits`;
- racing-clock times and Real claim keys through `f64::to_bits`;
- complete tick reports, including race-time and key contests plus a tiled count view;
- a Real aggregate expression classified off the tiled path and evaluated with
  unchanged bits;
- a wrong-typed cross-model parameter environment, proving the tiled and fallback
  paths return the same diagnostic across the full worker/tile matrix.

A source-structure test also asserts that production `executor.rs` contains
exactly one scoped-thread region and production `eval.rs` contains none.

## Tile-size sweep

The sweep used the real 1,000,000-row evaluator for six ticks. Each row is
wall/user/sys seconds. The column arm forces the old column path; single and
parallel arms force tiling with one and ten workers respectively.

| Tile rows | Column | Tiled, 1 worker | Tiled, 10 workers |
|---:|---:|---:|---:|
| 256 | — | 2.25 / 1.89 / 0.35 | 1.74 / 2.05 / 0.35 |
| 512 | — | 2.27 / 1.89 / 0.37 | 1.85 / 2.07 / 0.40 |
| **1,024** | **2.15 / 1.77 / 0.37** | **2.28 / 1.88 / 0.38** | **1.96 / 2.06 / 0.47** |
| 2,048 | — | 2.23 / 1.86 / 0.36 | 1.71 / 1.96 / 0.34 |
| 4,096 | — | 2.31 / 1.87 / 0.40 | 1.82 / 1.98 / 0.42 |

All outputs had CSV SHA-256
`486f528ac450e839d9a47a8adbe0b3175a8a32157e868db6a1b1b1d226f687fc`.

The production constant is 1,024 rows. Larger tiles were slightly faster in
some single runs, but they exceed the L1 target once simultaneously live
expression operands are included. In the benchmark's deepest guard, one 8 KiB
borrowed Int attribute, one 8 KiB materialised literal, comparison results, and
enclosing Bool operands peak conservatively at about **20 KiB**. Final guard,
hazard, and Ref claim roots total at most **13 KiB**. Both fit within the M2
Pro's 32 KiB L1 data cache. Leaf columns are borrowed; only tile intermediates
are materialised.

## Threshold sweep

Three 24-tick runs were made at each size. The table reports fastest user time;
parentheses contain fastest wall time. `Column` forces fallback, `single` forces
tiling with one worker, and `parallel` forces tiling with ten workers.

| Rows | Column | Tiled, 1 worker | Tiled, 10 workers |
|---:|---:|---:|---:|
| 32,768 | 0.17 (0.19) | 0.19 (0.20) | 0.21 (0.15) |
| 65,536 | 0.36 (0.39) | 0.38 (0.43) | 0.42 (0.31) |
| 131,072 | 0.73 (0.84) | 0.79 (0.96) | 0.87 (0.71) |
| 262,144 | 1.54 (1.89) | 1.61 (1.93) | 1.96 (2.17) |

Every mode at every size produced an identical CSV for that size. Two of the
three 262,144-row single-worker runs were flagged contended; the table uses the
uncontended fastest run. Parallel wall time was lower through 131,072 rows but
not in the noisier 262,144-row repeat. The binding protocol makes user time
primary, and forced tiling consumed more single-worker user time through
262,144 rows. No user-time crossover was measured below the binding case. The
production threshold is therefore **1,000,000 rows**: smaller work keeps the
column path, while the binding case itself clears the threshold and exposes the
explicitly reported latency/total-CPU trade-off.

## Official five-run protocol

The fixed case is the no-grouped demographic model with 1,000,000 slots, 24
ticks, seed 9009, four areas, present fraction 0.8, streams
`birth:600,overseas:250,internal:150`, and the CPU backend. All measurements were
made in-run on an Apple M2 Pro with 10 physical/logical cores and 16 GiB RAM.
`available_parallelism()` reported 10.

Each run used:

```sh
/usr/bin/time -l -o <time-file> <binary> run <resized-no-grouped-model> \
  --seed 9009 --population <shared-1m-state> --backend cpu --ticks 24 \
  --out <run-output.csv>
```

Single-worker runs prefixed `SEMBLA_EVAL_THREADS=1`. A run is marked contended
when `wall - (user + sys) > 0.5 s`. No official run was contended. Negative gaps
in parallel runs are expected because user time sums CPU consumption across
workers.

| Build/mode | Run | Wall s | User s | Sys s | Gap s | Contended |
|---|---:|---:|---:|---:|---:|:---:|
| Before, column path | 1 | 7.88 | 6.01 | 1.54 | 0.33 | no |
| Before, column path | 2 | 7.68 | 6.05 | 1.52 | 0.11 | no |
| Before, column path | 3 | 7.71 | **6.00** | 1.53 | 0.18 | no |
| Before, column path | 4 | 8.28 | 6.21 | 1.75 | 0.32 | no |
| Before, column path | 5 | 8.56 | 6.47 | 1.75 | 0.34 | no |
| After, default 10 workers | 1 | 6.48 | 7.00 | 1.57 | -2.09 | no |
| After, default 10 workers | 2 | 5.47 | **6.66** | 1.30 | -2.49 | no |
| After, default 10 workers | 3 | 5.59 | 6.73 | 1.36 | -2.50 | no |
| After, default 10 workers | 4 | 5.77 | 6.75 | 1.44 | -2.42 | no |
| After, default 10 workers | 5 | 5.80 | 6.85 | 1.40 | -2.45 | no |
| After, explicit 1 worker | 1 | 7.65 | 6.17 | 1.43 | 0.05 | no |
| After, explicit 1 worker | 2 | 7.40 | 6.12 | 1.24 | 0.04 | no |
| After, explicit 1 worker | 3 | 7.32 | **6.10** | 1.18 | 0.04 | no |
| After, explicit 1 worker | 4 | 7.34 | **6.10** | 1.23 | 0.01 | no |
| After, explicit 1 worker | 5 | 7.41 | 6.14 | 1.21 | 0.06 | no |

Fastest uncontended runs by the load-bearing user metric:

| Mode | Wall s | User s | Sys s | User ratio vs before | Wall ratio vs before |
|---|---:|---:|---:|---:|---:|
| Before | 7.71 | 6.00 | 1.53 | 1.000× | 1.000× |
| Tiled, 1 worker | 7.32 | 6.10 | 1.18 | **0.984×** | **1.053×** |
| Tiled, 10 workers | 5.47 | 6.66 | 1.30 | **0.901×** | **1.410×** |

Medians were 7.88 / 6.05 / 1.54 seconds before, 7.40 / 6.12 / 1.23 with
one worker, and 5.77 / 6.75 / 1.40 with ten workers.

The separated result is important: partial tiling is essentially flat in
single-worker user time (and 5.3% better in wall time), while parallelism adds a
material wall-time reduction but consumes more aggregate CPU. This does **not**
reproduce the hand-written spike's 2.2× serial result. The initial scalar
row-interpreter shape was discarded after it regressed badly; moving expression
dispatch outside tile row loops and borrowing leaf slices recovered the result
to near-flat. The remaining gap is attributable to interpreter dispatch and
per-node tile allocations, plus the intentionally untiled numeric-view/effect
paths. Those are inputs to the later full whole-tick tiling scope, not hidden as
a claimed serial speedup here.

## Acceptance gates

Final-workspace gates passed:

- `cargo test --locked`;
- `scripts/check-rust.sh` (formatting, Clippy with warnings denied, tests,
  dependency policy, and lock checks);
- `python3 scripts/check-markdown-links.py` — 118 local links in 168 tracked
  Markdown files;
- `git diff --check`.

The full locked suite includes the checked example/CSV/hash goldens, frozen
demographic state artifact, run-manifest `final_state_sha256` checks, and CUDA
oracle/differential membership tests. A path-scoped diff check additionally
confirmed no tracked file under `examples/**`, `fixtures/**`,
`frontend/Fixtures/**`, or the pre-existing CUDA differential evidence changed.
No golden was regenerated or moved.

## Identity

Identity inputs and binaries:

- baseline commit: `44142f3`;
- before binary SHA-256:
  `406edaaf98d21e630b55409d9db72fde58ef0c989ceb53ac24421834c9c615ed`;
- after binary SHA-256:
  `5b93233b42429da08021eb8eb2945465505f5ca13ee0be680437da6616b86fb8`;
- resized model SHA-256:
  `601766d8c11443cb05da2500b00bb78fade375b8df2d0323bae35b7d8a17a130`;
- initial state SHA-256:
  `896e0062228b74ba24df95e53e28ca368df510f957ed03ef2f49160590a6922b`.

All five before, five after-default, and five after-single outputs matched:

- primary CSV SHA-256:
  `eb6d095740127bbf41576d6b05f1470656dbb5f85372ef2ff5f1751576303e37`;
- summaries CSV SHA-256:
  `329bc9e17af3032a81d4dd60263cd70c26fd734e33fce7cfcb8da66558bca6d3`;
- manifest SHA-256:
  `dbeaa57719ef88945ac46336ad8033c5cdac91d8cd5b8bb21679437e0122aa1f`;
- stdout SHA-256:
  `a034ac5ea499bc5ee82b57c93daf43a8d6580a5d98cf1c0757341aa71fc8154f`;
- `final_state_sha256`:
  `2d509ead9aa506e71be155faaa5608542f7ca32cee203ee42b0d3179d670020c`.

The complete machine-readable tile sweep, threshold sweep, official runs,
contention flags, medians, constants, and hashes are in `measurements.json`.
