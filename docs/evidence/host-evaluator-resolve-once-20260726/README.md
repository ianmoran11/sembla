# Host evaluator resolve-once evidence — 2026-07-26

This is the before/after evidence for
`docs/prds-host-evaluator-performance/0001-resolve-column-references-once.md`.
Both release binaries were built and measured on the same Apple M2 Pro in one
session. The baseline binary was built from commit
`f66a920d714d213e00e7d57333412d0af1abaf20`; the post-change binary was built
from the PRD worktree before commit.

## Fixed case

The binding README case was used without substitution:

```text
model:  fixtures/demographic/benchmark/demographic_slots.no-grouped.json
scale:  1,000,000 slots      ticks: 24      seed: 9009
areas:  4    present fraction: 0.8    streams: birth:600,overseas:250,internal:150
backend: cpu
```

A single synthesized state and resized model were shared by every run. Their
SHA-256 digests and both release-binary digests are recorded in
`measurements.json`. Each measured invocation was equivalent to:

```sh
/usr/bin/time -lp target/release/sembla run <resized-no-grouped-model> \
  --seed 9009 --population <shared-1m-state> --backend cpu --ticks 24 \
  --out <per-run-output.csv>
```

## Results

| Build | Run 1 wall | Run 2 wall | Run 3 wall | Median wall | Median user |
|---|---:|---:|---:|---:|---:|
| Before | 49.35 s | 49.66 s | 49.77 s | **49.66 s** | **46.57 s** |
| After | 14.65 s | 15.62 s | 15.81 s | **15.62 s** | **12.57 s** |

The median wall time fell by 68.55% (3.18x), while median user time fell by
73.01% (3.70x). This is a measured result, not a target asserted by the PRD.
All six primary CSV files had SHA-256
`eb6d095740127bbf41576d6b05f1470656dbb5f85372ef2ff5f1751576303e37`;
all six summary CSV files had SHA-256
`329bc9e17af3032a81d4dd60263cd70c26fd734e33fce7cfcb8da66558bca6d3`.

## Post-change profile

`post-change.sample.txt` is a 10-second, 1 ms interval macOS `sample` capture
from an additional post-change run of the same fixed case:

```sh
target/release/sembla run <resized-no-grouped-model> \
  --seed 9009 --population <shared-1m-state> --backend cpu --ticks 24 \
  --out <profile-output.csv> &
sample "$!" 10 1 -file post-change.sample.txt
```

The capture contains no `find_cell` frame. `find_column` remains visible because
column resolution still happens once per column read, as intended; the profile
is retained for scoping the next host-evaluator PRD rather than treating the
speedup as evidence for any unimplemented follow-up.
