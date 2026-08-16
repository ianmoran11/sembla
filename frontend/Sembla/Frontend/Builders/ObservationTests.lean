import Sembla.Frontend.Builders.Observation

/-!
Direct executable fixtures for exact observation construction, ordinal complete
assembly, surface output ordering, complete-candidate context use, structured
failure retention, and exact checked erasure.
-/
namespace Sembla.Frontend.Builders.ObservationTests

open Sembla
open Sembla.Frontend.Builders
open Sembla.Semantics

private instance {ε α : Type} [BEq ε] [BEq α] : BEq (Except ε α) where
  beq left right :=
    match left, right with
    | .ok left, .ok right => left == right
    | .error left, .error right => left == right
    | _, _ => false

private def sci (coefficient : Int) (exponent : Int := 0) : IR.Scientific :=
  ⟨coefficient, exponent⟩

private def regionTable : IR.Table :=
  tableRaw "Region" 3 [
    attributeRaw "rank" .int,
    attributeRaw "kind" (.enum ["urban", "rural"])
  ]

private def peopleTable : IR.Table :=
  tableRaw "People" 10 [
    attributeRaw "amount" .real,
    attributeRaw "count" .int,
    attributeRaw "kind" (.enum ["open", "closed"]),
    attributeRaw "region" (.ref "Region")
  ]

private def inputFlow : IR.PortDecl :=
  ObservationRaw.input "flow" [
    attributeRaw "amount" .real,
    attributeRaw "region" (.ref "Region")
  ]

private def inputControl : IR.PortDecl :=
  ObservationRaw.input "control" [attributeRaw "amount" .real]

private def outputSchema : List IR.Attr := [
  attributeRaw "total_count" .int,
  attributeRaw "total_amount" .real
]

private def countField : IR.OutputField :=
  ObservationRaw.outputField "total_count" .count (some (.bool true))

private def amountField : IR.OutputField :=
  ObservationRaw.outputField "total_amount" (.sum (.selfAttr "amount")) none

private def outputTotals : IR.OutputDecl :=
  ObservationRaw.output "totals" outputSchema
    (ObservationRaw.perTable "People" [countField, amountField])

private def outputTotalsCopy : IR.OutputDecl :=
  ObservationRaw.output "totals_copy" outputSchema
    (ObservationRaw.perTable "People" [countField, amountField])

private def viewCount : IR.ViewDecl :=
  ObservationRaw.view "people_count" "People" none none .count
private def viewSum : IR.ViewDecl :=
  ObservationRaw.view "amount_sum" "People" none (some (.selfAttr "amount")) .sum
private def viewMin : IR.ViewDecl :=
  ObservationRaw.view "count_min" "People" none (some (.selfAttr "count")) .min
private def viewMax : IR.ViewDecl :=
  ObservationRaw.view "amount_max" "People" (some (.bool true))
    (some (.selfAttr "amount")) .max
private def viewInputAggregate : IR.ViewDecl :=
  ObservationRaw.view "input_amount" "People" none
    (some (.input "flow" (.mk (.sum (.selfAttr "amount")) none))) .sum

private def groupedAll : IR.GroupedViewDecl :=
  ObservationRaw.groupedView "grouped" "People" none [
    ObservationRaw.groupKey "kind",
    ObservationRaw.groupKey "region",
    ObservationRaw.groupKey "count" (some 5)
  ]

private def summaries : List IR.SummaryDecl := [
  ObservationRaw.summary "sum" "Simulation" "amount_sum" .sum,
  ObservationRaw.summary "min" "Simulation" "amount_sum" .min,
  ObservationRaw.summary "max" "Simulation" "amount_sum" .max,
  ObservationRaw.summary "last" "Simulation" "amount_sum" .last,
  ObservationRaw.summary "argmax" "Simulation" "amount_sum" .argmaxTick
]

private def inputAggregateTransition : IR.Transition :=
  TransitionRaw.transition "advance" "People" (.bool true)
    (.input "flow" (.mk (.sum (.selfAttr "amount")) none)) [] []

private def idleTransition : IR.Transition :=
  TransitionRaw.transition "idle" "People" (.bool true) (.real (sci 1)) [] []

private def coreShell : CoreModelShell :=
  { name := "observation-fixture"
    dt := sci 1 (-1)
    params := []
    boxes := [
      { name := "Simulation", tables := [peopleTable, regionTable] },
      { name := "Replica", tables := [peopleTable, regionTable] }
    ] }

private def overlay : TransitionOverlaySpec :=
  { core := coreShell
    transitions := fun ordinal =>
      if ordinal.val = 0 then [inputAggregateTransition, idleTransition] else [] }

private def payload (ordinal : Fin overlay.core.boxes.length) : BoxObservationPayload :=
  if ordinal.val = 0 then
    { inputs := [inputFlow, inputControl]
      outputs := [outputTotals, outputTotalsCopy]
      views := [viewCount, viewSum, viewMin, viewMax, viewInputAggregate]
      groupedViews := [groupedAll] }
  else
    { inputs := [inputFlow]
      outputs := []
      views := [ObservationRaw.view "replica_count" "People" none none .count]
      groupedViews := [] }

private def opaqueWires : List IR.Wire := [
  { source := { box := "Simulation", port := "totals" }
    target := { box := "Replica", port := "flow" } },
  { source := { box := "unvalidated", port := "raw" }
    target := { box := "opaque", port := "wire" } }
]

private def completeSpec : CompleteModelSpec :=
  { overlay := overlay
    observations := payload
    summaries := summaries
    wires := opaqueWires }

private def surfaceOutputOfRaw (raw : IR.OutputDecl) : SurfaceOutputSpec :=
  match raw.builder with
  | .perTable table fields =>
      { name := raw.name, schema := raw.schema, table := table,
        fields := fields.map fun field =>
          { name := field.name, op := field.op, filter := field.filter } }

private def surfacePayload (ordinal : Fin overlay.core.boxes.length) :
    SurfaceBoxObservationPayload :=
  let raw := payload ordinal
  { inputs := raw.inputs, outputs := raw.outputs.map surfaceOutputOfRaw,
    views := raw.views, groupedViews := raw.groupedViews }

private def surfaceSpec : SurfaceCompleteModelSpec :=
  { overlay := overlay, observations := surfacePayload,
    summaries := summaries, wires := opaqueWires }

private def completeRaw : IR.Model := completeSpec.toRaw

/-! Constructor fidelity and every reducer/operation family. -/
#guard inputFlow.name == "flow"
#guard inputFlow.schema.map IR.Attr.name == ["amount", "region"]
#guard countField == { name := "total_count", op := .count, filter := some (.bool true) }
#guard amountField == { name := "total_amount", op := .sum (.selfAttr "amount"), filter := none }
#guard outputTotals.builder == .perTable "People" [countField, amountField]
#guard [viewCount.reduce, viewSum.reduce, viewMin.reduce, viewMax.reduce] ==
  [.count, .sum, .min, .max]
#guard groupedAll.keys == [
  { attr := "kind", bandWidth := none },
  { attr := "region", bandWidth := none },
  { attr := "count", bandWidth := some 5 }
]
#guard summaries.map IR.SummaryDecl.reduce ==
  [.sum, .min, .max, .last, .argmaxTick]

/-! Raw field order and current-surface schema projection are distinct. -/
private def authoredAmount : SurfaceOutputFieldSpec :=
  { name := "total_amount", op := .sum (.selfAttr "amount"), filter := none }
private def authoredCount : SurfaceOutputFieldSpec :=
  { name := "total_count", op := .count, filter := some (.bool true) }
private def authoredInterleaved : List SurfaceOutputFieldSpec :=
  [authoredAmount, authoredCount]

private def rawInterleaved : IR.OutputDecl :=
  ObservationRaw.output "raw-order" outputSchema
    (ObservationRaw.perTable "People" [amountField, countField])

#guard rawInterleaved.builder == .perTable "People" [amountField, countField]
#guard lowerSurfaceOutputFields outputSchema authoredInterleaved ==
  .ok [countField, amountField]

private def loweringDuplicate : Bool :=
  match lowerSurfaceOutputFields outputSchema
      [authoredAmount, authoredAmount] 3 4 with
  | .error error => error.category == .duplicateOutputField &&
      error.path == [.box 3, .output 4, .outputField 1]
  | _ => false
#guard loweringDuplicate

private def loweringExtra : Bool :=
  match lowerSurfaceOutputFields outputSchema
      (authoredInterleaved ++ [{ name := "extra", op := .count, filter := none }]) 3 4 with
  | .error error => error.category == .extraOutputField &&
      error.path == [.box 3, .output 4, .outputField 2]
  | _ => false
#guard loweringExtra

private def loweringMissing : Bool :=
  match lowerSurfaceOutputFields outputSchema [authoredAmount] 3 4 with
  | .error error => error.category == .missingOutputField &&
      error.path == [.box 3, .output 4, .outputSchema 0]
  | _ => false
#guard loweringMissing

/-! Exact complete assembly and ordinal attachment: transitions [2,0], inputs
[2,1], outputs [2,0], views [5,1], grouped views [1,0]. -/
#guard completeRaw.name == coreShell.name
#guard completeRaw.dt == coreShell.dt
#guard completeRaw.params == coreShell.params
#guard completeRaw.boxes.map IR.Box.name == ["Simulation", "Replica"]
#guard completeRaw.boxes.map IR.Box.tables ==
  [[peopleTable, regionTable], [peopleTable, regionTable]]
#guard completeRaw.boxes.map (fun box => box.transitions.length) == [2, 0]
#guard completeRaw.boxes.map (fun box => box.inputs.length) == [2, 1]
#guard completeRaw.boxes.map (fun box => box.outputs.length) == [2, 0]
#guard completeRaw.boxes.head?.map (·.inputs.map IR.PortDecl.name) ==
  some ["flow", "control"]
#guard completeRaw.boxes.head?.map (·.outputs.map IR.OutputDecl.name) ==
  some ["totals", "totals_copy"]
private def firstOutputFieldNames : Option (List (List String)) :=
  completeRaw.boxes.head?.map fun box => box.outputs.map fun output =>
    match output.builder with | .perTable _ fields => fields.map IR.OutputField.name
#guard firstOutputFieldNames ==
  some [["total_count", "total_amount"], ["total_count", "total_amount"]]
private def firstOutputFieldCounts : Option (List Nat) :=
  completeRaw.boxes.head?.map fun box => box.outputs.map fun output =>
    match output.builder with | .perTable _ fields => fields.length
#guard firstOutputFieldCounts == some [2, 2]
#guard completeRaw.boxes.map (fun box => box.views.length) == [5, 1]
#guard completeRaw.boxes.map (fun box => box.groupedViews.length) == [1, 0]
#guard completeRaw.summaries == summaries
#guard completeRaw.wires == opaqueWires

/-! The complete input-bearing context is assembled before transition adapters
and the final checker. -/
#guard (checkDeclarations completeRaw).isOk
#guard (checkModel completeRaw).isOk
#guard (buildCompleteModel completeSpec).isOk
#guard (buildSurfaceCompleteModel surfaceSpec).isOk

private def completeCheckedErasesExactly : Bool :=
  match buildCompleteModel completeSpec with
  | .ok checked => checked.erase == completeRaw
  | .error _ => false
#guard completeCheckedErasesExactly

private def surfaceCheckedErasesExactly : Bool :=
  match buildSurfaceCompleteModel surfaceSpec with
  | .ok checked => checked.erase == completeRaw
  | .error _ => false
#guard surfaceCheckedErasesExactly

private def withFirstSurfaceFields (fields : List SurfaceOutputFieldSpec) :
    SurfaceCompleteModelSpec :=
  { surfaceSpec with observations := (fun ordinal =>
      if ordinal.val = 0 then
        let base := surfacePayload ordinal
        { base with outputs :=
            ({ surfaceOutputOfRaw outputTotals with fields := fields } : SurfaceOutputSpec) ::
              [surfaceOutputOfRaw outputTotalsCopy] }
      else surfacePayload ordinal) }

private def surfaceLoweringFixturePasses (source : SurfaceCompleteModelSpec)
    (category : ObservationLoweringErrorCategory)
    (path : List ObservationSurfacePathSegment) : Bool :=
  match buildSurfaceCompleteModel source with
  | .error (.lowering error) => error.category == category && error.path == path
  | _ => false

#guard surfaceLoweringFixturePasses
  (withFirstSurfaceFields [authoredAmount, authoredAmount]) .duplicateOutputField
  [.box 0, .output 0, .outputField 1]
#guard surfaceLoweringFixturePasses
  (withFirstSurfaceFields (authoredInterleaved ++ [
    { name := "extra", op := .count, filter := none }])) .extraOutputField
  [.box 0, .output 0, .outputField 2]
#guard surfaceLoweringFixturePasses
  (withFirstSurfaceFields [authoredAmount]) .missingOutputField
  [.box 0, .output 0, .outputSchema 0]

/-! Exact representative wrappers and authoritative model paths. -/
private def badCoreSpec : CompleteModelSpec :=
  { completeSpec with overlay := { overlay with core := { coreShell with dt := sci 0 } } }

private def coreErrorExact : Bool :=
  match buildCompleteModel badCoreSpec with
  | .error (.core error) => error.category == .nonpositiveDt &&
      error.path == [.modelMetadata, .dt]
  | _ => false
#guard coreErrorExact

private def badTransitionSpec : CompleteModelSpec :=
  { completeSpec with
    overlay := { overlay with transitions := fun ordinal =>
      if ordinal.val = 0 then [{ inputAggregateTransition with guard := .int 1 }] else [] } }

private def transitionErrorExact : Bool :=
  match buildCompleteModel badTransitionSpec with
  | .error (.transition (.term error)) =>
      error.category == .expectedBool &&
        error.path == [.model, .box 0, .transition 0, .guard]
  | _ => false
#guard transitionErrorExact

private def surfaceKeySpec : SurfaceCompleteModelSpec :=
  { surfaceSpec with
    overlay := { overlay with transitions := fun ordinal =>
      if ordinal.val = 0 then [{ idleTransition with contests := [
        TransitionRaw.keyClaim (.selfAttr "region") (.selfAttr "count")
      ] }] else [] } }

private def surfaceKeyRejectedExact : Bool :=
  match buildSurfaceCompleteModel surfaceKeySpec with
  | .error (.transition (.unsupportedSurfaceKeyOrdering path)) =>
      path == [.box 0, .transition 0, .claim 0, .orderingKey]
  | _ => false
#guard surfaceKeyRejectedExact

private def withSurfaceTransition1 (changed : IR.Transition) : SurfaceCompleteModelSpec :=
  { surfaceSpec with overlay := { overlay with transitions := fun ordinal =>
      if ordinal.val = 0 then [inputAggregateTransition, changed] else [] } }

private def surfaceTransitionFixturePasses (source : SurfaceCompleteModelSpec)
    (category : TermCheckErrorCategory) (path : List ModelCheckPathSegment) : Bool :=
  match buildSurfaceCompleteModel source with
  | .error (.transition (.term error)) =>
      error.category == category && error.path == path
  | _ => false

/- Public structured counterparts for the used source/destination alias command
probes. The source pattern contributes the invalid right guard atom; the
destination pattern contributes the uncovered Ref write. -/
private def badAliasSourceGuard : IR.Expr :=
  .and (.eq (.selfAttr "region") (.selfAttr "region")) (.int 1)

private def badAliasSourceTransition : IR.Transition :=
  { idleTransition with guard := badAliasSourceGuard }

#guard surfaceTransitionFixturePasses
  (withSurfaceTransition1 badAliasSourceTransition)
  .expectedBool [.model, .box 0, .transition 1, .guard, .rhs]
#guard surfaceTransitionFixturePasses
  (withSurfaceTransition1 { idleTransition with effects := [
    .setAttr "region" (.selfAttr "region")] })
  .unclaimedRefWrite [.model, .box 0, .transition 1, .effects, .effect 0, .value]

private def withFirstPayload (replacement : BoxObservationPayload) : CompleteModelSpec :=
  { completeSpec with observations := fun ordinal =>
      if ordinal.val = 0 then replacement else payload ordinal }

private def baseFirstPayload : BoxObservationPayload := payload ⟨0, by decide⟩

private def badOutputTableSpec : CompleteModelSpec :=
  withFirstPayload { baseFirstPayload with outputs := [
    { outputTotals with builder := .perTable "Missing" [countField, amountField] }
  ] }

private def outputTableErrorExact : Bool :=
  match buildCompleteModel badOutputTableSpec with
  | .error (.modelCheck (.model error)) =>
      error.category == .unresolvedOutputTable &&
        error.path == [.model, .box 0, .output 0, .outputBuilder, .tableTarget]
  | _ => false
#guard outputTableErrorExact

private def badGroupedBandSpec : CompleteModelSpec :=
  withFirstPayload { baseFirstPayload with groupedViews := [
    { groupedAll with keys := [ObservationRaw.groupKey "count"] }
  ] }

private def groupedBandErrorExact : Bool :=
  match buildCompleteModel badGroupedBandSpec with
  | .error (.modelCheck (.model error)) =>
      error.category == .missingGroupedBand &&
        error.path == [.model, .box 0, .groupedView 0, .groupedKeys,
          .groupedKey 0, .groupedBand]
  | _ => false
#guard groupedBandErrorExact

private def badSummarySpec : CompleteModelSpec :=
  { completeSpec with summaries := [
      ObservationRaw.summary "bad" "Missing" "amount_sum" .sum ] }

private def summaryErrorExact : Bool :=
  match buildCompleteModel badSummarySpec with
  | .error (.modelCheck (.model error)) =>
      error.category == .unresolvedSummaryBox &&
        error.path == [.model, .summary 0, .summaryBox]
  | _ => false
#guard summaryErrorExact

/-! Full executable rejection corpus. Every fixture mutates one semantic leaf
of the accepted two-box baseline and checks the exact final wrapper, category,
and authoritative path. -/
private structure ModelErrorFixture where
  candidate : CompleteModelSpec
  category : ModelTermErrorCategory
  path : List ModelCheckPathSegment

private def modelFixturePasses (fixture : ModelErrorFixture) : Bool :=
  match buildCompleteModel fixture.candidate with
  | .error (.modelCheck (.model error)) =>
      error.category == fixture.category && error.path == fixture.path
  | _ => false

private structure TransitionErrorFixture where
  candidate : CompleteModelSpec
  category : TermCheckErrorCategory
  path : List ModelCheckPathSegment

private def transitionFixturePasses (fixture : TransitionErrorFixture) : Bool :=
  match buildCompleteModel fixture.candidate with
  | .error (.transition (.term error)) =>
      error.category == fixture.category && error.path == fixture.path
  | _ => false

private def withView0 (changed : IR.ViewDecl) : CompleteModelSpec :=
  withFirstPayload { baseFirstPayload with
    views := [changed, viewSum, viewMin, viewMax, viewInputAggregate] }

private def withOutput0 (changed : IR.OutputDecl) : CompleteModelSpec :=
  withFirstPayload { baseFirstPayload with outputs := [changed, outputTotalsCopy] }

private def withGrouped0 (changed : IR.GroupedViewDecl) : CompleteModelSpec :=
  withFirstPayload { baseFirstPayload with groupedViews := [changed] }

private def withSummary0 (changed : IR.SummaryDecl) : CompleteModelSpec :=
  { completeSpec with summaries := changed :: summaries.drop 1 }

private def withTransition1 (changed : IR.Transition) : CompleteModelSpec :=
  { completeSpec with overlay := { overlay with transitions := fun ordinal =>
      if ordinal.val = 0 then [inputAggregateTransition, changed] else [] } }

private def modelRoot : List ModelCheckPathSegment := [.model, .box 0]
private def viewFilterPath : List ModelCheckPathSegment :=
  modelRoot ++ [.view 0, .viewFilter]

private def observationErrorFixtures : List ModelErrorFixture := [
  ⟨withOutput0 { outputTotals with builder := (.perTable "Missing" [countField, amountField]) },
    .unresolvedOutputTable, modelRoot ++ [.output 0, .outputBuilder, .tableTarget]⟩,
  ⟨withOutput0 { outputTotals with builder := (.perTable "People"
      [countField, { amountField with name := "total_count" }]) },
    .duplicateOutputField,
    modelRoot ++ [.output 0, .outputFields, .outputField 1, .fieldName]⟩,
  ⟨withOutput0 { outputTotals with builder := (.perTable "People" [countField]) },
    .outputFieldCountMismatch,
    modelRoot ++ [.output 0, .outputFields, .outputSchema]⟩,
  ⟨withOutput0 { outputTotals with builder := (.perTable "People"
      [countField, { amountField with name := "wrong" }]) },
    .outputFieldNameMismatch,
    modelRoot ++ [.output 0, .outputFields, .outputField 1, .fieldName]⟩,
  ⟨withOutput0 { outputTotals with builder := (.perTable "People"
      [countField, { amountField with op := .count }]) },
    .outputFieldSortMismatch,
    modelRoot ++ [.output 0, .outputFields, .outputField 1, .fieldOperation]⟩,
  ⟨withView0 { viewCount with table := "Missing" }, .unresolvedViewTable,
    modelRoot ++ [.view 0, .viewTable]⟩,
  ⟨withView0 { viewCount with value := some (.int 1) }, .invalidViewReducerShape,
    modelRoot ++ [.view 0, .viewReducer]⟩,
  ⟨withGrouped0 { groupedAll with keys := [] }, .invalidGroupedKeyCount,
    modelRoot ++ [.groupedView 0, .groupedKeys]⟩,
  ⟨withGrouped0 { groupedAll with keys := [
      ObservationRaw.groupKey "missing", ObservationRaw.groupKey "region",
      ObservationRaw.groupKey "count" (some 5)] }, .unresolvedGroupedKey,
    modelRoot ++ [.groupedView 0, .groupedKeys, .groupedKey 0, .groupedAttribute]⟩,
  ⟨withGrouped0 { groupedAll with keys := [
      ObservationRaw.groupKey "amount", ObservationRaw.groupKey "region",
      ObservationRaw.groupKey "count" (some 5)] }, .invalidGroupedKeySort,
    modelRoot ++ [.groupedView 0, .groupedKeys, .groupedKey 0, .groupedAttribute]⟩,
  ⟨withGrouped0 { groupedAll with keys := [
      ObservationRaw.groupKey "kind", ObservationRaw.groupKey "region",
      ObservationRaw.groupKey "count"] }, .missingGroupedBand,
    modelRoot ++ [.groupedView 0, .groupedKeys, .groupedKey 2, .groupedBand]⟩,
  ⟨withGrouped0 { groupedAll with keys := [
      ObservationRaw.groupKey "kind" (some 1), ObservationRaw.groupKey "region",
      ObservationRaw.groupKey "count" (some 5)] }, .unexpectedGroupedBand,
    modelRoot ++ [.groupedView 0, .groupedKeys, .groupedKey 0, .groupedBand]⟩,
  ⟨withGrouped0 { groupedAll with keys := [
      ObservationRaw.groupKey "kind", ObservationRaw.groupKey "region",
      ObservationRaw.groupKey "count" (some 0)] }, .nonpositiveGroupedBand,
    modelRoot ++ [.groupedView 0, .groupedKeys, .groupedKey 2, .groupedBand]⟩,
  ⟨withGrouped0 { groupedAll with
      filter := some (.input "flow" (.mk .count none)) }, .aggregateInGroupedFilter,
    modelRoot ++ [.groupedView 0, .viewFilter]⟩,
  ⟨withSummary0 (ObservationRaw.summary "sum" "Missing" "amount_sum" .sum),
    .unresolvedSummaryBox, [.model, .summary 0, .summaryBox]⟩,
  ⟨withSummary0 (ObservationRaw.summary "sum" "Simulation" "Missing" .sum),
    .unresolvedSummaryView, [.model, .summary 0, .summaryView]⟩
]

private def viewFilterSpec (filter : IR.Expr) : CompleteModelSpec :=
  withView0 { viewCount with filter := some filter }

private def enrichedPeopleTable : IR.Table :=
  { peopleTable with attrs := peopleTable.attrs ++ [attributeRaw "manager" (.ref "People")] }

private def enrichedCoreShell : CoreModelShell :=
  { coreShell with boxes := [
      { name := "Simulation", tables := [enrichedPeopleTable, regionTable] },
      { name := "Replica", tables := [enrichedPeopleTable, regionTable] }
    ] }

private def enrichedOverlay : TransitionOverlaySpec :=
  { overlay with core := enrichedCoreShell }

private def enrichedSpec : CompleteModelSpec :=
  { completeSpec with overlay := enrichedOverlay }

private def enrichedViewFilterSpec (filter : IR.Expr) : CompleteModelSpec :=
  { enrichedSpec with observations := (fun ordinal =>
      if ordinal.val = 0 then
        { baseFirstPayload with views :=
            ({ viewCount with filter := some filter } : IR.ViewDecl) ::
              [viewSum, viewMin, viewMax, viewInputAggregate] }
      else payload ordinal) }

#guard (buildCompleteModel enrichedSpec).isOk

private def nestedModelTermFixtures : List ModelErrorFixture := [
  ⟨viewFilterSpec (.param "missing"), .term .unknownParameter, viewFilterPath⟩,
  ⟨viewFilterSpec (.selfAttr "missing"), .term .unknownAttribute, viewFilterPath⟩,
  ⟨viewFilterSpec (.enumIs "kind" "missing"), .term .unknownEnumVariant, viewFilterPath⟩,
  ⟨viewFilterSpec (.input "missing" (.mk .count none)), .term .unknownInput,
    viewFilterPath ++ [.inputPort]⟩,
  ⟨viewFilterSpec (.agg .count "Missing" "region" "region" (.bool true)),
    .term .unknownTable, viewFilterPath ++ [.tableTarget]⟩,
  ⟨viewFilterSpec (.agg .count "People" "missing" "region" (.bool true)),
    .term .unknownJoinAttribute, viewFilterPath ++ [.joinForeignAttribute]⟩,
  ⟨viewFilterSpec (.input "flow"
      (.mk (.sum (.input "flow" (.mk .count none))) none)),
    .term .nestedInputAggregate, viewFilterPath ++ [.aggregate, .aggregateValue]⟩,
  ⟨viewFilterSpec (.eq (.enum "open") (.enum "closed")),
    .term .cannotInferEnumOwner, viewFilterPath⟩,
  ⟨viewFilterSpec (.int 1), .term .expectedBool, viewFilterPath⟩,
  ⟨viewFilterSpec (.add (.bool true) (.int 1)), .term .expectedNumeric, viewFilterPath⟩,
  ⟨viewFilterSpec (.agg .count "People" "count" "region" (.bool true)),
    .term .expectedReference, viewFilterPath ++ [.joinForeignAttribute]⟩,
  ⟨viewFilterSpec (.enumIs "count" "open"), .term .sortMismatch, viewFilterPath⟩,
  ⟨viewFilterSpec (.eq (.bool true) (.int 1)),
    .term .incompatibleEquality, viewFilterPath⟩,
  ⟨enrichedViewFilterSpec
      (.agg .count "People" "region" "manager" (.bool true)),
    .term .incompatibleJoinTargets, viewFilterPath⟩
]

private def claimRegion : IR.ResourceClaim :=
  { resource := .selfAttr "region", ordering := .raceTime }

private def nestedTransitionTermFixtures : List TransitionErrorFixture := [
  ⟨withTransition1 { idleTransition with hazard := .int 1 }, .expectedReal,
    modelRoot ++ [.transition 1, .hazard]⟩,
  ⟨withTransition1 { idleTransition with contests := [
      { resource := .selfAttr "region", ordering := .key (.bool true) }] },
    .expectedOrderable, modelRoot ++ [.transition 1, .contests, .claim 0, .orderingKey]⟩,
  ⟨withTransition1 { idleTransition with contests := [claimRegion, claimRegion] },
    .duplicateResourceClaim,
    modelRoot ++ [.transition 1, .contests, .claim 1, .resource]⟩,
  ⟨withTransition1 { idleTransition with effects := [
      .setAttr "region" (.selfAttr "region")] }, .unclaimedRefWrite,
    modelRoot ++ [.transition 1, .effects, .effect 0, .value]⟩
]

#guard observationErrorFixtures.length == 16
#guard nestedModelTermFixtures.length == 14
#guard nestedTransitionTermFixtures.length == 4
#guard observationErrorFixtures.all modelFixturePasses
#guard nestedModelTermFixtures.all modelFixturePasses
#guard nestedTransitionTermFixtures.all transitionFixturePasses
#guard observationErrorFixtures.length + nestedModelTermFixtures.length +
  nestedTransitionTermFixtures.length == 34

/-! Declaration failures are exact final-checker errors, while a surface-only
transition restriction retains precedence over a later model-check failure. -/
private def declarationInvalidSpec : CompleteModelSpec :=
  withFirstPayload { baseFirstPayload with inputs := [inputFlow, inputFlow] }

private def declarationFailureIsExact : Bool :=
  match checkModel declarationInvalidSpec.toRaw, buildCompleteModel declarationInvalidSpec with
  | .error expected, .error (.modelCheck actual) => expected == actual
  | _, _ => false
#guard declarationFailureIsExact

private def dualDefectSurfaceSpec : SurfaceCompleteModelSpec :=
  { surfaceKeySpec with summaries := [
      ObservationRaw.summary "missing" "Simulation" "missing" .sum] }

private def surfaceTransitionPrecedesModelCheck : Bool :=
  match dualDefectSurfaceSpec.toCompleteModelSpec with
  | .error _ => false
  | .ok rawSpec =>
      match checkModel rawSpec.toRaw, buildSurfaceCompleteModel dualDefectSurfaceSpec with
      | .error _, .error (.transition (.unsupportedSurfaceKeyOrdering _)) => true
      | _, _ => false
#guard surfaceTransitionPrecedesModelCheck

/-! Public proof matrix remains mechanically name-checkable. -/
#check ObservationRaw.input_exact
#check ObservationRaw.outputField_exact
#check ObservationRaw.output_fields_preserve_supplied_order
#check ObservationRaw.view_exact
#check ObservationRaw.groupKey_exact
#check ObservationRaw.groupedView_exact
#check ObservationRaw.summary_exact
#check lowerSurfaceOutputFields_schema_order
#check CompleteModelSpec.rawBoxes_get
#check CompleteModelSpec.toRaw_core_box_names_exact
#check CompleteModelSpec.toRaw_core_tables_exact
#check CompleteModelSpec.toRaw_transitions_exact
#check CompleteModelSpec.toRaw_inputs_exact
#check CompleteModelSpec.toRaw_outputs_exact
#check CompleteModelSpec.toRaw_views_exact
#check CompleteModelSpec.toRaw_groupedViews_exact
#check CompleteModelSpec.toRaw_summaries_exact
#check CompleteModelSpec.toRaw_wires_exact
#check buildSurfaceOutputFields_error_iff
#check buildSurfaceCompleteModel_sound
#check buildSurfaceCompleteModel_model_acceptance_and_erasure
#check buildSurfaceCompleteModel_failure_iff
#check buildCompleteModel_sound
#check buildCompleteModel_model_acceptance_and_erasure
#check buildCompleteModel_complete
#check buildCompleteModel_failure_iff
#check buildCompleteModel_model_check_error_iff

example {checked} (success : buildCompleteModel completeSpec = .ok checked) :
    ModelWellFormed completeSpec.toRaw ∧
      checkModel completeSpec.toRaw = .ok checked ∧
      checked.erase = completeSpec.toRaw :=
  buildCompleteModel_sound success

end Sembla.Frontend.Builders.ObservationTests
