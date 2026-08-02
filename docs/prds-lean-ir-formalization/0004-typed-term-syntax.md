# PRD 0004: Define typed expressions, aggregates, effects and claims

## Dependencies

PRDs 0001–0003 accepted.

Binding contract: [track README](../prds-lean-ir-formalization/README.md).

## Context

The checked domains can now index syntax so unresolved names, cross-schema
substitutions and type errors are absent from evaluators. PRD 0003 established
the concrete `ModelSchema`, owner-indexed table schemas, finite identifiers and
`ScalarSort`/`ScalarValue` families that this PRD must use rather than replacing
with parallel contexts.

This PRD owns syntax-level input and aggregate signatures only. A
post-acceptance ownership clarification assigns static resolved checked
output-field/builder declarations to PRD 0006, because PRD 0009 must prove its
observation builders are accepted by that checker. PRD 0012 continues to own
input-snapshot values and traversal, output values and materialization. This
clarification does not change the accepted PRD 0004 implementation or move any
evaluation behavior earlier.

## Goal

Define complete intrinsically typed term-level syntax and exact erasure for the
current V1 expressions, aggregate operators, transition-term fields, effects and
contest claims.

## Requirements

1. Define expression syntax indexed by the accepted `ModelSchema`, current
   `TableTarget`, syntax-level input signature and result `ScalarSort`.
   Parameters, self attributes, enum variants, relational targets and references
   use the resolved identifiers introduced by PRD 0003; no syntax node stores an
   unresolved name as its semantic identity.
2. Define constructors for every current raw `IR.Expr` form: exact raw-preserving
   Real, Int, Boolean and enum literals; parameters; self attributes; arithmetic;
   equality and numeric comparisons; Boolean operators; enum tests; input
   aggregates; and relational aggregates.
3. Preserve raw Real encodings with `ScientificLiteral`, not an arbitrary runtime
   `ℝ`. Bridge each `ParamSort` to its corresponding `ScalarSort`. Enum literals
   and tests carry the owning enum schema and `VariantId`; self attributes carry
   their owning `AttributeId`.
4. Represent Int-to-Real coercion explicitly in typed syntax. Addition,
   subtraction and multiplication accept one numeric result sort after explicit
   coercion; division returns Real and coerces Int operands explicitly. Boolean
   operators require Boolean operands. Numeric ordering comparisons require
   numeric operands. Equality/inequality require the same sort, except mixed
   numeric operands may use the explicit coercion.
5. Define typed aggregate operators and syntax-level input/relational aggregate
   signatures. `count` returns Int. `sum` requires a numeric value expression and
   returns that numeric sort. Nested filter/value terms are indexed by the row
   schema they inspect. Relational aggregate join identifiers retain both table
   owners and require compatible reference targets.
6. Define a syntax-level transition-term bundle whose guard is Boolean, hazard is
   Real, and effects and claims are scoped to the transition's current table.
   Leave transition names, raw declaration resolution and checked-model assembly
   to PRDs 0005–0006.
7. Define effects whose destination is an `AttributeId` and whose value has
   exactly that attribute's `ScalarSort`; implicit assignment coercion is not
   allowed.
8. Define resource claims whose resource expression has a reference sort and
   retains its resolved `TableTarget`. Define ordering domains so race-time and
   keyed ordering remain explicit: race-time ordering has the Real domain, while
   key expressions may be Real, Int or enum. Boolean and reference keys are
   unconstructable. Compatibility between all
   claims for one resource and winner selection remain owned by PRDs 0006 and
   0015.
9. Keep raw `ClaimOrdering.key` representable and erasable even though the current
   surface syntax produces race-time ordering only. Record it as raw/checkable,
   not surface-producible.
10. Define erasure for every owned constructor with exact raw constructor,
    spelling and operand/list-order preservation. Inserted numeric coercions erase
    transparently to their original raw numeric subtree; erasure must not
    normalize scientific literals or reorder terms.
11. Prove every theorem family in the obligation matrix and add positive and
    compile-fail fixtures covering every owned constructor and typing boundary.

## Theorem and fixture obligation matrix

| Obligation | Required result |
| --- | --- |
| Result sort | Every typed expression has its unique indexed result sort; the syntax-level theorem is distinct from PRD 0006's checker-result uniqueness theorem. |
| Schema ownership | Parameter, attribute, variant, table and reference identifiers cannot be substituted across model, box, table or enum owners, even when structures or cardinalities coincide. |
| Numeric typing | Int-to-Real coercion is explicit; division is Real; Boolean/reference arithmetic and ordering are impossible. |
| Aggregate scope | Filter/value terms use the aggregate row schema; count is Int; sum preserves its numeric sort; incompatible relational join targets are impossible. |
| Transition terms | Guards are Boolean, hazards are Real, effects have exact destination sorts and claim resources have reference sorts. |
| Claim ordering | Key domains are exactly Real, Int or enum; Boolean/reference keys are impossible; ordering-domain information survives to later compatibility checking. |
| Erasure | Every owned raw constructor is reproduced with exact spelling and order; `ScientificLiteral` encodings are unchanged and inserted coercions erase transparently. |
| Constructor fidelity | Erasure of each non-coercion constructor has the corresponding raw constructor and preserves all recursively erased operands. |

Required positive fixtures include every `IR.Expr`, `IR.AggOp`, `IR.Aggregate`,
`IR.Effect` and `IR.ClaimOrdering` constructor plus every `IR.ResourceClaim`
field and the PRD-0004-owned transition-term fields. Include mixed Int/Real
arithmetic, Int division, each allowed claim-key domain, input aggregation and a
relational aggregate with resolved compatible join targets.

Compile-fail fixtures must reject at least cross-model and cross-table
identifiers, a foreign enum variant, Boolean/reference arithmetic, a non-Boolean
filter/guard, a non-Real hazard, an incompatible relational join, an assignment
with the wrong destination sort, a non-reference claim resource and
Boolean/reference claim keys.

## Allowed files

- `frontend/Sembla/Semantics/Syntax.lean`
- `frontend/Sembla/Semantics/SyntaxTests.lean`
- `frontend/Sembla/Semantics.lean`
- `frontend/Sembla.lean`
- `docs/design/lean-ir-coverage.md`

## Non-goals

- Raw declaration, expression or model checking.
- Evaluation, supplied-state validation or semantic errors.
- Input-snapshot values, row traversal or wire delivery.
- `IR.OutputField`/`IR.OutputBuilder` checked declarations or output
  materialization; PRD 0006 owns their static checked declarations and PRD 0012
  owns their traversal, values and materialization.
- Views or summaries as executable observations.
- Claim-set compatibility, contest winner algorithms or effect commit semantics.

## Test and proof guidance

Keep typed syntax independent of every evaluator and checker. Intrinsic indices
should carry semantic ownership; do not recover it later through Boolean side
conditions. Compile-fail examples supplement, but do not replace, named
uniqueness, fidelity and impossibility theorems.

Run at least:

- `cd frontend && lake build Sembla.Semantics.SyntaxTests`
- `bash frontend/scripts/check-proofs.sh`
- `python3 scripts/check-prd-allowlist.py <this PRD at its current path>`
- `bash scripts/check.sh`
- `git diff --check`

## Acceptance criteria

1. Every syntax/meaning item assigned owner PRD 0004 in
   `docs/design/lean-ir-coverage.md` has typed syntax, exact erasure and linked
   fixture/theorem evidence; output fields/builders remain outside this PRD and
   are split between PRD 0006 static checking and PRD 0012 evaluation.
2. Expressions, aggregates, transition-term fields, effects and claims use the
   accepted PRD 0003 schema/identifier domains with the indices required above.
3. Every owned raw constructor erases exactly; raw scientific encodings and
   operand/list order are preserved, while inserted coercions erase
   transparently.
4. Every theorem family in the obligation matrix passes the automated axiom
   audit without a weakened statement.
5. The required positive and compile-fail fixture families pass, including mixed
   numeric, aggregate-scope, assignment and claim-domain cases.
6. No raw checking, evaluation, state validation, output materialization or
   contest algorithm is introduced.
7. Build, proof hygiene, PRD allowlist and full repository checks pass within the
   allowed module map.
