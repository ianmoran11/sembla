# CPU runtime review — 2026-09-05

Focused changes to the CPU evaluator, conflict bookkeeping, effect staging, and grouped observations. The release CLI was measured before and after on one Apple M2 Pro (10 logical CPUs), Rust 1.79.0, with five runs per model and worker setting. Baseline: `58e191aa3071310230070fee9d2df965d01681e8`.

Each run uses 1,000,000 persons/slots, 24 ticks, and seed 9009. The demographic state uses four areas, present fraction 0.8, and streams `birth:600,overseas:250,internal:150`. Both demographic templates have `person_slot` and `slot_resource` resized to 1,000,000 in temporary copies. SIR uses `examples/sir.json --population 1000000`. Ten workers is the unmodified default on this machine; single-worker runs set `SEMBLA_EVAL_THREADS=1`. Other evaluator tuning variables were unset.

| Workers | Model | Fastest wall, before → after | Median wall, before → after | Minimum user CPU, before → after |
|---:|---|---:|---:|---:|
| 10 | Demographic, no grouped output | 3.74s → 2.16s | 3.78s → 2.32s | 5.11s → 3.71s |
| 10 | Demographic, grouped output | 7.40s → 3.45s | 7.49s → 3.52s | 8.64s → 4.90s |
| 10 | SIR | 0.31s → 0.31s | 0.31s → 0.32s | 0.22s → 0.22s |
| 1 | Demographic, no grouped output | 5.33s → 3.95s | 5.41s → 4.02s | 4.53s → 3.17s |
| 1 | Demographic, grouped output | 8.97s → 5.20s | 9.08s → 5.23s | 8.07s → 4.38s |
| 1 | SIR | 0.34s → 0.33s | 0.35s → 0.35s | 0.24s → 0.24s |

At the default worker count, the fastest grouped demographic run is 53.4% shorter and the no-grouped run is 42.2% shorter. Their median reductions are 53.0% and 38.6%. SIR is unchanged at this measurement resolution. Single-worker minimum CPU time falls 45.7% and 30.0%, respectively.

[Every measurement](cpu-runtime-review-2026-09-05.csv) includes wall time, user/system CPU time, and peak RSS from `/usr/bin/time -l`. For parallel runs, `wall_over_fastest` exposes variation against the fastest run in that arm/model; it is not a proof of quiescence. No single-worker run crosses the documented `wall − (user + sys) > 0.5s` contention threshold. Runs were sequential and excluded builds, tests, and sampling profiles. These are local measurements, not a hardware-independent performance guarantee. RSS varied substantially, including a larger post-change outlier; no peak-memory reduction is claimed.

Changes:

- Replace candidate-by-table nested Boolean allocations with flat markers; skip conflict bookkeeping when there are no claims. Resource ordering, all-claims-must-win behavior, and once-per-candidate/table counts remain intact.
- Retain the contiguous candidate range for each transition. Effect staging scans each range once and obtains the fired count from the same winners, removing repeated whole-candidate scans and report lookups.
- Resolve grouped key columns on the first selected row, then borrow their typed slices. Reuse a tuple buffer and allocate stored keys only for new buckets. Empty/fully filtered tables keep lazy diagnostics; key order and signed band division remain unchanged.
- Remove permanently disabled element-wise parallel branches and their helpers. Whole-tick tiling remains the parallel execution path. The CPU production-source budget falls by 828 tokens and the largest-file limit falls from 2,382 to 2,248 code lines.

Separate full-duration sampling runs covered the grouped demographic case before and after. The baseline repeatedly sampled column-name lookup and small allocation/free paths under grouped observation and conflict staging. The post-change grouped row loop no longer resolves names or allocates a key for an existing bucket. Timings measure the combined change; they do not isolate the contribution of each optimization.

Correctness:

- All 60 benchmark runs produce byte-identical output sets for their model: result CSV, summaries, grouped CSVs, and manifests, including final-state and observation hashes. This covers both builds and worker settings.
- Regression tests cover repeated losses/wins within a resource table, partial wins across tables, no contests, numeric grouped-key order, negative/extreme integer bands, repeated buckets, filtered rows, empty tables, and lazy column-type errors.
- `./scripts/check-rust.sh` passed: architecture, context limits, formatting, both Clippy configurations, workspace tests, dependency policy, and lockfile checks. The first attempt encountered `NotFound` during temporary-directory cleanup in the unchanged runtime state-artifact fixture test; its isolated retry and the complete gate rerun passed. No fixture was regenerated.
- `./scripts/check-determinism.sh` passed for run results, summaries, manifests, and sweep output trees.

Reproduction: build the release CLI at the baseline and changed revisions and retain both executables. Generate the demographic state with:

```sh
sembla synth-state --model fixtures/demographic/benchmark/demographic_slots.full.json \
  --slots 1000000 --areas 4 --present-fraction 0.8 \
  --streams birth:600,overseas:250,internal:150 --seed 9009 --out /tmp/initial.state
```

Resize temporary copies of the `full` and `no-grouped` templates as described above. Measure each binary five times per model with:

```sh
/usr/bin/time -l sembla run /tmp/no-grouped.json --backend cpu \
  --population /tmp/initial.state --seed 9009 --ticks 24 --out /tmp/run.csv
```

For the full model add `--enable grouped-observations`; for SIR use the original model and integer population above. Repeat with `SEMBLA_EVAL_THREADS=1`. Compare all output files, including manifests and grouped sidecars.

Provenance (SHA-256):

| Artifact | Digest |
|---|---|
| Baseline executable | `82778ef847d5422abd9b6a95e7c79ff56cc3a745c732af48b9bb0df8ad7c9550` |
| Changed executable | `a077c1977dd1ffced498df2e04ace54fe93b8cd33210d2aac8e68e7aa5dbf66f` |
| Initial state artifact | `896e0062228b74ba24df95e53e28ca368df510f957ed03ef2f49160590a6922b` |
| No-grouped model | `de996335a6520b24e25bcfe6cbacdb828d7af974c2b48d3bd0d002d86bfa0ae8` |
| Full model | `33e5745cea824f31cbd7f8aa1f4d03b5aa3caa3cdca03af9f525bbcebb969423` |
| SIR model | `50f05bd8f0269d22f535c02f2fc0cc53ae8ee8bb6938b346a65f5d287c3aea73` |
