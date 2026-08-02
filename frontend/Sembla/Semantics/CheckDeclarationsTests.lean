import Sembla.Semantics.CheckDeclarations

/-!
Executable and theorem-facing fixtures for PRD 0005.  Negative fixtures contain
one declaration defect and compare structured category/path values rather than
prose.  The positive fixture includes every prior family, priorless parameters,
a zero-sized table, a zero-table box, separate same-named input/output ports,
and forward plus mutual table references.
-/
namespace Sembla.Semantics.CheckDeclarationsTests

open Sembla
open Sembla.Semantics

private def realParam (name : String) (prior : Option IR.Prior := none) : IR.ParamDecl :=
  { name := name, ty := .real, default := .real ⟨1, -1⟩, prior := prior }

private def intParam (name : String) : IR.ParamDecl :=
  { name := name, ty := .int, default := .int 3, prior := none }

private def attr (name : String) (ty : IR.AttrType) : IR.Attr := ⟨name, ty⟩

private def table (name : String) (attrs : List IR.Attr := [])
    (sizeHint : Nat := 1) : IR.Table :=
  { name := name, sizeHint := sizeHint, attrs := attrs }

private def transition (name table : String) : IR.Transition :=
  { name := name
    table := table
    guard := .bool true
    hazard := .real ⟨1, 0⟩
    effects := []
    contests := [] }

private def input (name : String) (schema : List IR.Attr := []) : IR.PortDecl :=
  { name := name, schema := schema }

private def output (name : String) (schema : List IR.Attr := []) : IR.OutputDecl :=
  { name := name, schema := schema, builder := .perTable "deferred" [] }

private def view (name : String) : IR.ViewDecl :=
  { name := name, table := "deferred", filter := none, value := none, reduce := .count }

private def groupedView (name : String) : IR.GroupedViewDecl :=
  { name := name, table := "deferred", filter := none, keys := [] }

private def summary (name : String) : IR.SummaryDecl :=
  { name := name, box := "deferred", view := "deferred", reduce := .last }

private def box (name : String) (tables : List IR.Table := [])
    (transitions : List IR.Transition := []) (inputs : List IR.PortDecl := [])
    (outputs : List IR.OutputDecl := []) (views : List IR.ViewDecl := [])
    (groupedViews : List IR.GroupedViewDecl := []) : IR.Box :=
  { name := name
    tables := tables
    transitions := transitions
    inputs := inputs
    outputs := outputs
    views := views
    groupedViews := groupedViews }

private def model (params : List IR.ParamDecl := []) (boxes : List IR.Box := [])
    (summaries : List IR.SummaryDecl := []) (dt : IR.Scientific := ⟨1, -1⟩) : IR.Model :=
  { name := "fixture"
    dt := dt
    params := params
    boxes := boxes
    wires := []
    summaries := summaries }

private def positiveModel : IR.Model :=
  model
    [ realParam "plain-real"
    , intParam "plain-int"
    , realParam "normal" (some { family := .normal, args := [⟨0, 0⟩, ⟨2, 0⟩] })
    , realParam "log-normal"
        (some { family := .logNormal, args := [⟨0, 0⟩, ⟨2, 0⟩] })
    , realParam "uniform"
        (some { family := .uniform, args := [⟨10, -1⟩, ⟨2, 0⟩] }) ]
    [ box "network"
        [ table "A" [attr "to-b" (.ref "B")] 0
        , table "B" [attr "to-a" (.ref "A"), attr "state" (.enum ["s", "i"])] ]
        [transition "step" "A"]
        [input "shared" [attr "a" (.ref "A")]]
        [output "shared" [attr "b" (.ref "B")]]
        [view "ordinary"]
        [groupedView "grouped"]
    , box "empty" [] [] [input "same"] [output "same"] ]
    [summary "total"]

/-- A genuinely empty box has no tables, transitions, ports, views or grouped views. -/
private def emptyBoxModel : IR.Model :=
  model (boxes := [box "empty"])

private theorem emptyBoxWellFormed : DeclarationsWellFormed emptyBoxModel := by decide

private def emptyBoxListsAreEmpty : Bool :=
  match emptyBoxModel.boxes with
  | [candidate] =>
      candidate.tables.isEmpty && candidate.transitions.isEmpty &&
        candidate.inputs.isEmpty && candidate.outputs.isEmpty &&
        candidate.views.isEmpty && candidate.groupedViews.isEmpty
  | _ => false

#guard emptyBoxListsAreEmpty
#guard (checkDeclarations emptyBoxModel).isOk

example : ∃ ctx, checkDeclarations emptyBoxModel = .ok ctx :=
  checkDeclarations_complete emptyBoxWellFormed

private theorem positiveWellFormed : DeclarationsWellFormed positiveModel := by
  norm_num [positiveModel, model, box, table, transition, input, output, attr, view,
    groupedView, summary, realParam, intParam, DeclarationsWellFormed,
    BoxDeclarationsWellFormed, SchemaWellFormed, NamesUnique,
    ParameterWellFormed, parameterWellFormedBool, PriorWellFormed,
    priorWellFormedBool, AttributeTypeWellFormed, attributeTypeWellFormedBool,
    scientificLt, scientificRat]
  decide

example : ∃ ctx, checkDeclarations positiveModel = .ok ctx :=
  checkDeclarations_complete positiveWellFormed
#guard scientificPositive positiveModel.dt
#guard scientificLt ⟨10, -1⟩ ⟨2, 0⟩
#guard !(scientificLt ⟨2, 0⟩ ⟨2, 0⟩)

private def positiveContext : DeclarationContext :=
  ⟨positiveModel, positiveWellFormed⟩

private def networkBox : BoxId positiveContext.modelSchema.catalog :=
  ⟨⟨0, by decide⟩⟩

private def emptyBox : BoxId positiveContext.modelSchema.catalog :=
  ⟨⟨1, by decide⟩⟩

private def networkA : TableId positiveContext.modelSchema.catalog networkBox :=
  ⟨⟨0, by decide⟩⟩

private def networkTransition : Fin (positiveContext.transitions networkBox).length :=
  ⟨0, by decide⟩

private def networkInputSchema : BoxPortSchema positiveContext.modelSchema.catalog networkBox :=
  (positiveContext.inputPortSchemas networkBox).get ⟨0, by decide⟩

private def networkOutputSchema : BoxPortSchema positiveContext.modelSchema.catalog networkBox :=
  (positiveContext.outputPortSchemas networkBox).get ⟨0, by decide⟩

#guard positiveContext.modelSchema.catalog.tableName
  ⟨networkBox, positiveContext.resolveTransitionTarget networkBox networkTransition⟩ == "A"
#guard (networkInputSchema.instantiate networkA).eraseAttributes == networkInputSchema.source
#guard (networkOutputSchema.instantiate networkA).eraseAttributes == networkOutputSchema.source
#guard (positiveContext.inputPortSchemas emptyBox).length == 1
#guard (positiveContext.outputPortSchemas emptyBox).length == 1
#guard positiveContext.modelSchema.eraseParameters == positiveModel.params
#guard positiveContext.modelSchema.eraseTables networkBox ==
  (sourceBox positiveContext.source positiveContext.wellFormed networkBox).tables
#guard eraseDeclarations positiveContext == projectDeclarations positiveModel

example :
    positiveContext.modelSchema.catalog.tableName
      ⟨networkBox, positiveContext.resolveTransitionTarget networkBox networkTransition⟩ =
      (positiveContext.transitionAt networkBox networkTransition).table :=
  positiveContext.checkedTransition_target_name networkBox networkTransition

example {raw : IR.Model} {ctx : DeclarationContext}
    (checked : checkDeclarations raw = .ok ctx) :
    DeclarationsWellFormed raw ∧ eraseDeclarations ctx = projectDeclarations raw :=
  checkDeclarations_sound checked

example {raw : IR.Model} (wellFormed : DeclarationsWellFormed raw) :
    ∃ ctx, checkDeclarations raw = .ok ctx :=
  checkDeclarations_complete wellFormed

private def hasError (raw : IR.Model) (category : CheckErrorCategory)
    (path : List CheckPathSegment) : Bool :=
  firstDeclarationError raw == some { category := category, path := path }

/-! Global metadata and namespace failures. -/
#guard hasError (model (dt := ⟨0, 0⟩)) .nonpositiveDt [.dt]
#guard hasError (model (dt := ⟨-1, 0⟩)) .nonpositiveDt [.dt]
#guard hasError (model [realParam "p", realParam "p"]) .duplicateName
  [.parameters, .parameter 1, .name]
#guard hasError (model (boxes := [box "b", box "b"])) .duplicateName
  [.boxes, .box 1, .name]
#guard hasError (model (summaries := [summary "s", summary "s"])) .duplicateName
  [.summaries, .summary 1, .name]

/-! Parameter failures. -/
private def badDefault : IR.ParamDecl :=
  { name := "p", ty := .real, default := .int 1, prior := none }
private def badIntPrior : IR.ParamDecl :=
  { name := "p", ty := .int, default := .int 1,
    prior := some { family := .normal, args := [⟨0, 0⟩, ⟨1, 0⟩] } }

#guard hasError (model [badDefault]) .parameterDefaultMismatch
  [.parameters, .parameter 0, .default]
#guard hasError (model [badIntPrior]) .integerPrior
  [.parameters, .parameter 0, .prior]
#guard hasError
  (model [realParam "p" (some { family := .normal, args := [⟨0, 0⟩] })])
  .priorArity [.parameters, .parameter 0, .prior]
#guard hasError
  (model [realParam "p" (some { family := .uniform, args := [⟨1, 0⟩, ⟨1, 0⟩] })])
  .unorderedUniform [.parameters, .parameter 0, .prior, .argument 1]
#guard hasError
  (model [realParam "p" (some { family := .uniform, args := [⟨2, 0⟩, ⟨1, 0⟩] })])
  .unorderedUniform [.parameters, .parameter 0, .prior, .argument 1]

/-! Every local namespace. -/
#guard hasError (model (boxes := [box "b" [table "t", table "t"]])) .duplicateName
  [.boxes, .box 0, .tables, .table 1, .name]
#guard hasError
  (model (boxes := [box "b" [table "t"] [transition "x" "t", transition "x" "t"]]))
  .duplicateName [.boxes, .box 0, .transitions, .transition 1, .name]
#guard hasError (model (boxes := [box "b" [] [] [input "p", input "p"]]))
  .duplicateName [.boxes, .box 0, .inputs, .input 1, .name]
#guard hasError (model (boxes := [box "b" [] [] [] [output "p", output "p"]]))
  .duplicateName [.boxes, .box 0, .outputs, .output 1, .name]
#guard hasError (model (boxes := [box "b" [] [] [] [] [view "v", view "v"]]))
  .duplicateName [.boxes, .box 0, .views, .view 1, .name]
#guard hasError
  (model (boxes := [box "b" [] [] [] [] [view "v"] [groupedView "v"]]))
  .duplicateName [.boxes, .box 0, .groupedViews, .groupedView 0, .name]

/-! Independent table/input/output schema namespaces. -/
#guard hasError
  (model (boxes := [box "b" [table "t" [attr "a" .real, attr "a" .int]]]))
  .duplicateName [.boxes, .box 0, .tables, .table 0, .schema, .attribute 1, .name]
#guard hasError
  (model (boxes := [box "b" [] [] [input "p" [attr "a" .real, attr "a" .int]]]))
  .duplicateName [.boxes, .box 0, .inputs, .input 0, .schema, .attribute 1, .name]
#guard hasError
  (model (boxes := [box "b" [] [] []
    [output "p" [attr "a" .real, attr "a" .int]]]))
  .duplicateName [.boxes, .box 0, .outputs, .output 0, .schema, .attribute 1, .name]

/-! Enum and resolved-reference failures for every owned schema family. -/
#guard hasError
  (model (boxes := [box "b" [table "t" [attr "e" (.enum [])]]]))
  .emptyEnum [.boxes, .box 0, .tables, .table 0, .schema, .attribute 0]
#guard hasError
  (model (boxes := [box "b" [table "t" [attr "e" (.enum ["x", "x"])]]]))
  .duplicateEnumVariant
  [.boxes, .box 0, .tables, .table 0, .schema, .attribute 0, .enumVariant 1]
#guard hasError
  (model (boxes := [box "b" [table "t" [attr "r" (.ref "missing")]]]))
  .unresolvedTableReference
  [.boxes, .box 0, .tables, .table 0, .schema, .attribute 0, .tableTarget]
#guard hasError
  (model (boxes := [box "b" [table "t"] []
    [input "p" [attr "r" (.ref "missing")]]]))
  .unresolvedTableReference
  [.boxes, .box 0, .inputs, .input 0, .schema, .attribute 0, .tableTarget]
#guard hasError
  (model (boxes := [box "b" [table "t"] [] []
    [output "p" [attr "r" (.ref "missing")]]]))
  .unresolvedTableReference
  [.boxes, .box 0, .outputs, .output 0, .schema, .attribute 0, .tableTarget]
#guard hasError
  (model (boxes := [box "b" [table "t"] [transition "x" "missing"]]))
  .unresolvedTransitionTable
  [.boxes, .box 0, .transitions, .transition 0, .tableTarget]

/-- Separate input/output namespaces accept the same spelling. -/
example : DeclarationsWellFormed
    (model (boxes := [box "b" [] [] [input "same"] [output "same"]])) := by decide

end Sembla.Semantics.CheckDeclarationsTests
