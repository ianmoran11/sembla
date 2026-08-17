# Evidence upload manifests

Each paid benchmark session was delivered on its own `evidence/hyperstack-*`
branch. Those branches carried the run's own top-level `README.md` and
`bench-results.md`, which were never merged into the main line — the curated
per-run directories under this tree were written separately.

These manifests are preserved here because they carry measured verdicts
(for example §L4 CUDA/CPU ratios and ageing-share replicates) that appear
nowhere else. Each directory records its source branch and commit in
`PROVENANCE.txt`.

The delivery branches themselves were deleted on 2026-08-18. Their only other
unique content was raw per-sample CSV data, discarded under the same policy
recorded in [`../PRUNED.md`](../PRUNED.md).
