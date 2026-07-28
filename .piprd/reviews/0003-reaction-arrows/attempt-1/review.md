# PRD 0003 re-review

## Decision

**APPROVED** — no blocking issues remain.

## Acceptance criteria

1. **Four forms and exact lowering:** PASS. All four forms lower to one `Expr.enumIs` guard, the supplied Real hazard, one `Effect.setAttr`, and no contests.
2. **Structural, byte, and widget twins:** PASS. Imported guards compare complete `Model` values, exact `toJson` output, `StateDiagramProps`, and `HazardPanelProps` for every representative case.
3. **Deterministic inference:** PASS. Unlabelled compatibility now requires one enum attribute containing both endpoints. System selection and sole-enum attribute inference remain separate. The split-first/later-valid twin proves complete-set selection without declaration-order bias, and explicit labels resolve both ambiguity classes.
4. **Negatives and self-loops:** PASS. Self-loops compile. Every required failure uses complete exact positioned-diagnostic comparison, including split attributes, zero/multiple systems, multiple enum columns, invalid hazards, and trailing guards/effects.
5. **Legacy and canonical parity:** PASS. General transitions retain the shared validation/emission path; canonical exports and execution hashes remain byte-identical.
6. **Frozen scope:** PASS. No IR, JSON, canonical fixture, Rust, dependency, CI, or public-documentation changes exist.
7. **Required checks:** PASS.

## Independent checks

- `cd frontend && lake build` — pass
- `cd frontend && bash scripts/test-negative.sh` — pass
- `bash frontend/scripts/check-parity.sh` — pass
- `./scripts/check.sh` — pass
- `git diff --check` — pass
- Frozen-path diff — pass

## Blocking issues

None.
