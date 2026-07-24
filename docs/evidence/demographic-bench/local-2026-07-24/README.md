# Local demographic benchmark evidence — 2026-07-24

Machine class: Apple M2 Pro (`arm64`), 16 GiB RAM, CPU-only,
moderate-memory local development machine. No hostname or workspace path is
recorded.

Command:

```sh
scripts/bench-demographic.sh \
  --scales 10000,100000,1000000 \
  --seed 9009 \
  --ticks 24 \
  --out docs/evidence/demographic-bench/local-2026-07-24 \
  --machine-class "Apple M2 Pro, 16 GiB, CPU-only moderate-memory local"
```

- `bench-results.json` is the machine-readable evidence.
- `bench-results.md` is the rendered table.

These are single-run local measurements rather than a regression gate. The
10M/50M CPU and CUDA measurements remain pending on the hardware specified in
`docs/demographic-benchmark.md`.
