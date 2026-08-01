# PRD 0005: Check declarations, parameters and references

## Dependencies

PRDs 0001–0004 accepted.

Binding contract: [track README](../prds-lean-ir-formalization/README.md).

## Context

Raw models must first establish global/local namespaces and schemas before term checking can resolve expressions.

## Goal

Implement and prove the declaration/reference fragment of raw checking.

## Requirements

1. Define stable structured check-error categories/paths; exact wording is not a theorem target.
2. Check exact positive `dt`, unique model/parameter/box/summary names, and unique local table/attribute/enum/transition/port/view names.
3. Check parameter defaults, integer-prior exclusion, current two-argument prior metadata and ordered Uniform bounds. Classify priors as structural metadata with no sampling denotation.
4. Resolve table/attribute/ref domains, enum variants and finite schemas.
5. Define a declarative `DeclarationsWellFormed` judgment and a terminating checker returning the checked declaration context.
6. Prove soundness and completeness for this fragment and exact erasure of accepted declarations—no normalization allowance.
7. Add positive/negative fixtures for every declaration error category, including zero/negative `dt`.

## Allowed files

- `frontend/Sembla/Semantics/CheckDeclarations.lean`
- `frontend/Sembla/Semantics/CheckDeclarationsTests.lean`
- `frontend/Sembla/Semantics.lean`
- `frontend/Sembla.lean`
- `docs/design/lean-ir-coverage.md`
- `docs/design/lean-ir-semantics.md`

## Non-goals

- Expression/effect/observation checking.
- Changing accepted prior syntax or defining probability distributions.
- Composition-source namespaces.

## Test and proof guidance

The theorem signatures must relate the Boolean/`Except` checker directly to the declarative judgment. Negative fixtures assert categories and paths, not prose.

## Acceptance criteria

1. The checker decides exactly `DeclarationsWellFormed`.
2. Soundness, completeness and exact-erasure theorems pass the automated audit.
3. Every owned invariant/error has a fixture and coverage link.
4. Build, proof hygiene and full checks pass.
