# CUDA validation §L4 frozen benchmark evidence

This directory records one frozen-case session on one host at repository commit
`1688e51ac107511908d762e389bb26f4850fd747`. The collector asserted that every arm used binary SHA-256
`a2a6b09b3eb9d453e9534a92e533378b2cbb782b37ffd0b0bcac7016f094305b` and initial-state SHA-256 `02934c1f4161ced37395e82dacf64039cdb99f1d12434e83c5a87f0b07c9b57c`; it aborted if the commit,
binary, or state changed.

## Gate result

- CUDA no-grouped replicates: 6.110, 6.210, 6.060 s; median **6.110 s**; spread 6.060–6.210 s.
- CPU no-grouped replicates: 48.840, 49.920, 50.340 s; median **49.920 s**; spread 48.840–50.340 s.
- Same-host CPU-median / CUDA-median ratio: **8.170×**.
- §L4 verdict: **MET** (required: CUDA at least 3× faster).

## Ageing share

Paired full/no-ageing CPU replicates produce ageing shares
41.86%, 42.51%, 41.28%; median **41.86%**;
spread 41.28%–42.51%. This **strengthens**
the existing evidence for the §K2 10% trigger. It does **not** decide §K2.

`bench-results.json` is the machine-readable record. `bench-results.md` lists all
raw replicate timings. GPU, CPU, and RAM provenance are in the three named
`*-provenance.txt` files. Verify the directory with `sha256sum -c SHA256SUMS`
(or `shasum -a 256 -c SHA256SUMS` on macOS).
