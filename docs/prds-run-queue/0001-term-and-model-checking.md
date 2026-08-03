# PRD 0006: Check terms and assemble checked models

## Dependencies

PRDs 0001–0005 accepted.

Binding contract: [track README](../prds-lean-ir-formalization/README.md).

## Context

PRD 0005 provides an exact `DeclarationContext`, accepted `ModelSchema`,
box-owned port schemas, resolved transition table targets and the original raw
source. PRD 0004 provides owner-indexed typed expressions, aggregates, effects,
claims and exact syntax erasers. This increment must connect those accepted APIs
without changing their representations or accepted checking decisions. A narrow
additive compatibility bridge in PRD 0005's module is permitted only to expose
source-order and erasure theorems already established by its private builders.

This PRD owns the remaining **model-local static** raw-to-checked boundary:
terms, transitions and resolved checked declarations for outputs, ordinary
views, grouped views and summaries. Later PRDs own their evaluation and
behavior. Wires are preserved exactly but deliberately not validated here;
wire structure remains PRD 0019 work.

## Goal

Complete canonical raw-to-checked elaboration for the owned V1 model fragment,
with independent judgments, structured failures, exact reconstructive erasure,
and non-vacuous checked round-trip proofs.

## Binding ownership and deferment table

| Area | PRD 0006 obligation | Deferred owner |
| --- | --- | --- |
| Declarations | Invoke and consume PRD 0005's `checkDeclarations`/`DeclarationContext`; do not duplicate its namespace, schema, prior or `dt` checks | PRD 0005 accepted |
| Expressions and aggregates | Elaborate every owned raw `Expr`, `AggOp` and `Aggregate` occurrence into PRD 0004 syntax | Evaluation PRDs 0010–0012 |
| Transitions | Use the resolved source transition/table identity; check guard, hazard, effects and individual claims | Candidate, contest and commit behavior PRDs 0014–0016 |
| Outputs | Resolve builder table; check field/schema order, names and sorts, count/sum operands and Boolean filters; produce checked declarations | Traversal, values and materialization PRD 0012 |
| Ordinary views | Resolve table; check reducer/value shape, numeric result sort and Boolean filter; produce checked declarations | Denotation and empty behavior PRDs 0013/0017 |
| Grouped views | Resolve table/key attributes; check the frozen V1 key/band shape and Boolean aggregate-free filter; preserve key order | Grouping/materialization PRDs 0013/0018 |
| Summaries | Resolve box and ordinary scalar-view target; retain reducer exactly | Fold/error behavior PRDs 0013/0017 |
| Wires | Preserve the raw `List IR.Wire` structurally exactly and in source order; perform no endpoint, direction, schema, fan-in, delay or delivery check | Structural validity PRD 0019; composition behavior after this track |
| Contests | Check each claim independently, reject duplicate resources within one transition, and retain its ordering domain | Compatibility among actual claimants for a resolved resource and winner selection PRD 0015 |
| State, draws and evaluation | None | PRDs 0010–0017 |

`ModelWellFormed` means declarations plus every model-local static obligation
owned in this table. It intentionally excludes wire validity. PRD 0019 must
combine it with its separate wire/plan structural judgment rather than silently
strengthening this one.

## Required checked structures and checker architecture

0. Before constructing checked terms, expose additive public coherence theorems
   from `CheckDeclarations.lean` proving: input and output header/schema length,
   ordinal, name and raw-schema correspondence; full
   `BoxPortSchema.instantiate` attribute erasure; source-order correspondence
   between catalog box identities and `DeclarationContext.source.boxes`; and
   exact `ModelSchema.eraseTables` correspondence with each selected source box.
   These theorems must derive from the existing builders and may not change
   declaration validity, diagnostics, structures, schemas or lookup behavior.
1. Define a `TermContext` from the accepted `DeclarationContext`, box identity,
   current table target and the box's source-ordered input-port schemas,
   instantiated at that current table as PRD 0004 `InputSignature`s. Use PRD
   0005 identifiers and lookups directly; do not create parallel name maps or
   replacement schemas.
2. Define an existential packed checked-expression result carrying its unique
   result sort and intrinsic PRD 0004 expression.
3. Define source-ordered checked structures equivalent to:
   - `CheckedTransition`, retaining source ordinal/header identity, resolved
     table target, derived input signature and `TransitionTerms`;
   - `CheckedOutputField`/`CheckedOutputDecl`, retaining source field ordinal and
     name, corresponding output-schema attribute identity/sort, typed aggregate
     operation and optional Boolean filter over the resolved builder table;
   - `CheckedViewDecl`, retaining resolved table, optional Boolean filter,
     reducer-indexed value shape and result sort;
   - `CheckedGroupKey`/`CheckedGroupedViewDecl`, retaining resolved table and
     attribute identities, band evidence/shape, filter and source key order;
   - `CheckedSummaryDecl`, retaining resolved box and ordinary-view identity plus
     the raw reducer;
   - a dependent, source-ordered `CheckedBox` indexed by its PRD 0005 box
     identity; and
   - `Checked.Model`, containing the PRD 0005 context, source-ordered checked
     boxes, checked summaries and only the explicitly deferred raw wire list.
4. Implement terminating canonical term checkers and
   `checkModel : IR.Model → Except ModelCheckError Checked.Model`.
   `checkModel` must first call PRD 0005's checker and lift any declaration
   failure unchanged into the model-check error wrapper.
5. Define erasers for every checked structure. Whole-model erasure must
   reconstruct all owned transition and observation payloads from checked
   components and PRD 0004 erasers. It may recover accepted declaration spelling
   fields from the PRD 0005 context and copy the deferred wire list, but may not
   define `Checked.Model.erase` as `ctx.source` or otherwise bypass elaborated
   terms.
6. Preserve exact raw constructor choice, names, scientific encodings, operand
   and list order. Checker-inserted Int-to-Real coercions erase transparently;
   no normalization or reordering is permitted.

## Independent judgments and canonical bidirectional rules

Define independent syntax-directed relations equivalent to:

- `ExprSynthesizes Γ raw sort term`;
- `ExprChecks Γ raw expected term`;
- `AggOpSynthesizes` and `AggregateSynthesizes`;
- `EffectWellTyped`, `ClaimWellTyped` and `TransitionWellTyped`;
- `OutputWellFormed`, `ViewWellFormed`, `GroupedViewWellFormed` and
  `SummaryWellFormed`;
- `BoxTermsWellFormed`; and
- `ModelWellFormed`.

None may be an alias for checker success, contain the executable checker result
as a field, or be defined merely by existence of a successful checker result.

The checker and judgments must freeze these rules:

1. Real, Int, Boolean, parameters, self attributes, enum tests and aggregate
   forms synthesize canonically. A bare enum literal checks only against an
   expected enum owner. Equality/inequality may synthesize one side and use its
   sort to check an otherwise unanchored enum literal on the other. Two
   unanchored enum literals fail with `cannotInferEnumOwner`.
2. Arithmetic performs canonical mixed Int/Real promotion with explicit
   coercions; division returns Real. Boolean operators require Boolean terms,
   ordering requires numeric terms, and equality requires one common sort after
   numeric promotion or enum-owner anchoring.
3. Aggregate filters are Boolean. `count` returns Int. `sum` requires a numeric
   value and preserves its numeric sort. Input aggregates use the resolved port
   schema. To preserve the current V1 raw boundary, an input aggregate's filter
   and sum value recursively contain neither `IR.Expr.input` nor `IR.Expr.agg`;
   nested occurrences fail at their complete aggregate/filter/value path.
   Relational aggregates resolve both table/attribute joins and require equal
   reference targets; this input-row restriction does not silently extend to
   relational aggregates.
4. Guards are Boolean and hazards are Real. Static checking does not inspect a
   hazard's numeric sign: a negative Real literal is statically accepted and is
   the dynamic error owned by PRD 0014.
5. An effect destination resolves in the transition's current table. Its RHS
   uses an exact-result boundary: ordinarily synthesize it and require equality
   with the destination sort; a bare enum literal instead checks against the
   destination's expected enum owner. A raw Int term is therefore rejected for a
   Real destination, and expected-sort checking may not add a top-level numeric
   assignment coercion. Checker-inserted coercions occur only inside mixed
   numeric operations and division, whose synthesized outer sort is already
   Real. Output-sum/schema, guard, hazard and filter boundaries use the same
   exact-result rule, with expected checking only where enum-literal ownership
   must be anchored.
6. Every claim resource is Ref-typed. Race-time ordering retains Real as its
   ordering domain; key ordering accepts exactly Real, Int or enum. Boolean and
   Ref keys are rejected. Duplicate claim resources within one transition are
   rejected by structural raw-expression equality.
7. Every effect writing a Ref destination requires at least one claim whose
   resource expression is structurally equal to that effect's raw RHS. This is
   syntactic coverage of the new referenced resource, not evaluation or semantic
   equality. Claim order and unrelated claims do not affect coverage.
8. Do not require all possible claims to share one key domain. Heterogeneous
   individually valid ordering domains are statically accepted; PRD 0015 checks
   compatibility only among actual claimants for one evaluated resource.
9. Synthesis-result uniqueness applies to canonical synthesis derivations and
   checker results. Expected checking is not sort-unique because a bare enum
   spelling may be valid under distinct explicitly supplied enum owners; it is
   not a route for top-level numeric promotion.

## Frozen static observation boundary

1. An output's per-table builder target resolves in its box. Builder fields and
   output schema have equal length and correspond one-to-one in the same raw
   order; each field name equals the corresponding schema attribute name.
   Duplicate builder-field names are rejected. A count field has Int sort; a sum
   field requires a numeric value and must exactly match its corresponding Int
   or Real schema sort. Every optional field filter is Boolean.
2. An ordinary view table resolves. `count` has no value and result sort Int.
   `sum`, `min` and `max` each require a numeric value and preserve that value's
   Int or Real sort. Every optional view filter is Boolean.
3. A grouped-view table resolves and it has between one and four keys inclusive.
   Each key attribute resolves in that table. Enum and Ref keys have no band;
   Int keys require `some bandWidth` with `0 < bandWidth`; Real keys are rejected.
   Repeated key attributes are not rejected by this PRD, and source key order is
   preserved. The optional filter is Boolean and, for current V1 compatibility,
   recursively contains neither `IR.Expr.input` nor `IR.Expr.agg`.
4. A summary resolves a box and an ordinary scalar view in that box. A grouped
   view with the same spelling is not a valid summary target. The summary
   reducer is retained exactly; its fold and empty-input behavior remain later
   semantics.
5. Restrictions enforced only by current Lean authoring macros outside the rules
   above—such as rejecting aggregate expressions in effects—remain adapter
   behavior and are not raw-checker validity rules.

## Structured error and path contract

Define a new model-check wrapper rather than extending PRD 0005's closed
inductives. It must include `declaration CheckErrorCategory` plus term/model
categories sufficient to distinguish at least:

- unknown parameter, attribute, enum variant, input, table and join attribute;
- nested aggregate in an input-aggregate filter or value;
- `cannotInferEnumOwner`, expected Bool/Real/numeric/reference/orderable,
  sort mismatch, incompatible equality and incompatible join targets;
- duplicate resource claim and unclaimed Ref write;
- unresolved output table, duplicate output field, field/schema length or name
  mismatch and output field sort mismatch;
- unresolved view table and invalid reducer/value shape;
- invalid grouped key count, unresolved key, invalid key sort, missing,
  unexpected or nonpositive band, and aggregate in grouped filter; and
- unresolved summary box or ordinary-view target.

Paths must retain source indices and recursively identify at least model, box,
transition, output, ordinary view, grouped view and summary; guard/hazard;
effects/effect index/destination/value; contests/claim index/resource/ordering
key; output builder/table/schema/fields/field index/name/op/filter/value; view
table/filter/value/reducer; grouped keys/key index/attribute/band; summary box,
view and reducer; and expression lhs/rhs/operand, aggregate filter/value, input
port, table target and join attributes.

For single-defect fixtures the category and full structured path are stable.
Diagnostic prose and precedence among simultaneous independent defects remain
non-normative.

## Required theorem matrix

The named theorem family must include statements equivalent to:

| Obligation | Required statement |
| --- | --- |
| Term synthesis soundness | A successful synthesis result yields the independent synthesis judgment, its returned sort, and exact raw erasure. |
| Term checking soundness | A successful expected-sort check yields the independent checking judgment and exact raw erasure. |
| Term completeness | Every declarative synthesis/checking derivation is reproduced by the canonical checker up to the checked-term equivalence defined here. |
| Synthesis sort uniqueness | Two canonical synthesis derivations/results for one raw term have equal result sorts. |
| Model soundness | `checkModel raw = .ok checked` implies `ModelWellFormed raw` and `checked.erase = raw`. |
| Model completeness | `ModelWellFormed raw` implies some `checked` with `checkModel raw = .ok checked`. |
| Failure characterization | Model-check failure is equivalent to failure of the independent model judgment; no unique category is required for simultaneous defects. |
| Checker canonicality | Every successful `checkModel` result satisfies the independently stated `Checked.Model.Canonical` invariant. |
| Erasure validity | Every canonical `Checked.Model` erases to a `ModelWellFormed` raw model. |
| Checked round trip | For canonical checked `m`, checking `m.erase` succeeds with some `m'` and `m' ≈ m`. |

Define `Checked.Model.Canonical` structurally; it must be inhabited by every
successful checker result and may not mention checker success in its definition.
Define `≈` before any round-trip theorem and prove it is an equivalence relation.
It must be a pointwise dependent structural relation preserving resolved owner
ordinals, declaration/constructor/list order, exact literals and observation
shapes, while ignoring proof witnesses and only checker-inserted transparent
coercions. It must not be defined as equality of erasures. Either make
canonicality an invariant of `Checked.Model` or state it explicitly as the
round-trip premise; do not claim round trip for arbitrary hand-constructed
noncanonical terms.

All named theorem/lemma declarations must pass the existing automated axiom and
opaque-proposition audit. Computed fixtures do not replace these statements.

## Required fixture matrix

Positive and exact-erasure fixtures must cover:

- all 22 raw `Expr` constructors, both aggregate operators, filtered and
  unfiltered aggregates, every effect and claim-ordering form;
- mixed Int/Real arithmetic, Int division, mixed numeric equality,
  expected-sort-anchored enum literals, input aggregates and compatible
  relational joins;
- Boolean guard, Real including negative-literal hazard, multiple effects and
  claims, every valid claim-key domain, and a Ref write with matching RHS claim;
- output count, Int sum, Real sum, filtered fields and exact schema/field order;
- every ordinary-view reducer and every allowed filter/value shape;
- grouped Enum, Ref and positive-banded Int keys, one-key and four-key boundaries
  and exact key order;
- every summary reducer resolving an ordinary view;
- same-named input/output ports, forward references inherited from PRD 0005,
  empty and zero-table boxes where applicable;
- exact non-normalized scientific encodings and every owned list order;
- heterogeneous valid claim ordering domains accepted statically; and
- a malformed/unresolved wire accepted and erased unchanged, plus whole-model
  exact erasure and checked round trip.

Single-defect negatives must assert category and structured path for every error
category above, including:

- unanchored enum literal, each invalid operand/result sort, unresolved names at
  each scope, incompatible joins, nested input aggregates in both filter and sum
  value, non-Boolean filters/guard, non-Real hazard, exact Int-to-Real effect
  rejection, duplicate claim, unclaimed Ref write and Boolean/Ref keys;
- output missing/extra/duplicate/reordered fields, exact Int-sum-to-Real-schema
  rejection, wrong count/sum destination
  sort and unresolved builder table;
- invalid view reducer/value combinations;
- grouped zero/five key counts, invalid key types, missing/unexpected/nonpositive
  bands and aggregate-bearing filters; and
- unresolved summary box, unresolved ordinary view and grouped-view-only target.

Maintain a constructor/category/path fixture table in
`docs/design/lean-ir-coverage.md` linking every owned item to executable evidence.

## Allowed files

- `frontend/Sembla/Semantics/CheckDeclarations.lean`
- `frontend/Sembla/Semantics/CheckDeclarationsTests.lean`
- `frontend/Sembla/Semantics/CheckTerms.lean`
- `frontend/Sembla/Semantics/CheckModel.lean`
- `frontend/Sembla/Semantics/CheckModelTests.lean`
- `frontend/Sembla/Semantics.lean`
- `frontend/Sembla.lean`
- `docs/design/lean-ir-coverage.md`
- `docs/design/lean-ir-semantics.md`

## Non-goals

- Editing accepted `Types.lean` or `Syntax.lean` APIs, or changing any accepted
  `CheckDeclarations.lean` representation, checker decision, diagnostic or
  lookup behavior. Only the additive coherence theorems explicitly required
  above are permitted; if they are insufficient, stop and amend this PRD again
  rather than introducing replacement schemas, secondary name maps, duplicate
  checks or impossible-mismatch errors.
- Evaluation, supplied-state validation, draw semantics, candidate generation,
  contest winners, effect commit, observation values, summary folds or frontend
  macro refactoring.
- Wire endpoint/schema/fan-in validation, delivery or composition-source linking.
- Normalization, sampling, `Float`/`f64`, or changing raw syntax/export bytes.
- Proving Lean metaprogram behavior or downstream Rust/CPU/CUDA refinement.

## Test and proof guidance

Use exact arithmetic only. Keep checker implementation terminating and
canonical. Use single-defect fixtures for stable category/path assertions and do
not make general error precedence a theorem. The whole-model eraser and checked
equivalence definitions are explicit review targets, not implementation details.

Run at least:

```bash
cd frontend && lake build Sembla.Semantics.CheckDeclarationsTests Sembla.Semantics.CheckModelTests
bash frontend/scripts/check-proofs.sh
python3 scripts/check-prd-allowlist.py <this PRD at its current path>
bash scripts/check.sh
git diff --check
```

Acceptance must cite the focused fixture table and the automated axiom inventory.
No `sorry`, `admit`, `axiom`, `native_decide`, `unsafe`, `implemented_by` or
opaque semantic proposition is permitted.

## Acceptance criteria

1. `checkModel` decides exactly the independent `ModelWellFormed` judgment for
   the binding owned fragment and composes with PRD 0005 rather than duplicating
   declaration checking.
2. Checked structures resolve and intrinsically type every owned term and static
   observation declaration in source order, while wires remain unchecked and
   exactly preserved.
3. The term/model soundness, completeness, synthesis uniqueness, checker
   canonicality, exact erasure, failure, erasure-validity and checked-round-trip
   theorem families pass the
   automated audit without circular or weakened definitions.
4. Whole-model erasure reconstructs checked payloads and is not a retained-source
   shortcut; exact scientific encodings, spellings, constructors and list order
   are preserved.
5. Every required constructor, category and path fixture passes, including the
   static/dynamic claim-domain boundary, Ref-write coverage, negative-literal
   hazard and malformed-wire deferment cases.
6. Focused build, proof hygiene, movable-path allowlist, full repository checks
   and `git diff --check` pass within the allowed file list.
