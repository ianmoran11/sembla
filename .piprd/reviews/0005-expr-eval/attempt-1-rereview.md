# PRD 0005 Rereview — Attempt 1

## Assessment

**APPROVED** — all prior blockers are resolved and no new blocking issues were found.

## Acceptance criteria

1. **Workspace checks: PASS.** `cargo test --workspace`, formatting, warnings-denied Clippy/rustdoc, `scripts/check.sh`, and `git diff --check` pass.
2. **Expression coverage: PASS.** Every v0.1 `Expr` arm is implemented and tested, including nested arithmetic over `SelfAttr`, `EnumIs` in an aggregate filter, typed empty Input, and the same parameter expression under defaults and overrides.
3. **Lumping equivalence: PASS.** The 1,003-row Person→Employer group-by count matches a test-local O(n²) reference row-for-row.
4. **Aggregate caching: PASS.** Structurally identical aggregates share one accumulator build; keys include box, target table, op, join, and filter with bitwise `f64` structure.
5. **Determinism: PASS.** Real Sum reduces once in ascending target-row order, and `[1e16, -1e16, 1]` freezes the order-sensitive result at `1.0` across repeated evaluations.
6. **Snapshot-only: PASS.** Evaluator signatures accept `&Snapshot`; no `WriteBuffer` access exists.

## Prior blocker resolution

- Ambiguous root enum literals use explicit validated destination context through `EvalTable::with_expected_attr`; tests verify shared variant `I` maps to distinct indices in two enum types.
- Public `ValueColumn` contains exactly Real, Int, Bool, and Enum. Private Ref columns support nested evaluation, and `eval_ref_column` preserves valid Ref-root evaluation without extending the frozen result contract.
- `AggCache` holds live references to the exact model, Snapshot object, and ParamEnv and verifies scope identity. Those borrows prevent allocator address reuse while entries live; state-generation/address-key changes were removed and `state.rs` is unchanged.

No parallelism, transition execution, persistence, or incremental evaluation was introduced.
