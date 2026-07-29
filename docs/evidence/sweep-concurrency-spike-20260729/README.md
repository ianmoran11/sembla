# Concurrent sweep draw spike — preliminary CPU evidence

This directory records the first direct same-result arms for the concurrent-draw
candidate scoped in `docs/prds-sweep-throughput/README.md`. It is **preliminary
spike evidence**, not a completed §M1 gate and not evidence for a default worker
count.

## Provenance

- Source HEAD: `5616dbe56cddb26e6a6541bead3572639827a8c2`.
- The concurrency spike implementation was uncommitted; each raw JSON records
  the complete dirty-worktree status.
- Release binary SHA-256:
  `48dc4fa8db73d62e11f1e54f7817ac97299aa639994fe2b7972bba9b4570bbd0`.
- Host: Apple M2 Pro Mac mini (`Mac14,12`), 10 logical CPUs, 16 GiB RAM.
- OS: macOS 15.5 (24F74), Darwin 24.5.0 arm64.
- Backend: CPU, fixed total logical-worker budget 10.
- Model: synthesized `demographic_slots.full`, four areas, present fraction
  `0.8`, streams `birth:600,overseas:250,internal:150`.
- Protocol: 20 draws, 24 ticks, seed 9009, independent noise, grouped
  observations enabled.
- Arms: outer draw concurrency 1/2/4; inner evaluator workers 10/5/2. The
  four-lane arm deliberately uses eight rather than oversubscribing past the
  ten-worker budget.

The runnable driver was:

```sh
python3 scripts/run-sweep-concurrency-spike.py \
  --binary target/release/sembla \
  --model <synthesized-model.json> --population <synthesized-state> \
  --backend cpu --cpu-total-threads 10 \
  --output-root <new-directory> \
  --workers 1 2 4 --draws 20 --ticks 24 --noise independent \
  --enable grouped-observations
```

## Results

### 100,000 slots

| outer draws | inner workers/draw | whole sweep s | speedup | draws/s | draw p50 ms | draw p95 ms | peak RSS MiB |
|---:|---:|---:|---:|---:|---:|---:|---:|
| 1 | 10 | 12.412 | 1.000× | 1.611 | 617.3 | 634.0 | 163.4 |
| 2 | 5 | 6.646 | 1.868× | 3.009 | 647.8 | 753.2 | 206.0 |
| 4 | 2 | 3.777 | **3.286×** | 5.295 | 735.8 | 746.2 | 307.3 |

Raw record: [`cpu-demographic-100k.json`](cpu-demographic-100k.json).

### 1,000,000 slots

| outer draws | inner workers/draw | whole sweep s | speedup | draws/s | draw p50 ms | draw p95 ms | peak RSS MiB |
|---:|---:|---:|---:|---:|---:|---:|---:|
| 1 | 10 | 132.643 | 1.000× | 0.151 | 6,572.1 | 6,842.2 | 491.6 |
| 2 | 5 | 68.819 | 1.927× | 0.291 | 6,866.8 | 7,189.9 | 781.1 |
| 4 | 2 | 41.191 | **3.220×** | 0.486 | 8,026.8 | 8,740.0 | 1,226.8 |

Raw record: [`cpu-demographic-1m.json`](cpu-demographic-1m.json).

## Correctness and check validity

For both scales:

- workers 1, 2, and 4 produced identical complete scientific output file sets
  and SHA-256 values;
- the comparison includes grouped sidecars, per-draw outputs, summaries, and
  manifests;
- theta and independent replica seeds therefore remained stable by draw index;
- the driver's negative control appended one line to only
  `draw_0.grouped.deaths_cells.csv`; the comparator reported exactly that file
  as changed and returned non-equality.

Separate integration tests also cover contests and grouped observations under
both CRN and independent noise, plus a forced completion-order inversion whose
published output tree remains byte-identical.

## Finding and limits

This is a strong positive **CPU feasibility** result at 100k and 1M: bounded
outer concurrency substantially improves whole-sweep throughput even after the
fixed CPU budget is divided across draws. Four lanes improve whole-sweep wall
by 3.22–3.29× while increasing per-draw latency and peak RSS, the expected
throughput/latency/memory trade.

It does not yet authorize a CPU implementation PRD or a universal count:

- each arm has one replicate;
- 10M, a second model shape, CRN timing, topology/affinity/NUMA controls, and
  adjacent repeated arms remain unmeasured;
- RSS rises with lanes and must be treated as a capacity constraint;
- no CUDA arm has run; CUDA requires separate H100 evidence for contexts,
  streams, compilation cost, VRAM, and actual kernel overlap.

Negative or capacity-limited results at 10M or on CUDA remain valid outcomes.
