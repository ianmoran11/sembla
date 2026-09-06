# CUDA runtime priorities — 2026-09-06

The three priority changes are implemented and pass local checks. A local
one-tick CPU benchmark confirms lower input-preparation time and memory.
CUDA correctness, kernel timing, and whole-sweep performance remain pending a
new approved H100 session; local CPU gains are not CUDA speedup estimates.

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
  geometry overrides remain supported. Error-path latency may increase and
  needs hardware measurement alongside the successful path.
- **Compact deferred flags.** Flags cover candidate × contested table, using
  a sorted compact index, while reported counts retain global table order.
  Repeated claims share one table flag. The demographic 10M buffer falls from
  300 MB to 100 MB per worker. Allocation/capacity estimates and fused slot
  strides use the same compact shape; the no-contest case retains only a
  one-byte placeholder.

Public CUDA reference generation, fixtures, canonical bytes, hash domains,
Cargo.lock, and backend boundaries are unchanged. Execution-source hashes
continue to identify the actual optimized translation unit.

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
41 passed, four hardware tests ignored. Existing negative initializer cases
now compare borrowed and consuming errors; the new snapshot test replaces a
file after identity loading and verifies decoding still uses those exact bytes.

The hardware corpus now also exercises errors at row 65,530 of 65,539, beyond
one recovery-grid iteration, comparing every committed status word with a
full-grid launch and checking rollback. Its CPU oracle passes locally.
The sparse multi-table conflict test checks compact allocation lengths and
fused widths 2 → 1 → 2 across resets, with exact CPU report/state comparison.

With `BENCH_CORPUS=1 BENCH_PROFILE=1 BENCH_SWEEP=1`, the collector will run the
full differential corpus, a bounded Compute Sanitizer memcheck of CUDA library
tests, grouped/no-grouped profiles, adjacent CPU/CUDA sweeps, and the unchanged
frozen gate. Modern baselines additionally receive three alternating CUDA
repeat pairs after warming both arms. Native timing and peak RSS are collected
before deleting either the input state or original build checkout. Each
repeat tree and exported pair file must match its primary arm and its partner.

The repeat-loop smoke test passes and rejects a corrupted output; all 12
collector flag tests and shell parsing pass. Hardware results must precede any
claim about further CUDA throughput. Fresh paid-plan approval and the usual
watchdog, artifact verification, destruction and provider reconciliation remain
required by the infrastructure runbook.
