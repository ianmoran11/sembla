import Sembla.Semantics.CheckModel

namespace Sembla.Semantics.CheckModelTests

open Sembla Sembla.Semantics

private def sci (coefficient : Int) (exponent : Int := 0) : IR.Scientific :=
  ⟨coefficient, exponent⟩

private def regionTable : IR.Table :=
  { name := "Region", sizeHint := 3
    attrs := [
      { name := "rank", ty := .int },
      { name := "kind", ty := .enum ["urban", "rural"] }
    ] }

private def peopleTable : IR.Table :=
  { name := "People", sizeHint := 10
    attrs := [
      { name := "rate", ty := .real },
      { name := "count", ty := .int },
      { name := "status", ty := .enum ["open", "closed"] },
      { name := "region", ty := .ref "Region" },
      { name := "manager", ty := .ref "Region" },
      { name := "backup", ty := .ref "Region" },
      { name := "peer", ty := .ref "Region" }
    ] }

private def eventTable : IR.Table :=
  { name := "Event", sizeHint := 20
    attrs := [
      { name := "amount", ty := .real },
      { name := "region", ty := .ref "Region" },
      { name := "other", ty := .ref "Event" }
    ] }

private def inputPort : IR.PortDecl :=
  { name := "flow"
    schema := [
      { name := "amount", ty := .real },
      { name := "region", ty := .ref "Region" }
    ] }

private def outputDecl : IR.OutputDecl :=
  { name := "flow"
    schema := [
      { name := "rows", ty := .int },
      { name := "totalCount", ty := .int },
      { name := "total", ty := .real }
    ]
    builder := .perTable "People" [
      { name := "rows", op := .count, filter := some (.bool true) },
      { name := "totalCount", op := .sum (.selfAttr "count"), filter := none },
      { name := "total", op := .sum (.selfAttr "rate"),
        filter := some (.gt (.selfAttr "rate") (.real (sci 0))) }
    ] }

private def transition : IR.Transition :=
  { name := "step", table := "People"
    guard := .and
      (.eq (.selfAttr "status") (.enum "open"))
      (.not (.or (.bool false) (.enumIs "status" "closed")))
    hazard := .real (sci (-25) (-1))
    effects := [
      .setAttr "count" (.add (.selfAttr "count") (.int 1)),
      .setAttr "rate" (.div (.add (.selfAttr "rate") (.int 1)) (.int 2)),
      .setAttr "region" (.selfAttr "region")
    ]
    contests := [
      { resource := .selfAttr "region", ordering := .raceTime },
      { resource := .selfAttr "manager", ordering := .key (.selfAttr "status") },
      { resource := .selfAttr "backup", ordering := .key (.selfAttr "rate") },
      { resource := .selfAttr "peer", ordering := .key (.selfAttr "count") }
    ] }

private def allViews : List IR.ViewDecl := [
  { name := "vCount", table := "People", filter := none,
    value := none, reduce := .count },
  { name := "vSum", table := "People", filter := some (.bool true),
    value := some (.selfAttr "count"), reduce := .sum },
  { name := "vMin", table := "People", filter := none,
    value := some (.selfAttr "rate"), reduce := .min },
  { name := "vMax", table := "People", filter := none,
    value := some (.selfAttr "count"), reduce := .max },
  { name := "vSumReal", table := "People", filter := none,
    value := some (.selfAttr "rate"), reduce := .sum },
  { name := "vMinInt", table := "People", filter := some (.bool true),
    value := some (.selfAttr "count"), reduce := .min },
  { name := "vMaxReal", table := "People", filter := some (.bool true),
    value := some (.selfAttr "rate"), reduce := .max }
]

private def grouped : IR.GroupedViewDecl :=
  { name := "byKeys", table := "People", filter := some (.bool true)
    keys := [
      { attr := "status", bandWidth := none },
      { attr := "region", bandWidth := none },
      { attr := "count", bandWidth := some 5 }
    ] }

private def groupedOne : IR.GroupedViewDecl :=
  { name := "byOne", table := "People", filter := none
    keys := [{ attr := "region", bandWidth := none }] }

private def groupedFour : IR.GroupedViewDecl :=
  { name := "byFour", table := "People", filter := some (.bool true)
    keys := [
      { attr := "status", bandWidth := none },
      { attr := "manager", bandWidth := none },
      { attr := "count", bandWidth := some 7 },
      { attr := "region", bandWidth := none }
    ] }

private def box : IR.Box :=
  { name := "Sim"
    tables := [peopleTable, eventTable, regionTable]
    transitions := [transition]
    inputs := [inputPort]
    outputs := [outputDecl]
    views := allViews
    groupedViews := [grouped, groupedOne, groupedFour] }

private def emptyBox : IR.Box :=
  { name := "Empty", tables := [], transitions := [], inputs := [], outputs := [],
    views := [], groupedViews := [] }

private def malformedWire : IR.Wire :=
  { source := { box := "missing-source", port := "bad" }
    target := { box := "missing-target", port := "also-bad" } }

private def summaries : List IR.SummaryDecl := [
  { name := "summary0", box := "Sim", view := "vCount", reduce := .sum },
  { name := "summary1", box := "Sim", view := "vCount", reduce := .min },
  { name := "summary2", box := "Sim", view := "vCount", reduce := .max },
  { name := "summary3", box := "Sim", view := "vCount", reduce := .last },
  { name := "summary4", box := "Sim", view := "vCount", reduce := .argmaxTick }
]

private def positiveModel : IR.Model :=
  { name := "checked-model", dt := sci 1 (-1)
    params := [
      { name := "gain", ty := .real, default := .real (sci 125 (-2)), prior := none },
      { name := "offset", ty := .int, default := .int 2, prior := none }
    ]
    boxes := [box, emptyBox]
    wires := [malformedWire]
    summaries := summaries }

private theorem positiveDeclarations : DeclarationsWellFormed positiveModel := by decide

private def declarations : DeclarationContext := ⟨positiveModel, positiveDeclarations⟩
private def simBox : BoxId declarations.modelSchema.catalog := ⟨⟨0, by decide⟩⟩
private def peopleId : TableId declarations.modelSchema.catalog simBox := ⟨⟨0, by decide⟩⟩
private def Γ : TermContext := ⟨declarations, simBox, peopleId⟩
private def tableScope : RowScope Γ.model Γ.current Γ.inputs := .table Γ.current

private def synthesisOk (raw : IR.Expr) : Bool :=
  match synthExpr Γ tableScope raw with
  | .ok checked => checked.expr.erase == raw
  | .error _ => false

private def checkingOk (raw : IR.Expr) (sort : ScalarSort Γ.model.catalog)
    (origin : SortOrigin Γ tableScope sort) : Bool :=
  match checkExpr Γ tableScope raw sort origin with
  | .ok checked => checked.erase == raw
  | .error _ => false

/-- Constructor-class corpus: every one of the 22 raw expression constructors
is accepted in a valid context, with enum anchoring exercised through checking. -/
private def expressionCorpus : List IR.Expr := [
  .real (sci 17 (-1)), .int 4, .bool true,
  .param "gain", .selfAttr "rate",
  .add (.int 1) (.real (sci 2)),
  .sub (.selfAttr "count") (.int 1),
  .mul (.real (sci 3)) (.int 2),
  .div (.int 3) (.int 2),
  .eq (.selfAttr "status") (.enum "open"),
  .ne (.selfAttr "count") (.int 0),
  .lt (.int 1) (.real (sci 2)),
  .le (.int 1) (.int 2),
  .gt (.real (sci 2)) (.int 1),
  .ge (.int 2) (.int 2),
  .and (.bool true) (.bool false),
  .or (.bool true) (.bool false),
  .not (.bool false),
  .enumIs "status" "open",
  .input "flow" (.mk .count none),
  .agg (.sum (.selfAttr "amount")) "Event" "region" "region" (.bool true)
]

#guard expressionCorpus.length == 21
#guard expressionCorpus.all synthesisOk
#guard synthesisOk (.eq (.int 1) (.real (sci 1)))

private def statusAttr : AttributeId (Γ.model.schemaFor Γ.current) := ⟨⟨2, by decide⟩⟩
private def statusSchema : EnumSchema := ⟨["open", "closed"], by decide, by decide⟩
private theorem statusShape :
    ((Γ.model.schemaFor Γ.current).attr statusAttr).shape = .enum statusSchema := by rfl

#guard checkingOk (.enum "open")
  (.enum (Γ.model.schemaFor Γ.current) statusAttr statusSchema statusShape)
  (.enum (Γ := Γ) (scope := tableScope) statusAttr statusSchema statusShape)

#guard synthesisOk (.input "flow" (.mk (.sum (.selfAttr "amount"))
  (some (.gt (.selfAttr "amount") (.real (sci 0))))))

private def expectTermError (result : Except TermCheckError α)
    (category : TermCheckErrorCategory) (path : List ModelCheckPathSegment) : Bool :=
  match result with
  | .error error => error.category == category && error.path == path
  | .ok _ => false

#guard expectTermError (synthExpr Γ tableScope (.enum "open") [])
  .cannotInferEnumOwner []
#guard expectTermError (synthExpr Γ tableScope (.param "missing") [])
  .unknownParameter []
#guard expectTermError (synthExpr Γ tableScope (.selfAttr "missing") [])
  .unknownAttribute []
#guard expectTermError (synthExpr Γ tableScope (.input "missing" (.mk .count none)) [])
  .unknownInput [.inputPort]
#guard expectTermError (synthExpr Γ tableScope
  (.input "flow" (.mk (.sum (.input "flow" (.mk .count none))) none)) [])
  .nestedInputAggregate [.aggregate, .aggregateValue]
#guard expectTermError (synthExpr Γ tableScope
  (.input "flow" (.mk .count (some (.agg .count "Event" "region" "region" (.bool true))))) [])
  .nestedInputAggregate [.aggregate, .aggregateFilter]
#guard expectTermError (synthExpr Γ tableScope
  (.agg .count "missing" "region" "region" (.bool true)) [])
  .unknownTable [.tableTarget]
#guard expectTermError (synthExpr Γ tableScope
  (.agg .count "Event" "missing" "region" (.bool true)) [])
  .unknownJoinAttribute [.joinForeignAttribute]
#guard expectTermError (synthExpr Γ tableScope
  (.agg .count "Event" "other" "manager" (.bool true)) [])
  .incompatibleJoinTargets []
#guard expectTermError (synthExpr Γ tableScope (.add (.bool true) (.int 1)) [])
  .expectedNumeric []
#guard expectTermError (synthExpr Γ tableScope (.eq (.enum "open") (.enum "closed")) [])
  .cannotInferEnumOwner []
#guard expectTermError (checkExpr Γ tableScope (.enum "missing")
    (.enum (Γ.model.schemaFor Γ.current) statusAttr statusSchema statusShape)
    (.enum (Γ := Γ) (scope := tableScope) statusAttr statusSchema statusShape) [])
  .unknownEnumVariant []
#guard expectTermError (checkExpr Γ tableScope (.int 1) .bool .bool [])
  .expectedBool []
#guard expectTermError (checkExpr Γ tableScope (.int 1) .real .real [])
  .expectedReal []
#guard expectTermError (synthExpr Γ tableScope (.eq (.bool true) (.int 1)) [])
  .incompatibleEquality []
#guard expectTermError (synthExpr Γ tableScope
  (.input "flow" (.mk .count (some (.int 1)))) [])
  .expectedBool [.aggregate, .aggregateFilter]
#guard expectTermError (synthExpr Γ tableScope
  (.agg .count "Event" "region" "missing" (.bool true)) [])
  .unknownJoinAttribute [.joinSelfAttribute]

/- Representative nested-expression path propagation. -/
#guard expectTermError (synthExpr Γ tableScope
  (.and (.selfAttr "missing") (.bool true)) [])
  .unknownAttribute [.lhs]
#guard expectTermError (synthExpr Γ tableScope
  (.and (.bool true) (.selfAttr "missing")) [])
  .unknownAttribute [.rhs]
#guard expectTermError (synthExpr Γ tableScope
  (.not (.selfAttr "missing")) [])
  .unknownAttribute [.operand]

private def expectModelError (raw : IR.Model) (category : ModelTermErrorCategory)
    (path : List ModelCheckPathSegment) : Bool :=
  match checkModel raw with
  | .error (.model error) => error.category == category && error.path == path
  | _ => false

private def expectDeclarationError (raw : IR.Model) (category : CheckErrorCategory) : Bool :=
  match checkModel raw with
  | .error (.declaration error) => error.category == category
  | _ => false

private def badDeclaration : IR.Model := { positiveModel with dt := sci 0 }
#guard expectDeclarationError badDeclaration .nonpositiveDt

#guard (checkModel positiveModel).isOk
private def positiveRoundTrip : Bool :=
  match checkModel positiveModel with
  | .ok checked => checked.erase == positiveModel && checked.wires == [malformedWire]
  | _ => false
#guard positiveRoundTrip

/- Executable erase/recheck coverage; structural equivalence is certified by the
public checked round-trip theorem once the model correspondence bridge is used. -/
private def positiveEraseRecheck : Bool :=
  match checkModel positiveModel with
  | .ok first =>
      match checkModel first.erase with
      | .ok second => second.erase == first.erase
      | .error _ => false
  | .error _ => false
#guard positiveEraseRecheck

private def badOutputTable : IR.Model :=
  { positiveModel with boxes := [{ box with outputs := [
      { outputDecl with builder := .perTable "missing" [] }
    ] }] }
#guard expectModelError badOutputTable .unresolvedOutputTable
  [.model, .box 0, .output 0, .outputBuilder, .tableTarget]

private def badOutputCount : IR.Model :=
  { positiveModel with boxes := [{ box with outputs := [
      { outputDecl with builder := .perTable "People" [] }
    ] }] }
#guard expectModelError badOutputCount .outputFieldCountMismatch
  [.model, .box 0, .output 0, .outputFields, .outputSchema]

private def rowsField : IR.OutputField :=
  { name := "rows", op := .count, filter := some (.bool true) }
private def countField : IR.OutputField :=
  { name := "totalCount", op := .sum (.selfAttr "count"), filter := none }
private def realField : IR.OutputField :=
  { name := "total", op := .sum (.selfAttr "rate"),
    filter := some (.gt (.selfAttr "rate") (.real (sci 0))) }
private def outputFields : List IR.OutputField := [rowsField, countField, realField]

private def rowsAttr : IR.Attr := { name := "rows", ty := .int }
private def countAttr : IR.Attr := { name := "totalCount", ty := .int }
private def realAttr : IR.Attr := { name := "total", ty := .real }

private def withOutput (changed : IR.OutputDecl) : IR.Model :=
  { positiveModel with boxes := [{ box with outputs := [changed] }] }

private def badOutputExtra : IR.Model := withOutput
  { outputDecl with builder := .perTable "People" (outputFields ++ [{ name := "extra", op := .count, filter := none }]) }
#guard expectModelError badOutputExtra .outputFieldCountMismatch
  [.model, .box 0, .output 0, .outputFields, .outputSchema]

private def badOutputDuplicate : IR.Model := withOutput
  { outputDecl with builder := .perTable "People" [rowsField, { countField with name := "rows" }, realField] }
#guard expectModelError badOutputDuplicate .duplicateOutputField
  [.model, .box 0, .output 0, .outputFields, .outputField 1, .fieldName]

private def badOutputReordered : IR.Model := withOutput
  { outputDecl with builder := .perTable "People" [countField, rowsField, realField] }
#guard expectModelError badOutputReordered .outputFieldNameMismatch
  [.model, .box 0, .output 0, .outputFields, .outputField 0, .fieldName]

private def badOutputName : IR.Model := withOutput
  { outputDecl with builder := .perTable "People" [{ rowsField with name := "wrong" }, countField, realField] }
#guard expectModelError badOutputName .outputFieldNameMismatch
  [.model, .box 0, .output 0, .outputFields, .outputField 0, .fieldName]

private def badOutputFilter : IR.Model := withOutput
  { outputDecl with builder := (.perTable "People"
      [{ rowsField with filter := some (.int 1) }, countField, realField]) }
#guard expectModelError badOutputFilter (.term .expectedBool)
  [.model, .box 0, .output 0, .outputFields, .outputField 0, .fieldFilter]

private def badCountDestination : IR.Model := withOutput
  { outputDecl with schema := [{ rowsAttr with ty := .real }, countAttr, realAttr] }
#guard expectModelError badCountDestination .outputFieldSortMismatch
  [.model, .box 0, .output 0, .outputFields, .outputField 0, .fieldOperation]

private def badIntSumDestination : IR.Model := withOutput
  { outputDecl with schema := [rowsAttr, { countAttr with ty := .real }, realAttr] }
#guard expectModelError badIntSumDestination .outputFieldSortMismatch
  [.model, .box 0, .output 0, .outputFields, .outputField 1, .fieldOperation]

private def badRealSumDestination : IR.Model := withOutput
  { outputDecl with schema := [rowsAttr, countAttr, { realAttr with ty := .int }] }
#guard expectModelError badRealSumDestination .outputFieldSortMismatch
  [.model, .box 0, .output 0, .outputFields, .outputField 2, .fieldOperation]

private def badViewShape : IR.Model :=
  { positiveModel with
    boxes := [{ box with views := [
      { name := "bad", table := "People", filter := none,
        value := some (.int 1), reduce := .count }
    ] }]
    summaries := [] }
#guard expectModelError badViewShape .invalidViewReducerShape
  [.model, .box 0, .view 0, .viewReducer]

private def badViewValueSort : IR.Model :=
  { positiveModel with boxes := [{ box with views := [
      { name := "bad", table := "People", filter := none,
        value := some (.bool true), reduce := .sum }
    ] }, emptyBox] }
#guard expectModelError badViewValueSort .invalidViewReducerShape
  [.model, .box 0, .view 0, .viewValue]

private def invalidViewWithoutValue (reduce : IR.ViewReduce) : IR.Model :=
  { positiveModel with
    boxes := [{ box with views := [
      { name := "bad", table := "People", filter := none,
        value := none, reduce := reduce }
    ] }]
    summaries := [] }

#guard expectModelError (invalidViewWithoutValue .sum) .invalidViewReducerShape
  [.model, .box 0, .view 0, .viewReducer]
#guard expectModelError (invalidViewWithoutValue .min) .invalidViewReducerShape
  [.model, .box 0, .view 0, .viewReducer]
#guard expectModelError (invalidViewWithoutValue .max) .invalidViewReducerShape
  [.model, .box 0, .view 0, .viewReducer]

private def badGroupedCount : IR.Model :=
  { positiveModel with boxes := [{ box with groupedViews := [
      { grouped with keys := [] }
    ] }] }
#guard expectModelError badGroupedCount .invalidGroupedKeyCount
  [.model, .box 0, .groupedView 0, .groupedKeys]

private def badGroupedFive : IR.Model :=
  { positiveModel with boxes := [{ box with groupedViews := [
      { grouped with keys := groupedFour.keys ++ [{ attr := "status", bandWidth := none }] }
    ] }] }
#guard expectModelError badGroupedFive .invalidGroupedKeyCount
  [.model, .box 0, .groupedView 0, .groupedKeys]

private def badGroupedUnknownKey : IR.Model :=
  { positiveModel with boxes := [{ box with groupedViews := [
      { grouped with keys := [{ attr := "missing", bandWidth := none }] }
    ] }] }
#guard expectModelError badGroupedUnknownKey .unresolvedGroupedKey
  [.model, .box 0, .groupedView 0, .groupedKeys, .groupedKey 0, .groupedAttribute]

private def badGroupedRealKey : IR.Model :=
  { positiveModel with boxes := [{ box with groupedViews := [
      { grouped with keys := [{ attr := "rate", bandWidth := none }] }
    ] }] }
#guard expectModelError badGroupedRealKey .invalidGroupedKeySort
  [.model, .box 0, .groupedView 0, .groupedKeys, .groupedKey 0, .groupedAttribute]

private def badGroupedBand : IR.Model :=
  { positiveModel with boxes := [{ box with groupedViews := [
      { grouped with keys := [{ attr := "count", bandWidth := some 0 }] }
    ] }] }
#guard expectModelError badGroupedBand .nonpositiveGroupedBand
  [.model, .box 0, .groupedView 0, .groupedKeys, .groupedKey 0, .groupedBand]

private def badSummaryBox : IR.Model :=
  { positiveModel with summaries := [
      { name := "bad", box := "missing", view := "vCount", reduce := .sum }
    ] }
#guard expectModelError badSummaryBox .unresolvedSummaryBox
  [.model, .summary 0, .summaryBox]

private def badSummaryView : IR.Model :=
  { positiveModel with summaries := [
      { name := "bad", box := "Sim", view := "missing", reduce := .sum }
    ] }
#guard expectModelError badSummaryView .unresolvedSummaryView
  [.model, .summary 0, .summaryView]

private def badSummaryGroupedOnly : IR.Model :=
  { positiveModel with summaries := [
      { name := "bad", box := "Sim", view := "byKeys", reduce := .sum }
    ] }
#guard expectModelError badSummaryGroupedOnly .unresolvedSummaryView
  [.model, .summary 0, .summaryView]

private def withTransition (changed : IR.Transition) : IR.Model :=
  { positiveModel with boxes := [{ box with transitions := [changed] }] }

private def badGuard : IR.Model := withTransition { transition with guard := .int 1 }
#guard expectModelError badGuard (.term .expectedBool)
  [.model, .box 0, .transition 0, .guard]

private def badEnumTestSort : IR.Model := withTransition
  { transition with guard := .enumIs "count" "open" }
#guard expectModelError badEnumTestSort (.term .sortMismatch)
  [.model, .box 0, .transition 0, .guard]

private def badHazard : IR.Model := withTransition { transition with hazard := .int 1 }
#guard expectModelError badHazard (.term .expectedReal)
  [.model, .box 0, .transition 0, .hazard]

private def badAssignment : IR.Model := withTransition
  { transition with effects := [.setAttr "rate" (.int 1)] }
#guard expectModelError badAssignment (.term .expectedReal)
  [.model, .box 0, .transition 0, .effects, .effect 0, .value]

private def badEffectDestination : IR.Model := withTransition
  { transition with effects := [.setAttr "missing" (.int 1)] }
#guard expectModelError badEffectDestination (.term .unknownAttribute)
  [.model, .box 0, .transition 0, .effects, .effect 0, .destination]

private def badClaimResource : IR.Model := withTransition
  { transition with contests := [{ resource := .int 1, ordering := .raceTime }] }
#guard expectModelError badClaimResource (.term .expectedReference)
  [.model, .box 0, .transition 0, .contests, .claim 0, .resource]

private def duplicateClaim : IR.ResourceClaim :=
  { resource := .selfAttr "region", ordering := .key (.selfAttr "count") }
private def badDuplicateClaim : IR.Model := withTransition
  { transition with contests := [
      { resource := .selfAttr "region", ordering := .raceTime }, duplicateClaim
    ] }
#guard expectModelError badDuplicateClaim (.term .duplicateResourceClaim)
  [.model, .box 0, .transition 0, .contests, .claim 1, .resource]

private def badBoolClaimKey : IR.Model := withTransition
  { transition with contests := [{ resource := .selfAttr "region", ordering := .key (.bool true) }] }
#guard expectModelError badBoolClaimKey (.term .expectedOrderable)
  [.model, .box 0, .transition 0, .contests, .claim 0, .orderingKey]

private def badRefClaimKey : IR.Model := withTransition
  { transition with contests := [{ resource := .selfAttr "region", ordering := .key (.selfAttr "manager") }] }
#guard expectModelError badRefClaimKey (.term .expectedOrderable)
  [.model, .box 0, .transition 0, .contests, .claim 0, .orderingKey]

private def badUnclaimedWrite : IR.Model := withTransition
  { transition with effects := [.setAttr "region" (.selfAttr "region")], contests := [] }
#guard expectModelError badUnclaimedWrite (.term .unclaimedRefWrite)
  [.model, .box 0, .transition 0, .effects, .effect 0, .value]

private def badViewTable : IR.Model :=
  { positiveModel with
    boxes := [{ box with views := [
      { name := "bad", table := "missing", filter := none,
        value := none, reduce := .count }
    ] }]
    summaries := [] }
#guard expectModelError badViewTable .unresolvedViewTable
  [.model, .box 0, .view 0, .viewTable]

private def badViewFilter : IR.Model :=
  { positiveModel with
    boxes := [{ box with views := [
      { name := "bad", table := "People", filter := some (.int 1),
        value := none, reduce := .count }
    ] }]
    summaries := [] }
#guard expectModelError badViewFilter (.term .expectedBool)
  [.model, .box 0, .view 0, .viewFilter]

private def badGroupedMissingBand : IR.Model :=
  { positiveModel with boxes := [{ box with groupedViews := [
      { grouped with keys := [{ attr := "count", bandWidth := none }] }
    ] }] }
#guard expectModelError badGroupedMissingBand .missingGroupedBand
  [.model, .box 0, .groupedView 0, .groupedKeys, .groupedKey 0, .groupedBand]

private def badGroupedUnexpectedBand : IR.Model :=
  { positiveModel with boxes := [{ box with groupedViews := [
      { grouped with keys := [{ attr := "status", bandWidth := some 1 }] }
    ] }] }
#guard expectModelError badGroupedUnexpectedBand .unexpectedGroupedBand
  [.model, .box 0, .groupedView 0, .groupedKeys, .groupedKey 0, .groupedBand]

private def badGroupedFilter : IR.Model :=
  { positiveModel with boxes := [{ box with groupedViews := [
      { grouped with filter := some (.input "flow" (.mk .count none)) }
    ] }] }
#guard expectModelError badGroupedFilter .aggregateInGroupedFilter
  [.model, .box 0, .groupedView 0, .viewFilter]

private def badGroupedBooleanFilter : IR.Model :=
  { positiveModel with boxes := [{ box with groupedViews := [
      { grouped with filter := some (.int 1) }
    ] }] }
#guard expectModelError badGroupedBooleanFilter (.term .expectedBool)
  [.model, .box 0, .groupedView 0, .viewFilter]

/-- The declaration bridge is exercised at generic and concrete boundaries. -/
example : (declarations.inputPortSchemas simBox).map BoxPortSchema.name = ["flow"] := by
  simpa [declarations, positiveModel, box, inputPort] using
    declarations.inputPortSchemas_names simBox
example : (declarations.outputPortSchemas simBox).map BoxPortSchema.source =
    [outputDecl.schema] := by
  simpa [declarations, positiveModel, box, outputDecl] using
    declarations.outputPortSchemas_sources simBox

/- Required term-correspondence declarations are part of the compiled API. -/
#check synthExpr_sound
#check checkExpr_sound
#check synthExpr_complete
#check checkExpr_complete
#check ExprSynthesizes.sort_unique
#check checkTransitionTerms_sound
#check checkTransitionTerms_complete
#check checkModel_elaborates
#check checkModel_sound
#check ModelElaborates.checkModel_exists
#check checkModel_complete
#check checkModel_failure_iff
#check Checked.Model.checkModel_canonical
#check Checked.Model.checkModel_equivalent_of_elaborates
#check Checked.Model.checkModel_checked_round_trip

/-- A concrete successful first pass has a theorem-backed erase/recheck result
that is structurally equivalent to the first checked model. -/
example {first : Checked.Model} (success : checkModel positiveModel = .ok first) :
    ∃ second, checkModel first.erase = .ok second ∧ second.Equivalent first := by
  exact Checked.Model.checkModel_checked_round_trip
    (Checked.Model.checkModel_canonical success)

/-- Structural model equivalence is a genuine equivalence relation. -/
example (checked : Checked.Model) : checked.Equivalent checked :=
  Checked.Model.Equivalent.refl checked

example {left right : Checked.Model} (same : left.Equivalent right) :
    right.Equivalent left := Checked.Model.Equivalent.symm same

example {first second third : Checked.Model} (left : first.Equivalent second)
    (right : second.Equivalent third) : first.Equivalent third :=
  Checked.Model.Equivalent.trans left right

/-- Independent model validity supplies reconstructive erasure without adding a
wire-validity premise. -/
example {raw : IR.Model} (wellFormed : ModelWellFormed raw) :
    ∃ checked : Checked.Model, checked.erase = raw :=
  wellFormed.has_checked_erasure

end Sembla.Semantics.CheckModelTests
