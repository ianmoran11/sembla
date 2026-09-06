# CUDA validation §L4 frozen benchmark evidence

This directory records one frozen-case session on one host at repository commit
`f84bb05b4d7d3c9fb4d6ce829570569ab42ac408`. The collector asserted that every arm used binary SHA-256
`5c90fc66d0f66f5111fbde220e6c08db9fd086fd9cebe34c33c265ff756748be` and initial-state SHA-256 `02934c1f4161ced37395e82dacf64039cdb99f1d12434e83c5a87f0b07c9b57c`; it aborted if the commit,
binary, or state changed.

## Gate result

- CUDA no-grouped replicates: 6.200, 6.280, 6.220 s; median **6.220 s**; spread 6.200–6.280 s.
- CPU no-grouped replicates: 50.120, 49.640, 50.990 s; median **50.120 s**; spread 49.640–50.990 s.
- Same-host CPU-median / CUDA-median ratio: **8.058×**.
- §L4 verdict: **MET** (required: CUDA at least 3× faster).

## Ageing share

Paired full/no-ageing CPU replicates produce ageing shares
42.66%, 40.84%, 41.61%; median **41.61%**;
spread 40.84%–42.66%. This **strengthens**
the existing evidence for the §K2 10% trigger. It does **not** decide §K2.

`bench-results.json` is the machine-readable record. `bench-results.md` lists all
raw replicate timings. GPU, CPU, and RAM provenance are in the three named
`*-provenance.txt` files. Verify the directory with `sha256sum -c SHA256SUMS`
(or `shasum -a 256 -c SHA256SUMS` on macOS).
