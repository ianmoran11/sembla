# Raw local sweep timing records

These files back the tables in
[`../sweep-backend-reuse-20260728.md`](../sweep-backend-reuse-20260728.md).
They are retained in-repository so the reported medians can be recomputed.

- `baseline-1m-20x24-timing.json`: baseline binary, exact 1M/24-tick/20-draw
  run. A Python supervisor recorded each completed `draw_N.csv` using the same
  3 ms polling method implemented by the documented `BENCH_SWEEP` collector.
- `retained-1m-20x24-timing.json`: matching retained-backend run using the
  same external completed-file observer and timing boundary.
- `retained-1m-native-timing.json`: supplementary opt-in
  `sembla-sweep-timing-v1` output, which separates setup and draw execution but
  deliberately is not compared directly with the external baseline.
- `baseline-10m-clone-isolation.json` and
  `retained-10m-clone-isolation.json`: matching external-observer records for
  the explicitly labeled zero-tick 10M clone/reset isolation case.
- `retained-10m-native-timing.json`: supplementary native timing for that
  isolation run. The 10M records are not 24-tick scientific-run evidence.

The matching 1M command was:

```bash
/usr/bin/time -p "$BIN" sweep /tmp/sweep-prd-baseline/no-grouped-1m.json \
  --population /tmp/sweep-prd-baseline/initial-1m.state \
  --seed 9009 --draws 20 --ticks 24 --noise independent --backend cpu \
  --timing-json /tmp/sweep-timing.json --out /tmp/sweep-output
```

The baseline binary predates `--timing-json`; use the
`sweep_measure_observed` function in
[`../../../spikes/precision/infra-hyperstack/run-demographic-benchmark.sh`](../../../spikes/precision/infra-hyperstack/run-demographic-benchmark.sh)
for the exact external observation procedure. The implementation binary was
built from the recorded dirty worktree at HEAD
`c0acc2c03d0676750178686c029ffc0ecdadc0ea`; its content hash, not the HEAD
alone, identifies it.
