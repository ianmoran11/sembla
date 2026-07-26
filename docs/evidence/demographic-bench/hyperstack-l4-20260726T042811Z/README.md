# CUDA validation §L4 frozen benchmark evidence

This directory records one frozen-case session on one host at repository commit
`206900c25e5124039f95b1d64d0f3a60dbd8ed7d`. The collector asserted that every arm used binary SHA-256
`a8a6743cbc6971caadbc900691cf65970dd2b48d8e387283051783a90397631e` and initial-state SHA-256 `02934c1f4161ced37395e82dacf64039cdb99f1d12434e83c5a87f0b07c9b57c`; it aborted if the commit,
binary, or state changed.

## Gate result

- CUDA no-grouped replicates: 171.200, 169.410, 178.430 s; median **171.200 s**; spread 169.410–178.430 s.
- CPU no-grouped replicates: 433.500, 438.710, 445.550 s; median **438.710 s**; spread 433.500–445.550 s.
- Same-host CPU-median / CUDA-median ratio: **2.563×**.
- §L4 verdict: **NOT MET** (required: CUDA at least 3× faster).

## Ageing share

Paired full/no-ageing CPU replicates produce ageing shares
12.17%, 11.58%, 13.30%; median **12.17%**;
spread 11.58%–13.30%. This **strengthens**
the existing evidence for the §K2 10% trigger. It does **not** decide §K2.

`bench-results.json` is the machine-readable record. `bench-results.md` lists all
raw replicate timings. GPU, CPU, and RAM provenance are in the three named
`*-provenance.txt` files. Verify the directory with `sha256sum -c SHA256SUMS`
(or `shasum -a 256 -c SHA256SUMS` on macOS).
