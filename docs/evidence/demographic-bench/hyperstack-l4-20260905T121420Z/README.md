# CUDA validation §L4 frozen benchmark evidence

This directory records one frozen-case session on one host at repository commit
`4458a9de2f7477b34d0bb5b7585685185e65ea63`. The collector asserted that every arm used binary SHA-256
`600b25321628166d6535f3064d9611a73c7df7667ef72468300568e00d8e80fa` and initial-state SHA-256 `02934c1f4161ced37395e82dacf64039cdb99f1d12434e83c5a87f0b07c9b57c`; it aborted if the commit,
binary, or state changed.

## Gate result

- CUDA no-grouped replicates: 6.280, 6.320, 6.370 s; median **6.320 s**; spread 6.280–6.370 s.
- CPU no-grouped replicates: 52.860, 49.940, 49.930 s; median **49.940 s**; spread 49.930–52.860 s.
- Same-host CPU-median / CUDA-median ratio: **7.902×**.
- §L4 verdict: **MET** (required: CUDA at least 3× faster).

## Ageing share

Paired full/no-ageing CPU replicates produce ageing shares
42.66%, 42.76%, 41.67%; median **42.66%**;
spread 41.67%–42.76%. This **strengthens**
the existing evidence for the §K2 10% trigger. It does **not** decide §K2.

`bench-results.json` is the machine-readable record. `bench-results.md` lists all
raw replicate timings. GPU, CPU, and RAM provenance are in the three named
`*-provenance.txt` files. Verify the directory with `sha256sum -c SHA256SUMS`
(or `shasum -a 256 -c SHA256SUMS` on macOS).
