# PRD 0012 Review — Attempt 1

## Assessment

**REVISE**

## Blocking issue

The benchmark uses `f32` hazard and exponential-race arithmetic in Rust/WGSL, while `docs/prds/README.md` makes `f64` hazard arithmetic binding on every PRD. The documentation honestly identifies the mismatch and treats the result as directional, but disclosure does not waive the convention. The measured 74.014 ticks/sec therefore does not answer throughput for the required production numeric workload.

Resolve this with either a compliant `f64` measurement or an explicit authority-approved exception permitting `f32` for this portable throwaway spike.

## Acceptance criteria

1. **PASS:** Standalone spike build/tests pass and root workspace excludes it.
2. **PASS:** Four GPU Philox KAT vectors pass with frozen coordinate packing.
3. **PASS:** The 10k GPU smoke test matches exact scalar candidate flags/counts, winners, loser suppression, and fired count.
4. **IMPLEMENTED:** Release runner and populated results artifact include adapter, sizes, timings, throughput, workload choices, extrapolation, and verdict.
5. **PENDING MANAGED COMMIT:** `RESULTS.md` is ready but intentionally remains untracked until approval.

All explicit GPU workload, conflict, timing, scaling, software-adapter, and results-document mechanics are otherwise implemented correctly.
