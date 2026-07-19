# CUDA differential corpus — 2026-07-19

Answered from the retrieved evidence bundle in
[`cuda-differential-corpus-20260719/`](cuda-differential-corpus-20260719/).
Every retrieved file's SHA-256 matches the value printed on the remote machine
during the run (`SHA256SUMS` verified locally; the five hashes also match the
operator's session transcript).

- PRD 0009 implementation commit: `8681b4c2d895a7c0c31718f56d5a2281be54fa86`
  (the pushed HEAD; the VM checked out exactly this commit and `provenance.txt`
  records it)
- Provider / flavor: Hyperstack `CANADA-1`, `n3-H100x1`
- GPU: NVIDIA H100 PCIe (full-rate FP64 class)
- Driver: `570.195.03`
- Run (UTC): `2026-07-19T12-19-04Z`
- GPU differential tests: **2 passed, 0 failed**
  (`cuda_manifest_verify_and_level_a_bytes_round_trip`,
  `differential_corpus_passes`) — `tests.log`
- Corpus verdict: **equal — all 11 example models bitwise-equal between the
  CPU oracle and CUDA native `f64`** at `--population 100 --seed 7
  --ticks 20` — `corpus.log`
- Full 26M-row check: `examples/sir.json` with a 26,000,000-person /
  1,300,000-employer synthetic population, seed 77, 1 tick —
  **verdict=equal** (bitwise oracle equality at the ADR workload scale) —
  `full-rate-26m.log`

## Informational throughput (differential mode — not production numbers)

`diff-backends` measures with differential overhead included: NVRTC model
compilation, population upload, and per-tick state download + SHA-256 hashing
on both backends, amortized over the tick count. These rates are therefore
**not comparable** to the ADR 0001 benchmark (resident kernels, 100 measured
ticks, no per-tick downloads):

- Corpus models (population 100, 20 ticks): CPU ~5.9–133k ticks/sec, CUDA
  ~1.4–11.6k ticks/sec — launch overhead dominates at tiny populations, as
  expected.
- 26M rows, 1 tick: CPU 0.247 ticks/sec, CUDA 0.041 ticks/sec — dominated by
  one-time compile/upload and the per-tick hash download.
- ADR full-rate reference for the production kernel shape: **1,380.5
  ticks/sec** at 26M rows
  ([three-run H100 bundle](hyperstack-h100-20260718/README.md)). A
  production-mode (resident, multi-tick, no per-tick download) throughput
  measurement of the *backend* remains future work and must not be inferred
  from the differential-mode rates above.

## Provenance notes

Correctness was established on full-rate hardware, which also satisfies the
weaker "any CUDA-capable NVIDIA GPU" requirement; the informational rates
above make no performance claim. The host-key fingerprint for the retrieval
connection was accepted trust-on-first-use; content integrity is nonetheless
established by the in-session hash printout matching the retrieved files
byte-for-byte. Artifacts were retrieved and hash-verified **before** teardown
(the preflight lesson); destroying the VM (`sembla-precision-9941d2e76482`,
`69.19.140.75`) is the immediate next runbook step after this note lands.
