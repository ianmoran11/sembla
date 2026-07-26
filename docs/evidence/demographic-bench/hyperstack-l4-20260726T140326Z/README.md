# CUDA validation §L4 frozen benchmark evidence

This directory records one frozen-case session on one host at repository commit
`917d9309a1a77d465fed3a3133b5f5552244f6db`. The collector asserted that every arm used binary SHA-256
`9d2a222f2d302738001a06070caa9df7ff579cfbfafc1ec84551e8adcf9598b5` and initial-state SHA-256 `02934c1f4161ced37395e82dacf64039cdb99f1d12434e83c5a87f0b07c9b57c`; it aborted if the commit,
binary, or state changed.

## Gate result

- CUDA no-grouped replicates: 31.670, 33.610, 31.820 s; median **31.820 s**; spread 31.670–33.610 s.
- CPU no-grouped replicates: 134.090, 133.860, 133.850 s; median **133.860 s**; spread 133.850–134.090 s.
- Same-host CPU-median / CUDA-median ratio: **4.207×**.
- §L4 verdict: **MET** (required: CUDA at least 3× faster).

## Ageing share

Paired full/no-ageing CPU replicates produce ageing shares
32.08%, 32.80%, 32.97%; median **32.80%**;
spread 32.08%–32.97%. This **strengthens**
the existing evidence for the §K2 10% trigger. It does **not** decide §K2.

`bench-results.json` is the machine-readable record. `bench-results.md` lists all
raw replicate timings. GPU, CPU, and RAM provenance are in the three named
`*-provenance.txt` files. Verify the directory with `sha256sum -c SHA256SUMS`
(or `shasum -a 256 -c SHA256SUMS` on macOS).
