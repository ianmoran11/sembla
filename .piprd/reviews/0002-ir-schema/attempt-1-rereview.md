# PRD 0002 Re-review — Attempt 1

## Assessment

**APPROVED** — no blocking issues remain.

## Acceptance criteria

1. **Workspace tests: PASS.** `./scripts/check.sh` passed formatting, clippy with warnings denied, and 15 workspace tests, including golden, invalid-fixture, CLI, aggregate, rule-ID, parameter, and contest-coverage tests.
2. **CLI validation: PASS.** `examples/two_state.json` exited 0. Each of the six `examples/invalid/*.json` fixtures exited 1 and printed a path-bearing error to stderr.
3. **Canonical serialization: PASS.** Golden byte comparison and parse→serialize→parse→serialize idempotence tests pass.
4. **Agg infect pattern: PASS.** The employer-Ref/health-I count aggregate is constructed and validates.
5. **rule_id: PASS.** IDs are global `u32` values assigned in box/transition declaration order and tested across boxes.
6. **Parameters: PASS.** Param resolution/type checking, duplicate names, prior arity, valid prior metadata, and named fixture errors are covered.
7. **Rustdoc: PASS.** Required types cite the relevant DESIGN.md sections and document symbolic, never-inlined parameters. Rustdoc builds with warnings denied.

## Resolved prior blocker

`crates/sembla-ir/src/validate.rs` now requires every Ref-typed `SetAttr` write to have a claim whose resource is structurally equal to the effect value. Missing coverage reports the exact effect-value path. `crates/sembla-ir/tests/validation.rs` covers missing, matching, mismatched, and duplicate claims, and `ResourceClaim` rustdoc records the convention.

## Other evidence

- Six distinct invalid fixtures exceed the minimum of five.
- Serde types, parser, canonical serializer, references, expression typing, output builders, wire schema validation, and validated rule metadata are present.
- No execution, RNG, Lean, birth/death, scheduled-clock, or Level B/C implementation was added.
- `git diff --check` passes. The unrelated `.DS_Store` and managed `.piprd*` state are not blockers.
