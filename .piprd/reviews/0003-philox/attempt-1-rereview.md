# PRD 0003 Re-review — Attempt 1

## Assessment

**APPROVED** — no blocking issues remain.

## Acceptance criteria

1. **Random123 known answers: PASS.** All-zero and all-ones Philox4x32-10 vectors pass; an asymmetric reference vector also freezes key/counter packing.
2. **Purity and coordinate sensitivity: PASS.** Every public sampler is deterministic under repeated arguments. One thousand coordinate sets independently perturb seed, tick, rule ID, entity ID, and draw index and reject any full 128-bit collision.
3. **Uniform sampler: PASS.** One million draws remain strictly in `(0,1)`, meet the mean tolerance, and satisfy the 100-bucket 5% bound. Extreme mantissas explicitly test both endpoints.
4. **Exponential sampler: PASS.** One million λ=2 draws meet the mean tolerance; zero and negative rates return infinity.
5. **No external RNG: PASS.** `sembla-runtime` has no dependencies. `scripts/check.sh` now inspects Cargo's resolved direct graph across all features, targets, and normal/build/dev edges and rejects any dependency, closing table-syntax, literal-string alias, renamed-package, optional/target, and workspace-inheritance bypasses.
6. **Rustdoc and CRN: PASS.** Exact seed/counter word packing and identical-coordinate behavior across scenario variants are documented.

## Checks

- `./scripts/check.sh`: passed; 21 tests passed
- Rustdoc with warnings denied: passed
- `git diff --check`: passed
- Resolved runtime dependency graph contains only `sembla-runtime`

The implementation remains within RNG-module scope. The unrelated `.DS_Store` and managed `.piprd*` state do not affect approval.
