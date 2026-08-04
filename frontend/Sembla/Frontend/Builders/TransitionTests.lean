import Sembla.Frontend.Builders.Transition

/-!
Executable fixtures for the complete transition-builder fragment. Positive
fixtures retain exact raw values/order; every negative fixture has one defect
and asserts the stable nested checker category/path.
-/
namespace Sembla.Frontend.Builders.TransitionTests

open Sembla
open Sembla.Frontend.Builders
open Sembla.Semantics

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

/-- Input-bearing context proves the term builders are not specialized to the
transition-only model projection. -/
private def contextRaw : IR.Model :=
  { name := "transition-context"
    dt := sci 1 (-1)
    params := [
      { name := "gain", ty := .real, default := .real (sci 125 (-2)), prior := none },
      { name := "offset", ty := .int, default := .int 2, prior := none }
    ]
    boxes := [{
      name := "Sim"
      tables := [peopleTable, eventTable, regionTable]
      transitions := []
      inputs := [inputPort]
      outputs := []
      views := []
      groupedViews := []
    }]
    wires := []
    summaries := [] }

private theorem contextDeclarations : DeclarationsWellFormed contextRaw := by decide
private def declarations : DeclarationContext := ⟨contextRaw, contextDeclarations⟩
private def simBox : BoxId declarations.modelSchema.catalog := ⟨⟨0, by decide⟩⟩
private def peopleId : TableId declarations.modelSchema.catalog simBox := ⟨⟨0, by decide⟩⟩
private def Γ : TermContext := ⟨declarations, simBox, peopleId⟩
private def tableScope : RowScope Γ.model Γ.current Γ.inputs := .table Γ.current

private def synthesisOk (raw : IR.Expr) : Bool :=
  match buildSynthExpr Γ tableScope raw with
  | .ok checked => checked.expr.erase == raw
  | .error _ => false

private def checkingOk (raw : IR.Expr) (sort : ScalarSort Γ.model.catalog)
    (origin : SortOrigin Γ tableScope sort) : Bool :=
  match buildExpectedExpr Γ tableScope raw sort origin with
  | .ok checked => checked.erase == raw
  | .error _ => false

/-! Every raw expression and aggregate constructor, exact scientific spelling,
and mixed numeric coercion through the authoritative checker. -/
private def expressionCorpus : List IR.Expr := [
  TransitionRaw.real (sci 170 (-2)),
  TransitionRaw.int 4,
  TransitionRaw.bool true,
  TransitionRaw.parameter "gain",
  TransitionRaw.selfAttribute "rate",
  TransitionRaw.add (.int 1) (.real (sci 20 (-1))),
  TransitionRaw.sub (.selfAttr "count") (.int 1),
  TransitionRaw.mul (.real (sci 3)) (.int 2),
  TransitionRaw.div (.int 3) (.int 2),
  TransitionRaw.eq (.selfAttr "status") (.enum "open"),
  TransitionRaw.ne (.selfAttr "count") (.int 0),
  TransitionRaw.lt (.int 1) (.real (sci 2)),
  TransitionRaw.le (.int 1) (.int 2),
  TransitionRaw.gt (.real (sci 2)) (.int 1),
  TransitionRaw.ge (.int 2) (.int 2),
  TransitionRaw.and (.bool true) (.bool false),
  TransitionRaw.or (.bool true) (.bool false),
  TransitionRaw.not (.bool false),
  TransitionRaw.enumIs "status" "open",
  TransitionRaw.input "flow" (TransitionRaw.aggregate TransitionRaw.count),
  TransitionRaw.relatedAggregate
    (TransitionRaw.sum (.selfAttr "amount")) "Event" "region" "region" (.bool true)
]

#guard expressionCorpus.length == 21
#guard expressionCorpus.all synthesisOk
#guard synthesisOk (TransitionRaw.eq (.int 1) (.real (sci 10 (-1))))
#guard synthesisOk (TransitionRaw.input "flow"
  (TransitionRaw.aggregate (TransitionRaw.sum (.selfAttr "amount"))
    (some (.gt (.selfAttr "amount") (.real (sci 0))))))

private def statusAttr : AttributeId (Γ.model.schemaFor Γ.current) := ⟨⟨2, by decide⟩⟩
private def statusSchema : EnumSchema := ⟨["open", "closed"], by decide, by decide⟩
private theorem statusShape :
    ((Γ.model.schemaFor Γ.current).attr statusAttr).shape = .enum statusSchema := by rfl

#guard checkingOk (TransitionRaw.enum "open")
  (.enum (Γ.model.schemaFor Γ.current) statusAttr statusSchema statusShape)
  (.enum (Γ := Γ) (scope := tableScope) statusAttr statusSchema statusShape)

/-! Exact effect, claim, transition and ordering forms. -/
private def positiveTransition : IR.Transition :=
  TransitionRaw.transition "step" "People"
    (.and
      (.eq (.selfAttr "status") (.enum "open"))
      (.not (.or (.bool false) (.enumIs "status" "closed"))))
    (.real (sci (-250) (-2)))
    [ TransitionRaw.setAttribute "count" (.add (.selfAttr "count") (.int 1))
    , TransitionRaw.setAttribute "rate"
        (.div (.add (.selfAttr "rate") (.int 1)) (.int 2))
    , TransitionRaw.setAttribute "region" (.selfAttr "region") ]
    [ TransitionRaw.raceClaim (.selfAttr "region")
    , TransitionRaw.keyClaim (.selfAttr "manager") (.selfAttr "status")
    , TransitionRaw.keyClaim (.selfAttr "backup") (.selfAttr "rate")
    , TransitionRaw.keyClaim (.selfAttr "peer") (.selfAttr "count") ]

private def emptyTransition : IR.Transition :=
  TransitionRaw.transition "empty" "People" (.bool true) (.real (sci 1)) [] []

#guard (buildEffect Γ (TransitionRaw.setAttribute "count" (.int 3))).isOk
#guard (buildClaim Γ (TransitionRaw.raceClaim (.selfAttr "region"))).isOk
#guard (buildClaim Γ
  (TransitionRaw.keyClaim (.selfAttr "manager") (.selfAttr "rate"))).isOk
#guard (buildClaim Γ
  (TransitionRaw.keyClaim (.selfAttr "backup") (.selfAttr "count"))).isOk
#guard (buildClaim Γ
  (TransitionRaw.keyClaim (.selfAttr "peer") (.selfAttr "status"))).isOk
#guard (buildTransition Γ positiveTransition).isOk
#guard (buildTransition Γ emptyTransition).isOk
#guard positiveTransition.hazard == .real (sci (-250) (-2))
private def positiveEffectDestinations : List String :=
  positiveTransition.effects.map fun effect => match effect with
    | .setAttr destination _ => destination
#guard positiveEffectDestinations == ["count", "rate", "region"]
#guard positiveTransition.contests.map IR.ResourceClaim.resource ==
  [.selfAttr "region", .selfAttr "manager", .selfAttr "backup", .selfAttr "peer"]

private def surfaceRace : Bool :=
  (buildSurfaceTransition Γ
    { positiveTransition with contests := [TransitionRaw.raceClaim (.selfAttr "region")] }).isOk
#guard surfaceRace

private def surfaceKeyRejected : Bool :=
  match buildSurfaceTransition Γ positiveTransition 4 7 with
  | .error (.unsupportedSurfaceKeyOrdering path) =>
      path == [.box 4, .transition 7, .claim 1, .orderingKey]
  | _ => false
#guard surfaceKeyRejected

private def surfaceTermErrorDelegated : Bool :=
  match buildSurfaceTransition Γ { emptyTransition with guard := .int 1 } 4 8 with
  | .error (.term error) =>
      error.category == .expectedBool && error.path == [.guard]
  | _ => false
#guard surfaceTermErrorDelegated

/-! Dependent source-ordinal overlay with two boxes and transition counts [2, 0]. -/
private def coreShell : CoreModelShell :=
  { name := "transition-overlay"
    dt := sci 1 (-1)
    params := contextRaw.params
    boxes := [
      { name := "Sim", tables := [peopleTable, eventTable, regionTable] },
      { name := "Empty", tables := [] }
    ] }

private def secondTransition : IR.Transition :=
  { emptyTransition with name := "idle" }

private def overlaySpec : TransitionOverlaySpec :=
  { core := coreShell
    transitions := fun ordinal =>
      if ordinal.val = 0 then [positiveTransition, secondTransition] else [] }

private def overlayRaw : IR.Model := overlaySpec.toRaw

#guard overlayRaw.name == coreShell.name
#guard overlayRaw.dt == coreShell.dt
#guard overlayRaw.params == coreShell.params
#guard overlayRaw.boxes.map IR.Box.name == ["Sim", "Empty"]
#guard overlayRaw.boxes.map IR.Box.tables ==
  [[peopleTable, eventTable, regionTable], []]
#guard overlayRaw.boxes.map IR.Box.transitions ==
  [[positiveTransition, secondTransition], []]
#guard overlayRaw.boxes.all fun box =>
  box.inputs.isEmpty && box.outputs.isEmpty && box.views.isEmpty && box.groupedViews.isEmpty
#guard overlayRaw.wires.isEmpty && overlayRaw.summaries.isEmpty
#guard (buildTransitionOverlay overlaySpec).isOk
#guard (checkDeclarations overlayRaw).isOk
#guard (checkModel overlayRaw).isOk

private def overlayCheckedErasesExactly : Bool :=
  match buildTransitionOverlay overlaySpec with
  | .ok checked => checked.erase == overlayRaw
  | .error _ => false
#guard overlayCheckedErasesExactly

example {checked} (success : buildTransitionOverlay overlaySpec = .ok checked) :
    ModelWellFormed overlaySpec.toRaw ∧
      checkModel overlaySpec.toRaw = .ok checked ∧ checked.erase = overlaySpec.toRaw :=
  buildTransitionOverlay_sound success

#check TransitionTyped.effect_erase_exact
#check TransitionTyped.raceClaim_erase_exact
#check TransitionTyped.keyClaim_erase_exact
#check buildSurfaceTransition_unsupported_iff
#check buildSurfaceTransition_failure_iff
#check buildSurfaceTransition_success_race_only
#check buildSurfaceTransition_race_only
#check buildTransition_error_iff
#check buildTransitionOverlay_core_error_iff
#check buildTransitionOverlay_declaration_error_iff
#check buildTransitionOverlay_model_check_error_iff
#check buildTransitionOverlay_complete
#check buildTransitionOverlay_failure_iff
#check buildTransitionOverlay_model_acceptance_and_erasure
#check TransitionOverlaySpec.rawBoxes_get
#check TransitionOverlaySpec.toRaw_box_names_exact
#check TransitionOverlaySpec.toRaw_box_tables_exact
#check TransitionOverlaySpec.toRaw_slice_boundary

/-! Stable nested failures. -/
private def expectTermError (result : Except TransitionBuilderError α)
    (category : TermCheckErrorCategory) (path : List ModelCheckPathSegment) : Bool :=
  match result with
  | .error (.term error) => error.category == category && error.path == path
  | _ => false

#guard expectTermError (buildSynthExpr Γ tableScope (.enum "open"))
  .cannotInferEnumOwner []
#guard expectTermError (buildSynthExpr Γ tableScope (.param "missing"))
  .unknownParameter []
#guard expectTermError (buildSynthExpr Γ tableScope (.selfAttr "missing"))
  .unknownAttribute []
#guard expectTermError (buildSynthExpr Γ tableScope
  (.input "missing" (.mk .count none))) .unknownInput [.inputPort]
#guard expectTermError (buildSynthExpr Γ tableScope
  (.input "flow" (.mk (.sum (.input "flow" (.mk .count none))) none)))
  .nestedInputAggregate [.aggregate, .aggregateValue]
#guard expectTermError (buildSynthExpr Γ tableScope
  (.input "flow" (.mk .count
    (some (.agg .count "Event" "region" "region" (.bool true))))))
  .nestedInputAggregate [.aggregate, .aggregateFilter]
#guard expectTermError (buildSynthExpr Γ tableScope
  (.agg .count "missing" "region" "region" (.bool true)))
  .unknownTable [.tableTarget]
#guard expectTermError (buildSynthExpr Γ tableScope
  (.agg .count "Event" "missing" "region" (.bool true)))
  .unknownJoinAttribute [.joinForeignAttribute]
#guard expectTermError (buildSynthExpr Γ tableScope
  (.agg .count "Event" "region" "missing" (.bool true)))
  .unknownJoinAttribute [.joinSelfAttribute]
#guard expectTermError (buildSynthExpr Γ tableScope
  (.agg .count "Event" "other" "manager" (.bool true)))
  .incompatibleJoinTargets []
#guard expectTermError (buildSynthExpr Γ tableScope (.add (.bool true) (.int 1)))
  .expectedNumeric []
#guard expectTermError (buildSynthExpr Γ tableScope (.eq (.bool true) (.int 1)))
  .incompatibleEquality []
#guard expectTermError (buildSynthExpr Γ tableScope (.enumIs "count" "open"))
  .sortMismatch []
#guard expectTermError (buildExpectedExpr Γ tableScope (.enum "missing")
    (.enum (Γ.model.schemaFor Γ.current) statusAttr statusSchema statusShape)
    (.enum (Γ := Γ) (scope := tableScope) statusAttr statusSchema statusShape))
  .unknownEnumVariant []
#guard expectTermError (buildExpectedExpr Γ tableScope (.int 1) .bool .bool)
  .expectedBool []
#guard expectTermError (buildExpectedExpr Γ tableScope (.int 1) .real .real)
  .expectedReal []
#guard expectTermError (buildEffect Γ (.setAttr "missing" (.int 1)))
  .unknownAttribute [.destination]
#guard expectTermError (buildEffect Γ (.setAttr "rate" (.int 1)))
  .expectedReal [.value]
#guard expectTermError (buildClaim Γ
    { resource := .int 1, ordering := .raceTime })
  .expectedReference [.resource]
#guard expectTermError (buildClaim Γ
    { resource := .selfAttr "region", ordering := .key (.bool true) })
  .expectedOrderable [.orderingKey]
#guard expectTermError (buildClaim Γ
    { resource := .selfAttr "region", ordering := .key (.selfAttr "manager") })
  .expectedOrderable [.orderingKey]

private def duplicateClaimTransition : IR.Transition :=
  { positiveTransition with
    effects := []
    contests := [
      TransitionRaw.raceClaim (.selfAttr "region"),
      TransitionRaw.keyClaim (.selfAttr "region") (.selfAttr "count")
    ] }
#guard expectTermError (buildTransition Γ duplicateClaimTransition)
  .duplicateResourceClaim [.contests, .claim 1, .resource]

private def unclaimedWriteTransition : IR.Transition :=
  { positiveTransition with
    effects := [.setAttr "region" (.selfAttr "region")]
    contests := [] }
#guard expectTermError (buildTransition Γ unclaimedWriteTransition)
  .unclaimedRefWrite [.effects, .effect 0, .value]

private def expectDeclarationError (spec : TransitionOverlaySpec)
    (category : CheckErrorCategory) (path : List CheckPathSegment) : Bool :=
  match buildTransitionOverlay spec with
  | .error (.declaration error) => error.category == category && error.path == path
  | _ => false

private def duplicateTransitionSpec : TransitionOverlaySpec :=
  { core := coreShell
    transitions := fun ordinal =>
      if ordinal.val = 0 then [positiveTransition, { positiveTransition with name := "step" }]
      else [] }

#guard expectDeclarationError duplicateTransitionSpec .duplicateName
  [.boxes, .box 0, .transitions, .transition 1, .name]

private def unresolvedTargetSpec : TransitionOverlaySpec :=
  { core := coreShell
    transitions := fun ordinal =>
      if ordinal.val = 0 then [{ positiveTransition with table := "Missing" }] else [] }

#guard expectDeclarationError unresolvedTargetSpec .unresolvedTransitionTable
  [.boxes, .box 0, .transitions, .transition 0, .tableTarget]

private def badGuardSpec : TransitionOverlaySpec :=
  { core := coreShell
    transitions := fun ordinal =>
      if ordinal.val = 0 then [{ positiveTransition with guard := .int 1 }] else [] }

private def modelTermErrorPreserved : Bool :=
  match buildTransitionOverlay badGuardSpec with
  | .error (.modelCheck (.model error)) =>
      error.category == .term .expectedBool &&
        error.path == [.model, .box 0, .transition 0, .guard]
  | _ => false
#guard modelTermErrorPreserved

private def badCore : CoreModelShell := { coreShell with dt := sci 0 (-99) }
private def badCoreSpec : TransitionOverlaySpec :=
  { core := badCore, transitions := fun _ => [] }

private def coreErrorPreserved : Bool :=
  match buildTransitionOverlay badCoreSpec with
  | .error (.core error) =>
      error.category == .nonpositiveDt && error.path == [.modelMetadata, .dt]
  | _ => false
#guard coreErrorPreserved

end Sembla.Frontend.Builders.TransitionTests
