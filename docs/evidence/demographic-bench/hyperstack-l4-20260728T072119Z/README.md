# CUDA validation §L4 frozen benchmark evidence

This directory records one frozen-case session on one host at repository commit
`04ada45cb2c8e0b365bae4d4d3c1415b111ed7eb`. The collector asserted that every arm used binary SHA-256
`80ffa27998c4946edf43fc091c42e218c7ba6fc5ba3fe0cce718c728d07ed41c` and initial-state SHA-256 `02934c1f4161ced37395e82dacf64039cdb99f1d12434e83c5a87f0b07c9b57c`; it aborted if the commit,
binary, or state changed.

## Gate result

- CUDA no-grouped replicates: 13.900, 14.100, 14.240 s; median **14.100 s**; spread 13.900–14.240 s.
- CPU no-grouped replicates: 49.070, 49.040, 50.070 s; median **49.070 s**; spread 49.040–50.070 s.
- Same-host CPU-median / CUDA-median ratio: **3.480×**.
- §L4 verdict: **MET** (required: CUDA at least 3× faster).

## Ageing share

Paired full/no-ageing CPU replicates produce ageing shares
40.38%, 41.13%, 41.28%; median **41.13%**;
spread 40.38%–41.28%. This **strengthens**
the existing evidence for the §K2 10% trigger. It does **not** decide §K2.

`bench-results.json` is the machine-readable record. `bench-results.md` lists all
raw replicate timings. GPU, CPU, and RAM provenance are in the three named
`*-provenance.txt` files. Verify the directory with `sha256sum -c SHA256SUMS`
(or `shasum -a 256 -c SHA256SUMS` on macOS).
