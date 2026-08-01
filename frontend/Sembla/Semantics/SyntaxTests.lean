import Sembla.Semantics.Syntax

/-!
Positive erasure, theorem and elaboration-failure fixtures for PRD 0004.
The fixtures use same-looking owner scopes and explicit dependent identifiers;
`fail_if_success` blocks verify that forbidden terms do not elaborate.
-/
namespace Sembla.Semantics.SyntaxTests

open Sembla
open Sembla.Semantics

private def realParam : CheckedParamDecl :=
  { name := "rate", sort := .real, default := ⟨⟨10, -1⟩⟩, prior := none }

private def intParam : CheckedParamDecl :=
  { name := "seed", sort := .int, default := (7 : Int), prior := none }

private def params : ParamContext :=
  { scope := "syntax:model"
    entries := [realParam, intParam]
    uniqueNames := by decide }

private def clonedParams : ParamContext :=
  { scope := "syntax:clone"
    entries := [realParam, intParam]
    uniqueNames := by decide }

private def rateParam : ParameterId params := ⟨⟨0, by decide⟩⟩
private def seedParam : ParameterId params := ⟨⟨1, by decide⟩⟩
private def clonedRateParam : ParameterId clonedParams := ⟨⟨0, by decide⟩⟩

private theorem params_ne_cloned : params ≠ clonedParams := by
  intro same
  have scopeEqual := congrArg (fun context : ParamContext => context.scope) same
  simp [params, clonedParams] at scopeEqual

private def tableHeaders : OrderedContext TableHeader TableHeader.name :=
  { scope := "syntax:box"
    entries :=
      [ { name := "people", sizeHint := 2 }
      , { name := "events", sizeHint := 3 }
      , { name := "regions", sizeHint := 1 } ]
    uniqueNames := by decide }

private def catalog : SchemaUniverse :=
  { boxes :=
      { scope := "syntax:boxes"
        entries := [{ name := "main", tables := tableHeaders }]
        uniqueNames := by decide } }

private def mainBox : BoxId catalog := ⟨⟨0, by decide⟩⟩
private def peopleTable : TableId catalog mainBox := ⟨⟨0, by decide⟩⟩
private def eventsTable : TableId catalog mainBox := ⟨⟨1, by decide⟩⟩
private def regionsTable : TableId catalog mainBox := ⟨⟨2, by decide⟩⟩
private def peopleTarget : TableTarget catalog := ⟨mainBox, peopleTable⟩
private def eventsTarget : TableTarget catalog := ⟨mainBox, eventsTable⟩
private def regionsTarget : TableTarget catalog := ⟨mainBox, regionsTable⟩

/-- The fixture catalog has one box and a box-local third table. Constructing the
identifier from each dependent box owner avoids erasing ownership through a
cast while giving distinct row schemas one common join target. -/
private def commonJoinTable (box : BoxId catalog) : TableId catalog box :=
  ⟨⟨2, by
    rcases box with ⟨ordinal⟩
    fin_cases ordinal
    decide⟩⟩

private def commonJoinTarget : TableTarget catalog :=
  ⟨mainBox, commonJoinTable mainBox⟩

/-- A small, separate two-box corpus exercises box-indexed table ownership
without complicating the relational model fixture. -/
private def ownerTables : OrderedContext TableHeader TableHeader.name :=
  { scope := "syntax:owner-tables"
    entries := [{ name := "same", sizeHint := 1 }]
    uniqueNames := by decide }

private def ownerCatalog : SchemaUniverse :=
  { boxes :=
      { scope := "syntax:owner-boxes"
        entries :=
          [ { name := "left", tables := ownerTables }
          , { name := "right", tables := ownerTables } ]
        uniqueNames := by decide } }

private def ownerLeftBox : BoxId ownerCatalog := ⟨⟨0, by decide⟩⟩
private def ownerRightBox : BoxId ownerCatalog := ⟨⟨1, by decide⟩⟩
private def ownerLeftTable : TableId ownerCatalog ownerLeftBox := ⟨⟨0, by decide⟩⟩
private def ownerRightTable : TableId ownerCatalog ownerRightBox := ⟨⟨0, by decide⟩⟩

private theorem ownerBoxes_ne : ownerLeftBox ≠ ownerRightBox := by
  intro same
  have ordinalEqual := congrArg (fun box : BoxId ownerCatalog => box.ordinal.val) same
  norm_num [ownerLeftBox, ownerRightBox] at ordinalEqual

private def statusEnum : EnumSchema :=
  { variants := ["susceptible", "infected"]
    nonempty := by decide
    uniqueVariants := by decide }

private def sameLookingEnum : EnumSchema :=
  { variants := ["susceptible", "infected"]
    nonempty := by decide
    uniqueVariants := by decide }

/-- Every table has equal-cardinality schemas, but the schema type retains its
owner. The two reference attrs deliberately target different tables. -/
private def tableSchemas (target : TableTarget catalog) : TableSchema catalog target :=
  { attributes :=
      { scope := catalog.tableName target ++ ":syntax"
        entries :=
          [ { name := "measure", shape := .real }
          , { name := "count", shape := .int }
          , { name := "status", shape := .enum statusEnum }
          , { name := "otherStatus", shape := .enum sameLookingEnum }
          , { name := "eventRegion", shape := .ref (commonJoinTable target.box) }
          , { name := "homeRegion", shape := .ref (commonJoinTable target.box) }
          , { name := "self", shape := .ref target.table } ]
        uniqueNames := by simp } }

private def model : ModelSchema :=
  { params := params, catalog := catalog, tableSchemas := tableSchemas }

private def clonedModel : ModelSchema :=
  { params := clonedParams, catalog := catalog, tableSchemas := tableSchemas }

private def currentSchema : TableSchema catalog peopleTarget := model.schemaFor peopleTarget
private def relatedSchema : TableSchema catalog eventsTarget := model.schemaFor eventsTarget

private def measureAttr : AttributeId currentSchema := ⟨⟨0, by decide⟩⟩
private def countAttr : AttributeId currentSchema := ⟨⟨1, by decide⟩⟩
private def statusAttr : AttributeId currentSchema := ⟨⟨2, by decide⟩⟩
private def otherStatusAttr : AttributeId currentSchema := ⟨⟨3, by decide⟩⟩
private def homeRegionAttr : AttributeId currentSchema := ⟨⟨5, by decide⟩⟩
private def selfRefAttr : AttributeId currentSchema := ⟨⟨6, by decide⟩⟩
private def relatedMeasureAttr : AttributeId relatedSchema := ⟨⟨0, by decide⟩⟩
private def relatedEventRegionAttr : AttributeId relatedSchema := ⟨⟨4, by decide⟩⟩

private def statusShape : (currentSchema.attr statusAttr).shape = .enum statusEnum := rfl
private def otherStatusShape :
    (currentSchema.attr otherStatusAttr).shape = .enum sameLookingEnum := rfl
private def susceptible : VariantId currentSchema statusAttr statusEnum :=
  ⟨statusShape, ⟨0, by decide⟩⟩
private def foreignSusceptible : VariantId currentSchema otherStatusAttr sameLookingEnum :=
  ⟨otherStatusShape, ⟨0, by decide⟩⟩

private def currentHomeRegionRef :
    ReferenceAttributeId currentSchema (commonJoinTable peopleTarget.box) :=
  { id := homeRegionAttr, shapeEq := rfl, sortEq := rfl }
private def relatedEventRegionRef :
    ReferenceAttributeId relatedSchema (commonJoinTable eventsTarget.box) :=
  { id := relatedEventRegionAttr, shapeEq := rfl, sortEq := rfl }
private def currentSelfRef : ReferenceAttributeId currentSchema peopleTable :=
  { id := selfRefAttr, shapeEq := rfl, sortEq := rfl }

private def inputs : InputSignature model peopleTarget :=
  { ports :=
      { scope := "syntax:inputs"
        entries := [{ name := "observations", rowSchema := currentSchema }]
        uniqueNames := by decide } }

private def observations : InputId inputs := ⟨⟨0, by decide⟩⟩
private def inputMeasureAttr : AttributeId (inputs.schema observations) := ⟨⟨0, by decide⟩⟩

private def scientific : ScientificLiteral := ⟨⟨10, -1⟩⟩
private def realLiteral : Term model peopleTarget inputs .real := .real scientific
private def intLiteral : Term model peopleTarget inputs .int := .int 2
private def boolLiteral : Term model peopleTarget inputs .bool := .bool true
private def enumLiteral : Term model peopleTarget inputs
    (.enum currentSchema statusAttr statusEnum susceptible.shapeEq) :=
  Expr.enum (model := model) (current := peopleTarget) (inputs := inputs)
    (scope := .table peopleTarget) statusAttr statusEnum susceptible
private def realParameter : Term model peopleTarget inputs .real :=
  Expr.param (model := model) (current := peopleTarget) (inputs := inputs)
    (scope := .table peopleTarget) rateParam
private def intParameter : Term model peopleTarget inputs .int :=
  Expr.param (model := model) (current := peopleTarget) (inputs := inputs)
    (scope := .table peopleTarget) seedParam
private def realAttribute : Term model peopleTarget inputs .real :=
  Expr.selfAttr (model := model) (current := peopleTarget) (inputs := inputs)
    (scope := .table peopleTarget) measureAttr
private def intAttribute : Term model peopleTarget inputs .int :=
  Expr.selfAttr (model := model) (current := peopleTarget) (inputs := inputs)
    (scope := .table peopleTarget) countAttr
private def resource : Term model peopleTarget inputs (.ref commonJoinTarget) :=
  Expr.selfAttr (model := model) (current := peopleTarget) (inputs := inputs)
    (scope := .table peopleTarget) homeRegionAttr

private def intAdd : Term model peopleTarget inputs .int :=
  .add .int intLiteral intAttribute
private def realSub : Term model peopleTarget inputs .real :=
  .sub .real realLiteral realAttribute
private def realMul : Term model peopleTarget inputs .real :=
  .mul .real realLiteral realParameter
private def intDivision : Term model peopleTarget inputs .real :=
  .div (.intToReal intLiteral) (.intToReal intParameter)
private def sameSortEq : Term model peopleTarget inputs .bool := .eq intLiteral intAttribute
private def sameSortNe : Term model peopleTarget inputs .bool := .ne realLiteral realAttribute
private def lessThan : Term model peopleTarget inputs .bool := .lt .int intLiteral intAttribute
private def lessEqual : Term model peopleTarget inputs .bool := .le .real realLiteral realAttribute
private def greaterThan : Term model peopleTarget inputs .bool := .gt .int intLiteral intAttribute
private def greaterEqual : Term model peopleTarget inputs .bool := .ge .real realLiteral realAttribute
private def conjunction : Term model peopleTarget inputs .bool := .and boolLiteral sameSortEq
private def disjunction : Term model peopleTarget inputs .bool := .or boolLiteral sameSortNe
private def negation : Term model peopleTarget inputs .bool := .not boolLiteral
private def enumTest : Term model peopleTarget inputs .bool :=
  Expr.enumIs (model := model) (current := peopleTarget) (inputs := inputs)
    (scope := .table peopleTarget) statusAttr statusEnum susceptible

private def mixedAdd : Term model peopleTarget inputs .real :=
  .add .real (.intToReal intLiteral) realLiteral
private def mixedEquality : Term model peopleTarget inputs .bool :=
  .eq (.intToReal intLiteral) realLiteral
private def mixedOrdering : Term model peopleTarget inputs .bool :=
  .lt .real (.intToReal intLiteral) realLiteral

private def inputCountAggregate :
    Aggregate model peopleTarget inputs (.input observations) .int :=
  .unfiltered .count
private def inputMeasure : Expr model peopleTarget inputs (.input observations) .real :=
  Expr.selfAttr (model := model) (current := peopleTarget) (inputs := inputs)
    (scope := .input observations) inputMeasureAttr
private def inputSumAggregate :
    Aggregate model peopleTarget inputs (.input observations) .real :=
  .filtered (.sum .real inputMeasure) (.bool true)
private def inputCount : Term model peopleTarget inputs .int :=
  .input observations inputCountAggregate
private def inputSum : Term model peopleTarget inputs .real :=
  .input observations inputSumAggregate

private def eventMeasure : Expr model peopleTarget inputs (.table eventsTarget) .real :=
  Expr.selfAttr (model := model) (current := peopleTarget) (inputs := inputs)
    (scope := .table eventsTarget) relatedMeasureAttr
private def eventSum : AggOp model peopleTarget inputs (.table eventsTarget) .real :=
  .sum .real eventMeasure
private def eventFilter : Expr model peopleTarget inputs (.table eventsTarget) .bool :=
  .bool true
private def eventInt : Expr model peopleTarget inputs (.table eventsTarget) .int :=
  .int 1
private def relationalAggregate : Term model peopleTarget inputs .real :=
  .agg eventsTable (commonJoinTable peopleTarget.box) relatedEventRegionRef
    currentHomeRegionRef eventSum eventFilter

private def setMeasure : Effect model peopleTarget inputs := .setAttr measureAttr realLiteral
private def raceOrdering : ClaimOrdering model peopleTarget inputs .real .surfaceProduced :=
  .raceTime
private def realKey : ClaimOrdering model peopleTarget inputs .real .rawCheckable :=
  .key .real realLiteral
private def intKey : ClaimOrdering model peopleTarget inputs .int .rawCheckable :=
  .key .int intLiteral
private def enumDomain : OrderingDomain model :=
  .enum currentSchema statusAttr statusEnum statusShape
private def enumKey : ClaimOrdering model peopleTarget inputs enumDomain .rawCheckable :=
  .key enumDomain enumLiteral

private def raceClaim : ResourceClaim model peopleTarget inputs :=
  { resourceTarget := commonJoinTarget
    resource := resource
    orderingDomain := .real
    orderingAvailability := .surfaceProduced
    ordering := raceOrdering }
private def realKeyClaim : ResourceClaim model peopleTarget inputs :=
  { resourceTarget := commonJoinTarget
    resource := resource
    orderingDomain := .real
    orderingAvailability := .rawCheckable
    ordering := realKey }
private def intKeyClaim : ResourceClaim model peopleTarget inputs :=
  { resourceTarget := commonJoinTarget
    resource := resource
    orderingDomain := .int
    orderingAvailability := .rawCheckable
    ordering := intKey }
private def enumKeyClaim : ResourceClaim model peopleTarget inputs :=
  { resourceTarget := commonJoinTarget
    resource := resource
    orderingDomain := enumDomain
    orderingAvailability := .rawCheckable
    ordering := enumKey }

private def transitionTerms : TransitionTerms model peopleTarget inputs :=
  { guard := conjunction
    hazard := realLiteral
    effects := [setMeasure]
    claims := [raceClaim, realKeyClaim, intKeyClaim, enumKeyClaim] }

/-- Every raw `IR.Expr` constructor appears once, in declaration order. -/
private def allExprErasures : List IR.Expr :=
  [ realLiteral.erase
  , intLiteral.erase
  , boolLiteral.erase
  , enumLiteral.erase
  , realParameter.erase
  , realAttribute.erase
  , intAdd.erase
  , realSub.erase
  , realMul.erase
  , intDivision.erase
  , sameSortEq.erase
  , sameSortNe.erase
  , lessThan.erase
  , lessEqual.erase
  , greaterThan.erase
  , greaterEqual.erase
  , conjunction.erase
  , disjunction.erase
  , negation.erase
  , enumTest.erase
  , inputCount.erase
  , relationalAggregate.erase ]

#guard allExprErasures ==
  [ .real ⟨10, -1⟩
  , .int 2
  , .bool true
  , .enum "susceptible"
  , .param "rate"
  , .selfAttr "measure"
  , .add (.int 2) (.selfAttr "count")
  , .sub (.real ⟨10, -1⟩) (.selfAttr "measure")
  , .mul (.real ⟨10, -1⟩) (.param "rate")
  , .div (.int 2) (.param "seed")
  , .eq (.int 2) (.selfAttr "count")
  , .ne (.real ⟨10, -1⟩) (.selfAttr "measure")
  , .lt (.int 2) (.selfAttr "count")
  , .le (.real ⟨10, -1⟩) (.selfAttr "measure")
  , .gt (.int 2) (.selfAttr "count")
  , .ge (.real ⟨10, -1⟩) (.selfAttr "measure")
  , .and (.bool true) (.eq (.int 2) (.selfAttr "count"))
  , .or (.bool true) (.ne (.real ⟨10, -1⟩) (.selfAttr "measure"))
  , .not (.bool true)
  , .enumIs "status" "susceptible"
  , .input "observations" (.mk .count none)
  , .agg (.sum (.selfAttr "measure")) "events" "eventRegion" "homeRegion"
      (.bool true) ]

/-- Explicit coercion nodes erase without changing their raw subtree. -/
example : mixedAdd.erase = .add (.int 2) (.real ⟨10, -1⟩) := rfl
example : mixedEquality.erase = .eq (.int 2) (.real ⟨10, -1⟩) := rfl
example : mixedOrdering.erase = .lt (.int 2) (.real ⟨10, -1⟩) := rfl
example : inputSum.erase =
    .input "observations" (.mk (.sum (.selfAttr "measure")) (some (.bool true))) := rfl

/-- Aggregate/operator, effect, ordering, resource and transition-field erasure
preserves every raw field and list order. -/
example : (AggOp.count : AggOp model peopleTarget inputs (.input observations) .int).erase =
    .count := rfl
example : inputSumAggregate.erase =
    .mk (.sum (.selfAttr "measure")) (some (.bool true)) := rfl
example : setMeasure.erase = .setAttr "measure" (.real ⟨10, -1⟩) := rfl
example : raceOrdering.erase = .raceTime := rfl
example : realKey.erase = .key (.real ⟨10, -1⟩) := rfl
example : intKey.erase = .key (.int 2) := rfl
example : enumKey.erase = .key (.enum "susceptible") := rfl
example : raceClaim.erase =
    { resource := .selfAttr "homeRegion", ordering := .raceTime } := rfl
example : transitionTerms.eraseGuard = conjunction.erase := rfl
example : transitionTerms.eraseHazard = .real ⟨10, -1⟩ := rfl
example : transitionTerms.eraseEffects = [.setAttr "measure" (.real ⟨10, -1⟩)] := rfl
example : transitionTerms.eraseClaims =
    [ { resource := .selfAttr "homeRegion", ordering := .raceTime }
    , { resource := .selfAttr "homeRegion", ordering := .key (.real ⟨10, -1⟩) }
    , { resource := .selfAttr "homeRegion", ordering := .key (.int 2) }
    , { resource := .selfAttr "homeRegion", ordering := .key (.enum "susceptible") } ] := rfl

/-- Named theorem families state intrinsic result uniqueness, owner-preserving
reference compatibility, and the ordering producer boundary. -/
example (packed : PackedExpr model peopleTarget inputs (.table peopleTarget))
    {left right : ScalarSort catalog} (hl : packed.HasResult left)
    (hr : packed.HasResult right) : left = right := packed.resultSort_unique hl hr
example : currentSchema.attributeSort currentHomeRegionRef.id = .ref commonJoinTarget :=
  currentHomeRegionRef.attributeSort_eq_ref
example : relatedSchema.attributeSort relatedEventRegionRef.id = .ref commonJoinTarget :=
  relatedEventRegionRef.attributeSort_eq_ref
example : relatedSchema.attributeSort relatedEventRegionRef.id =
    currentSchema.attributeSort currentHomeRegionRef.id :=
  Expr.relationalJoin_compatible (model := model) (current := peopleTarget)
    (inputs := inputs) (scope := .table peopleTarget) eventsTable
    (commonJoinTable peopleTarget.box) relatedEventRegionRef currentHomeRegionRef

/-- Equality of proof-only identifier packages exposes every dependent owner. -/
example (same : (⟨params, rateParam⟩ : PackedParameterId) =
    ⟨clonedParams, clonedRateParam⟩) : params = clonedParams :=
  PackedParameterId.owner_eq_of_eq same

example : (⟨params, rateParam⟩ : PackedParameterId) ≠
    ⟨clonedParams, clonedRateParam⟩ :=
  packedOwner_ne_of_owner_ne params_ne_cloned

example (same : (⟨catalog, mainBox⟩ : PackedBoxId) =
    ⟨ownerCatalog, ownerLeftBox⟩) : catalog = ownerCatalog :=
  PackedBoxId.catalog_eq_of_eq same

example (same :
    (⟨ownerCatalog, ⟨ownerLeftBox, ownerLeftTable⟩⟩ : PackedTableId) =
      ⟨ownerCatalog, ⟨ownerRightBox, ownerRightTable⟩⟩) :
    HEq ownerLeftBox ownerRightBox := PackedTableId.box_heq_of_eq same

example : (⟨ownerLeftBox, ownerLeftTable⟩ :
    Σ box : BoxId ownerCatalog, TableId ownerCatalog box) ≠
    ⟨ownerRightBox, ownerRightTable⟩ :=
  packedOwner_ne_of_owner_ne ownerBoxes_ne

example (same :
    (⟨catalog, ⟨peopleTarget, ⟨currentSchema, measureAttr⟩⟩⟩ : PackedAttributeId) =
      ⟨catalog, ⟨eventsTarget, ⟨relatedSchema, relatedMeasureAttr⟩⟩⟩) :
    HEq peopleTarget eventsTarget ∧ HEq currentSchema relatedSchema :=
  ⟨PackedAttributeId.owner_heq_of_eq same,
    PackedAttributeId.schema_heq_of_eq same⟩

example (same :
    (⟨catalog,
      ⟨peopleTarget, ⟨currentSchema, ⟨statusAttr, ⟨statusEnum, susceptible⟩⟩⟩⟩⟩ :
      PackedVariantId) =
    ⟨catalog,
      ⟨peopleTarget,
        ⟨currentSchema,
          ⟨otherStatusAttr, ⟨sameLookingEnum, foreignSusceptible⟩⟩⟩⟩⟩) :
    HEq currentSchema currentSchema ∧ HEq statusAttr otherStatusAttr ∧
      HEq statusEnum sameLookingEnum :=
  PackedVariantId.schema_attr_enum_heq_of_eq same

example (same :
    (⟨catalog,
      ⟨peopleTarget,
        ⟨currentSchema, ⟨commonJoinTable peopleTarget.box, currentHomeRegionRef⟩⟩⟩⟩ :
      PackedReferenceAttributeId) =
    ⟨catalog,
      ⟨eventsTarget,
        ⟨relatedSchema,
          ⟨commonJoinTable eventsTarget.box, relatedEventRegionRef⟩⟩⟩⟩) :
    HEq peopleTarget eventsTarget ∧ HEq currentSchema relatedSchema ∧
      HEq (commonJoinTable peopleTarget.box) (commonJoinTable eventsTarget.box) :=
  PackedReferenceAttributeId.owner_schema_target_heq_of_eq same

example (domain : OrderingDomain model)
    (ordering : ClaimOrdering model peopleTarget inputs domain .surfaceProduced) :
    domain = .real := ordering.surface_domain_real
example : ∃ expr : Term model peopleTarget inputs (.real : OrderingDomain model).scalarSort,
    realKey = .key .real expr := realKey.raw_checkable_is_key

/-- Cross-model parameter identities do not elaborate in this model. -/
example : True := by
  fail_if_success
    have _bad : Term model peopleTarget inputs .real := .param clonedRateParam
  trivial

/-- Equal-named tables from different boxes cannot cross their box owner. -/
example : True := by
  fail_if_success
    have _bad : TableId ownerCatalog ownerLeftBox := ownerRightTable
  trivial

/-- Equal-cardinality attributes from a different table cannot inhabit this row scope. -/
example : True := by
  fail_if_success
    have _bad : Term model peopleTarget inputs .real :=
      Expr.selfAttr (model := model) (current := peopleTarget) (inputs := inputs)
        (scope := .table peopleTarget) relatedMeasureAttr
  trivial

/-- A same-looking variant owned by another enum attribute is rejected. -/
example : True := by
  fail_if_success
    have _bad : Term model peopleTarget inputs
        (.enum currentSchema statusAttr statusEnum susceptible.shapeEq) :=
      Expr.enum (model := model) (current := peopleTarget) (inputs := inputs)
        (scope := .table peopleTarget) statusAttr statusEnum foreignSusceptible
  trivial

/-- Boolean and reference arithmetic are unconstructable. -/
example : True := by
  fail_if_success
    have _bad : Term model peopleTarget inputs .int := .add .int boolLiteral boolLiteral
  trivial

example : True := by
  fail_if_success
    have _bad : Term model peopleTarget inputs .int := .add .int resource resource
  trivial

/-- Reference ordering comparisons are unconstructable. -/
example : True := by
  fail_if_success
    have _bad : Term model peopleTarget inputs .bool := .lt .int resource resource
  trivial

/-- Aggregate values must be numeric and filters must be Boolean. -/
example : True := by
  fail_if_success
    have _bad : AggOp model peopleTarget inputs (.input observations) .int :=
      .sum .int (.bool true)
  trivial

example : True := by
  fail_if_success
    have _bad : Aggregate model peopleTarget inputs (.input observations) .int :=
      .filtered .count inputMeasure
  trivial

example : True := by
  fail_if_success
    have _bad : Term model peopleTarget inputs .real :=
      .agg eventsTable (commonJoinTable peopleTarget.box) relatedEventRegionRef
        currentHomeRegionRef eventSum eventInt
  trivial

/-- Transition guards are Boolean and hazards are Real. -/
example : True := by
  fail_if_success
    have _bad : TransitionTerms model peopleTarget inputs :=
      { guard := intLiteral, hazard := realLiteral, effects := [], claims := [] }
  trivial

example : True := by
  fail_if_success
    have _bad : TransitionTerms model peopleTarget inputs :=
      { guard := boolLiteral, hazard := intLiteral, effects := [], claims := [] }
  trivial

/-- Both relational join attributes must share the exact destination index. -/
example : True := by
  fail_if_success
    have _bad : Term model peopleTarget inputs .real :=
      .agg eventsTable (commonJoinTable peopleTarget.box) relatedEventRegionRef
        currentSelfRef eventSum eventFilter
  trivial

/-- Assignment values must have the exact destination sort. -/
example : True := by
  fail_if_success
    have _bad : Effect model peopleTarget inputs := .setAttr measureAttr intLiteral
  trivial

/-- Claim resources must be references. -/
example : True := by
  fail_if_success
    have _bad : ResourceClaim model peopleTarget inputs :=
      { resourceTarget := commonJoinTarget
        resource := intLiteral
        orderingDomain := .real
        orderingAvailability := .surfaceProduced
        ordering := raceOrdering }
  trivial

/-- Boolean and reference claim keys cannot inhabit any ordering domain. -/
example : True := by
  fail_if_success
    have _bad : ClaimOrdering model peopleTarget inputs .real .rawCheckable :=
      .key .real boolLiteral
  trivial

example : True := by
  fail_if_success
    have _bad : ClaimOrdering model peopleTarget inputs .real .rawCheckable :=
      .key .real resource
  trivial

/-- The clone is used only to ensure model-global parameter ownership differs. -/
example : clonedModel.params = clonedParams := rfl

end Sembla.Semantics.SyntaxTests
