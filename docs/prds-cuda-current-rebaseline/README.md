# CUDA current-path rebaseline PRDs

Prepare a small, local-only collector for observing the promoted packed-pageable
CUDA sweep path during a later, separately approved H100 session. Run from the
Sembla repository with:

```text
/piprd run docs/prds-cuda-current-rebaseline
```

`README.md` is ignored by `/piprd run`; the numbered PRD reads this contract.

## Why this folder exists

`DECISIONS.md` §L14 promoted packed-pageable final-state hashing after a focused
27-execution A/B/C experiment. B improved whole-command wall time by 15.21% at
workers 1 and 7.33% at workers 4 while preserving byte-identical scientific
outputs and canonical SHA-256. That experiment selected a winner; it is too
large to use as routine production-path observation.

The representative workers-4 B profile still attributed about 1,246 ms to
pageable D2H and 1,052 ms to CPU SHA-256 across four draws. One timed B
repetition also showed setup rising to 4.169 s. Before selecting another
optimisation, collect a small current-path rebaseline that exposes setup,
execution, publication, transfer, hashing and profile evidence without reopening
A/B/C selection.

## Binding scope

- Prepare exactly six benchmark executions and 18 draws: adjacent one-draw
  explicit-materialized and unset-selector current-path preflights; three
  four-draw workers-4 unset-selector timed repetitions; and one separate
  four-draw workers-4 unset-selector Nsight profile.
- The promoted ordinary path is tested with
  `SEMBLA_SWEEP_CUDA_FINAL_STATE_MODE` absent, not explicitly set to
  `packed-pageable`. Retired device-SHA variables are absent too.
- The materialized command is correctness-only. Historical A/B results and the
  new explicit-A command must not be used to claim a new paired speedup.
- Report absolute current-path median, min, max, range and raw repetitions. Do
  not add a pass/fail performance threshold or optimisation authorization.
- Preserve canonical SHA-256, complete scientific output bytes, manifests,
  scheduling, RNG and all CUDA/runtime source. Timing and profiling artifacts
  remain non-scientific.
- Do not run C, CRN, workers 1/2 performance repetitions, a 20-draw matrix,
  broad profiling, NCU, or a second adaptive execution after seeing results.
- Local implementation and validation must not provision a VM, firewall rule or
  other paid resource. Hardware execution requires later explicit approval of
  the exact saved plan under the Hyperstack runbook.
- `KEEP_VM=1` is rejected before artifacts. EXIT, TERM, INT, timeout, collection
  failure and analysis failure all use the existing bounded idempotent teardown,
  empty Terraform-state check and orphan reconciliation.

## Interpretation

This is a rebaseline, not an experiment selecting a treatment. Cross-session
comparisons to the 2026-07-31 H100 evidence are contextual only because commit,
host load, image, driver and Nsight versions can differ. If spread is high,
preserve the six-command result and propose a separate follow-up; never extend a
paid session silently.

## Status

- PRD 0001 is ready for local implementation.
- No paid resource is approved or created by this folder.
