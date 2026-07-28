# PRD 0012 Re-review — Attempt 1

## Assessment

**APPROVED**

## Acceptance criteria

1. **PASS:** Spike-local build/tests pass, root workspace builds/tests remain green, and root metadata excludes the standalone spike.
2. **PASS:** Four copied WGSL Philox known-answer vectors pass with local scalar verification and frozen PRD-0003 coordinate packing.
3. **PASS:** The 10k smoke test checks exact candidate flags/counts, lexicographic winners, loser suppression, and fired count against a local scalar implementation.
4. **PASS:** The release runner writes a populated results document containing adapter identity/capabilities, requested and actual sizes, 100-tick medians, per-kernel and total timing, throughput, extrapolation, reduction/conflict choices, and verdict.
5. **PASS pending managed commit:** The measured results artifact is ready for the managed commit after approval.

## Precision resolution

The authorized exception is narrowly limited to PRD 0012 when the selected portable WGSL adapter lacks shader `f64`. Adapter capability is detected and recorded; the Apple M2 Pro artifact reports it unsupported. The result identifies `f32` explicitly, treats its rate as directional kernel-shape evidence, leaves production-`f64` throughput unanswered, and preserves the production numeric contract.

No blocking issues found.
