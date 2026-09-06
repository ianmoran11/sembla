# CPU runtime follow-up — 2026-09-05

This pass starts from the changed executable in the [first CPU review](cpu-runtime-review-2026-09-05.md), not the original Git baseline. It targets the remaining grouped-observation loop, candidate/write-buffer copies, and aggregate-cache copies. Measurements use the same Apple M2 Pro, Rust 1.79.0, seed 9009, and 1,000,000-row demographic state as that review.

## Results

| Workers | Model | Fastest wall, baseline → final | Median wall, baseline → final | Minimum user CPU, baseline → final |
|---:|---|---:|---:|---:|
| 10 | Demographic, no grouped output | 2.36s → 1.93s | 2.38s → 1.96s | 3.69s → 3.52s |
| 10 | Demographic, grouped output | 3.62s → 2.47s | 3.65s → 2.53s | 4.92s → 4.06s |
| 10 | SIR | 0.33s → 0.29s | 0.33s → 0.29s | 0.23s → 0.23s |
| 10 | Unique groups | 1.12s → 1.14s | 1.15s → 1.16s | 0.99s → 1.01s |
| 10 | Repeated aggregates | 1.17s → 1.13s | 1.24s → 1.15s | 0.90s → 0.89s |
| 1 | Demographic, no grouped output | 3.94s → 3.42s | 3.95s → 3.52s | 3.17s → 2.97s |
| 1 | Demographic, grouped output | 5.16s → 4.00s | 5.25s → 4.03s | 4.36s → 3.49s |
| 1 | SIR | 0.33s → 0.31s | 0.35s → 0.31s | 0.22s → 0.22s |
| 1 | Repeated aggregates | 1.23s → 1.04s | 1.28s → 1.11s | 0.92s → 0.87s |

At the default worker count, the fastest demographic runs are a further 18.2% shorter without grouped output and 31.8% shorter with it. Median reductions are 17.6% and 30.7%. Single-worker user CPU falls 6.3% and 20.0%, respectively. SIR's minimum user CPU is unchanged; its small wall-time movement is not evidence of an evaluator throughput improvement.

Intermediate executables help locate the gains. Buffer changes alone measured 2.00s ungrouped and 3.20s grouped; adding hash grouping measured 2.42s grouped. The later cache change measured 1.13s on repeated aggregates versus 1.17s immediately before it, with unchanged minimum user CPU at ten workers. These are separate five-run series, so small differences include run-to-run variation; their deltas should not be added together.

The million-distinct-groups case has essentially unchanged wall time, but its median peak RSS rises from 442.8 MB to 497.5 MB (12.4%, decimal MB). Hash-table capacity and sorting storage are a tradeoff for the repeated-key speedup. RSS also varies substantially across the normal runs. No general peak-memory reduction is claimed. No single-worker run crosses the protocol's `wall − (user + sys) > 0.5s` contention threshold. Parallel ratios in the CSV show variability, not proof of an uncontended machine.

## Changes

- Shrink each candidate from 56 to 32 bytes on this 64-bit target by recovering transition identity from its enclosing range and using the already checked entity ID as its row. Reuse the first prepared candidate buffer, retain winner rows directly, and reserve pending-write capacity once per transition.
- Check and apply each box's existing write buffer directly. All boxes still finish staging before duplicate detection, and all duplicate checks finish before any writes are applied. Box declaration order preserves the first duplicate diagnostic; failed application still discards the write buffer.
- Count grouped keys in a hash table, then sort only the distinct keys for publication. Keys fit in `i64` internally, including signed integer bands; convert to the public `i128` representation at output. Widths above `i64::MAX` retain their exact signed-band behavior. Hash seeds cannot affect output order or counts.
- Borrow cached aggregate accumulators instead of copying the entire group vector on each query. Match cache keys by reference and allocate the owned key only on a miss. Structural comparisons still distinguish floating-point bit patterns. Cache insertion accounts for nested aggregates built during a miss.

These changes remove a further 41 production tokens and lower the CPU largest-file limit from 2,248 to 2,245 formatted code lines. No dependencies, public APIs, fixture contracts, or canonical output formats change.

## Method

Five sequential runs per executable, model, and worker setting, timed with `/usr/bin/time -l`. Ten workers is this machine's default; the serial measurements set `SEMBLA_EVAL_THREADS=1`. Other evaluator tuning variables were unset. Builds, tests, and sampling profiles ran outside the timed series. The [raw measurements](cpu-runtime-followup-2026-09-05.csv) include wall/user/system time, peak RSS, variation against the fastest run, and the serial contention flag used by the repository's performance protocol.

The normal shapes use 24 ticks: the demographic `no-grouped` and `full` templates resized in temporary files, and the unchanged SIR example. Two additional temporary models test workloads with different observation behavior:

- **Unique groups:** start from the resized full model, clear transitions, scalar views, and top-level summaries, and replace grouped views with one unfiltered view named `each_slot`, keyed by `person_slot.slot_resource`. Run one tick with grouped observations enabled, producing one million distinct buckets.
- **Repeated aggregates:** start from the resized no-grouped model, clear transitions, grouped views, and top-level summaries, and install twelve scalar views named `count_0` through `count_11`. Each view sums the same aggregate count over `person_slot`, grouped and queried through `slot_resource`, with an always-true aggregate filter. Run 24 ticks. This creates one million accumulator groups and exercises repeated cache hits.

Use the state-generation and normal-run commands in the first review. The exact extra view declarations are:

```json
{"name":"each_slot","table":"person_slot","filter":null,"keys":[{"attr":"slot_resource"}]}
```

```json
{"name":"count_0","table":"person_slot","filter":null,"value":{"kind":"agg","op":{"kind":"count"},"table":"person_slot","on":{"fk_attr":"slot_resource","self_fk_attr":"slot_resource"},"filter":{"kind":"bool","value":true}},"reduce":"sum"}
```

Repeat the second declaration with the twelve distinct names. The unique-groups runs additionally write `--timing-json` outside the compared output tree. No checked-in fixture or example is regenerated.

## Remaining opportunities

A separate final-binary sampling run of the grouped demographic case recorded 2,122 main-thread samples at a nominal 1 ms interval. Its tick timers report 1,409 ms executing ticks and 770 ms observing views across 24 ticks, excluding initial loading and final publication. Sampling can perturb timing, so these figures locate work rather than predict a speedup.

1. **Bounded direct indexing for repeated grouped keys.** Grouped observations still account for 475 main-thread samples. The current loop hashes a tuple for every selected row, even when many rows share a small set of enum/reference/band combinations. A size-bounded dense counter or specialized small-key representation could avoid hashing in eligible models, with the existing path retained for large or unbounded key spaces. Any such path must preserve sorted publication and avoid an allocation proportional to the product of huge domains. See `observe_grouped_views` in `crates/sembla-cpu/src/executor.rs`.
2. **Borrowed inputs and reusable results for full-column integer effects.** The stage evaluator accounts for 171 main-thread samples under arithmetic; 39 of those are the source-column copy. `age_months + 1` still materializes the attribute, constant, and result columns before selecting winners. Borrowing attribute columns and broadcasting constants can reduce that work while keeping whole-column overflow checks and their error order. Evaluating only winners would change observable errors and is not a valid shortcut. See `eval_arithmetic` in `crates/sembla-cpu/src/eval.rs` and effect staging in `executor/staging.rs`.
3. **Fewer prepared-expression temporaries.** Worker stacks repeatedly visit `eval_prepared_rows` and vector construction; the main thread spends 249 samples joining candidate workers. Constants currently become repeated vectors, and each operator creates a result vector. Scalar broadcasts and reuse of owned intermediate buffers are candidates for measurement. Worker samples overlap and recursive frames are counted repeatedly, so they cannot be converted directly into wall-time savings.

Cell-by-cell duplicate checking and write application also remain visible, at 167 and 121 main-thread samples respectively. A future batch-write design should be evaluated against the now smaller cost, while preserving rollback and declaration-ordered diagnostics. Per-candidate claim allocations need a contest-heavy profile before further specialization; they are not a leading cost in this benchmark.

## Correctness and provenance

All 120 timed runs across the baseline, intermediate, and final executables produce byte-identical output sets for their model: result CSV, summary and grouped sidecars, and manifests containing final-state and observation hashes. This includes fresh randomized grouping maps and both worker settings. Regression tests cover independent box-local destinations, duplicate-error ordering before write application, rollback, the full unsigned integer-band width range, nested aggregate insertion, and the existing floating-point cache identity contract.

`./scripts/check-rust.sh` passed architecture and context limits, formatting, both Clippy configurations, all workspace tests, dependency policy, and lockfile checks. `./scripts/check-determinism.sh` passed its run and sweep output comparisons. The context limits use the final formatted-source measurements; an initial gate stopped at the earlier pre-format counts, which were corrected before the complete successful rerun.

Executable SHA-256 identities distinguish the uncommitted implementations, whose Git revision is the same `58e191aa3071310230070fee9d2df965d01681e8`:

| Artifact | SHA-256 |
|---|---|
| Baseline: first-review result | `a077c1977dd1ffced498df2e04ace54fe93b8cd33210d2aac8e68e7aa5dbf66f` |
| Buffer changes | `55e88d915325a852f3c83d97c8b54ce031098fb591085efb47f26cda1e4362a3` |
| Buffer and grouping changes | `22bb58884fd01aacc6c94c2f3fb945585e2e72cb76ad8a86658de16562cac7a8` |
| Final: also borrow aggregate cache | `1ad6f8d824090dbc3753c4b94a02f842d9dfea72698e8f25e6a38627864ce2f5` |
| Unique-groups model | `0ff4b39e65d7dcecdb0baa070e4bc2210a40b48fb86fa7ad843162e7bdbfa25e` |
| Repeated-aggregates model | `491fba0465770322f8b2e7e8ab33b2b944b8e7ecb05fd2158f85f08ac5a44f70` |

Initial state and normal model identities are unchanged from the first review. Raw CSV arms `copies` and `grouping` correspond to the two intermediate executable identities above.
