# PRD 0015: Define resource claims and deterministic contest winners

## Dependencies

PRDs 0001–0014 accepted.

Binding contract: [track README](../prds-lean-ir-formalization/README.md).

## Context

Candidates may claim multiple resources. Current V1 resolves each resource independently and accepts a candidate only if it wins every claim; crossing claims can therefore defer all candidates.

## Goal

Formalize that exact deterministic contest rule before effects/commit are added.

## Requirements

1. Define semantic resources from checked typed references and claim compatibility.
2. For each resource, require compatible ordering domains, choose the lower race/key value, and break ties by stable transition/row identity.
3. Select one unique winner per nonempty compatible resource claimant set.
4. Mark a candidate accepted iff it wins every claim; candidates with no claims are accepted. Incompatible order domains produce explicit error.
5. Count/record deferred candidates without changing future semantic state here.
6. Prove per-resource winner uniqueness, fixed-input determinism, traversal/permutation invariance under identity preservation, and the accepted-iff-wins-all characterization.
7. Add race, numeric/enum key, tie, incompatible, multi-resource and crossed-claim fixtures where all candidates defer.

## Allowed files

- `frontend/Sembla/Semantics/Contest.lean`
- `frontend/Sembla/Semantics/ContestTests.lean`
- `frontend/Sembla/Semantics.lean`
- `frontend/Sembla.lean`
- `docs/design/lean-ir-coverage.md`
- `docs/design/lean-ir-semantics.md`

## Non-goals

- Effects, write conflicts or state commit.
- New global optimization/maximal-matching rule.

## Test and proof guidance

Do not claim one winner per connected conflict component; the theorem is one winner per resource plus all-claims acceptance.

## Acceptance criteria

1. The frozen multi-resource rule is executable and fixture-covered.
2. Uniqueness, determinism, invariance and characterization theorems pass the audit.
3. Crossing and incompatible cases have explicit outcomes.
4. Build, proof hygiene and full checks pass.
