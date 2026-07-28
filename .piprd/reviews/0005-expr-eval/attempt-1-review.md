# PRD 0005 Review — Attempt 1

## Assessment

**REVISE**

## Blocking issues

1. **Root enum expressions lose their validator-supplied expected type.** `eval_column` has no expected result type. `infer_root_type` instead scans every enum attribute on the table and rejects a variant as ambiguous when it appears in two distinct enum types (`crates/sembla-runtime/src/eval.rs`). Such an `Expr::Enum` is valid in a destination-attribute context under PRD 0002, so this does not evaluate every validated v0.1 expression reliably. Preserve validated expression type metadata or accept an explicit expected type rather than guessing from the row schema.
2. **The public `ValueColumn` contract is extended without specification support.** PRD 0005 defines Real, Int, Bool, and Enum result columns, but `ValueColumn` also exposes `Ref(Vec<u32>)`. Ref evaluation may be needed later, but the PRD/API contract must either be followed here or explicitly amended before adding the public variant.
3. **Cache scope identity is based on reusable addresses.** Snapshot scope uses `(StateData address, per-store generation)` and aggregate keys use the `ValidatedModel` address. If a cache outlives a dropped store/model and allocator addresses are reused at the same generation, entries can be treated as belonging to the replacement scope. Use lifetime-enforced per-tick cache ownership or stable non-reusable identities.

## Acceptance evidence

- `cargo test --workspace`, formatting, warnings-denied Clippy, rustdoc, and `git diff --check` pass.
- Every syntactic `Expr` arm is exercised, including nested `SelfAttr` arithmetic, parameter defaults/overrides, and `EnumIs` in an aggregate filter.
- The 1,003-row group-by count matches a test-local O(n²) implementation.
- Identical aggregates build one accumulator.
- Cancellation-sensitive Sum uses ascending target-row accumulation and repeats exactly.
- `eval_column` accepts `&Snapshot` and has no `WriteBuffer` access.
- IEEE division, typed empty Input, signed-zero cache structure, normal parameter/snapshot invalidation, and non-goal scope are otherwise implemented correctly.
