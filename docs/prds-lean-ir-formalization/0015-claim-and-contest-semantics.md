# PRD 0015: Define resource claims and deterministic contest winners

## Dependencies

PRDs 0001–0014 accepted.

Binding contract: [track README](../prds-lean-ir-formalization/README.md).

## Context

Candidates may claim multiple resources. Current V1 resolves each resource independently and accepts a candidate only if it wins every claim; crossing claims can therefore defer all candidates.

## Goal

Formalize that exact deterministic contest rule before effects/commit are added.

## Requirements

1. Evaluate each candidate's source-ordered checked claims against the candidate's
   retained pre-tick state/row/input context. Evaluate the typed reference
   resource first; for `.raceTime`, use the candidate race value and retained
   `.real` domain; for `.key`, evaluate the retained typed key expression and
   preserve its exact `orderingDomain`.
2. Define a semantic resource from the evaluated typed reference and define
   compatibility as exact retained `OrderingDomain` equality among actual
   claimants for that resource. Race-time and Real-key claims are compatible;
   Int and Real are incompatible; owner-indexed enum keys are compatible only in
   the same retained enum domain.
3. Traverse candidates and each candidate's claims in preserved source order for
   evaluation and first-error precedence, retaining claim source ordinals in
   errors/traces. Once claims are evaluated, choose the lower race/key value and
   break ties by stable transition/row identity, independently of claimant
   traversal order.
4. Select one unique winner per nonempty compatible resource claimant set.
5. Mark a candidate accepted iff it wins every claim; candidates with no claims
   are accepted. Incompatible ordering domains or resource/key evaluation
   failures produce explicit local errors.
6. Count/record deferred candidates without changing future semantic state here.
7. Prove per-resource winner uniqueness, fixed-input determinism, evaluated-claim
   source fidelity, winner traversal/permutation invariance under identity
   preservation, and the accepted-iff-wins-all characterization.
8. Add mixed race-time/Real-key, Int-vs-Real incompatible, same-enum compatible,
   distinct-owner-enum incompatible, tie, resource/key evaluation error,
   multi-resource and crossed-claim fixtures where all candidates defer.

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
3. Crossing, exact-domain compatibility and claim-evaluation error cases have
   explicit outcomes.
4. Build, proof hygiene and full checks pass.
