# Lean IR coverage

Status: **PRD 0002 raw inventory, PRD 0003 checked scalar/schema/state,
PRD 0004 intrinsically typed term domains, and PRD 0005 declaration checking**.
This document distinguishes serialization declarations from their accepted
checked representations. It does not claim whole-model term checking,
supplied-state validation, evaluation, output materialization, or behavioral
semantics.

## Exact boundary and exclusions

The inventory boundary is exactly the public structures, their fields and the
inductive constructors declared in:

- [`Sembla.IR`](../../frontend/Sembla/IR.lean);
- [`Sembla.Composition.Source`](../../frontend/Sembla/Composition/Source.lean);
- [`Sembla.Composition.SourceMap`](../../frontend/Sembla/Composition/SourceMap.lean);
- [`Sembla.Plan`](../../frontend/Sembla/Plan.lean).

There are **55 inductive constructors and 163 structure fields (218 items)**.
Every item below has **PRD 0002** as its raw inventory owner. The separate
meaning/invariant owner is shown in the tables. `Sembla.Plan` version constants
are governing fixture evidence for version fields, not additional inventory
items.

Excluded are `PlanJson.CJson`, canonical encoders and bytes, hash
implementations, linker algorithms/errors and bundle artifact containers.

## Classification notation

The two classification axes are independent:

- foundational role: `S` semantic, `T` structural, `O` observational, `P`
  provenance-only;
- frontend/checking status: `SP` surface-produced, `RO` raw-only accepted,
  `CR` contextually rejected for some raw values, `DC` deferred composition
  input.

An entry such as `dt (S/CR/0005 → 0010,0016)` means role `S`, status `CR`,
meaning/invariant owner PRD 0005 and theorem dependencies PRDs 0010 and 0016.
Dependencies may be prerequisites or later consumers; the arrow does not imply
numeric run order. `FC` means the explicitly deferred future composition
formalization track. It is an owner, not a theorem claimed by the present track.

## PRD 0003 checked-domain discharge

The following table discharges every inventory item whose meaning owner is PRD
0003. Definitions and theorem families live in
[`Sembla.Semantics.Types`][checked-types] and
[`Sembla.Semantics.State`][checked-state]; positive and checked-failure evidence
lives in [`Sembla.Semantics.TypesTests`][checked-fixtures]. Source order and an
explicit resolved scope identity are held in one `OrderedContext`; lookups are
derived from that list rather than a second map.

| Raw items owned by 0003 | Checked definition and invariant | Proof / fixture evidence |
| --- | --- | --- |
| `Scientific.coefficient`, `Scientific.exponent` | `scientificDenote` interprets `coefficient × 10^exponent` in `ℝ`; `ScientificLiteral` separately retains raw-origin syntax | `scientificDenote_equation`, `scientificDenote_congruent`, `scientific_one_eq_ten_tenth`, `ScientificLiteral.erase_exact`; fixtures preserve both `1 × 10^0` and `10 × 10^-1` |
| `ParamType.real`, `ParamType.int`, `ParamValue.real`, `ParamValue.int`, `ParamDecl.ty`, `Model.params` | `ParamSort`, dependent `ParamLiteral`, `CheckedParamDecl`, model-global `ParamContext` and `ParameterId` | exact default/prior erasure; `ParamContext.erase_exact`, `parameterLookup_name`, `parameterLookup_unique`; real and integer fixtures |
| `AttrType.real`, `AttrType.int`, `AttrType.enum`, `AttrType.ref`, `Attr.ty` | `AttrShape`, `EnumSchema`, `CheckedAttribute`, owner-indexed `AttributeId` and `VariantId`, and intrinsically typed `ScalarValue` | nonempty/duplicate-free ordered enums; exact `CheckedAttribute.erase_exact`; attr/enum lookup theorems; expected failures for cross-sort, cross-attr enum and cross-target references |
| `Table.sizeHint`, `Table.attrs`, `Box.tables` | phase-one `SchemaUniverse`/`TableTarget`, phase-two `TableSchema`/`ModelSchema`, owner-indexed `RowId`, `TypedRow`, `ValidTableState` and `ValidModelState` | table/box lookup theorems; `RowId.bound`, `RowId.zero_size_elim`; exact table/schema erasure; typed projection and row/table/model reconstruction/extensionality theorems; zero/nonzero, same-name cross-box, equal-cardinality and forward/mutual-reference fixtures |

`SchemaUniverse` establishes every ordered unique box/table header before
`ModelSchema.tableSchemas` resolves attrs. This is the constructor boundary for
forward and mutual table references. PRD 0005 now checks those raw declaration
lists, proves the uniqueness inputs, resolves raw names and provides structured
diagnostic paths. `SuppliedValue`, `SuppliedRow`,
`SuppliedTable` and `SuppliedState` deliberately admit wrong row counts, column
layouts, scalar types, enum ordinals and reference ordinals; PRD 0011 remains
responsible for their validation, state lookup, `invalidState` and
`invalidReference` behavior.

## PRD 0004 typed-term discharge

Every expression, aggregate, effect and claim item whose meaning owner is PRD
0004 now has an intrinsically typed representation and structural eraser in
[`Sembla.Semantics.Syntax`][typed-syntax]. Positive constructor, erasure,
ordering-domain and transition-payload evidence plus elaboration-failure
boundaries live in [`Sembla.Semantics.SyntaxTests`][typed-syntax-fixtures].
`ModelSchema`, `TableTarget`, `ParameterId`, `AttributeId` and `VariantId` remain
the accepted PRD 0003 owners; no parallel identifier context was introduced.
Proof-only dependent `Packed*Id` sigma views expose catalog, parameter-context,
box, table-schema, enum and reference-target ownership when identities are
equal.

| Raw items owned by 0004 | Typed definition and invariant | Proof / fixture evidence |
| --- | --- | --- |
| Every `Expr` constructor | `Expr model current inputs scope sort`; `NumericSort`; explicit `Expr.intToReal`; `RowScope` table/input indices | `PackedExpr.resultSort_unique`; `packedOwner_ne_of_owner_ne`; `PackedParameterId.owner_eq_of_eq`, `PackedBoxId.catalog_eq_of_eq`, `PackedTableId.box_heq_of_eq`, and the attribute/variant/reference packed-owner theorem families; one exact-erasure fixture for all 22 constructors; same-looking cross-model/box/table/enum and Boolean/reference arithmetic failures |
| `AggOp.count`, `AggOp.sum`, `Aggregate.mk`, `Expr.input`, `Expr.agg` | `AggOp` fixes count to Int and sum to its numeric sort; `Aggregate.unfiltered`/`filtered` preserves raw `none`/`some`; `ReferenceAttributeId` gives both join attrs one target index | `AggOp.erase_count`/`erase_sum`; `Aggregate.erase_unfiltered`/`erase_filtered`; `ReferenceAttributeId.attributeSort_eq_ref`; `Expr.relationalJoin_compatible`; a distinct-owner `events.eventRegion = self.homeRegion` erasure fixture; nonnumeric value, non-Boolean filter and incompatible-join failures |
| `Effect.setAttr` | `Effect.setAttr` indexes its value by the exact destination `attributeSort` | `Effect.erase_setAttr`; exact erasure fixture; wrong-destination-sort failure |
| `ClaimOrdering.raceTime`, `ClaimOrdering.key`; both `ResourceClaim` fields | `OrderingDomain` admits only Real, Int and owner-indexed enum; `OrderingAvailability` distinguishes surface-produced race time from raw-checkable keys; `ResourceClaim.resource` is indexed by `.ref resourceTarget` | ordering-domain exclusion theorems; `surface_domain_real`, `raw_checkable_is_key`; race/Real/Int/enum erasure fixtures; non-reference resource and Boolean/reference key failures |
| `Transition.guard`, `Transition.hazard`, `Transition.effects` | `TransitionTerms` fixes guard to Boolean, hazard to Real and preserves effect/claim list order without assembling deferred raw name/table fields | `eraseGuard_exact`, `eraseHazard_exact`, `eraseEffects_eq_map`, `eraseClaims_eq_map`; transition projection fixtures; non-Boolean guard and non-Real hazard failures |

Inserted coercions erase transparently through `Expr.erase_intToReal`; raw Real
literals use `ScientificLiteral` and therefore retain `IR.Scientific` coefficient
and exponent exactly. All other non-coercion constructors have named structural
erasure equations in `Syntax.lean`. The exhaustive classifier retains PRD 0012
as the primary observation meaning owner for output fields/builders and snapshot
values; approved PRD 0006 separately owns their static checked declarations.
Transition-name/table assembly, checking, evaluation, actual-claimant
compatibility and winner selection remain later work.

## PRD 0005 declaration-checking discharge

[`Sembla.Semantics.CheckDeclarations`][declaration-checker] defines the
checker-independent `DeclarationsWellFormed` judgment, exact rational scientific
comparisons, structured `CheckErrorCategory`/`CheckPathSegment` diagnostics,
`DeclarationContext`, box-owned port schemas, transition target resolution and
exact `DeclarationProjection` erasure. Evidence is executable in
[`Sembla.Semantics.CheckDeclarationsTests`][declaration-fixtures].

| Owned invariant / family | Checked evidence | Fixture evidence |
| --- | --- | --- |
| Exact positive `Model.dt` and strict Uniform ordering | `scientificPositive_iff_denote_pos`, `scientificLt_iff_denote_lt`; no `Float` path | positive exact decimals; zero/negative `dt`; equal/reversed Uniform bounds |
| Parameter names/defaults/priors | independent `ParameterWellFormed`; `checkedParameterLookup_name`; `modelSchema_eraseParameters_exact` | real/int priorless parameters; all prior families; duplicate names, mismatched defaults, integer prior and arity failures |
| Global and box-local namespaces | `DeclarationsWellFormed`, `BoxDeclarationsWellFormed`; separate input/output catalogs and one combined view namespace | duplicate parameter/box/summary/table/transition/input/output/view cases and accepted same-named input/output ports |
| Table/input/output schemas | phased `DeclarationContext.modelSchema`; `BoxPortSchema.instantiate`; owner-indexed lookup wrappers | zero-sized tables, zero-table boxes with ports, fully empty accepted boxes, forward/mutual refs, duplicate attrs, empty/duplicate enums and unresolved refs in all three schema owners |
| Transition headers | `resolveTransitionTarget`, `checkedTransition_target_name` | successful resolved target and unresolved-target category/path |
| Checker correspondence and fidelity | `firstDeclarationError_none_iff`, `checkDeclarations_sound`, `checkDeclarations_complete`, `checkDeclarations_failure_iff`, `checkDeclarations_erases_exact` | positive checker existence, exact projection equality and every stable category/path family |

Output names and schemas are shallow declaration obligations here. The
exhaustive raw inventory retains output meaning under PRD 0012 and
view/grouped-view/summary meaning under PRDs 0013/0017; approved PRD 0006 owns
the intervening static checked declarations without changing those primary
meaning-owner classifiers. Wires and whole-model term checking remain deferred.

## Approved PRD 0006 static-checking boundary

The exhaustive `Raw.lean` classifiers below record each raw item's primary
meaning/invariant owner and remain unchanged. The following orthogonal discharge
records the model-local static obligations approved for PRD 0006 so static
checking is not confused with later denotation:

| Static family | PRD 0006 checked obligation | Later meaning owner |
| --- | --- | --- |
| Expressions/aggregates | Bidirectional elaboration, owner resolution, canonical coercions and exact erasure; no nested input/relational aggregate inside an input-aggregate filter or sum value | PRDs 0010–0012 |
| Transitions/claims | Guard/hazard/effect typing, duplicate claim rejection, syntactic Ref-write RHS coverage and retained individual ordering domains | PRDs 0014–0016; actual-claimant compatibility PRD 0015 |
| Output fields/builders | Builder table resolution, ordered field/schema correspondence, Boolean filters and count/sum result sorts | Values/traversal/materialization PRD 0012 |
| Ordinary/grouped views | Table/key resolution, reducer/value and filter typing, V1 key/band shape and exact source order | Denotation/grouping PRDs 0013/0018 |
| Summaries | Box and ordinary-view target resolution plus exact reducer retention | Fold/error behavior PRDs 0013/0017 |
| Wires | No static discharge in PRD 0006; preserve structurally exactly and in source order | `WiresWellFormed` and plan structure PRD 0019 |

Executable evidence now lives in
[`CheckModelTests.lean`](../../frontend/Sembla/Semantics/CheckModelTests.lean):

| Checked family | Executable evidence |
| --- | --- |
| Declaration bridge | exact input/output name and schema correspondence; catalog/table reconstruction fixtures |
| Expressions and aggregates | the 22 raw expression constructors (bare enum by expected checking), count/sum, filtered/unfiltered input aggregation, mixed arithmetic and mixed numeric equality, enum anchoring and relational joins |
| Transition payloads | Boolean guard, negative Real hazard, multiple exact assignments, Ref-write claim coverage, and heterogeneous race-time/Real/Int/enum ordering domains |
| Observation declarations | filtered count, Int-sum and Real-sum output fields in exact schema order; all seven ordinary-view reducer/result-sort shapes; Enum/Ref/banded-Int grouped keys; one-key and four-key boundaries; every summary reducer |
| Declaration composition | same-spelled input/output port `flow`, forward table references, and a genuinely empty zero-table box |
| Deferred wires and erasure | `malformedWire`, `positiveRoundTrip`, `positiveEraseRecheck`, and the theorem-backed `checkModel_checked_round_trip` example preserve the wire, prove exact erasure, and certify structural checked equivalence |
| Model correspondence | compiled `#check` evidence for `checkModel_elaborates`, `checkModel_sound`, `ModelElaborates.checkModel_exists`, `checkModel_complete`, `checkModel_failure_iff`, `checkModel_canonical`, `checkModel_equivalent_of_elaborates`, and `checkModel_checked_round_trip` |
| Structured term failures | focused checker-path guards cover unknown names/variants, enum inference, numeric/Boolean/Real/reference/orderable boundaries, incompatible equality/joins and both nested-input aggregate positions; model-rooted guards cover guard/hazard/effect/claim policies, duplicate claims and uncovered Ref writes |
| Structured observation failures | model-rooted guards cover output missing/extra/duplicate/reordered/name/filter/count/sum sort policies; count-with-value and sum/min/max-without-value view shapes plus table/filter failures; grouped zero/five/unresolved/Real/band/filter policies; and unresolved/grouped-only summary targets |
| Declaration wrapper | a nonpositive-`dt` declaration failure is retained under `ModelCheckError.declaration` |

The owned constructor and diagnostic evidence is traced literally below.
`expressionCorpus[n]` uses the zero-based source order in
`CheckModelTests.lean`; every diagnostic row is a single-defect guard.

| Required positive item | Literal executable evidence |
| --- | --- |
| Aggregate operators and filters | `outputDecl.builder.fields[0]` (`count`, filtered), fields `[1]`/`[2]` (Int/Real `sum`, unfiltered/filtered), plus the filtered input-sum `#guard` |
| Effects | `transition.effects[0..2]` cover multiple exact `setAttr` assignments, Int arithmetic, mixed Real arithmetic, and the claimed Ref write |
| Claim ordering | `transition.contests[0]` is `raceTime`; `[1]`, `[2]`, `[3]` are heterogeneous Enum, Real, and Int keys |
| Mixed numeric and enum anchoring | `expressionCorpus[5..14]`, the mixed numeric equality `#guard`, and `checkingOk (.enum "open")` |
| Input aggregates and joins | `expressionCorpus[19]`, the filtered input-sum `#guard`, and `expressionCorpus[20]` |
| Output shapes and order | `outputDecl.schema` and `outputDecl.builder` provide count, Int sum, Real sum, filters, and exact three-field order |
| Ordinary-view shapes | `allViews[0..6]` cover count, Int/Real sum, Int/Real min, Int/Real max, and permitted filter/value combinations |
| Grouped-view boundaries and order | `grouped` covers Enum/Ref/banded-Int ordered keys; `groupedOne` and `groupedFour` cover one/four-key boundaries |
| Summary reducers | `summaries[0..4]` cover sum, min, max, last, and argmax-tick against ordinary `vCount` |
| Declaration composition | `inputPort` and `outputDecl` share `flow`; `box.tables` has the forward `Region` reference; `emptyBox` is zero-table |
| Exact encodings and source order | `positiveModel.dt`, parameter `gain.default`, and negative `transition.hazard` retain non-normalized scientific encodings; `positiveRoundTrip` checks whole-model structural erasure |
| Deferred wire and checked round trip | `malformedWire`, `positiveRoundTrip`, `positiveEraseRecheck`, and the theorem-backed `Equivalent` example |

| Raw expression constructor | Executable evidence |
| --- | --- |
| `real` | `expressionCorpus[0]` |
| `int` | `expressionCorpus[1]` |
| `bool` | `expressionCorpus[2]` |
| `enum` | `checkingOk (.enum "open")` |
| `param` | `expressionCorpus[3]` |
| `selfAttr` | `expressionCorpus[4]` |
| `add` | `expressionCorpus[5]` |
| `sub` | `expressionCorpus[6]` |
| `mul` | `expressionCorpus[7]` |
| `div` | `expressionCorpus[8]` |
| `eq` | `expressionCorpus[9]` and the mixed Int/Real equality guard |
| `ne` | `expressionCorpus[10]` |
| `lt` | `expressionCorpus[11]` |
| `le` | `expressionCorpus[12]` |
| `gt` | `expressionCorpus[13]` |
| `ge` | `expressionCorpus[14]` |
| `and` | `expressionCorpus[15]` |
| `or` | `expressionCorpus[16]` |
| `not` | `expressionCorpus[17]` |
| `enumIs` | `expressionCorpus[18]` |
| `input` | `expressionCorpus[19]` plus filtered input-sum guard |
| `agg` | `expressionCorpus[20]` |

| Diagnostic category | Executable evidence | Exact asserted path |
| --- | --- | --- |
| `cannotInferEnumOwner` | bare enum and two bare enums | `[]` |
| `unknownParameter` | missing parameter | `[]` |
| `unknownAttribute` | missing self attribute, nested binary/unary operands and effect destination | `[]`; `lhs`; `rhs`; `operand`; `model/box[0]/transition[0]/effects/effect[0]/destination` |
| `unknownEnumVariant` | missing anchored variant | `[]` |
| `unknownInput` | missing input | `inputPort` |
| `unknownTable` | missing relational table | `tableTarget` |
| `unknownJoinAttribute` | missing foreign/self join attributes | `joinForeignAttribute`; `joinSelfAttribute` |
| `nestedInputAggregate` | nested sum value / filter | `aggregate/aggregateValue`; `aggregate/aggregateFilter` |
| `expectedBool` | input filter, guard, output/view/grouped filters | each corresponding aggregate/model filter path |
| `expectedReal` | hazard and exact Real assignment | `model/box[0]/transition[0]/hazard`; `.../effect[0]/value` |
| `expectedNumeric` | Boolean arithmetic | `[]` |
| `expectedReference` | non-reference claim | `model/box[0]/transition[0]/contests/claim[0]/resource` |
| `expectedOrderable` | Boolean and Ref claim keys | `model/box[0]/transition[0]/contests/claim[0]/orderingKey` |
| `sortMismatch` | enum test on Int | `model/box[0]/transition[0]/guard` |
| `incompatibleEquality` | Boolean/Int equality | `[]` |
| `incompatibleJoinTargets` | mismatched relational references | `[]` |
| `duplicateResourceClaim` | repeated resource | `model/box[0]/transition[0]/contests/claim[1]/resource` |
| `unclaimedRefWrite` | uncovered Ref write | `model/box[0]/transition[0]/effects/effect[0]/value` |
| `unresolvedOutputTable` | `badOutputTable` | `model/box[0]/output[0]/outputBuilder/tableTarget` |
| `duplicateOutputField` | `badOutputDuplicate` | `model/box[0]/output[0]/outputFields/outputField[1]/fieldName` |
| `outputFieldCountMismatch` | missing/extra fields | `model/box[0]/output[0]/outputFields/outputSchema` |
| `outputFieldNameMismatch` | reordered/wrong name | `model/box[0]/output[0]/outputFields/outputField[0]/fieldName` |
| `outputFieldSortMismatch` | bad count, Int sum and Real sum destinations | corresponding `outputField[n]/fieldOperation` |
| `unresolvedViewTable` | `badViewTable` | `model/box[0]/view[0]/viewTable` |
| `invalidViewReducerShape` | count-with-value, sum/min/max-without-value and nonnumeric value corpus | `model/box[0]/view[0]/viewReducer`; `model/box[0]/view[0]/viewValue` |
| `invalidGroupedKeyCount` | zero/five keys | `model/box[0]/groupedView[0]/groupedKeys` |
| `unresolvedGroupedKey` | missing key | `.../groupedKeys/groupedKey[0]/groupedAttribute` |
| `invalidGroupedKeySort` | Real key | `.../groupedKeys/groupedKey[0]/groupedAttribute` |
| `missingGroupedBand` | Int without band | `.../groupedKeys/groupedKey[0]/groupedBand` |
| `unexpectedGroupedBand` | enum with band | `.../groupedKeys/groupedKey[0]/groupedBand` |
| `nonpositiveGroupedBand` | zero band | `.../groupedKeys/groupedKey[0]/groupedBand` |
| `aggregateInGroupedFilter` | input aggregate filter | `model/box[0]/groupedView[0]/viewFilter` |
| `unresolvedSummaryBox` | `badSummaryBox` | `model/summary[0]/summaryBox` |
| `unresolvedSummaryView` | missing and grouped-only view targets | `model/summary[0]/summaryView` |

`positiveEraseRecheck` executes the erase/recheck path. Its adjacent theorem-backed
example applies `checkModel_canonical` and `checkModel_checked_round_trip` to an
actual successful first checker result and produces a second result structurally
`Equivalent` to the first.

The independent syntax-directed term and model judgments and component erasure
lemmas are present in `CheckTerms.lean` and `CheckModel.lean`. Simultaneous
fuel-parametric term correspondence, exact erasure, declarative completeness,
canonical synthesis-sort uniqueness, structural raw-expression equality,
transition correspondence, whole-model soundness/completeness/failure,
successful-result canonicality, and structural checked round trip are proved.
Executable fixtures remain regression evidence rather than substitutes for
these theorems. Accepted raw classifier metadata remains unchanged.

All classifier links below refer to exhaustive functions in
[`Sembla.Semantics.Raw`][raw-classifiers]. Inductive functions pattern-match
every constructor. Structure functions use every positional `mk` argument, so
constructor arity changes fail compilation. Every function is exercised by
[`constructorCoverage` or `structureCoverage`][raw-fixtures]; the combined
`coverageFixture` checks item count, unique names and raw owner `0002`.

## `Sembla.IR` inductive constructors

| Declaration | Constructors and classification | Guard / fixture |
| --- | --- | --- |
| `ParamType` | `real`, `int` (S/SP/0003 → 0005,0006,0007) | `classifyParamTypeConstructor`; `constructorCoverage` |
| `ParamValue` | `real`, `int` (S/SP/0003 → 0005,0007) | `classifyParamValueConstructor`; `parameterFixtures` |
| `PriorFamily` | `normal`, `logNormal`, `uniform` (T/SP/0005 → 0007) | `classifyPriorFamilyConstructor`; `parameterFixtures` |
| `AttrType` | `real`, `int`, `enum` (S/SP/0003 → 0005,0006); `ref` (S/SP/0003 → 0005,0006,0011) | `classifyAttrTypeConstructor`; `attributeFixtures` |
| `Expr` | `real`, `int`, `bool`, `enum`, `param`, `add`, `sub`, `mul`, `div`, `eq`, `ne`, `lt`, `le`, `gt`, `ge`, `and`, `or`, `not` (S/SP/0004 → 0006,0010); `selfAttr`, `enumIs` (S/SP/0004 → 0006,0010,0011); `input`, `agg` (S/SP/0004 → 0006,0012) | `classifyExprConstructor`; `expressionVariants` |
| `AggOp` | `count`, `sum` (S/SP/0004 → 0006,0012) | `classifyAggOpConstructor`; `aggregateVariants` |
| `Aggregate` | `mk` (S/SP/0004 → 0006,0012) | `classifyAggregateConstructor`; `aggregateVariants` |
| `Effect` | `setAttr` (S/SP/0004 → 0006,0016) | `classifyEffectConstructor`; `transitionFixture` |
| `ClaimOrdering` | `raceTime` (S/SP/0004 → 0006,0015); `key` (S/RO/0004 → 0006,0015) | `classifyClaimOrderingConstructor`; `claimOrderingVariants` |
| `OutputBuilder` | `perTable` (O/SP/0012 → 0005,0006,0009) | `classifyOutputBuilderConstructor`; `outputFixture` |
| `ViewReduce` | `sum`, `count` (O/SP/0013 → 0005,0006); `min`, `max` (O/SP/0013 → 0005,0006,0017) | `classifyViewReduceConstructor`; `viewFixtures` |
| `SummaryReduce` | `sum`, `min`, `max`, `last`, `argmaxTick` (O/SP/0013 → 0017) | `classifySummaryReduceConstructor`; `summaryFixtures` |

## `Sembla.IR` structure fields

| Declaration | Every field: role/status/meaning owner → theorem dependencies | Guard / fixture |
| --- | --- | --- |
| `Scientific` | `coefficient`, `exponent` (S/SP/0003 → 0010,0020) | `classifyScientificFields`; `rawModelFixture` |
| `Prior` | `family` (S/SP/0005 → 0007); `args` (S/CR/0005 → 0007) | `classifyPriorFields`; `parameterFixtures` |
| `ParamDecl` | `name` (T/SP/0005 → 0007); `ty` (S/SP/0003 → 0005,0007); `default`, `prior` (S/CR/0005 → 0007) | `classifyParamDeclFields`; `parameterFixtures` |
| `Attr` | `name` (T/SP/0005 → 0007); `ty` (S/SP/0003 → 0005,0007) | `classifyAttrFields`; `attributeFixtures` |
| `Table` | `name` (T/SP/0005 → 0007); `sizeHint` (T/SP/0003 → 0007,0011); `attrs` (S/SP/0003 → 0005,0007,0011) | `classifyTableFields`; `rawModelFixture` |
| `ResourceClaim` | `resource`, `ordering` (S/SP/0004 → 0006,0015) | `classifyResourceClaimFields`; `transitionFixture` |
| `Transition` | `name` (T/SP/0005 → 0006,0008,0014); `table` (T/CR/0005 → 0006,0008,0014); `guard` (S/SP/0004 → 0006,0008,0014); `hazard` (S/CR/0004 → 0006,0008,0014); `effects` (S/SP/0004 → 0006,0008,0016); `contests` (S/SP/0015 → 0006,0008) | `classifyTransitionFields`; `transitionFixture` |
| `PortDecl` | `name` (T/SP/0005 → 0009,0012); `schema` (S/SP/0005 → 0009,0012) | `classifyPortDeclFields`; `inputPortFixture` |
| `OutputField` | `name` (T/SP/0012 → 0005,0006,0009); `op` (O/SP/0012 → 0005,0006,0009); `filter` (O/CR/0012 → 0005,0006,0009) | `classifyOutputFieldFields`; `outputFields` |
| `OutputDecl` | `name` (T/SP/0005 → 0006,0009,0012); `schema` (S/SP/0005 → 0006,0009,0012); `builder` (O/SP/0012 → 0005,0006,0009) | `classifyOutputDeclFields`; shallow name/schema evidence in `CheckDeclarationsTests`; deferred builder in `outputFixture` |
| `ViewDecl` | `name` (T/SP/0005 → 0006,0009,0013); `table` (T/CR/0013 → 0005,0006,0009); `filter`, `value` (O/CR/0013 → 0005,0006,0009); `reduce` (O/SP/0013 → 0005,0006,0009) | `classifyViewDeclFields`; shallow catalog evidence in `CheckDeclarationsTests`; deferred meaning in `viewFixtures` |
| `GroupKey` | `attr` (T/CR/0013 → 0005,0006,0009,0018); `bandWidth` (O/CR/0013 → 0005,0006,0009,0018) | `classifyGroupKeyFields`; `groupKeys` |
| `GroupedViewDecl` | `name` (T/SP/0005 → 0006,0009,0013,0018); `table` (T/CR/0013 → 0005,0006,0009,0018); `filter` (O/CR/0013 → 0005,0006,0009,0018); `keys` (O/SP/0013 → 0005,0006,0009,0018) | `classifyGroupedViewDeclFields`; shared shallow view-namespace evidence in `CheckDeclarationsTests`; deferred meaning in `groupedViewFixture` |
| `Box` | `name` (T/SP/0005 → 0007,0019); `tables` (S/SP/0005 → 0003,0006,0007); `transitions` (S/SP/0006 → 0008,0014,0015,0016); `inputs` (S/SP/0012 → 0005,0006,0009); `outputs` (O/SP/0012 → 0005,0006,0009); `views` (O/SP/0013 → 0005,0006,0009); `groupedViews` (O/SP/0013 → 0005,0006,0009,0018) | `classifyBoxFields`; `boxFixture` |
| `WireEndpoint` | `box`, `port` (T/RO/0019 → 0020) | `classifyWireEndpointFields`; `modelWire` |
| `Wire` | `source`, `target` (T/RO/0019 → 0020) | `classifyWireFields`; `modelWire` |
| `SummaryDecl` | `name` (T/SP/0005 → 0006,0009,0013); `box`, `view` (T/CR/0013 → 0005,0006); `reduce` (O/SP/0013 → 0017) | `classifySummaryDeclFields`; shallow name-catalog evidence in `CheckDeclarationsTests`; deferred meaning in `summaryFixtures` |
| `Model` | `name` (T/SP/0005 → 0007,0019); `dt` (S/CR/0005 → 0010,0016); `params` (S/SP/0005 → 0003,0007); `boxes` (S/SP/0006 → 0019); `wires` (T/RO/0019 → 0020); `summaries` (O/SP/0013 → 0017) | `classifyModelFields`; `rawModelFixture` |

## Composition-source inventory

Every item in this section has status `DC` and meaning/invariant owner `FC`.
That classification records raw input and a future obligation only; it does not
invent a current-track composition checker or denotation.

| Declaration | Constructors or fields by foundational role | Guard / fixture |
| --- | --- | --- |
| `StableId` | `raw` (T) | `classifyStableIdFields`; `compositionSourceFixture` |
| `PortDirection` | `input`, `output` (T) | `classifyPortDirectionConstructor`; `compositionInputPort`, `compositionOutputPort` |
| `PortDeclV1` | `id`, `direction` (T); `displayName` (P); `schema` (S) | `classifyCompositionPortDeclFields`; composition port fixtures |
| `ParameterBinding` | `requirement`, `parameter` (T) | `classifyParameterBindingFields`; `bindingFixture` |
| `InstanceDeclV1` | `id`, `definition`, `parameterBindings` (T); `displayName` (P) | `classifyInstanceDeclFields`; `instanceFixture` |
| `WireDeclV1` | `id`, `sourceInstance`, `sourcePort`, `targetInstance`, `targetPort` (T); `delayTicks` (S) | `classifyWireDeclFields`; `compositionWireFixture` |
| `ExposureDeclV1` | `id`, `innerInstance`, `innerPort`, `outerPort` (T) | `classifyExposureDeclFields`; `exposureFixture` |
| `HiddenPortV1` | `instance_`, `port` (T) | `classifyHiddenPortFields`; `hiddenPortFixture` |
| `PrimitiveBodyV1` | `tables`, `transitions`, `inputs` (S); `outputs`, `views` (O) | `classifyPrimitiveBodyFields`; `primitiveBodyFixture` |
| `CompositeBodyV1` | `instances`, `wires`, `exposures`, `hiddenPorts` (T) | `classifyCompositeBodyFields`; `compositeBodyFixture` |
| `ComponentBodyV1` | `primitive`, `composite` (T) | `classifyComponentBodyConstructor`; `constructorCoverage` |
| `ComponentDefinitionV1` | `id`, `parameterRequirements`, `ports` (T); `displayName` (P); `body` (S) | `classifyComponentDefinitionFields`; primitive/composite definitions |
| `SourceSummaryV1` | `name`, `instancePath` (T); `reduce`, `view` (O) | `classifySourceSummaryFields`; `sourceSummaryFixture` |
| `CompositionSourceV1` | `schemaVersion`, `modelId`, `definitions`, `rootDefinition`, `requiredFeatures` (T); `displayName` (P); `outerDt`, `parameters` (S); `summaries` (O) | `classifyCompositionSourceFields`; `compositionSourceFixture` |

## Source-map provenance inventory

All source-map fields are `P/DC/FC`.

| Declaration | Every field | Guard / fixture |
| --- | --- | --- |
| `SourceMapLeafV1` | `occurrence`, `definition`, `instancePath`, `displayPath` | `classifySourceMapLeafFields`; `sourceMapFixture.leaves` |
| `SourceMapBoundaryV1` | `outer`, `leaf`, `port`, `path` | `classifySourceMapBoundaryFields`; `sourceMapFixture.boundary` |
| `SourceMapHiddenV1` | `instance_`, `port` | `classifySourceMapHiddenFields`; `sourceMapFixture.hidden` |
| `SourceMapV1` | `schemaVersion`, `leaves`, `boundary`, `hidden` | `classifySourceMapFields`; `sourceMapFixture` |

## Plan inventory

| Declaration | Constructors or fields: role/status/meaning owner → theorem dependencies | Guard / fixture |
| --- | --- | --- |
| `PlanOrigin` | `linked`, `directStable` (T/RO/0019 → 0020) | `classifyPlanOriginConstructor`; linked/direct plan fixtures |
| `HashRecordV1` | `algorithm`, `domain`, `digest` (P/RO/0020) | `classifyHashRecordFields`; `hashFixture` |
| `LinkerDescriptorV1` | `semantics`, `sourceSchema`, `planSchema`, `identityScheme`, `canonicalEncoding`, `sourceMapSchema` (P/RO/0020) | `classifyLinkerDescriptorFields`; `linkerFixture` |
| `LinkedProvenanceV1` | `sourceHash`, `linker`, `sourceMap` (P/RO/0020 → 0019) | `classifyLinkedProvenanceFields`; `linkedProvenanceFixture` |
| `SchedulerDomainV1` | `id`, `algorithm`, `leaves` (T/RO/0020 → 0019) | `classifySchedulerDomainFields`; `schedulerFixture` |
| `LeafIdentityV1` | `box`, `occurrence` (T/RO/0020 → 0019) | `classifyLeafIdentityFields`; `leafIdentityFixture` |
| `TransitionIdentityV1` | `box`, `name`, `identity`, `ruleWord` (T/RO/0020 → 0019) | `classifyTransitionIdentityFields`; `transitionIdentityFixture` |
| `MailboxIdentityV1` | `identity`, `sourceBox`, `sourcePort`, `targetBox`, `targetPort` (T/RO/0020 → 0019) | `classifyMailboxIdentityFields`; `mailboxIdentityFixture` |
| `IdentityMapV1` | `modelId`, `enabledFeatures`, `schedulerDomains`, `leaves`, `transitions`, `mailboxes` (T/RO/0020 → 0019) | `classifyIdentityMapFields`; `identityFixture` |
| `ExecutablePlanV1` | `schemaVersion`, `identityScheme`, `origin` (T/RO/0019 → 0020); `model` (S/RO/0019 → 0006); `identity` (T/RO/0020 → 0019); `linkedProvenance` (P/RO/0020 → 0019) | `classifyExecutablePlanFields`; linked/direct plan fixtures |

## Fixture matrix

[`Sembla.Semantics.RawTests`][raw-fixtures] contains only raw data and computed
`#guard` checks:

- `parameterFixtures`, `attributeFixtures`, `expressionVariants`,
  `aggregateVariants`, `viewFixtures`, `summaryFixtures` and
  `rawModelFixture` cover every IR inductive variant, all prior families,
  nested aggregates, grouped views and version-independent raw model fields;
- `transitionFixture` contains two resource claims and
  `ClaimOrdering.key` raw syntax;
- `compositionSourceFixture` includes both port directions, both component-body
  constructors, populated parameter bindings, wires, exposures, hidden ports,
  source summaries and source-schema/`outerDt` fields;
- `sourceMapFixture` populates leaves, boundary aliases and hidden-port sections;
- `linkedPlanFixture` and `directPlanFixture` cover both origins, `some`/`none`
  linked provenance, identity sections, enabled features and governing version
  strings;
- `constructorCoverage`, `structureCoverage` and `coverageFixture` guard all
  218 item classifications and reject duplicate item names.

These fixtures do not call canonical encoders, hashing implementations or the
linker, and therefore do not extend the assurance boundary.

## Negative mutation evidence

The PRD 0002 implementation test temporarily added
`IR.ParamType.coverageSentinel`; building `Sembla.Semantics.Raw` failed at the
`classifyParamTypeConstructor` match with `missing cases:
IR.ParamType.coverageSentinel`. It then temporarily added
`IR.Attr.coverageSentinel`; the build failed at the positional
`classifyAttrFields` match because `IR.Attr.mk` had gained a third explicit
field. Both mutations were restored immediately. The final acceptance run
verifies zero diff and the original Git blob IDs for all four boundary modules.
This is mutation evidence for inventory exhaustiveness, not a persistent schema
change.

[raw-classifiers]: ../../frontend/Sembla/Semantics/Raw.lean
[raw-fixtures]: ../../frontend/Sembla/Semantics/RawTests.lean
[checked-types]: ../../frontend/Sembla/Semantics/Types.lean
[checked-state]: ../../frontend/Sembla/Semantics/State.lean
[checked-fixtures]: ../../frontend/Sembla/Semantics/TypesTests.lean
[typed-syntax]: ../../frontend/Sembla/Semantics/Syntax.lean
[typed-syntax-fixtures]: ../../frontend/Sembla/Semantics/SyntaxTests.lean
[declaration-checker]: ../../frontend/Sembla/Semantics/CheckDeclarations.lean
[declaration-fixtures]: ../../frontend/Sembla/Semantics/CheckDeclarationsTests.lean
