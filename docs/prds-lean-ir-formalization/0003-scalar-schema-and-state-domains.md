# PRD 0003: Define scalar, schema and finite-state domains

## Dependencies

PRDs 0001–0002 accepted.

Binding contract: [track README](../prds-lean-ir-formalization/README.md).

## Context

Reasoning needs typed values, resolved finite names and valid finite state before
typed expression syntax can be introduced. This PRD is the representation gate
for later syntax, checking and evaluation work, so it must distinguish exact raw
syntax from mathematical values and valid dependent state from malformed
supplied data.

## Goal

Define the foundational checked domains and prove their lookup, typing, order
and finite-cardinality invariants, without defining expression, transition or
state-evaluation behavior.

## Requirements

1. Define scalar sorts and runtime values for mathematical reals, integers,
   Booleans, finite enums and typed table references. Keep runtime mathematical
   values distinct from raw-origin payloads that must erase exactly.
2. Define exact scientific denotation
   `coefficient × 10^exponent : IR.Scientific → ℝ`, including negative
   exponents. Prove that numerically equal encodings have equal denotation while
   raw-origin checked literals retain their original `IR.Scientific` encoding
   for exact erasure. Arbitrary runtime `ℝ` values have no inverse erasure to
   `IR.Scientific`.
3. Define ordered, proof-carrying schema contexts and finite resolved
   identifiers with these scopes: parameters are model-global, tables are
   box-local, attributes are table-local and enum variants are attribute-local.
   A reference target retains its resolved box/table identity; equal names,
   schemas or cardinalities in another context are not interchangeable.
4. Support forward and mutual table references through two-phase schema
   construction: establish the ordered unique table universe before resolving
   attribute schemas against it. PRD 0003 defines the checked domains and their
   constructors; PRD 0005 owns checking raw declaration lists and constructing
   these domains with structured errors.
5. Enum schemas are nonempty, duplicate-free and ordered. Preserve declaration
   order once and derive finite lookup from that representation rather than
   maintaining unrelated ordered and extensional structures.
6. Treat each validated V1 table's `sizeHint` as its exact finite execution-state
   cardinality. It fixes the valid row ordinal type but does not initialize row
   values. A valid row ordinal for table `t` is bounded by that table's
   `sizeHint`; zero-sized tables are valid and have no row ordinals.
7. Define schema-indexed typed rows and valid model state. Stored enum values and
   references are intrinsically typed; a valid reference is indexed by its
   target table and contains an in-range target row ordinal. Do not add
   active/retired-row, vacancy, generation or liveness machinery.
8. Define an explicitly unvalidated supplied-state boundary capable of carrying
   wrong row counts, column/value types, enum ordinals and reference ordinals.
   Do not implement its validation or error precedence here: PRD 0011 owns
   conversion to valid state plus `invalidState`/`invalidReference` behavior and
   state lookup operations.
9. Define exact erasure for raw-origin parameter, attribute and table/schema
   structures, preserving names, declaration order, `sizeHint`, enum order,
   reference target names and original scientific encodings. There is no
   `Sembla.IR` state value to erase to; for typed rows and valid state, prove
   extensional reconstruction/equality laws instead of claiming raw IR erasure.
10. Prove the theorem families in the obligation matrix below and add positive
    examples for every sort, zero/nonzero table cardinalities, same table names
    in different boxes, forward references and exact scientific denotation.
    Add checked compile-fail evidence for cross-context values/references using
    an in-module expected-error mechanism such as `#guard_msgs`; do not add an
    unlisted standalone negative-test module.

## Theorem and fixture obligation matrix

| Obligation | Required result |
| --- | --- |
| Scientific denotation | The denotation equation holds for every encoding; distinct encodings such as `1 × 10^0` and `10 × 10^-1` have equal denotation while each erases to its original syntax. |
| Sort indexing | Every typed value has its unique declared sort; cross-sort construction is impossible. |
| Ordered lookup | Parameter, table, attribute and enum lookups are deterministic and unique, and successful lookup agrees with the ordered finite identifier. |
| Erasure | Erasing each raw-origin checked declaration reproduces the exact ordered raw parameter/table/schema structure with no normalization. |
| Finite bounds | Every row and enum ordinal satisfies its owning `sizeHint`/variant bound; a zero-sized table has no row identifier. |
| Row projection | Projecting a typed row by an attribute identifier returns a value of exactly that attribute's sort. |
| Reference identity | A reference preserves its box/table target and row bound; equal cardinality or structural schema equality does not permit cross-context substitution. |
| State extensionality | Valid rows, tables and model states are equal when all schema-indexed projections are equal; reconstruction from those projections is exact. |

Every named theorem or lemma introduced for this matrix must pass the automated
axiom inventory. Computed fixtures and expected typing failures supplement but
do not replace these theorems.

## Allowed files

- `frontend/Sembla/Semantics/Types.lean`
- `frontend/Sembla/Semantics/State.lean`
- `frontend/Sembla/Semantics/TypesTests.lean`
- `frontend/Sembla/Semantics.lean`
- `frontend/Sembla.lean`
- `docs/design/lean-ir-coverage.md`

## Non-goals

- Typed expression syntax or checking.
- Raw declaration checking, diagnostic categories or error precedence.
- State lookup/evaluation, supplied-state validation or semantic error results.
- State evolution, row allocation, vacancy or retirement.
- Priors as distributions.
- A generic serializer for arbitrary mathematical runtime values or state.

## Test and proof guidance

Prefer schema-indexed finite/dependent types that make cross-context values,
resolved identifiers and valid references unconstructable. Preserve source order
in one proof-carrying representation and derive lookup from it. Avoid a bare
`Fin n` when the owning schema/table is not retained in the type. Do not use
runtime row-liveness conventions.

The representation may choose practical Mathlib finite containers, but it must
meet the observable typing, ordering, erasure and extensionality obligations
above without equality-transport axioms or opaque proposition placeholders.

## Acceptance criteria

1. Every scalar/schema/state coverage item assigned meaning owner PRD 0003 in
   `docs/design/lean-ir-coverage.md` has a checked definition and the exact
   denotation, erasure or extensional-state law assigned above.
2. Scientific denotation preserves mathematical equality without losing the raw
   source encoding required by declaration erasure.
3. Parameter/table/attribute/enum scopes, two-phase forward-reference support
   and the PRD 0003/0005 ownership boundary are explicit in definitions and
   fixtures.
4. Valid state uses each table's `sizeHint` as exact cardinality; typed rows and
   references are bounded and cross-context substitution fails by typing.
5. The unvalidated supplied-state boundary remains representable for PRD 0011,
   while this PRD introduces no validation/error semantics.
6. Every theorem family in the obligation matrix passes the automated axiom
   audit, and expected-error fixtures demonstrate the required typing failures
   within the allowed module map.
7. Build, proof hygiene, PRD allowlist and full repository checks pass.
