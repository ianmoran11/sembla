# PRD 0003 implementation notes — attempt 1

## CUDA export decision

CUDA export is supported through the existing synchronized host-state path; it
is not rejected.

The CLI CUDA runner calls `CudaBackend::run_tick_observed` for every tick.
That method executes and commits the device tick, then calls
`download_state_store`, which downloads the current device state and inputs and
reconstructs a validated host `StateStore`. `run_results_output_cuda` assigns
that downloaded store to its host `state` on every tick, and returns the final
host store in `BackendRunOutput.state`. The existing final-state hash is already
computed from that same returned store. Export therefore reads the same final
committed host state without adding a second CUDA synchronization path or
claiming unavailable state.

The CPU runner returns its directly executed final `StateStore` through the
same `BackendRunOutput.state` field, so the export path is backend-independent
after execution.

## Overwrite discipline

`--out` retains its historical overwrite behavior. `--export-state` is
stricter: it preflights an existing path before execution and the artifact
writer also uses `create_new` to close the race. A chain-link artifact is never
silently replaced.

## Scope and validation

The implementation adds optional all-present-or-absent manifest tuples,
state-artifact export from committed tables, run-only flag parsing, chained-run
regressions, and documentation. Sweep/compare automation, checkpoint cursors,
and state-hash re-derivation in `verify-run` remain out of scope.
