# PRD 0004 review

## Decision

**APPROVED** — no blocking issues.

## Acceptance criteria

1. **Exact syntax and selected Ref key:** PASS. Only the parenthesized `freq (...) over key` form elaborates; recovery forms emit deliberate teaching diagnostics. Key lookup is against the selected system and requires Ref type.
2. **Exact lowering and byte twins:** PASS. `freq`, `countBy`, and `sizeBy` share `keyedCountTerm`. Direct guards pin numerator/denominator order and strings; complete models and literal serialized `Sembla.IR.toJson` strings are equal.
3. **Row-local/type restrictions:** PASS. Recursive validation rejects input aggregates, `countBy`, `sizeBy`, nested `freq`, and unknown row/model names at source tokens. Predicate Bool and key Ref rules have exact diagnostics. No public contextless host was introduced.
4. **Reaction and widget behavior:** PASS. Inferred and explicit-system arrows, Greek parameters, conjunction predicates, `StateDiagramProps`, `HazardPanelProps`, and deterministic expanded pretty-printing are covered.
5. **Legacy and canonical compatibility:** PASS. Legacy aggregates compile and canonical byte/hash parity is unchanged.
6. **Non-goals and frozen scope:** PASS. No C(i), IR/JSON, Rust/runtime, dependency, documentation, or widget-style changes exist.
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
