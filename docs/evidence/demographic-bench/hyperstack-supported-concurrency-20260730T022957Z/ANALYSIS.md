# Analysis: supported concurrent CUDA sweep draws

## Scope and provenance

- Repository commit:
  `d72057f183d94dd311d21c33c90aaae97200cea6`.
- Release binary SHA-256:
  `f7a0aeb6e953b75c2d41b92ac16d2c3c17fe5a401b343fcf153b8def996af3d3`.
- GPU: NVIDIA H100 PCIe, 81,559 MiB, driver `570.195.03`, CUDA `12.8`,
  MIG disabled.
- Workload: demographic model, 1M and 10M slots, 20 draws × 24 ticks,
  grouped observations enabled, seed 9009.
- Interface under test: `sweep --backend cuda --draw-workers 1/2/4`.
- Concurrent execution identity: `cuda-free-nonblocking-streams`.
- Independent noise: three repetitions per worker count and scale.
- CRN noise: one correctness repetition per worker count and scale.
- Raw artifact branch: `evidence/hyperstack-20260730T023904Z`.
- Repository cleanliness: the exact-commit collector guard reached completion,
  which proves no tracked-source mismatch; a direct pre-run
  `git status --porcelain` record was not retained, so untracked remote files
  are not independently enumerated.

This is the deferred hardware stage for PRD 0004. It tests the supported flag,
not the hidden spike environment controls.

## Verdict

**Pass. Hardware criteria 10–13 are discharged for the measured H100 and
model shape.** Correctness, bounded admission, and non-default-stream execution
all match the PRD contract. The production interface reproduces the Gate 1
performance result: 1.756× at 10M/four workers versus the spike's 1.745×.

This is not evidence for an automatic worker policy or another materially
different model/device shape. The default remains one worker.

## Timing

External wall time is the headline measure. Each cell below is the median of
three independent-noise repetitions; speedup divides the worker-one median by
the corresponding concurrent median.

| slots | workers | external runs (s) | median (s) | speedup |
|---:|---:|---|---:|---:|
| 1M | 1 | 6.403, 5.153, 5.154 | 5.154 | 1.000× |
| 1M | 2 | 4.044, 4.034, 4.052 | 4.044 | 1.275× |
| 1M | 4 | 4.052, 4.032, 3.765 | 4.032 | 1.278× |
| 10M | 1 | 37.383, 37.334, 37.001 | 37.334 | 1.000× |
| 10M | 2 | 23.576, 23.500, 23.667 | 23.576 | 1.584× |
| 10M | 4 | 21.554, 21.265, 21.027 | 21.265 | 1.756× |

The first 1M/worker-one arm includes a cold setup outlier; using the specified
median keeps it from determining the result. The concurrent two- and
four-worker arms are nearly tied at 1M. At 10M the four-worker arm is fastest,
but the 2→4 gain remains much smaller than the 1→2 gain.

### Internal phase medians

The concurrent timing schema separates lane setup, the overlapping execution
window, publication, and whole-sweep internal wall. Worker one retains the
sequential timing schema, so it has setup and whole-sweep values only.

| slots | workers | setup (s) | execution window (s) | publication (s) | internal whole sweep (s) |
|---:|---:|---:|---:|---:|---:|
| 1M | 1 | 1.027 | — | — | 5.103 |
| 1M | 2 | 0.961 | 2.351 | 0.013 | 3.949 |
| 1M | 4 | 1.356 | 1.934 | 0.015 | 3.921 |
| 10M | 1 | 2.569 | — | — | 37.028 |
| 10M | 2 | 2.771 | 17.703 | 0.011 | 23.393 |
| 10M | 4 | 3.345 | 15.031 | 0.009 | 20.965 |

Publication remains negligible. Additional lanes reduce the execution window
but increase retained-backend setup, explaining the flattening at 1M.

## Correctness

All four machine-readable summaries report `passed: true`.

| noise | scales | repetitions | worker counts | complete-tree comparisons | result |
|---|---:|---:|---|---:|---|
| independent | 1M, 10M | 3 | 1, 2, 4 | 18 | all byte-equal |
| CRN | 1M, 10M | 1 | 1, 2, 4 | 6 | all byte-equal |

The comparator checks the complete normalized file set and bytes, including
manifests, summaries, grouped sidecars, exported pairs and metadata, and final
state hashes. Each scale/noise summary perturbs
`draw_0.grouped.deaths_cells.csv`; all four comparisons report `equal: false`
with that file changed.

The 1M schedule control delays low `k`, producing completion inversion. Its
check records:

```text
PASS: free-stream execution mode, completion inversion, and byte-identical publication
```

The timing record still publishes draw timings in ascending `k` and reports
`requested_draw_workers = effective_draw_workers = 2` with execution mode
`cuda-free-nonblocking-streams`.

## Capacity admission

The 10M oversized arm requests 20 workers and exits 1. The exact preflight
message reports:

- conservative device bound: 158,456.0 MiB;
- free device memory: 80,632.5 MiB (81,089.4 MiB total at the query point);
- per-lane bound: 6,312.6 MiB;
- fixed bound: 512.0 MiB;
- safety margin: 25%;
- host-memory assumption: at least 54,028.6 MiB.

It explicitly states that no lanes were constructed. The collector confirms no
scientific output directory was created. This satisfies the required clear
failure with no silent cap, fallback, or partial publication.

## Nsight Systems overlap

The 1M/two-worker supported-flag trace contains 39,552 kernels in one CUDA
context, split exactly evenly:

| context | stream | kernels |
|---:|---:|---:|
| 2 | 20 | 19,776 |
| 2 | 21 | 19,776 |

An interval sweep over `nsys-cuda-gpu-trace.csv` gives:

- summed kernel duration: 340.425536 ms;
- union kernel duration: 319.242835 ms;
- time at kernel concurrency ≥2: 21.182701 ms;
- overlap as a fraction of union: 6.635%;
- maximum simultaneous kernels: 2.

This proves the supported interface reaches distinct non-default streams and
real kernel overlap. As specified by the PRD, overlap is supporting evidence;
whole-sweep wall and exact output parity decide the verdict.

## Resources

Peak samples for the independent-noise timing arms:

| slots | workers | peak VRAM (MiB) | peak process RSS (KiB) | peak GPU utilization |
|---:|---:|---:|---:|---:|
| 1M | 1 | 1,033 | 436,636 | 87% |
| 1M | 2 | 1,613 | 748,920 | 93% |
| 1M | 4 | 2,733 | 1,123,876 | 100% |
| 10M | 1 | 6,025 | 2,552,112 | 99% |
| 10M | 2 | 11,597 | 5,296,620 | 100% |
| 10M | 4 | 22,701 | 9,144,144 | 100% |

The session-wide process-RSS maximum was 9,264,296 KiB in the 10M/four-worker
CRN arm. Device memory remains approximately draw-major and is the reason the
interface requires explicit bounded admission.

## Evidence integrity and teardown

The original remote `README.md` is retained byte-for-byte as
`README.remote.md`; the expanded local narrative now occupies `README.md`.
`remote-checksum-reconciliation.txt` maps that single rename and reproducibly
verifies all 3,170 remote-manifest entries: 3,169 direct matches, one renamed
match, zero mismatches. The final local `SHA256SUMS` verifies every retained
file after adding this analysis and the teardown records.

`collector-teardown.txt` is a sanitized extraction anchored by the SHA-256 of
the complete local driver log. It records collector exit-path events: evidence
push, transferred-checksum verification, destroy of two resources, empty-state
assertion, and final-checksum verification. `operator-teardown.txt` records the
post-collector commands and results:

- `terraform state list`: zero resources;
- provider reconciliation: zero `sembla-precision*` orphans;
- billing watchdog: disarmed;
- paid plan, ephemeral host-key directory, and known-hosts file: absent;
- session deploy-key ID: absent from GitHub after revocation;
- Tailscale, console, deploy, and host-key launchctl variables: absent;
- evidence branch commit:
  `24d5973728a2129338a1d09a9c5b6177ae1d7b5e`.

An offline `sembla-bench` node may remain visible in the Tailscale admin console;
that inventory entry is not a VM and may be deleted manually.
