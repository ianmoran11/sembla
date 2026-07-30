# Supported concurrent CUDA sweep-draw evidence

This paid H100 session verifies the production `sweep --draw-workers` interface
implemented by PRD
[`0004-run-cuda-draws-concurrently`](../../../prds-sweep-throughput/0004-run-cuda-draws-concurrently.md).
It ran repository commit
`d72057f183d94dd311d21c33c90aaae97200cea6` and did not rerun the unrelated
frozen §L4 gate.

## Verdict

**Hardware criteria 10–13 pass.** The supported, explicit, default-off CUDA
interface preserves exact scientific output and retains the measured
free-running non-blocking-stream speedup:

| slots | workers 1 | workers 2 | speedup | workers 4 | speedup |
|---:|---:|---:|---:|---:|---:|
| 1M | 5.154 s | 4.044 s | 1.275× | 4.032 s | 1.278× |
| 10M | 37.334 s | 23.576 s | 1.584× | 21.265 s | 1.756× |

Values are medians of three independent-noise external wall measurements for
20 draws × 24 ticks. The CRN matrix is correctness-only.

- All 18 independent-noise and 6 CRN complete output-tree comparisons are
  byte-equal to their sequential references.
- All four deliberate negative controls reject their perturbed grouped file.
- Forced completion inversion preserves ascending-`k` publication and byte
  identity.
- A 10M request for 20 workers fails capacity preflight before constructing a
  lane or creating a scientific output directory.
- Nsight Systems records 39,552 kernels, split evenly across non-default
  streams 20 and 21. Kernel overlap is real: 21.183 ms at concurrency ≥2,
  maximum concurrency 2.
- Remote transfer checksums reconcile 3,170/3,170 (the original remote README
  is retained as `README.remote.md`), final local checksums pass, and sanitized
  collector/operator transcripts prove destroy, empty state, zero provider
  orphans, and credential teardown.

See [`ANALYSIS.md`](ANALYSIS.md) for calculations and criterion-by-criterion
assessment. Machine-readable evidence is under `sweep-concurrency/`; the
remote artifact was also delivered on
`evidence/hyperstack-20260730T023904Z`.
