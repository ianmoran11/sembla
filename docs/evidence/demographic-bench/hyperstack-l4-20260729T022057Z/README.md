# CUDA validation §L4 frozen benchmark evidence

This directory records one frozen-case session on one host at repository commit
`5616dbe56cddb26e6a6541bead3572639827a8c2`. The collector asserted that every arm used binary SHA-256
`ce4f568b9301bf268ba615e388b634efee8ce75ce6a8bbe70affaf91f9e9c1f1` and initial-state SHA-256 `02934c1f4161ced37395e82dacf64039cdb99f1d12434e83c5a87f0b07c9b57c`; it aborted if the commit,
binary, or state changed.

## Gate result

- CUDA no-grouped replicates: 6.550, 6.480, 6.500 s; median **6.500 s**; spread 6.480–6.550 s.
- CPU no-grouped replicates: 50.830, 51.310, 50.750 s; median **50.830 s**; spread 50.750–51.310 s.
- Same-host CPU-median / CUDA-median ratio: **7.820×**.
- §L4 verdict: **MET** (required: CUDA at least 3× faster).

## Ageing share

Paired full/no-ageing CPU replicates produce ageing shares
40.76%, 40.67%, 41.03%; median **40.76%**;
spread 40.67%–41.03%. This **strengthens**
the existing evidence for the §K2 10% trigger. It does **not** decide §K2.

`bench-results.json` is the machine-readable record. `bench-results.md` lists all
raw replicate timings. GPU, CPU, and RAM provenance are in the three named
`*-provenance.txt` files. Verify the directory with `sha256sum -c SHA256SUMS`
(or `shasum -a 256 -c SHA256SUMS` on macOS).
