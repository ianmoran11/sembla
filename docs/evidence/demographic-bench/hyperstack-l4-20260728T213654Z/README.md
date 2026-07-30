# CUDA validation §L4 frozen benchmark evidence

This directory records one frozen-case session on one host at repository commit
`ca235b07ec765d71cea2c0bbb4fcf3efcf67a63d`. The collector asserted that every arm used binary SHA-256
`36410342ca6c9afe2f59587e629c5313ec1a4255334da808a60ebda53cecb00a` and initial-state SHA-256 `02934c1f4161ced37395e82dacf64039cdb99f1d12434e83c5a87f0b07c9b57c`; it aborted if the commit,
binary, or state changed.

## Gate result

- CUDA no-grouped replicates: 14.800, 14.910, 15.100 s; median **14.910 s**; spread 14.800–15.100 s.
- CPU no-grouped replicates: 49.820, 50.730, 49.830 s; median **49.830 s**; spread 49.820–50.730 s.
- Same-host CPU-median / CUDA-median ratio: **3.342×**.
- §L4 verdict: **MET** (required: CUDA at least 3× faster).

## Ageing share

Paired full/no-ageing CPU replicates produce ageing shares
41.52%, 40.27%, 39.52%; median **40.27%**;
spread 39.52%–41.52%. This **strengthens**
the existing evidence for the §K2 10% trigger. It does **not** decide §K2.

`bench-results.json` is the machine-readable record. `bench-results.md` lists all
raw replicate timings. GPU, CPU, and RAM provenance are in the three named
`*-provenance.txt` files. Verify the directory with `sha256sum -c SHA256SUMS`
(or `shasum -a 256 -c SHA256SUMS` on macOS).
