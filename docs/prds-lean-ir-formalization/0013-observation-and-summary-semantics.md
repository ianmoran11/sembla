# PRD 0013: Define views, grouped views and summaries

## Dependencies

PRDs 0001–0012 accepted.

Binding contract: [track README](../prds-lean-ir-formalization/README.md).

## Context

Observation declarations are sinks over state and finite histories. Their edge
behavior and non-feedback property must be explicit before transition traces are
introduced. `DrawCoordinate` is first owned by PRD 0014, so this increment states
purity structurally by giving observers no oracle argument; coordinate-trace
noninterference is deferred to PRD 0017.

## Goal

Give all current scalar/grouped views and summary reducers deterministic meanings with frozen empty-input behavior.

## Requirements

1. Define scalar view sum/count/min/max and optional filters/values.
2. Define grouped views as count-only observations. Group-key tuples follow
   declaration order; Enum and Ref keys retain their typed ordinal identities;
   integer bands use Euclidean division, including negative values; emitted
   groups use deterministic lexicographic key order; zero-count groups are
   omitted; empty input emits no groups.
3. Define summaries over ordinary scalar views only, matching the checked
   `viewOrdinal` target. Their input is a finite history carrying strictly
   increasing absolute `Nat` tick labels together with values; this representation
   must be reusable by PRD 0017 without editing `Summary.lean`. Define
   sum/min/max/last/argmaxTick, with earliest-absolute-tick tie behavior for
   argmax. `argmaxTick` returns the least maximizing absolute tick label as Int;
   other reducers retain the targeted view's numeric sort.
4. Implement the README empty-reduction table exactly: count and typed sum return
   zero; no empty groups are emitted; min/max/last/argmaxTick return the local
   `emptyReduction` error. The typed-zero summary behavior intentionally differs
   from the current Rust executor's reject-all-empty implementation; this PRD
   makes no Rust-refinement claim.
5. Define observations as projections that neither mutate state nor accept a draw
   oracle or draw-coordinate argument.
6. Prove determinism, type/schema preservation, grouping partition/total/order
   laws, absolute-history well-formedness, earliest-argmax uniqueness, summary
   target/result-sort correctness, summary-fold determinism and structural
   observation purity.
7. Add fixtures for every reducer, empty input, earliest ties, positive and
   negative bands, Enum/Ref group keys, lexicographic group order and errors.

## Allowed files

- `frontend/Sembla/Semantics/Observation.lean`
- `frontend/Sembla/Semantics/Summary.lean`
- `frontend/Sembla/Semantics/ObservationTests.lean`
- `frontend/Sembla/Semantics.lean`
- `frontend/Sembla.lean`
- `docs/design/lean-ir-coverage.md`

## Non-goals

- Multi-tick noninterference or transition behavior.
- Group-by/broadcast optimizer syntax not present in V1.

## Test and proof guidance

Observation purity is established here by the observer API having no oracle
argument. PRD 0017 must lift that structural fact to equality of consulted
`DrawCoordinate` sets/traces; do not introduce a stream position.

## Acceptance criteria

1. Every view/grouped-view/summary constructor and edge case has semantics and fixtures.
2. Partition, total/order, tie, result-sort, determinism and structural purity
   theorems pass the audit.
3. No observer can consult a draw by definition.
4. Build, proof hygiene and full checks pass.
