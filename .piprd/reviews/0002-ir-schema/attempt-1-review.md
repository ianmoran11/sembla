# PRD 0002 Review — Attempt 1

## Assessment

**REVISE** — one blocking specification gap remains.

## Blocking issue

### Missing contested-resource coverage validation

`crates/sembla-ir/src/validate.rs` validates effects and resource claims independently, checks that each claimed resource is Ref-typed, and rejects duplicate claims, but it never verifies the PRD/DESIGN §5.1 coverage obligation that claims cover Ref-resource writes. A transition can `SetAttr` on a Ref-typed attribute with an empty `contests` list and still validate. `crates/sembla-ir/tests/validation.rs` tests duplicate claims only; it has no missing-claim coverage test.

Required revision: define the v0.1 mapping from a Ref-typed write/resource to its required claim, reject uncovered transitions with a path-bearing error, and add a focused failing/validating test (and fixture if desired).

## Acceptance criteria

1. **Tests: PASS.** `cargo test --workspace` passes, including golden, invalid-fixture, semantic validation, and CLI integration tests.
2. **CLI fixtures: PASS.** `examples/two_state.json` exits 0; all six invalid fixtures exit 1 and emit path-bearing stderr.
3. **Canonical serialization: PASS.** Golden byte comparison and parse→serialize→parse→serialize idempotence tests pass.
4. **Agg infect pattern: PASS.** The constructed employer-Ref/health-I count expression validates.
5. **rule_id: PASS.** IDs are global `u32` values assigned by box/transition declaration order and tested across boxes.
6. **Params: PASS.** Declared Param typing, unresolved name, duplicate declaration, prior arity, and valid prior metadata are covered.
7. **Rustdoc: PASS.** Required types document the relevant DESIGN.md sections and the no-inlining parameter rule; rustdoc builds with warnings denied.

## Additional evidence

- Serde model, parser, canonical serializer, reference/type validation, output builders, and wire schema equality are implemented.
- `scripts/check.sh`, clippy with warnings denied, formatting, tests, and rustdoc pass.
- No execution, RNG, Lean, birth/death, scheduled-clock, or Level B/C implementation was introduced.
