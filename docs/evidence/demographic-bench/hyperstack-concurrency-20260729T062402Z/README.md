# Rejected concurrent CUDA sweep attempt — invalid worker context

This attempt is retained as diagnostic evidence and is not admissible as a
successful concurrency measurement.

- Repository commit: `a0b8607c1a5b7b4e3ad8821802a75cd090f8dd08`.
- The same-commit CPU/CUDA differential corpus passed on the H100.
- Workers 1 completed at 1M.
- Workers 2 failed on draw 0 with
  `CUDA_ERROR_INVALID_CONTEXT (invalid device context)`.
- Root cause: each CUDA backend was constructed on the coordinator thread and
  then moved to a worker, but CUDA contexts are thread-current.
- Fix: commit `b2bb18e19a756d824d5089a096d263c1be674f01`
  constructs, owns, uses, and drops each isolated backend on one worker thread.

The failed collector deliberately left the tracked VM running. The independent
watchdog remained armed; the same VM was reused for the corrected attempts and
was ultimately destroyed and provider-reconciled by the successful
`hyperstack-concurrency-20260729T064051Z` session.
