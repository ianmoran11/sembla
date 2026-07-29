# Rejected concurrent CUDA sweep attempt — control artifact mismatch

This attempt is retained as diagnostic evidence and is not the final accepted
session.

- Repository commit: `b2bb18e19a756d824d5089a096d263c1be674f01`.
- The same-commit CPU/CUDA differential corpus passed.
- All workers 1/2/4 measurements at 1M completed across three repetitions, and
  every scientific output-tree comparison passed.
- The forced completion inversion itself succeeded: draw 1 finished before draw
  0 while ascending-`k` publication remained deterministic.
- The collector then rejected its comparison because the reference arm used
  `--export-pairs`, which emits per-draw summary sidecars, while the forced arm
  omitted that option. The recorded diff therefore contained only those
  intentionally different artifact-presence entries, not a scientific-value
  mismatch.
- Commit `5769e717a4fa5fa5d2924c95bcf03d73b3d717d9` made the forced arm use the
  same artifact options. A subsequent hot check produced an exact tree.

The failed collector left the tracked VM running under the armed watchdog. The
final `hyperstack-concurrency-20260729T064051Z` session completed both scales,
collected Nsight evidence, destroyed the VM, and passed provider reconciliation.
