# Host evaluator Ref resolve-once evidence — 2026-07-26

This directory records the local before/after measurements and full-duration
post-change profile for
`docs/prds-host-evaluator-performance/0002-resolve-reference-columns-once.md`.

## Fixed case

The binding README case was used without substitution:

```text
model:  fixtures/demographic/benchmark/demographic_slots.no-grouped.json
scale:  1,000,000 slots      ticks: 24      seed: 9009
areas:  4    present fraction: 0.8    streams: birth:600,overseas:250,internal:150
backend: cpu
```

A single synthesized state and resized companion model were shared by every
run. Their SHA-256 digests and both release-binary digests are recorded in
`measurements.json`. The before binary was built from baseline commit
`2a9dac41de05371197cfd855ca4638c64f788c1e`; the after binary was built from
that checkout plus only the PRD 0002 implementation diff. Each measured
invocation was equivalent to:

```sh
/usr/bin/time -lp target/release/sembla run <resized-no-grouped-model> \
  --seed 9009 --population <shared-1m-state> --backend cpu --ticks 24 \
  --out <per-run-output.csv>
```

## Results

| Build | Run 1 wall | Run 2 wall | Run 3 wall | Median wall | Median user |
| --- | ---: | ---: | ---: | ---: | ---: |
| Before | 14.93 s | 23.92 s | 17.64 s | 17.64 s | 13.05 s |
| After | 10.83 s | 10.93 s | 10.90 s | 10.90 s | 9.21 s |

The wall-time median improved by **1.62×** (38.21%). Every primary CSV and
summary CSV from all six measured runs was byte-identical; the additional
profiled run produced the same bytes too. The shared hashes are recorded in
`measurements.json`.

## Full-duration post-change profile

`post-change-full-duration.sample.txt` covers the profiled process lifetime,
not a fixed prefix. `/usr/bin/sample` was started before the benchmark with
`-wait -mayDie`; 600 seconds was only an upper bound, and sampling ended when
the benchmark process exited:

```sh
sample sembla-prd0002-profile 600 1 -wait -mayDie \
  -file post-change-full-duration.sample.txt &
target/release/sembla-prd0002-profile run <resized-no-grouped-model> \
  --seed 9009 --population <shared-1m-state> --backend cpu --ticks 24 \
  --out <profile-output.csv>
wait
```

The capture includes 8,508 one-millisecond main execution samples. It contains
no `find_cell`, `Snapshot::reference`, or `find_column` frame. Its SHA-256 is
`522df27f0ae8a4f8ed88934e040aa411b8dc0d32abe6edd62a03c4e715bb211d`.
