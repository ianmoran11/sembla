# PRD 0013: Define views, grouped views and summaries

## Dependencies

PRDs 0001–0012 accepted.

Binding contract: [track README](../prds-lean-ir-formalization/README.md).

## Context

Observation declarations are sinks over state and finite histories. Their edge behavior and non-feedback property must be explicit before transition traces are introduced.

## Goal

Give all current scalar/grouped views and summary reducers deterministic meanings with frozen empty-input behavior.

## Requirements

1. Define scalar view sum/count/min/max and optional filters/values.
2. Define grouped-view keys, integer bands, group identity/order and empty grouping.
3. Define summary sum/min/max/last/argmaxTick, with earliest-tick tie behavior for argmax.
4. Implement the README empty-reduction table exactly: count/sum zero; no empty groups; min/max/last/argmaxTick explicit error.
5. Define observations as projections that neither mutate state nor consult draw coordinates.
6. Prove determinism, type/schema preservation, grouping partition/total laws, earliest-argmax uniqueness, summary-fold determinism and observation purity.
7. Add fixtures for every reducer, empty input, ties, bands and errors.

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

Observation purity must mention the set/trace of consulted draw coordinates, not a nonexistent stream position.

## Acceptance criteria

1. Every view/grouped-view/summary constructor and edge case has semantics and fixtures.
2. Partition, total, tie, determinism and purity theorems pass the audit.
3. No observer can consult a draw by definition.
4. Build, proof hygiene and full checks pass.
