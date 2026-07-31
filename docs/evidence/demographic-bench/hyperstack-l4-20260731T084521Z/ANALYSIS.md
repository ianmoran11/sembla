# CUDA current-path rebaseline analysis

## Status

This paid H100 session is a **complete six-command collection but a failed
official analysis**. Preserve that distinction.

All six frozen commands and all 18 draws returned zero without timing out. The
materialized/current correctness preflight passed complete output-tree and
canonical final-state SHA-256 parity. All four current-path four-draw output
trees were mutually byte-identical, and the deliberate one-byte mutation of
`draw_0.csv` was rejected.

The official analyzer then failed closed with:

```text
CUDA current-path rebaseline analysis failed: repository or artifact identity is malformed/dirty
```

The manifest correctly recorded the sole checkout difference:

```text
?? scripts/__pycache__/run-cuda-final-state-decision.cpython-310.pyc
```

The current collector dynamically imported that tracked support script without
suppressing Python bytecode. The import itself created the untracked `.pyc`
after the wrapper's clean-checkout gate. This is collection-tool contamination,
not a scientific-output mismatch, CUDA failure, timeout, or performance result.
The fail-closed analyzer behaved as designed.

No evidence file or recorded manifest was repaired in place. The original
`SHA256SUMS` and `partial/SHA256SUMS.partial` each verify unchanged.

## Provisional observational extraction

For diagnosis only, a temporary copy of `protocol/` was made outside this
evidence tree. Its single recorded `repository_status` field was changed from
the `.pyc` line above to the empty string, its copied checksum entry was
recomputed, and the unchanged analyzer was run. This answers what the already
collected timing/profile files contain; it does **not** convert this session
into officially complete evidence and does not authorize a rerun or
optimization.

Three unprofiled workers-4 repetitions reported:

| Repetition | Whole wall ms | Setup ms | Execution ms | Publication ms | Mean draw wall ms | Aggregate final-state seam ms | Pageable D2H host API ms | CPU SHA-256 ms | Peak RSS GiB | Peak VRAM GiB |
|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| 1 | 8,974.135 | 2,955.025 | 3,186.749 | 1.794 | 2,824.747 | 2,737.065 | 1,683.533 | 1,053.530 | 9.015 | 22.160 |
| 2 | 9,038.253 | 3,010.837 | 3,183.771 | 1.574 | 2,820.127 | 2,765.599 | 1,712.368 | 1,053.229 | 9.217 | 22.160 |
| 3 | 9,181.839 | 3,043.010 | 3,250.180 | 9.424 | 2,863.002 | 2,874.991 | 1,813.725 | 1,061.264 | 9.370 | 22.160 |

Selected medians and ranges:

| Metric | Median | Range |
|---|---:|---:|
| Whole wall | 9,038.253 ms | 207.703 ms |
| Setup | 3,010.837 ms | 87.985 ms |
| Execution | 3,186.749 ms | 66.409 ms |
| Publication | 1.794 ms | 7.850 ms |
| Aggregate four-draw final-state seam | 2,765.599 ms | 137.926 ms |
| Aggregate pageable D2H host API | 1,712.368 ms | 130.192 ms |
| Aggregate CPU SHA-256 | 1,053.530 ms | 8.035 ms |
| Peak RSS | 9.217 GiB | 0.355 GiB |
| Peak VRAM | 22.160 GiB | 0 GiB |

The separate Nsight profile found four 480,000,000-byte pageable D2H copies
(1,920,000,000 bytes total). Their summed/union duration was 944.351 ms,
32.747 ms overlapped kernels, and 911.605 ms was exposed. The CUDA API summary
reported 772 `cuMemcpyDtoHAsync_v2` calls totalling 6,297.675 ms across the full
profiled command; that API total includes calls beyond the four identified
large final-state transfers.

## Interpretation

- The earlier one-off setup rise to about 4.169 seconds did not recur. Setup was
  approximately 2.955–3.043 seconds across these three repetitions.
- CPU SHA-256 was very stable: the aggregate four-draw range was only about
  8 ms. The larger final-state variation came from pageable transfer time.
- In the median repetition, pageable D2H accounted for about 62% of the
  measured final-state seam and CPU SHA-256 about 38%.
- The profile confirms that the four large final-state transfers are mostly
  exposed rather than hidden behind kernels.
- Historical cross-session values remain contextual and non-binding. Relative
  to the earlier representative workers-4 B observation, CPU SHA time is
  essentially unchanged while pageable D2H is higher; this session alone
  cannot establish why host-transfer time moved.
- There is no performance threshold here and no optimization is authorized.
  First fix the collector's bytecode hygiene and obtain review. Any paid rerun
  requires a new exact-plan approval; do not adaptively extend this session.

## Teardown and security

The benchmark status is `1` because the official analyzer rejected the dirty
identity. Teardown status is `0`: Terraform destroy succeeded, final Terraform
state is empty, final provider reconciliation reports tracked zero/no orphans,
and both evidence checksum manifests verify. `RmProfilingAdminOnly` was `1`
before and after the session and `counter-restoration-status.txt` records
`remained-admin-only` in the remote partial evidence. The independent billing
watchdog was disarmed only after zero-resource verification. The per-session
write deploy key was revoked and the saved launchctl secrets and host-key
material were removed.
