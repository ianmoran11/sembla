# Pruned raw benchmark data

**Pruned 2026-08-18.** This directory retains the analysis, summaries, provenance,
and integrity records for 27 demographic benchmark runs collected 2026-07-24 to
2026-07-30. The raw per-sample data was removed: 17320 files, 561 MB.

## What was removed

| Kind | Reason |
|---|---|
| `*.csv` | Per-tick and per-replicate sample rows. No Markdown, script, or test referenced any of them. |
| `*.sqlite` | Nsight Systems session databases, regenerable from a rerun. |
| `*.nsys-rep`, `*.ncu-rep` | Nsight profiler binary captures, readable only with matching NVIDIA tooling. |

## What was kept

Every `ANALYSIS.md`, `README.md`, `summary.json`, `bench-results.json`,
`SHA256SUMS`, `*.sha256`, provenance text, and captured stdout/stderr — 1,954
files. Every conclusion drawn from these runs remains readable and attributable.

## Integrity caveat

`SHA256SUMS` and `*.sha256` still list digests for removed files. Those entries
are a historical record of what was collected, not a verifiable local manifest.
Verification of a pruned run therefore reports missing inputs; this is expected.

## Why

These runs are CUDA throughput evidence gathered on paid hardware before CPU was
made the authoritative backend. They are cold historical evidence, and at 91% of
all tracked files they dominated every clone, grep, and status check.

Full contents remain in Git history at tag `pre-cleanup-20260818`.
