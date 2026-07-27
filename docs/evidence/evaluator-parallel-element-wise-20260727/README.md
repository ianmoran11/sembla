# Parallel element-wise evaluator evidence — 2026-07-27

## Fixed case and host

The before and after release binaries ran the binding no-grouped demographic
case in one in-run session: 1,000,000 slots, 24 ticks, seed 9009, four areas,
present fraction 0.8, streams `birth:600,overseas:250,internal:150`, and the
CPU backend. The resized model and initial state were shared byte-for-byte.

The host was an Apple M2 Pro with 10 physical/logical cores, 16 GiB RAM, and
macOS 15.5 (24F74). Rust `available_parallelism()` reported 10. The default
worker budget was therefore 10; `SEMBLA_EVAL_THREADS=1` selected the explicit
single-worker measurements. The 1M binding case is below the measured 5M-row
parallel threshold, so both official after corpora intentionally executed on
the calling thread. Above the threshold the implementation uses up to 10
scoped workers on this host.

Each official run used:

```sh
/usr/bin/time -l -o <time-file> <binary> run <resized-no-grouped-model> \
  --seed 9009 --population <shared-1m-state> --backend cpu --ticks 24 \
  --out <run-output.csv>
```

The single-worker after runs prefixed the command with
`env SEMBLA_EVAL_THREADS=1`.

Identity:

- repository commit: `b8c3cb1a44824c30f583ba96237e4d7a763a3a80`
- before binary SHA-256: `1514b954552c5cfa0bb8e7bb2b0a0ed295ab6d56754ffe421c29eed11febd7d9`
- after binary SHA-256: `406edaaf98d21e630b55409d9db72fde58ef0c989ceb53ac24421834c9c615ed`
- resized model SHA-256: `601766d8c11443cb05da2500b00bb78fade375b8df2d0323bae35b7d8a17a130`
- initial state SHA-256: `896e0062228b74ba24df95e53e28ca368df510f957ed03ef2f49160590a6922b`

## Threshold measurement and decision

The repository's bit-checking `threading_spike` was run for 31 repetitions at
small sizes and 21 at larger sizes on the same 10-core host. Representative
medians in milliseconds were:

| Rows | Guard 1 thread | Guard 10 threads | Clock 1 thread | Clock 10 threads |
|---:|---:|---:|---:|---:|
| 32,768 | 0.02 | 0.11 | 0.61 | 0.20 |
| 131,072 | 0.09 | 0.12 | 2.47 | 0.53 |
| 262,144 | 0.45 | 0.19 | 5.28 | 0.99 |
| 1,048,576 | 0.95 | 0.21 | 20.33 | 3.64 |

The isolated maps cross over early, but one scoped region per expression node
is the load-bearing cost in the actual evaluator. A candidate 262,144-row
threshold made the binding 1M/24-tick case regress from 7.30 / 5.81 / 1.38 s
wall/user/sys to 10.19 / 12.08 / 4.21 s. At 2M rows, one paired trial regressed
from 15.18 / 11.93 / 2.81 s to 19.77 / 23.78 / 9.09 s. Those trials used the
final direct-fill fixed-chunk worker design; an earlier chunk-collection design
was worse and was discarded.

The production threshold is therefore 5,000,000 rows: the size at which the
binding spike measured 5.3× guard and 5.6× racing-clock speedups. Below it the
original serial iterators and racing-clock loop remain in use. The threshold
changes only *how* independent rows execute, never their operations or values.
This deliberately reports the per-node scope cost instead of pre-empting the
later tiling PRD's one-region-per-tick design.

Fixed chunks contain 16,384 rows. Their boundaries are determined only by row
index and row count. Worker count changes which worker receives a complete
fixed chunk, not any boundary or output position.

## Official results

A run is flagged contended when `wall − (user + sys) > 0.5 s`.

| Build/mode | Run | Wall s | User s | Sys s | Gap s | Contended |
|---|---:|---:|---:|---:|---:|:---:|
| Before | 1 | 8.03 | 5.91 | 1.57 | 0.55 | yes |
| Before | 2 | 7.65 | 5.95 | 1.51 | 0.19 | no |
| Before | 3 | 8.08 | 6.10 | 1.68 | 0.30 | no |
| Before | 4 | 7.43 | 5.86 | 1.42 | 0.15 | no |
| Before | 5 | 7.30 | 5.81 | 1.38 | 0.11 | no |
| After, default budget 10; threshold kept serial | 1 | 7.25 | 5.79 | 1.36 | 0.10 | no |
| After, default budget 10; threshold kept serial | 2 | 7.25 | 5.79 | 1.40 | 0.06 | no |
| After, default budget 10; threshold kept serial | 3 | 7.31 | 5.82 | 1.33 | 0.16 | no |
| After, default budget 10; threshold kept serial | 4 | 7.23 | 5.83 | 1.31 | 0.09 | no |
| After, default budget 10; threshold kept serial | 5 | 7.38 | 5.82 | 1.45 | 0.11 | no |
| After, explicit single worker | 1 | 7.26 | 5.80 | 1.41 | 0.05 | no |
| After, explicit single worker | 2 | 7.27 | 5.83 | 1.38 | 0.06 | no |
| After, explicit single worker | 3 | 7.59 | 5.86 | 1.55 | 0.18 | no |
| After, explicit single worker | 4 | 7.28 | 5.81 | 1.39 | 0.08 | no |
| After, explicit single worker | 5 | 7.51 | 5.86 | 1.53 | 0.12 | no |

The fastest uncontended headline improved negligibly from **7.30 s wall /
5.81 s user** to **7.25 s wall / 5.79 s user** (1.003× by the load-bearing user
time). Medians were 7.65 / 5.91 s before and 7.25 / 5.82 s after. This near-1×
result is expected because measurement established that the binding 1M case is
too small for one scoped region per node.

The explicit single-worker headline was **7.26 s wall / 5.80 s user**, and its
median was 7.28 / 5.83 s. It therefore shows no hidden loss of serial
efficiency relative to the 7.30 / 5.81 s before headline.

## Result identity

All five before, five after-default, and five after-single runs were identical:

- primary CSV SHA-256: `eb6d095740127bbf41576d6b05f1470656dbb5f85372ef2ff5f1751576303e37`
- summaries CSV SHA-256: `329bc9e17af3032a81d4dd60263cd70c26fd734e33fce7cfcb8da66558bca6d3`
- manifest SHA-256: `dbeaa57719ef88945ac46336ad8033c5cdac91d8cd5b8bb21679437e0122aa1f`
- stdout SHA-256: `a034ac5ea499bc5ee82b57c93daf43a8d6580a5d98cf1c0757341aa71fc8154f`
- `final_state_sha256`: `2d509ead9aa506e71be155faaa5608542f7ca32cee203ee42b0d3179d670020c`

The implementation's determinism tests additionally compare a Real arithmetic
chain and racing-clock values bitwise at 1, 2, and 4 workers. The aggregate test
compares an expression containing an `f64` sum at those worker counts and
asserts structurally that the canonical ascending-row reduction loop contains
no parallel map.

## Why this evidence exists without an implementation (added 2026-07-27)

The PRD run that produced these measurements was stopped without committing.
The cause was procedural, not technical: commit `529f1a2` landed on `main` at
00:29:58 UTC while the run — started 23:40:26 — was in progress. It touched
eight `crates/sembla-runtime/examples/*_spike.rs` files to fix a quality-gate
failure, which put files outside the PRD's allowed list into the diff from the
run's recorded baseline. The reviewer correctly enforced the allowed-file
restriction against a baseline that had moved underneath it, and the run then
exhausted its attempts on a blocker no in-run action could clear.

Attempt 1's review records every other criterion passing: bitwise determinism
across 1, 2 and 4 workers; `f64` reductions and conflict resolution left
sequential; goldens unchanged; `cargo test --locked` and `scripts/check-rust.sh`
green. The implementation itself was sound and is not preserved here.

**The measurement is kept because its finding stands independently of that
mess**, and it redirected the plan: per-node parallel regions do not pay at
realistic scale, so parallelism belongs inside whole-tick tiling rather than in
a PRD of its own. See `docs/performance-model.md`.
