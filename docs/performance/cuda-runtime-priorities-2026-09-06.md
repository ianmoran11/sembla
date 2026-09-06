# CUDA runtime priorities — 2026-09-06

The three priority changes reduce median whole CUDA sweep time by **4.9% at 1M**
and **7.7% at 10M** on the tested H100. Correctness and Compute Sanitizer checks
pass. Most of the benefit is loading and construction, with little change to
steady draw time. The 1M CPU control slowed on the AMD host; that limitation
remains unresolved and is detailed below.

## Changes

- **Read and decode input once.** `run` and `sweep` reuse the bytes loaded for
  raw population identity when detecting and decoding state/population files.
  The state path previously performed four full-file reads and two decodes.
  A consuming initializer API moves validated column buffers into `TableInit`.
  Both the raw population SHA256 and frozen domain-separated state-artifact
  hash still describe the exact input bytes. Numeric input identity is unchanged.
  Other initializer callers also avoid repeated decoding, although commands
  that independently request population identity can still reread the file.
  The standalone magic detector now reads only the 12-byte prefix.
- **Avoid the ordinary CUDA upload clone.** Single-slot construction uploads
  the existing packed byte slice directly, removing `.repeat(1)` and its
  480,000,032-byte temporary copy at 10M rows. Fused construction still uploads
  every slot, and state/next-state/pristine device buffers remain separate.
- **Cap diagnostic recovery grids.** Phase zero retains the original full
  grid. Phases one through three use at most 32 blocks for transition, effect,
  write-preparation, and output validation. They return immediately on success;
  on error their existing grid-stride loops still scan every row and recover
  the same ordered status payload. Empty-table scalar checks and explicit test
  geometry overrides remain supported. Error recovery is verified on hardware;
  its latency has not been measured separately.
- **Compact deferred flags.** Flags cover candidate × contested table, using
  a sorted compact index, while reported counts retain global table order.
  Repeated claims share one table flag. The demographic 10M buffer falls from
  300 MB to 100 MB per worker. Allocation/capacity estimates and fused slot
  strides use the same compact shape; the no-contest case retains only a
  one-byte placeholder.

Public CUDA reference generation, fixtures, canonical bytes, hash domains,
Cargo.lock, and backend boundaries are unchanged. Execution-source hashes
continue to identify the actual optimized translation unit.

## H100 measurement

H100 PCIe, driver 570.195.03, 28-vCPU AMD EPYC 9554 host, CUDA-enabled release
binaries. Baseline: `d93c8a301f1fc6e0e44ec6ed06504c5b45ff363e`; measured candidate:
`107a686cebbcc4b002aa90b34cbdfb4448405860`. Production runtime code is unchanged
from the implementation commit below. Each sweep has 20 independent draws of
24 ticks with grouped observations. Both arms are warmed, then three alternating
pairs are measured. Medians exclude the warmup pair.

| Slots | Whole command before → after | Reduction | Peak host RSS before → after |
|---|---:|---:|---:|
| 1M | 3.622 → 3.444 s | 4.9% | 420.99 → 406.82 MiB |
| 10M | 23.669 → 21.851 s | 7.7% | 2510.98 → 2468.50 MiB |

All six measured pairs improve, but the 10M pair reductions range from 2.2% to
8.4%. These are three pairs on one spot host, not a universal speedup estimate.
Typical later draws change from 104.61 → 103.91 ms at 1M and 913.90 → 909.92 ms
at 10M. At 10M, median constructor time falls from 2.530 → 2.266 s; time outside
construction and timed draws falls from 2.796 → 1.381 s. That latter interval
also includes publication and other command work, so it is not an isolated
input-phase timer. Its reduction is consistent with the loading changes.
Separate phase medians should not be added as if they describe one run.

The device flags shrink by 200 MB per 10M worker, and the temporary upload copy
is eliminated, but peak host RSS falls by only 42.47 MiB in these sweeps. These
allocation reductions should not be reported as an equal reduction in total
peak memory. The Nsight trace confirms 32-block recovery passes. No ablation
isolates each change's performance contribution.

All scientific trees, pair exports, grouped sidecars and hashes match. The
full differential corpus and four hardware tests pass under Compute Sanitizer
with zero errors. The frozen §L4 gate is MET at 6.321× relative to CPU on its
separate no-grouped workload. All 3,162 remote checksum entries verify. The VM
and SSH rule were destroyed, provider reconciliation is empty, and disposable
credentials were removed. The rate-based session estimate is US$1.99.

[Raw results, independent verification and closeout](../evidence/demographic-bench/hyperstack-l4-20260906T203715Z/review/README.md).

### CPU control limitation

The AMD host's 1M CPU sweeps slowed in both collections: 54.79 → 63.04 s (+15.1%)
and 58.04 → 65.60 s (+13.0%). The 10M pair was 715.47 → 716.38 s (+0.1%). CPU
execution code is identical, but input allocation changed; the cause is not
established. These controls do **not** demonstrate CPU neutrality or a CPU
throughput gain. A follow-up M2 Pro test, using the exact same 1M input and five
24-tick draws, gave 10.853 → 10.770 s across three warmed pairs with exact
outputs. It did not reproduce the AMD-host slowdown, which remains the next
CPU investigation before claiming this path improves both backends.

## Local measurement

Baseline: `d93c8a301f1fc6e0e44ec6ed06504c5b45ff363e`.
Measured implementation: `fc01302e3db9f8aa2f874ccf4f38514c1d85b8a7`.
The baseline snapshots the existing developer changes, including prior CPU
and CUDA work. Both arms therefore have identical CPU execution code; the
before/after comparison isolates this round's changes.

Apple M2 Pro, macOS 15.5, release builds with CUDA feature compiled in but
`--backend cpu`. Both binaries run one tick with grouped observations, seed
9009, four areas, 80% present population, and the runbook's input streams.
At each scale both arms are warmed, followed by three alternating pairs.
The table reports medians of those three measured runs. Lifecycle phases
have separate medians and should not be summed as one representative run.

| Slots | Input preparation before → after | Reduction | Whole command before → after | Reduction | Peak RSS before → after |
|---|---:|---:|---:|---:|---:|
| 1M | 304.451 → 272.072 ms | 10.6% | 570.031 → 535.338 ms | 6.1% | 322.83 → 313.13 MiB |
| 10M | 3147.904 → 2766.254 ms | 12.1% | 5618.489 → 5218.077 ms | 7.1% | 1715.17 → 1565.86 MiB |

Every output tree matches byte-for-byte within each pair, including summaries,
grouped sidecars, manifests and hashes. Input hashes are verified again after
all runs. These are warm-file-cache measurements on one host; they do not
measure the new CUDA upload path, GPU flags, or diagnostic launch geometry.

[Raw evidence, commands and validation logs](../evidence/demographic-bench/cuda-priorities-local-20260906/README.md).

## Validation and GPU protocol

`./scripts/check-rust.sh` (serial test execution) and
`./scripts/check-determinism.sh` pass. The later input-snapshot regression and
final all-feature clippy check also pass. CUDA-feature library tests report
42 passed, four hardware tests ignored after the diagnostic harness fix. Existing negative initializer cases
now compare borrowed and consuming errors; the new snapshot test replaces a
file after identity loading and verifies decoding still uses those exact bytes.

The hardware corpus now also exercises errors at row 65,530 of 65,539, beyond
one recovery-grid iteration, comparing every committed status word with a
full-grid launch and checking rollback. Its CPU oracle passes locally.
The sparse multi-table conflict test checks compact allocation lengths and
fused widths 2 → 1 → 2 across resets, with exact CPU report/state comparison.

With `BENCH_CORPUS=1 BENCH_PROFILE=1 BENCH_SWEEP=1`, the collector ran the
full differential corpus, a bounded Compute Sanitizer memcheck of CUDA library
tests, grouped/no-grouped profiles, adjacent CPU/CUDA sweeps, and the unchanged
frozen gate. Modern baselines additionally receive three alternating CUDA
repeat pairs after warming both arms. Native timing and peak RSS are collected
before deleting either the input state or original build checkout. Each
repeat tree and exported pair file must match its primary arm and its partner.

The repeat-loop smoke test passes and rejects a corrupted output; all 12
collector flag tests and shell parsing pass. The corrected GPU collection, artifact verification and teardown are now
complete. A changing local `.DS_Store` caused a final metadata checksum failure
after successful transfer and destruction; the collector now excludes Finder
metadata from local manifests. Both actual checksum generators and all 12
collector flag tests pass, and the final evidence manifests verify.

## Diagnostic harness correction during GPU validation

The initial collection exposed a pre-existing test-selection bug: the diagnostic
parent launched its child with a test name missing the `backend::` module
prefix. Rust's test runner returned success after selecting zero tests. That
initial reported pass did not establish diagnostic correctness, including the
new large-row recovery coverage. The collection was stopped during the 10M CPU
baseline, and its incomplete evidence is retained separately.

Commit `107a686cebbcc4b002aa90b34cbdfb4448405860` derives the complete test path
from the module, adds a regression that asks the actual test runner to list the
selected test, and makes the corpus reject successful logs with no diagnostic
case results. This changes the test harness only; production runtime code is
identical to `fc01302e3db9f8aa2f874ccf4f38514c1d85b8a7`.

The full protocol was restarted on the same H100 with the original destruction
deadline retained. Both the regular diagnostic log and Compute Sanitizer log
now contain all 20 case/geometry results and all four large-row recovery results.
The complete corpus passes, including nine CLI hardware tests; all four CUDA
library hardware tests pass under memcheck with zero reported errors. The
corrected host library reports 42 passing tests and four hardware tests ignored.
`./scripts/check-rust.sh` passes again after the harness correction.
