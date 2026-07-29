# PRD 0013 Review — Attempt 1

## Assessment

**REVISE**

## Blocking issue

The Box–Muller choice is implemented and documented, but it is not frozen by a golden test as required. Existing 100,000-draw moment tests would still pass if the implementation switched branches or changed sampler counter/lane ordering. Add hard-coded IEEE-754 bit-pattern vectors for Uniform, Normal, and LogNormal at fixed PRD-0003 coordinates.

## Acceptance criteria

1. **PASS:** Full workspace tests and all prior tests pass.
2. **PASS except deterministic freeze blocker:** 100,000 samples per family satisfy documented moment tolerances, including LogNormal log-domain checks.
3. **PASS:** Repeated invocations are byte-identical and changing seed changes every artifact.
4. **PASS:** Draw 3 is identical between K=5 and K=50 collections.
5. **PASS:** Artificially identical theta yields identical per-draw files under CRN.
6. **PASS:** Gamma pin is marked and constant while beta varies.
7. **PASS:** Exact 100k-person, K=20, T=50 integration test completes with monotone quantile bands.
8. **PASS:** SIR documentation contains commands, summary interpretation, sampler/namespace details, and CRN caveat.

Workspace isolation, integer-prior rejection, pin/default behavior, sequential execution, hashes, stale-output cleanup, and quantile generation are otherwise correct.
