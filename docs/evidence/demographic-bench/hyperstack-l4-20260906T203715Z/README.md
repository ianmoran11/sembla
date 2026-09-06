# CUDA validation §L4 frozen benchmark evidence

This directory records one frozen-case session on one host at repository commit
`107a686cebbcc4b002aa90b34cbdfb4448405860`. The collector asserted that every arm used binary SHA-256
`8893c16cefd337ae714f62e6c2e181bea4f72d5426c3f38e0269a9b3bfa8b5f1` and initial-state SHA-256 `02934c1f4161ced37395e82dacf64039cdb99f1d12434e83c5a87f0b07c9b57c`; it aborted if the commit,
binary, or state changed.

## Gate result

- CUDA no-grouped replicates: 4.670, 4.670, 4.690 s; median **4.670 s**; spread 4.670–4.690 s.
- CPU no-grouped replicates: 29.520, 30.190, 28.770 s; median **29.520 s**; spread 28.770–30.190 s.
- Same-host CPU-median / CUDA-median ratio: **6.321×**.
- §L4 verdict: **MET** (required: CUDA at least 3× faster).

## Ageing share

Paired full/no-ageing CPU replicates produce ageing shares
44.89%, 45.61%, 46.97%; median **45.61%**;
spread 44.89%–46.97%. This **strengthens**
the existing evidence for the §K2 10% trigger. It does **not** decide §K2.

`bench-results.json` is the machine-readable record. `bench-results.md` lists all
raw replicate timings. GPU, CPU, and RAM provenance are in the three named
`*-provenance.txt` files. Verify the directory with `sha256sum -c SHA256SUMS`
(or `shasum -a 256 -c SHA256SUMS` on macOS).
