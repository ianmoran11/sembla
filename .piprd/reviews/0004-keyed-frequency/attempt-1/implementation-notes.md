# PRD 0004 implementation notes

## Implementation

- Added the exact atomic `freq (<predicate>) over <ref>` surface form plus rejecting recovery forms for omitted keys and predicate parentheses.
- Added selected-system Ref-key validation with diagnostics that name the logical system, key attribute, and actual non-Ref type.
- Added recursive row-local predicate validation that rejects `inputSum`, `countBy`, `sizeBy`, nested `freq`, and unknown row/model names during Lean elaboration with the required teaching message.
- Kept `elaborateExpr`'s mandatory `SurfaceSystem` argument, so no public contextless expression host was introduced.
- Extracted `keyedCountTerm` and routed legacy `countBy`, legacy `sizeBy`, and `freq` through the same table/key/self-key construction.
- Lowered `freq` to the existing `Expr.div` of keyed count by keyed size with no IR or runtime changes.

## Positive coverage

Added and imported `frontend/Sembla/FrequencyTests.lean`, covering:

- direct expression-tree equality for keyed numerator/denominator order and strings;
- exact complete `Model` equality and literal serialized `Sembla.IR.toJson` string equality;
- a complete SIR-shaped inferred-arrow model;
- an explicit-system arrow with `∧` and a model parameter;
- derived table/key names and an explicit selected-table name override;
- deterministic expanded aggregate spelling in hazard widget data; and
- equal `StateDiagramProps` and `HazardPanelProps` for frequency/legacy twins.

## Negative coverage

Added complete exact positioned failures for unknown keys; Real, Int, and enum non-Ref keys; non-Boolean predicates; unknown row names; input aggregates; nested `countBy` and `sizeBy`; nested `freq`; missing keys; and omitted predicate parentheses. All are registered with `check_failure_exact`.

## Validation

All passed:

- `cd frontend && lake build`
- `cd frontend && bash scripts/test-negative.sh`
- `bash frontend/scripts/check-parity.sh`
- `./scripts/check.sh`
- `git diff --check`
- frozen-path diff over examples, IR/JSON, canonical models, manifests, Rust, CI, and docs

No canonical model/fixture, IR/JSON, Rust, dependency, CI, public documentation, or widget styling file changed. No commit was created.
