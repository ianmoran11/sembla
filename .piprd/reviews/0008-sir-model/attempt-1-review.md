# PRD 0008 Review — Attempt 1

## Assessment

**APPROVED** — the implementation satisfies the complete PRD 0008 specification and all six acceptance criteria.

## Acceptance criteria

1. **PASS:** `cargo test --workspace` passes, preserving PRD 0001–0007 behavior.
2. **PASS:** A compiled-CLI integration test independently synthesizes two identical 100,000-person population files, performs fresh 100-tick runs, compares exact CSV bytes and both reported hashes, and proves different run seed and different theta each change both results and final-state hashes.
3. **PASS:** The epidemiology test documents `R0 = beta/gamma = 8`, proves monotonic S and R, rising then falling I, final attack rate above 50%, and no infection growth at beta zero.
4. **PASS:** The active one-million-person test proves the two structurally distinct employer group-by accumulators are each built once per tick rather than per row.
5. **PASS:** The release harness runs one million people for ten ticks and measures 0.106 seconds/tick, below the two-second threshold.
6. **PASS:** `docs/examples/sir.md` documents the model, formula, deterministic generator and format, execution, CSV contract, and two-command determinism check.

## Specification evidence

`examples/sir.json` contains the required one-box person/employer schema, symbolic beta/gamma parameters with priors, frequency-dependent infection hazard, disjoint recovery transition, and no contests. `synth-pop` uses only PRD 0003 Philox coordinates in a documented reserved rule-ID namespace and writes a validated versioned binary format. The CLI implements binary population loading, visible positive finite `--dt`, typed named `--params` errors, self-describing resolved-theta CSV output, and SHA-256 output for both result bytes and final state.

No blockers found.
