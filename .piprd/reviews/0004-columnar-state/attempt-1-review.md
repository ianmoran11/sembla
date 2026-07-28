# PRD 0004 Review — Attempt 1

## Assessment

**APPROVED** — no blocking issues.

## Acceptance criteria

1. **Workspace tests: PASS.** `cargo test --workspace`, formatting, warnings-denied Clippy, rustdoc, `scripts/check.sh`, and `git diff --check` pass.
2. **Construction and bounds validation: PASS.** The validated `two_state.json` schema initializes 100 rows. Initial enum and Ref violations identify box, table, column, and row; typed physical columns are `Vec<f64>`, `Vec<i64>`, `Vec<u16>`, and `Vec<u32>`.
3. **Double buffering: PASS.** The eager-copy next buffer is disjoint from the old snapshot; tests observe old values while writing and new values only after `commit()` swaps buffers.
4. **Golden hash: PASS.** A fixed alternating 100-row `two_state.json` state is checked against hardcoded SHA-256 `d391a94faad7629c658791900c6f81aec11724d06da15570408f9ab55b761a33`.
5. **Hash determinism and sensitivity: PASS.** Independently rebuilt state has the same hash; changing every one of the 100 golden cells changes it. Real, Int, Enum, and Ref encodings also have separate sensitivity checks.
6. **Hash rustdoc: PASS.** Domain framing, box/table and column ordering, UTF-8 name lengths, counts, type tags, row order, and exact little-endian scalar encodings are documented and implemented consistently.

## Additional evidence

- Tables use box-major/table declaration order and columns use attribute declaration order without maps.
- Populations cannot resize through the public API.
- Ref targets are box-qualified and converted to global declaration-order table indices correctly.
- No transition execution, joins, persistence, or growth behavior was introduced.
- The evolved dependency guard permits only `sembla-ir` and `sha2` directly and retains resolved-tree external RNG rejection.

Unrelated `.DS_Store` and managed `.piprd*` state do not affect approval and should remain outside the implementation commit.
