import Sembla.Semantics.CheckTerms

/-!
Canonical model-local checking and checked-model assembly for the V1 IR.

This module consumes the declaration context and intrinsic term checker. It
reconstructs owned model payloads from checked components; the raw source is
used only for declaration spelling and the explicitly deferred wire list.
-/
namespace Sembla.Semantics

open Sembla

set_option maxHeartbeats 1000000

/-! ## Model diagnostics -/

inductive ModelTermErrorCategory where
  | term (category : TermCheckErrorCategory)
  | unresolvedOutputTable
  | duplicateOutputField
  | outputFieldCountMismatch
  | outputFieldNameMismatch
  | outputFieldSortMismatch
  | unresolvedViewTable
  | invalidViewReducerShape
  | invalidGroupedKeyCount
  | unresolvedGroupedKey
  | invalidGroupedKeySort
  | missingGroupedBand
  | unexpectedGroupedBand
  | nonpositiveGroupedBand
  | aggregateInGroupedFilter
  | unresolvedSummaryBox
  | unresolvedSummaryView
  deriving Repr, BEq, DecidableEq

structure ModelTermError where
  category : ModelTermErrorCategory
  path : List ModelCheckPathSegment
  deriving Repr, BEq, DecidableEq

inductive ModelCheckError where
  | declaration (error : CheckError)
  | model (error : ModelTermError)
  deriving Repr, BEq, DecidableEq

private def modelError (category : ModelTermErrorCategory)
    (path : List ModelCheckPathSegment) : Except ModelTermError α :=
  .error ⟨category, path⟩

private def liftTerm {α : Type} (result : Except TermCheckError α) :
    Except ModelTermError α :=
  match result with
  | .ok value => .ok value
  | .error error => .error ⟨.term error.category, error.path⟩

/-! ## Source-ordered checked declarations -/

structure CheckedTransition (Γ : TermContext) where
  sourceOrdinal : Nat
  name : String
  terms : TransitionTerms Γ.model Γ.current Γ.inputs

structure CheckedTransitionPack (ctx : DeclarationContext)
    (box : BoxId ctx.modelSchema.catalog) where
  target : TableId ctx.modelSchema.catalog box
  checked : CheckedTransition ⟨ctx, box, target⟩

structure CheckedOutputField (Γ : TermContext)
    (schema : TableSchema Γ.model.catalog Γ.current) where
  sourceOrdinal : Nat
  name : String
  schemaAttribute : AttributeId schema
  op : AggOp Γ.model Γ.current Γ.inputs (.table Γ.current)
    (schema.attributeSort schemaAttribute)
  filter : Option (Expr Γ.model Γ.current Γ.inputs (.table Γ.current) .bool)

structure CheckedOutputDecl (ctx : DeclarationContext)
    (box : BoxId ctx.modelSchema.catalog) where
  sourceOrdinal : Nat
  name : String
  target : TableId ctx.modelSchema.catalog box
  portSchema : BoxPortSchema ctx.modelSchema.catalog box
  fields : List (CheckedOutputField ⟨ctx, box, target⟩
    (portSchema.instantiate target))

inductive CheckedViewValue (Γ : TermContext) where
  | count
  | sumInt (value : Term Γ.model Γ.current Γ.inputs .int)
  | sumReal (value : Term Γ.model Γ.current Γ.inputs .real)
  | minInt (value : Term Γ.model Γ.current Γ.inputs .int)
  | minReal (value : Term Γ.model Γ.current Γ.inputs .real)
  | maxInt (value : Term Γ.model Γ.current Γ.inputs .int)
  | maxReal (value : Term Γ.model Γ.current Γ.inputs .real)

structure CheckedViewDecl (ctx : DeclarationContext)
    (box : BoxId ctx.modelSchema.catalog) where
  sourceOrdinal : Nat
  name : String
  target : TableId ctx.modelSchema.catalog box
  filter : Option (Term ctx.modelSchema ⟨box, target⟩
    (TermContext.inputs ⟨ctx, box, target⟩) .bool)
  value : CheckedViewValue ⟨ctx, box, target⟩

/-- The frozen V1 grouped-key band discipline, stated independently of the
checker and indexed by the resolved attribute. -/
def GroupBandValid {catalog : SchemaUniverse} {owner : TableTarget catalog}
    (schema : TableSchema catalog owner) (attr : AttributeId schema)
    (band : Option Nat) : Prop :=
  match (schema.attr attr).shape, band with
  | .int, some width => 0 < width
  | .enum _, none => True
  | .ref _, none => True
  | _, _ => False

structure CheckedGroupKey (Γ : TermContext) where
  sourceOrdinal : Nat
  attr : AttributeId (Γ.model.schemaFor Γ.current)
  band : Option Nat
  valid : GroupBandValid (Γ.model.schemaFor Γ.current) attr band

structure CheckedGroupedViewDecl (ctx : DeclarationContext)
    (box : BoxId ctx.modelSchema.catalog) where
  sourceOrdinal : Nat
  name : String
  target : TableId ctx.modelSchema.catalog box
  keys : List (CheckedGroupKey ⟨ctx, box, target⟩)
  keyCount : 1 ≤ keys.length ∧ keys.length ≤ 4
  filter : Option (Term ctx.modelSchema ⟨box, target⟩
    (TermContext.inputs ⟨ctx, box, target⟩) .bool)

structure CheckedSummaryDecl (ctx : DeclarationContext) where
  sourceOrdinal : Nat
  name : String
  box : BoxId ctx.modelSchema.catalog
  viewOrdinal : Fin (ctx.views box).length
  reduce : IR.SummaryReduce

structure CheckedBox (ctx : DeclarationContext)
    (box : BoxId ctx.modelSchema.catalog) where
  sourceOrdinal : Nat
  transitions : List (CheckedTransitionPack ctx box)
  outputs : List (CheckedOutputDecl ctx box)
  views : List (CheckedViewDecl ctx box)
  groupedViews : List (CheckedGroupedViewDecl ctx box)

structure CheckedBoxPack (ctx : DeclarationContext) where
  box : BoxId ctx.modelSchema.catalog
  checked : CheckedBox ctx box

namespace Checked

structure Model where
  declarations : DeclarationContext
  boxes : List (CheckedBoxPack declarations)
  summaries : List (CheckedSummaryDecl declarations)
  wires : List IR.Wire

end Checked

/-! ## Checked erasure -/

namespace CheckedTransition

def erase {Γ : TermContext} (checked : CheckedTransition Γ) : IR.Transition :=
  { name := checked.name
    table := Γ.model.catalog.tableName Γ.current
    guard := checked.terms.eraseGuard
    hazard := checked.terms.eraseHazard
    effects := checked.terms.eraseEffects
    contests := checked.terms.eraseClaims }

end CheckedTransition

namespace CheckedTransitionPack

def erase {ctx : DeclarationContext} {box : BoxId ctx.modelSchema.catalog}
    (checked : CheckedTransitionPack ctx box) : IR.Transition :=
  checked.checked.erase

end CheckedTransitionPack

namespace CheckedOutputField

def erase {Γ : TermContext} {schema : TableSchema Γ.model.catalog Γ.current}
    (checked : CheckedOutputField Γ schema) : IR.OutputField :=
  { name := checked.name
    op := checked.op.erase
    filter := checked.filter.map Expr.erase }

end CheckedOutputField

namespace CheckedOutputDecl

def erase {ctx : DeclarationContext} {box : BoxId ctx.modelSchema.catalog}
    (checked : CheckedOutputDecl ctx box) : IR.OutputDecl :=
  { name := checked.name
    schema := checked.portSchema.source
    builder := .perTable (ctx.modelSchema.catalog.tableName ⟨box, checked.target⟩)
      (checked.fields.map CheckedOutputField.erase) }

end CheckedOutputDecl

namespace CheckedViewValue

def erase {Γ : TermContext} : CheckedViewValue Γ → IR.ViewReduce × Option IR.Expr
  | .count => (.count, none)
  | .sumInt value => (.sum, some value.erase)
  | .sumReal value => (.sum, some value.erase)
  | .minInt value => (.min, some value.erase)
  | .minReal value => (.min, some value.erase)
  | .maxInt value => (.max, some value.erase)
  | .maxReal value => (.max, some value.erase)

end CheckedViewValue

namespace CheckedViewDecl

def erase {ctx : DeclarationContext} {box : BoxId ctx.modelSchema.catalog}
    (checked : CheckedViewDecl ctx box) : IR.ViewDecl :=
  let erased := checked.value.erase
  { name := checked.name
    table := ctx.modelSchema.catalog.tableName ⟨box, checked.target⟩
    filter := checked.filter.map Expr.erase
    value := erased.2
    reduce := erased.1 }

end CheckedViewDecl

namespace CheckedGroupKey

def erase {Γ : TermContext} (checked : CheckedGroupKey Γ) : IR.GroupKey :=
  { attr := (Γ.model.schemaFor Γ.current).attributeName checked.attr
    bandWidth := checked.band }

end CheckedGroupKey

namespace CheckedGroupedViewDecl

def erase {ctx : DeclarationContext} {box : BoxId ctx.modelSchema.catalog}
    (checked : CheckedGroupedViewDecl ctx box) : IR.GroupedViewDecl :=
  { name := checked.name
    table := ctx.modelSchema.catalog.tableName ⟨box, checked.target⟩
    filter := checked.filter.map Expr.erase
    keys := checked.keys.map CheckedGroupKey.erase }

end CheckedGroupedViewDecl

namespace CheckedSummaryDecl

def erase {ctx : DeclarationContext} (checked : CheckedSummaryDecl ctx) : IR.SummaryDecl :=
  { name := checked.name
    box := ctx.modelSchema.catalog.boxName checked.box
    view := (ctx.views checked.box).get checked.viewOrdinal |>.name
    reduce := checked.reduce }

end CheckedSummaryDecl

namespace CheckedBox

def erase {ctx : DeclarationContext} {box : BoxId ctx.modelSchema.catalog}
    (checked : CheckedBox ctx box) : IR.Box :=
  let source := sourceBox ctx.source ctx.wellFormed box
  { name := source.name
    tables := ctx.modelSchema.eraseTables box
    transitions := checked.transitions.map CheckedTransitionPack.erase
    inputs := source.inputs
    outputs := checked.outputs.map CheckedOutputDecl.erase
    views := checked.views.map CheckedViewDecl.erase
    groupedViews := checked.groupedViews.map CheckedGroupedViewDecl.erase }

end CheckedBox

namespace CheckedBoxPack

def erase {ctx : DeclarationContext} (checked : CheckedBoxPack ctx) : IR.Box :=
  checked.checked.erase

end CheckedBoxPack

namespace Checked.Model

/-- Reconstruct every owned payload from checked components. This is
intentionally not `declarations.source`. -/
def erase (checked : Checked.Model) : IR.Model :=
  { name := checked.declarations.source.name
    dt := checked.declarations.source.dt
    params := checked.declarations.modelSchema.eraseParameters
    boxes := checked.boxes.map CheckedBoxPack.erase
    wires := checked.wires
    summaries := checked.summaries.map CheckedSummaryDecl.erase }

end Checked.Model

/-! ## Executable declaration checking -/

private def checkOptionalBool (Γ : TermContext) (raw : Option IR.Expr)
    (path : List ModelCheckPathSegment) :
    Except ModelTermError (Option (Term Γ.model Γ.current Γ.inputs .bool)) :=
  match raw with
  | none => pure none
  | some value => return some (← liftTerm (checkExpr Γ (.table Γ.current)
      value .bool .bool path))

private def outputExpectedOrigin? {Γ : TermContext}
    (schema : TableSchema Γ.model.catalog Γ.current)
    (attr : AttributeId schema) :
    Option (SortOrigin Γ (.table Γ.current) (schema.attributeSort attr)) :=
  match shapeEq : (schema.attr attr).shape with
  | .int =>
      have expectedEq : schema.attributeSort attr = .int := by
        unfold TableSchema.attributeSort
        split <;> simp_all
      some (Eq.mpr (congrArg (SortOrigin Γ (.table Γ.current)) expectedEq) .int)
  | .real =>
      have expectedEq : schema.attributeSort attr = .real := by
        unfold TableSchema.attributeSort
        split <;> simp_all
      some (Eq.mpr (congrArg (SortOrigin Γ (.table Γ.current)) expectedEq) .real)
  | .enum _ | .ref _ => none

private def castAggOp {Γ : TermContext} {scope : RowScope Γ.model Γ.current Γ.inputs}
    {actual expected : ScalarSort Γ.model.catalog}
    (same : actual = expected)
    (op : AggOp Γ.model Γ.current Γ.inputs scope actual) :
    AggOp Γ.model Γ.current Γ.inputs scope expected :=
  Eq.mp (congrArg (fun sort => AggOp Γ.model Γ.current Γ.inputs scope sort) same) op

private def checkOutputField (Γ : TermContext)
    (schema : TableSchema Γ.model.catalog Γ.current)
    (raw : IR.OutputField) (index : Nat)
    (schemaOrdinal : Fin schema.attributes.entries.length)
    (path : List ModelCheckPathSegment) :
    Except ModelTermError (CheckedOutputField Γ schema) := do
  let attr : AttributeId schema := ⟨schemaOrdinal⟩
  if raw.name != schema.attributeName attr then
    modelError .outputFieldNameMismatch (path ++ [.outputField index, .fieldName])
  else
    let checked ← liftTerm
      (synthAggOpFuel (rawAggOpDepth raw.op * 4 + 8) Γ (.table Γ.current)
        raw.op false (path ++ [.outputField index, .fieldOperation]))
    let ⟨sort, op, _origin⟩ := checked
    match outputExpectedOrigin? schema attr with
    | some expectedOrigin =>
        match sameOriginSort? _origin expectedOrigin with
        | some same =>
            let filter ← checkOptionalBool Γ raw.filter
              (path ++ [.outputField index, .fieldFilter])
            pure ⟨index, raw.name, attr, castAggOp same.down op, filter⟩
        | none =>
            modelError .outputFieldSortMismatch
              (path ++ [.outputField index, .fieldOperation])
    | none =>
        modelError .outputFieldSortMismatch
          (path ++ [.outputField index, .fieldOperation])

private def checkOutputFieldsAux (Γ : TermContext)
    (schema : TableSchema Γ.model.catalog Γ.current)
    (path : List ModelCheckPathSegment) :
    Nat → List IR.OutputField →
      Except ModelTermError (List (CheckedOutputField Γ schema))
  | _, [] => pure []
  | index, raw :: raws => do
      if bound : index < schema.attributes.entries.length then
        let checked ← checkOutputField Γ schema raw index ⟨index, bound⟩ path
        let rest ← checkOutputFieldsAux Γ schema path (index + 1) raws
        pure (checked :: rest)
      else modelError .outputFieldCountMismatch (path ++ [.outputSchema])

private def checkOutputFields (Γ : TermContext)
    (schema : TableSchema Γ.model.catalog Γ.current)
    (raws : List IR.OutputField) (path : List ModelCheckPathSegment) :
    Except ModelTermError (List (CheckedOutputField Γ schema)) := do
  if raws.length != schema.attributes.entries.length then
    modelError .outputFieldCountMismatch (path ++ [.outputSchema])
  else if unique : (raws.map IR.OutputField.name).Nodup then
    checkOutputFieldsAux Γ schema path 0 raws
  else
    let index := (firstDuplicateIndex? (raws.map IR.OutputField.name)).getD 0
    modelError .duplicateOutputField (path ++ [.outputField index, .fieldName])

private def outputPortSchemaAt (ctx : DeclarationContext)
    (box : BoxId ctx.modelSchema.catalog) (index : Nat)
    (bound : index < (ctx.outputs box).length) :
    BoxPortSchema ctx.modelSchema.catalog box :=
  (ctx.outputPortSchemas box).get ⟨index, by
    rw [ctx.outputPortSchemas_length box]
    exact bound⟩

private def checkOutput (ctx : DeclarationContext)
    (box : BoxId ctx.modelSchema.catalog) (raw : IR.OutputDecl)
    (index : Nat) (bound : index < (ctx.outputs box).length)
    (root : List ModelCheckPathSegment) :
    Except ModelTermError (CheckedOutputDecl ctx box) := do
  let portSchema := outputPortSchemaAt ctx box index bound
  match raw.builder with
  | .perTable tableName fields =>
      match found : ctx.modelSchema.catalog.lookupTable box tableName with
      | none => modelError .unresolvedOutputTable (root ++ [.outputBuilder, .tableTarget])
      | some target =>
          let Γ : TermContext := ⟨ctx, box, target⟩
          let schema := portSchema.instantiate target
          let checkedFields ← checkOutputFields Γ schema fields (root ++ [.outputFields])
          pure ⟨index, raw.name, target, portSchema, checkedFields⟩

private def checkOutputsAux (ctx : DeclarationContext)
    (box : BoxId ctx.modelSchema.catalog) (boxIndex : Nat) :
    Nat → List IR.OutputDecl → Except ModelTermError (List (CheckedOutputDecl ctx box))
  | _, [] => pure []
  | index, raw :: raws => do
      if bound : index < (ctx.outputs box).length then
        let checked ← checkOutput ctx box raw index bound [.model, .box boxIndex, .output index]
        let rest ← checkOutputsAux ctx box boxIndex (index + 1) raws
        pure (checked :: rest)
      else modelError .outputFieldCountMismatch [.model, .box boxIndex, .output index]

private def checkViewValue (Γ : TermContext) (raw : IR.ViewDecl)
    (root : List ModelCheckPathSegment) : Except ModelTermError (CheckedViewValue Γ) := do
  match raw.reduce, raw.value with
  | .count, none => pure .count
  | .sum, some value | .min, some value | .max, some value =>
      let checked ← liftTerm (synthExpr Γ (.table Γ.current) value (root ++ [.viewValue]))
      match checked with
      | ⟨.int, expr, .int⟩ =>
          match raw.reduce with
          | .sum => pure (.sumInt expr)
          | .min => pure (.minInt expr)
          | .max => pure (.maxInt expr)
          | .count => modelError .invalidViewReducerShape (root ++ [.viewReducer])
      | ⟨.real, expr, .real⟩ =>
          match raw.reduce with
          | .sum => pure (.sumReal expr)
          | .min => pure (.minReal expr)
          | .max => pure (.maxReal expr)
          | .count => modelError .invalidViewReducerShape (root ++ [.viewReducer])
      | _ => modelError .invalidViewReducerShape (root ++ [.viewValue])
  | _, _ => modelError .invalidViewReducerShape (root ++ [.viewReducer])

private def checkView (ctx : DeclarationContext)
    (box : BoxId ctx.modelSchema.catalog) (raw : IR.ViewDecl)
    (index : Nat) (root : List ModelCheckPathSegment) :
    Except ModelTermError (CheckedViewDecl ctx box) := do
  match ctx.modelSchema.catalog.lookupTable box raw.table with
  | none => modelError .unresolvedViewTable (root ++ [.viewTable])
  | some target =>
      let Γ : TermContext := ⟨ctx, box, target⟩
      let filter ← checkOptionalBool Γ raw.filter (root ++ [.viewFilter])
      let value ← checkViewValue Γ raw root
      pure ⟨index, raw.name, target, filter, value⟩

private def checkViewsAux (ctx : DeclarationContext)
    (box : BoxId ctx.modelSchema.catalog) (boxIndex : Nat) :
    Nat → List IR.ViewDecl → Except ModelTermError (List (CheckedViewDecl ctx box))
  | _, [] => pure []
  | index, raw :: raws => do
      let checked ← checkView ctx box raw index [.model, .box boxIndex, .view index]
      let rest ← checkViewsAux ctx box boxIndex (index + 1) raws
      pure (checked :: rest)

private def checkGroupKey (Γ : TermContext) (raw : IR.GroupKey) (index : Nat)
    (root : List ModelCheckPathSegment) : Except ModelTermError (CheckedGroupKey Γ) := do
  match (Γ.model.schemaFor Γ.current).lookupAttribute raw.attr with
  | none => modelError .unresolvedGroupedKey (root ++ [.groupedKey index, .groupedAttribute])
  | some attr =>
      match shapeEq : ((Γ.model.schemaFor Γ.current).attr attr).shape with
      | .int =>
          match raw.bandWidth with
          | some width =>
              if positive : 0 < width then
                pure ⟨index, attr, some width, by simp [GroupBandValid, shapeEq, positive]⟩
              else modelError .nonpositiveGroupedBand (root ++ [.groupedKey index, .groupedBand])
          | none => modelError .missingGroupedBand (root ++ [.groupedKey index, .groupedBand])
      | .enum _ | .ref _ =>
          match raw.bandWidth with
          | none => pure ⟨index, attr, none, by simp [GroupBandValid, shapeEq]⟩
          | some _ => modelError .unexpectedGroupedBand (root ++ [.groupedKey index, .groupedBand])
      | .real => modelError .invalidGroupedKeySort (root ++ [.groupedKey index, .groupedAttribute])

private def checkGroupKeysAux (Γ : TermContext) (root : List ModelCheckPathSegment) :
    Nat → List IR.GroupKey → Except ModelTermError (List (CheckedGroupKey Γ))
  | _, [] => pure []
  | index, raw :: raws => do
      let checked ← checkGroupKey Γ raw index root
      let rest ← checkGroupKeysAux Γ root (index + 1) raws
      pure (checked :: rest)

private def checkGroupedFilter (Γ : TermContext) (raw : Option IR.Expr)
    (root : List ModelCheckPathSegment) :
    Except ModelTermError (Option (Term Γ.model Γ.current Γ.inputs .bool)) := do
  match raw with
  | none => pure none
  | some value =>
      if rawExprContainsAggregate value then
        modelError .aggregateInGroupedFilter (root ++ [.viewFilter])
      else
        pure (some (← liftTerm (checkExpr Γ (.table Γ.current)
          value .bool .bool (root ++ [.viewFilter]))))

private def checkGroupedView (ctx : DeclarationContext)
    (box : BoxId ctx.modelSchema.catalog) (raw : IR.GroupedViewDecl)
    (index : Nat) (root : List ModelCheckPathSegment) :
    Except ModelTermError (CheckedGroupedViewDecl ctx box) := do
  match ctx.modelSchema.catalog.lookupTable box raw.table with
  | none => modelError .unresolvedViewTable (root ++ [.viewTable])
  | some target =>
      if count : 1 ≤ raw.keys.length ∧ raw.keys.length ≤ 4 then
        let Γ : TermContext := ⟨ctx, box, target⟩
        let keys ← checkGroupKeysAux Γ (root ++ [.groupedKeys]) 0 raw.keys
        let filter ← checkGroupedFilter Γ raw.filter root
        if checkedCount : 1 ≤ keys.length ∧ keys.length ≤ 4 then
          pure ⟨index, raw.name, target, keys, checkedCount, filter⟩
        else modelError .invalidGroupedKeyCount (root ++ [.groupedKeys])
      else modelError .invalidGroupedKeyCount (root ++ [.groupedKeys])

private def checkGroupedViewsAux (ctx : DeclarationContext)
    (box : BoxId ctx.modelSchema.catalog) (boxIndex : Nat) :
    Nat → List IR.GroupedViewDecl →
      Except ModelTermError (List (CheckedGroupedViewDecl ctx box))
  | _, [] => pure []
  | index, raw :: raws => do
      let checked ← checkGroupedView ctx box raw index
        [.model, .box boxIndex, .groupedView index]
      let rest ← checkGroupedViewsAux ctx box boxIndex (index + 1) raws
      pure (checked :: rest)

private def checkTransitionsAux (ctx : DeclarationContext)
    (box : BoxId ctx.modelSchema.catalog) (boxIndex : Nat) :
    (index : Nat) → (raws : List IR.Transition) →
      Except ModelTermError (List (CheckedTransitionPack ctx box))
  | _, [] => pure []
  | index, raw :: raws => do
      if bound : index < (ctx.transitions box).length then
        let ordinal : Fin (ctx.transitions box).length := ⟨index, bound⟩
        let target := ctx.resolveTransitionTarget box ordinal
        let Γ : TermContext := ⟨ctx, box, target⟩
        let terms ← liftTerm (checkTransitionTerms Γ raw
          [.model, .box boxIndex, .transition index])
        let checked : CheckedTransition Γ :=
          ⟨index, raw.name, terms⟩
        let rest ← checkTransitionsAux ctx box boxIndex (index + 1) raws
        pure (⟨target, checked⟩ :: rest)
      else modelError .unresolvedViewTable [.model, .box boxIndex, .transition index]

private def checkBox (ctx : DeclarationContext)
    (box : BoxId ctx.modelSchema.catalog) (boxIndex : Nat) :
    Except ModelTermError (CheckedBox ctx box) := do
  let transitions ← checkTransitionsAux ctx box boxIndex 0 (ctx.transitions box)
  let outputs ← checkOutputsAux ctx box boxIndex 0 (ctx.outputs box)
  let views ← checkViewsAux ctx box boxIndex 0 (ctx.views box)
  let groupedViews ← checkGroupedViewsAux ctx box boxIndex 0 (ctx.groupedViews box)
  pure ⟨boxIndex, transitions, outputs, views, groupedViews⟩

private def checkBoxesAux (ctx : DeclarationContext) :
    Nat → List (Fin ctx.modelSchema.catalog.boxes.entries.length) →
      Except ModelTermError (List (CheckedBoxPack ctx))
  | _, [] => pure []
  | index, ordinal :: ordinals => do
      let box : BoxId ctx.modelSchema.catalog := ⟨ordinal⟩
      let checked ← checkBox ctx box index
      let rest ← checkBoxesAux ctx (index + 1) ordinals
      pure (⟨box, checked⟩ :: rest)

private def checkBoxes (ctx : DeclarationContext) :
    Except ModelTermError (List (CheckedBoxPack ctx)) :=
  checkBoxesAux ctx 0 (List.ofFn id)

private def checkSummary (ctx : DeclarationContext) (raw : IR.SummaryDecl)
    (index : Nat) : Except ModelTermError (CheckedSummaryDecl ctx) := do
  match ctx.modelSchema.catalog.lookupBox raw.box with
  | none => modelError .unresolvedSummaryBox [.model, .summary index, .summaryBox]
  | some box =>
      match found : (ctx.views box).findIdx? (fun view => view.name == raw.view) with
      | none => modelError .unresolvedSummaryView [.model, .summary index, .summaryView]
      | some viewIndex =>
          if bound : viewIndex < (ctx.views box).length then
            pure ⟨index, raw.name, box, ⟨viewIndex, bound⟩, raw.reduce⟩
          else modelError .unresolvedSummaryView [.model, .summary index, .summaryView]

private def checkSummariesAux (ctx : DeclarationContext) :
    Nat → List IR.SummaryDecl → Except ModelTermError (List (CheckedSummaryDecl ctx))
  | _, [] => pure []
  | index, raw :: raws => do
      let checked ← checkSummary ctx raw index
      let rest ← checkSummariesAux ctx (index + 1) raws
      pure (checked :: rest)

/-- Canonical whole-model checker. Declaration failures are preserved as the
exact PRD 0005 error value. -/
def checkModel (raw : IR.Model) : Except ModelCheckError Checked.Model :=
  match checkDeclarations raw with
  | .error error => .error (.declaration error)
  | .ok ctx =>
      match checkBoxes ctx with
      | .error error => .error (.model error)
      | .ok boxes =>
          match checkSummariesAux ctx 0 (ctx.summaries) with
          | .error error => .error (.model error)
          | .ok summaries => .ok ⟨ctx, boxes, summaries, raw.wires⟩

/-! ## Independent model-local judgments -/

/-- Pointwise optional relation used by expression-bearing declarations. -/
inductive OptionalRel {α β : Type} (relation : α → β → Prop) :
    Option α → Option β → Prop where
  | none : OptionalRel relation none none
  | some {left right} (related : relation left right) :
      OptionalRel relation (some left) (some right)

/-- The output-schema attribute selected for an aggregate field is numeric. -/
inductive OutputAttributeNumeric {catalog : SchemaUniverse} {owner : TableTarget catalog}
    (schema : TableSchema catalog owner) (attr : AttributeId schema) : Prop where
  | int (shape : (schema.attr attr).shape = .int) : OutputAttributeNumeric schema attr
  | real (shape : (schema.attr attr).shape = .real) : OutputAttributeNumeric schema attr

/-- One output field has the corresponding schema-indexed result sort. -/
inductive OutputFieldWellFormed {ctx : DeclarationContext}
    {box : BoxId ctx.modelSchema.catalog} {target : TableId ctx.modelSchema.catalog box}
    {portSchema : BoxPortSchema ctx.modelSchema.catalog box} :
    IR.OutputField →
      CheckedOutputField ⟨ctx, box, target⟩ (portSchema.instantiate target) → Prop where
  | mk {raw checked}
      (ordinal : checked.sourceOrdinal = checked.schemaAttribute.ordinal.val)
      (name : checked.name = raw.name)
      (schemaName :
        (portSchema.instantiate target).attributeName checked.schemaAttribute = raw.name)
      (numeric : OutputAttributeNumeric (portSchema.instantiate target)
        checked.schemaAttribute)
      (expectedOrigin : SortOrigin ⟨ctx, box, target⟩ (.table ⟨box, target⟩)
        ((portSchema.instantiate target).attributeSort checked.schemaAttribute))
      (expected : outputExpectedOrigin? (Γ := ⟨ctx, box, target⟩)
        (portSchema.instantiate target) checked.schemaAttribute = some expectedOrigin)
      (operation : AggOpSynthesizes ⟨ctx, box, target⟩ (.table ⟨box, target⟩)
        raw.op ((portSchema.instantiate target).attributeSort checked.schemaAttribute)
        checked.op)
      (filter : OptionalRel
        (ExprChecks ⟨ctx, box, target⟩ (.table ⟨box, target⟩) .bool)
        raw.filter checked.filter) :
      OutputFieldWellFormed raw checked

/-- Output declarations preserve ordered field/schema correspondence and reject
field-name duplication independently of executable diagnostics. -/
inductive OutputWellFormed {ctx : DeclarationContext}
    {box : BoxId ctx.modelSchema.catalog} :
    IR.OutputDecl → CheckedOutputDecl ctx box → Prop where
  | perTable {raw checked table fields}
      (ordinal : checked.sourceOrdinal < (ctx.outputs box).length)
      (portSchemaExact : checked.portSchema =
        outputPortSchemaAt ctx box checked.sourceOrdinal ordinal)
      (name : checked.name = raw.name)
      (portName : checked.portSchema.name = raw.name)
      (schema : checked.portSchema.source = raw.schema)
      (builder : raw.builder = .perTable table fields)
      (target : ctx.modelSchema.catalog.tableName ⟨box, checked.target⟩ = table)
      (unique : (fields.map IR.OutputField.name).Nodup)
      (schemaFieldCount :
        fields.length = (checked.portSchema.instantiate checked.target).attributes.entries.length)
      (fieldCount : fields.length = checked.fields.length)
      (fieldOrder : checked.fields.map CheckedOutputField.sourceOrdinal =
        List.range fields.length)
      (fieldsRelated : List.Forall₂ OutputFieldWellFormed fields checked.fields) :
      OutputWellFormed raw checked

/-- Reducer/value shape and numeric result sort for an ordinary view. -/
inductive ViewValueWellFormed (Γ : TermContext) :
    IR.ViewReduce → Option IR.Expr → CheckedViewValue Γ → Prop where
  | count : ViewValueWellFormed Γ .count none .count
  | sumInt {raw value} (typed : ExprSynthesizes Γ (.table Γ.current) raw .int value) :
      ViewValueWellFormed Γ .sum (some raw) (.sumInt value)
  | sumReal {raw value} (typed : ExprSynthesizes Γ (.table Γ.current) raw .real value) :
      ViewValueWellFormed Γ .sum (some raw) (.sumReal value)
  | minInt {raw value} (typed : ExprSynthesizes Γ (.table Γ.current) raw .int value) :
      ViewValueWellFormed Γ .min (some raw) (.minInt value)
  | minReal {raw value} (typed : ExprSynthesizes Γ (.table Γ.current) raw .real value) :
      ViewValueWellFormed Γ .min (some raw) (.minReal value)
  | maxInt {raw value} (typed : ExprSynthesizes Γ (.table Γ.current) raw .int value) :
      ViewValueWellFormed Γ .max (some raw) (.maxInt value)
  | maxReal {raw value} (typed : ExprSynthesizes Γ (.table Γ.current) raw .real value) :
      ViewValueWellFormed Γ .max (some raw) (.maxReal value)

inductive ViewWellFormed {ctx : DeclarationContext}
    {box : BoxId ctx.modelSchema.catalog} :
    IR.ViewDecl → CheckedViewDecl ctx box → Prop where
  | mk {raw checked}
      (ordinal : checked.sourceOrdinal < (ctx.views box).length)
      (name : checked.name = raw.name)
      (target : ctx.modelSchema.catalog.tableName ⟨box, checked.target⟩ = raw.table)
      (filter : OptionalRel
        (ExprChecks ⟨ctx, box, checked.target⟩ (.table ⟨box, checked.target⟩) .bool)
        raw.filter checked.filter)
      (value : ViewValueWellFormed ⟨ctx, box, checked.target⟩
        raw.reduce raw.value checked.value) :
      ViewWellFormed raw checked

/-- One grouped key resolves its attribute and carries exactly the frozen band
shape stored intrinsically in `CheckedGroupKey.valid`. -/
inductive GroupKeyWellFormed (Γ : TermContext) :
    IR.GroupKey → CheckedGroupKey Γ → Prop where
  | mk {raw checked}
      (name : (Γ.model.schemaFor Γ.current).attributeName checked.attr = raw.attr)
      (band : checked.band = raw.bandWidth) : GroupKeyWellFormed Γ raw checked

private def AggregateFreeFilter : Option IR.Expr → Prop
  | none => True
  | some raw => rawExprContainsAggregate raw = false

inductive GroupedViewWellFormed {ctx : DeclarationContext}
    {box : BoxId ctx.modelSchema.catalog} :
    IR.GroupedViewDecl → CheckedGroupedViewDecl ctx box → Prop where
  | mk {raw checked}
      (ordinal : checked.sourceOrdinal < (ctx.groupedViews box).length)
      (name : checked.name = raw.name)
      (target : ctx.modelSchema.catalog.tableName ⟨box, checked.target⟩ = raw.table)
      (keys : List.Forall₂ (GroupKeyWellFormed ⟨ctx, box, checked.target⟩)
        raw.keys checked.keys)
      (keyOrder : checked.keys.map CheckedGroupKey.sourceOrdinal =
        List.range raw.keys.length)
      (keyCount : 1 ≤ raw.keys.length ∧ raw.keys.length ≤ 4)
      (filter : OptionalRel
        (ExprChecks ⟨ctx, box, checked.target⟩ (.table ⟨box, checked.target⟩) .bool)
        raw.filter checked.filter)
      (aggregateFree : AggregateFreeFilter raw.filter) :
      GroupedViewWellFormed raw checked

inductive SummaryWellFormed {ctx : DeclarationContext} :
    IR.SummaryDecl → CheckedSummaryDecl ctx → Prop where
  | mk {raw checked}
      (ordinal : checked.sourceOrdinal < ctx.summaries.length)
      (name : checked.name = raw.name)
      (box : ctx.modelSchema.catalog.boxName checked.box = raw.box)
      (view : ((ctx.views checked.box).get checked.viewOrdinal).name = raw.view)
      (viewFound : (ctx.views checked.box).findIdx?
        (fun candidate => candidate.name == raw.view) = some checked.viewOrdinal.val)
      (reduce : checked.reduce = raw.reduce) : SummaryWellFormed raw checked

/-- Transition headers and payloads remain source ordered and use the resolved
PRD 0005 table identity. -/
inductive TransitionPackWellFormed {ctx : DeclarationContext}
    {box : BoxId ctx.modelSchema.catalog} :
    IR.Transition → CheckedTransitionPack ctx box → Prop where
  | mk {raw checked}
      (ordinal : checked.checked.sourceOrdinal < (ctx.transitions box).length)
      (source : raw = ctx.transitionAt box ⟨checked.checked.sourceOrdinal, ordinal⟩)
      (targetExact : checked.target =
        ctx.resolveTransitionTarget box ⟨checked.checked.sourceOrdinal, ordinal⟩)
      (name : checked.checked.name = raw.name)
      (target : ctx.modelSchema.catalog.tableName ⟨box, checked.target⟩ = raw.table)
      (terms : TransitionWellTyped ⟨ctx, box, checked.target⟩ raw checked.checked.terms) :
      TransitionPackWellFormed raw checked

/-- Box-local static validity is pointwise over every owned declaration. Tables,
inputs and box spelling are the already accepted declaration projection. -/
inductive BoxTermsWellFormed {ctx : DeclarationContext}
    {box : BoxId ctx.modelSchema.catalog} : IR.Box → CheckedBox ctx box → Prop where
  | mk {raw checked}
      (sourceExact : raw = sourceBox ctx.source ctx.wellFormed box)
      (ordinal : checked.sourceOrdinal = box.ordinal.val)
      (name : raw.name = (sourceBox ctx.source ctx.wellFormed box).name)
      (tables : raw.tables = ctx.modelSchema.eraseTables box)
      (inputs : raw.inputs = (sourceBox ctx.source ctx.wellFormed box).inputs)
      (transitions : List.Forall₂ TransitionPackWellFormed
        raw.transitions checked.transitions)
      (transitionOrder :
        checked.transitions.map (fun entry => entry.checked.sourceOrdinal) =
          List.range raw.transitions.length)
      (outputs : List.Forall₂ OutputWellFormed raw.outputs checked.outputs)
      (outputOrder : checked.outputs.map CheckedOutputDecl.sourceOrdinal =
        List.range raw.outputs.length)
      (views : List.Forall₂ ViewWellFormed raw.views checked.views)
      (viewOrder : checked.views.map CheckedViewDecl.sourceOrdinal =
        List.range raw.views.length)
      (grouped : List.Forall₂ GroupedViewWellFormed
        raw.groupedViews checked.groupedViews)
      (groupedOrder : checked.groupedViews.map CheckedGroupedViewDecl.sourceOrdinal =
        List.range raw.groupedViews.length) :
      BoxTermsWellFormed raw checked

/-- A source box is related to its dependent checked identity without converting
that identity into a parallel name map. -/
inductive BoxPackWellFormed {ctx : DeclarationContext} :
    IR.Box → CheckedBoxPack ctx → Prop where
  | mk {raw checked} (box : BoxTermsWellFormed raw checked.checked) :
      BoxPackWellFormed raw checked

/-- Independent, syntax-directed whole-model elaboration.  It relates the raw
model to the actual checked payload and records exact source order and deferred
wire retention without mentioning executable checker success. -/
inductive ModelElaborates : IR.Model → Checked.Model → Prop where
  | mk {raw : IR.Model} {checked : Checked.Model}
      (source : checked.declarations.source = raw)
      (declarations : DeclarationsWellFormed raw)
      (boxTerms : List.Forall₂ (BoxPackWellFormed (ctx := checked.declarations))
        raw.boxes checked.boxes)
      (boxOrder : checked.boxes.map (fun entry => entry.box.ordinal.val) =
        List.range raw.boxes.length)
      (summaryTerms : List.Forall₂
        (SummaryWellFormed (ctx := checked.declarations)) raw.summaries checked.summaries)
      (summaryOrder : checked.summaries.map CheckedSummaryDecl.sourceOrdinal =
        List.range raw.summaries.length)
      (wires : checked.wires = raw.wires) :
      ModelElaborates raw checked

/-- Independent model validity is existence of a structural elaboration
witness, never existence of successful checker execution. -/
def ModelWellFormed (raw : IR.Model) : Prop :=
  ∃ checked : Checked.Model, ModelElaborates raw checked

/-! ## Checker/judgment correspondence -/

private theorem liftTerm_ok_iff {α : Type} {result : Except TermCheckError α}
    {value : α} : liftTerm result = .ok value ↔ result = .ok value := by
  cases result <;> simp [liftTerm]

private theorem checkOptionalBool_sound {Γ : TermContext} {raw checked path}
    (success : checkOptionalBool Γ raw path = .ok checked) :
    OptionalRel (ExprChecks Γ (.table Γ.current) .bool) raw checked := by
  cases raw with
  | none =>
      cases success
      exact .none
  | some raw =>
      simp only [checkOptionalBool, Bind.bind, Except.bind] at success
      split at success
      · contradiction
      · rename_i term termOk
        have checkedOk := liftTerm_ok_iff.mp termOk
        cases success
        exact .some (checkExpr_sound checkedOk)

private theorem checkOptionalBool_complete {Γ : TermContext} {raw checked}
    (typed : OptionalRel (ExprChecks Γ (.table Γ.current) .bool) raw checked)
    (path : List ModelCheckPathSegment) :
    checkOptionalBool Γ raw path = .ok checked := by
  cases typed with
  | none => rfl
  | some typed =>
      simp only [checkOptionalBool, Bind.bind, Except.bind]
      rw [liftTerm_ok_iff.mpr (checkExpr_complete (path := path) typed .bool)]
      rfl

private theorem AggOpSynthesizes.cast {Γ : TermContext}
    {scope : RowScope Γ.model Γ.current Γ.inputs} {raw actual expected op}
    (typed : AggOpSynthesizes Γ scope raw actual op) (same : actual = expected) :
    AggOpSynthesizes Γ scope raw expected
      (Eq.mp (congrArg (fun sort => AggOp Γ.model Γ.current Γ.inputs scope sort) same) op) := by
  cases same
  exact typed

private theorem outputExpectedOrigin_sound {Γ : TermContext}
    {schema : TableSchema Γ.model.catalog Γ.current} {attr : AttributeId schema}
    {origin : SortOrigin Γ (.table Γ.current) (schema.attributeSort attr)}
    (success : outputExpectedOrigin? schema attr = some origin) :
    OutputAttributeNumeric schema attr := by
  simp only [outputExpectedOrigin?] at success
  split at success <;> rename_i shapeEq
  · exact .int shapeEq
  · exact .real shapeEq
  · contradiction
  · contradiction

private theorem checkOutputField_sound {Γ : TermContext}
    {portSchema : BoxPortSchema Γ.model.catalog Γ.box}
    {raw checked index schemaOrdinal path}
    (ordinal : index = schemaOrdinal.val)
    (success : checkOutputField Γ (portSchema.instantiate Γ.currentTable)
      raw index schemaOrdinal path = .ok checked) :
    OutputFieldWellFormed (portSchema := portSchema) raw checked ∧
      checked.sourceOrdinal = index := by
  simp only [checkOutputField] at success
  split at success
  · simp [modelError] at success
  · rename_i nameMatches
    have schemaName : (portSchema.instantiate Γ.currentTable).attributeName
        ⟨schemaOrdinal⟩ = raw.name := by
      by_contra different
      apply nameMatches
      have rawNe : raw.name ≠
          (portSchema.instantiate Γ.currentTable).attributeName ⟨schemaOrdinal⟩ :=
        fun same => different same.symm
      simpa only [bne_iff_ne] using rawNe
    cases opResult : synthAggOpFuel (rawAggOpDepth raw.op * 4 + 8) Γ
        (.table Γ.current) raw.op false
        (path ++ [.outputField index, .fieldOperation]) with
    | error error =>
        rw [opResult] at success
        simp only [liftTerm, Bind.bind, Except.bind] at success
        contradiction
    | ok result =>
        rw [opResult] at success
        simp only [liftTerm, Bind.bind, Except.bind] at success
        have operation := synthAggOpFuel_sound opResult
        cases result with
        | mk sort pair =>
          cases pair with
          | mk op origin =>
            cases expectedResult : outputExpectedOrigin?
                (Γ := Γ) (portSchema.instantiate Γ.currentTable) ⟨schemaOrdinal⟩ with
            | none =>
                rw [expectedResult] at success
                simp [modelError] at success
            | some expectedOrigin =>
                rw [expectedResult] at success
                simp only at success
                cases sameResult : sameOriginSort? origin expectedOrigin with
                | none => rw [sameResult] at success; simp [modelError] at success
                | some sortSame =>
                  rw [sameResult] at success
                  cases filterResult : checkOptionalBool Γ raw.filter
                      (path ++ [.outputField index, .fieldFilter]) with
                  | error error => simp [filterResult] at success
                  | ok filter =>
                      rw [filterResult] at success
                      simp only [Pure.pure, Except.pure] at success
                      have same := Except.ok.inj success
                      cases same
                      exact ⟨.mk ordinal rfl schemaName
                        (outputExpectedOrigin_sound expectedResult)
                        expectedOrigin expectedResult
                        (operation.cast sortSame.down)
                        (checkOptionalBool_sound filterResult), rfl⟩

private theorem OutputFieldWellFormed.source_eq_attribute {Γ : TermContext}
    {portSchema : BoxPortSchema Γ.model.catalog Γ.box} {raw checked}
    (typed : OutputFieldWellFormed (portSchema := portSchema) raw checked) :
    checked.sourceOrdinal = checked.schemaAttribute.ordinal.val := by
  cases typed
  assumption

private theorem checkOutputField_complete {Γ : TermContext}
    {portSchema : BoxPortSchema Γ.model.catalog Γ.box} {raw checked}
    (typed : OutputFieldWellFormed (portSchema := portSchema) raw checked)
    (path : List ModelCheckPathSegment) :
    checkOutputField Γ (portSchema.instantiate Γ.currentTable) raw
      checked.sourceOrdinal checked.schemaAttribute.ordinal path = .ok checked := by
  cases typed with
  | mk ordinal name schemaName numeric expectedOrigin expected operation filter =>
    cases raw with
    | mk rawName rawOp rawFilter =>
      simp only at name schemaName operation filter
      simp only [checkOutputField]
      rw [if_neg (by simp [schemaName])]
      obtain ⟨origin, operationOk⟩ := synthAggOpFuel_complete operation
        (rawAggOpDepth rawOp * 4 + 8) false
        (path ++ [ModelCheckPathSegment.outputField checked.sourceOrdinal,
          ModelCheckPathSegment.fieldOperation]) (by omega) (by simp)
      rw [liftTerm_ok_iff.mpr operationOk]
      simp only [liftTerm, Bind.bind, Except.bind]
      rw [expected]
      simp only
      obtain ⟨same, matchOk⟩ := sameOriginSort_complete origin expectedOrigin
      rw [matchOk]
      rw [checkOptionalBool_complete filter
        (path ++ [ModelCheckPathSegment.outputField checked.sourceOrdinal,
          ModelCheckPathSegment.fieldFilter])]
      cases name
      congr

private theorem checkOutputFieldsAux_sound {Γ : TermContext}
    {portSchema : BoxPortSchema Γ.model.catalog Γ.box}
    {path index raws checked}
    (bound : index + raws.length ≤
      (portSchema.instantiate Γ.currentTable).attributes.entries.length)
    (success : checkOutputFieldsAux Γ (portSchema.instantiate Γ.currentTable)
      path index raws = .ok checked) :
    List.Forall₂ (OutputFieldWellFormed (portSchema := portSchema)) raws checked ∧
      checked.map CheckedOutputField.sourceOrdinal = List.range' index raws.length := by
  induction raws generalizing index checked with
  | nil => cases success; exact ⟨.nil, rfl⟩
  | cons raw raws ih =>
      simp only [checkOutputFieldsAux, Bind.bind, Except.bind] at success
      split at success
      · rename_i headBound
        split at success
        · contradiction
        · rename_i head headOk
          split at success
          · contradiction
          · rename_i tail tailOk
            cases success
            have headTyped := checkOutputField_sound rfl headOk
            have rest := ih (index := index + 1) (by simp at bound ⊢; omega) tailOk
            exact ⟨.cons headTyped.1 rest.1,
              by simp [headTyped.2, rest.2, List.range'_succ]⟩
      · simp [modelError] at success

private theorem checkOutputFieldsAux_complete {Γ : TermContext}
    {portSchema : BoxPortSchema Γ.model.catalog Γ.box}
    {path index raws checked}
    (typed : List.Forall₂ (OutputFieldWellFormed (portSchema := portSchema)) raws checked)
    (order : checked.map CheckedOutputField.sourceOrdinal = List.range' index raws.length) :
    checkOutputFieldsAux Γ (portSchema.instantiate Γ.currentTable)
      path index raws = .ok checked := by
  induction typed generalizing index with
  | nil => rfl
  | cons head tail ih =>
      rename_i raw checked raws checkeds
      simp only [List.map_cons, List.length_cons, List.range'_succ] at order
      have parts := List.cons.inj order
      have sourceAttr := OutputFieldWellFormed.source_eq_attribute head
      have bound : index < (portSchema.instantiate Γ.currentTable).attributes.entries.length := by
        rw [← parts.1, sourceAttr]
        exact checked.schemaAttribute.ordinal.isLt
      have finEq : (⟨index, bound⟩ :
          Fin (portSchema.instantiate Γ.currentTable).attributes.entries.length) =
          checked.schemaAttribute.ordinal :=
        Fin.ext (parts.1.symm.trans sourceAttr)
      simp only [checkOutputFieldsAux, Bind.bind, Except.bind]
      rw [dif_pos bound]
      rw [finEq]
      rw [← parts.1]
      rw [checkOutputField_complete (path := path) head]
      simp only
      have tailOrder : List.map CheckedOutputField.sourceOrdinal checkeds =
          List.range' (checked.sourceOrdinal + 1) raws.length := by
        simpa [parts.1] using parts.2
      rw [ih tailOrder]
      rfl

private theorem checkOutputFields_sound {Γ : TermContext}
    {portSchema : BoxPortSchema Γ.model.catalog Γ.box} {raws checked path}
    (success : checkOutputFields Γ (portSchema.instantiate Γ.currentTable)
      raws path = .ok checked) :
    List.Forall₂ (OutputFieldWellFormed (portSchema := portSchema)) raws checked ∧
      checked.map CheckedOutputField.sourceOrdinal = List.range raws.length ∧
      raws.length = (portSchema.instantiate Γ.currentTable).attributes.entries.length ∧
      (raws.map IR.OutputField.name).Nodup := by
  simp only [checkOutputFields] at success
  split at success
  · simp [modelError] at success
  · rename_i sameLength
    have countEq : raws.length =
        (portSchema.instantiate Γ.currentTable).attributes.entries.length := by
      by_contra different
      apply sameLength
      simpa only [bne_iff_ne] using different
    split at success
    · rename_i unique
      have aux := checkOutputFieldsAux_sound
        (portSchema := portSchema) (index := 0)
        (by simpa [countEq]) success
      exact ⟨aux.1, by simpa only [List.range_eq_range'] using aux.2,
        countEq, unique⟩
    · simp [modelError] at success

private theorem checkOutputFields_complete {Γ : TermContext}
    {portSchema : BoxPortSchema Γ.model.catalog Γ.box} {raws checked path}
    (typed : List.Forall₂ (OutputFieldWellFormed (portSchema := portSchema)) raws checked)
    (order : checked.map CheckedOutputField.sourceOrdinal = List.range raws.length)
    (count : raws.length =
      (portSchema.instantiate Γ.currentTable).attributes.entries.length)
    (unique : (raws.map IR.OutputField.name).Nodup) :
    checkOutputFields Γ (portSchema.instantiate Γ.currentTable) raws path = .ok checked := by
  simp only [checkOutputFields]
  rw [if_neg (by simpa [count])]
  rw [dif_pos unique]
  exact checkOutputFieldsAux_complete typed
    (by simpa only [List.range_eq_range'] using order)

private theorem outputPortSchemaAt_name_exact (ctx : DeclarationContext)
    (box : BoxId ctx.modelSchema.catalog) (index : Nat)
    (bound : index < (ctx.outputs box).length) :
    (outputPortSchemaAt ctx box index bound).name =
      ((ctx.outputs box).get ⟨index, bound⟩).name := by
  have point := congrArg (fun names : List String => names.get? index)
    (ctx.outputPortSchemas_names box)
  simp only [List.get?_map] at point
  have schemaBound : index < (ctx.outputPortSchemas box).length := by
    simpa using bound
  rw [List.get?_eq_get schemaBound, List.get?_eq_get bound] at point
  simpa [outputPortSchemaAt] using Option.some.inj point

private theorem outputPortSchemaAt_source_exact (ctx : DeclarationContext)
    (box : BoxId ctx.modelSchema.catalog) (index : Nat)
    (bound : index < (ctx.outputs box).length) :
    (outputPortSchemaAt ctx box index bound).source =
      ((ctx.outputs box).get ⟨index, bound⟩).schema := by
  have point := congrArg (fun schemas : List (List IR.Attr) => schemas.get? index)
    (ctx.outputPortSchemas_sources box)
  simp only [List.get?_map] at point
  have schemaBound : index < (ctx.outputPortSchemas box).length := by
    simpa using bound
  rw [List.get?_eq_get schemaBound, List.get?_eq_get bound] at point
  simpa [outputPortSchemaAt] using Option.some.inj point

private theorem checkOutput_sound {ctx : DeclarationContext}
    {box : BoxId ctx.modelSchema.catalog} {raw checked index root}
    (bound : index < (ctx.outputs box).length)
    (portNameExact : (outputPortSchemaAt ctx box index bound).name = raw.name)
    (schemaExact : (outputPortSchemaAt ctx box index bound).source = raw.schema)
    (success : checkOutput ctx box raw index bound root = .ok checked) :
    OutputWellFormed raw checked ∧ checked.sourceOrdinal = index := by
  cases raw with
  | mk rawName rawSchema rawBuilder =>
    cases rawBuilder with
    | perTable table fields =>
      simp only [checkOutput, Bind.bind, Except.bind] at success
      split at success
      · contradiction
      · rename_i target found
        split at success
        · contradiction
        · rename_i checkedFields fieldsOk
          cases success
          have fieldsTyped := checkOutputFields_sound fieldsOk
          exact ⟨.perTable bound rfl rfl portNameExact schemaExact rfl
            (checkedTableLookup_name ctx box found) fieldsTyped.2.2.2
            fieldsTyped.2.2.1 (List.Forall₂.length_eq fieldsTyped.1)
            fieldsTyped.2.1 fieldsTyped.1, rfl⟩

private theorem checkOutput_complete {ctx : DeclarationContext}
    {box : BoxId ctx.modelSchema.catalog} {raw checked}
    (typed : OutputWellFormed raw checked)
    (root : List ModelCheckPathSegment) :
    checkOutput ctx box raw checked.sourceOrdinal (by cases typed; assumption) root = .ok checked := by
  cases typed with
  | perTable ordinal portSchemaExact name portName schema builder target unique
      schemaFieldCount fieldCount fieldOrder fieldsRelated =>
    cases raw with
    | mk rawName rawSchema rawBuilder =>
      simp only at name schema builder target
      subst rawBuilder
      simp only [checkOutput, Bind.bind, Except.bind]
      rw [← target, ctx.modelSchema.catalog.lookupTable_name_self]
      simp only
      rw [← portSchemaExact]
      rw [checkOutputFields_complete fieldsRelated fieldOrder schemaFieldCount unique]
      cases name
      rfl

private theorem checkOutputsAux_sound {ctx : DeclarationContext}
    {box : BoxId ctx.modelSchema.catalog} {boxIndex index raws checked}
    (prior : List IR.OutputDecl)
    (source : ctx.outputs box = prior ++ raws)
    (indexEq : index = prior.length)
    (success : checkOutputsAux ctx box boxIndex index raws = .ok checked) :
    List.Forall₂ OutputWellFormed raws checked ∧
      checked.map CheckedOutputDecl.sourceOrdinal = List.range' index raws.length := by
  induction raws generalizing prior index checked with
  | nil => cases success; exact ⟨.nil, rfl⟩
  | cons raw raws ih =>
      have bound : index < (ctx.outputs box).length := by
        rw [source, indexEq]
        simp
      have rawAt : (ctx.outputs box).get ⟨index, bound⟩ = raw := by
        have getSome : (ctx.outputs box).get? index = some raw := by
          rw [source, indexEq]
          apply List.get?_eq_some.mpr
          refine ⟨by simp, ?_⟩
          rw [List.get_eq_getElem]
          calc
            (prior ++ raw :: raws)[prior.length] =
                (raw :: raws)[prior.length - prior.length] :=
              List.getElem_append_right (le_refl prior.length)
            _ = raw := by simp
        obtain ⟨otherBound, exact⟩ := List.get?_eq_some.mp getSome
        simpa using exact
      have portNameExact : (outputPortSchemaAt ctx box index bound).name = raw.name := by
        rw [outputPortSchemaAt_name_exact ctx box index bound, rawAt]
      have schemaExact : (outputPortSchemaAt ctx box index bound).source = raw.schema := by
        rw [outputPortSchemaAt_source_exact ctx box index bound, rawAt]
      simp only [checkOutputsAux, Bind.bind, Except.bind] at success
      rw [dif_pos bound] at success
      split at success
      · contradiction
      · rename_i head headOk
        split at success
        · contradiction
        · rename_i tail tailOk
          cases success
          have headTyped := checkOutput_sound bound portNameExact schemaExact headOk
          have sourceTail : ctx.outputs box = (prior ++ [raw]) ++ raws := by
            simpa [List.append_assoc] using source
          have rest := ih (prior := prior ++ [raw]) (index := index + 1)
            sourceTail (by simp [indexEq]) tailOk
          exact ⟨.cons headTyped.1 rest.1,
            by simp [headTyped.2, rest.2, List.range'_succ]⟩

private theorem checkOutputsAux_complete {ctx : DeclarationContext}
    {box : BoxId ctx.modelSchema.catalog} {boxIndex index raws checked}
    (typed : List.Forall₂ OutputWellFormed raws checked)
    (order : checked.map CheckedOutputDecl.sourceOrdinal = List.range' index raws.length) :
    checkOutputsAux ctx box boxIndex index raws = .ok checked := by
  induction typed generalizing index with
  | nil => rfl
  | cons head tail ih =>
      rename_i raw checked raws checkeds
      simp only [List.map_cons, List.length_cons, List.range'_succ] at order
      have parts := List.cons.inj order
      cases parts.1
      have bound : checked.sourceOrdinal < (ctx.outputs box).length := by
        cases head
        assumption
      simp only [checkOutputsAux, Bind.bind, Except.bind]
      rw [dif_pos bound]
      rw [checkOutput_complete (root := [.model, .box boxIndex, .output checked.sourceOrdinal]) head]
      simp only
      have tailOrder : List.map CheckedOutputDecl.sourceOrdinal checkeds =
          List.range' (checked.sourceOrdinal + 1) raws.length := by
        simpa [parts.1] using parts.2
      rw [ih tailOrder]
      rfl

private theorem checkViewValue_sound {Γ : TermContext} {raw checked root}
    (success : checkViewValue Γ raw root = .ok checked) :
    ViewValueWellFormed Γ raw.reduce raw.value checked := by
  cases raw with
  | mk name table filter value reduce =>
    cases reduce with
    | count =>
        cases value with
        | none => cases success; exact .count
        | some value => simp [checkViewValue, modelError] at success
    | sum =>
        cases value with
        | none => simp [checkViewValue, modelError] at success
        | some value =>
            simp only [checkViewValue, Bind.bind, Except.bind] at success
            split at success
            · contradiction
            · rename_i result resultOk
              have rawOk := liftTerm_ok_iff.mp resultOk
              cases result with
              | mk sort expr origin =>
                cases origin with
                | real => cases success; exact .sumReal (synthExpr_sound rawOk)
                | int => cases success; exact .sumInt (synthExpr_sound rawOk)
                | bool => simp [modelError] at success
                | enum => simp [modelError] at success
                | ref => simp [modelError] at success
    | min =>
        cases value with
        | none => simp [checkViewValue, modelError] at success
        | some value =>
            simp only [checkViewValue, Bind.bind, Except.bind] at success
            split at success
            · contradiction
            · rename_i result resultOk
              have rawOk := liftTerm_ok_iff.mp resultOk
              cases result with
              | mk sort expr origin =>
                cases origin with
                | real => cases success; exact .minReal (synthExpr_sound rawOk)
                | int => cases success; exact .minInt (synthExpr_sound rawOk)
                | bool => simp [modelError] at success
                | enum => simp [modelError] at success
                | ref => simp [modelError] at success
    | max =>
        cases value with
        | none => simp [checkViewValue, modelError] at success
        | some value =>
            simp only [checkViewValue, Bind.bind, Except.bind] at success
            split at success
            · contradiction
            · rename_i result resultOk
              have rawOk := liftTerm_ok_iff.mp resultOk
              cases result with
              | mk sort expr origin =>
                cases origin with
                | real => cases success; exact .maxReal (synthExpr_sound rawOk)
                | int => cases success; exact .maxInt (synthExpr_sound rawOk)
                | bool => simp [modelError] at success
                | enum => simp [modelError] at success
                | ref => simp [modelError] at success

private theorem checkViewValue_complete {Γ : TermContext}
    {name table : String} {filter : Option IR.Expr} {reduce value checked}
    (typed : ViewValueWellFormed Γ reduce value checked)
    (root : List ModelCheckPathSegment) :
    checkViewValue Γ ⟨name, table, filter, value, reduce⟩ root = .ok checked := by
  cases typed with
  | count => rfl
  | sumInt typed | sumReal typed | minInt typed | minReal typed
  | maxInt typed | maxReal typed =>
      obtain ⟨origin, success⟩ := synthExpr_complete
        (path := root ++ [ModelCheckPathSegment.viewValue]) typed
      cases origin
      all_goals simp only [checkViewValue, Bind.bind, Except.bind]
      all_goals rw [liftTerm_ok_iff.mpr success]
      all_goals rfl

private theorem checkView_sound {ctx : DeclarationContext}
    {box : BoxId ctx.modelSchema.catalog} {raw checked index root}
    (bound : index < (ctx.views box).length)
    (success : checkView ctx box raw index root = .ok checked) :
    ViewWellFormed raw checked ∧ checked.sourceOrdinal = index := by
  simp only [checkView, Bind.bind, Except.bind] at success
  split at success
  · contradiction
  · rename_i target found
    split at success
    · contradiction
    · rename_i filter filterOk
      split at success
      · contradiction
      · rename_i value valueOk
        cases success
        exact ⟨.mk bound rfl (checkedTableLookup_name ctx box found)
          (checkOptionalBool_sound filterOk) (checkViewValue_sound valueOk), rfl⟩

private theorem checkView_complete {ctx : DeclarationContext}
    {box : BoxId ctx.modelSchema.catalog} {raw checked}
    (typed : ViewWellFormed raw checked)
    (root : List ModelCheckPathSegment) :
    checkView ctx box raw checked.sourceOrdinal root = .ok checked := by
  cases typed with
  | mk ordinal name target filter value =>
      cases raw with
      | mk rawName rawTable rawFilter rawValue rawReduce =>
        simp only at name target filter value
        simp only [checkView, Bind.bind, Except.bind]
        rw [← target, ctx.modelSchema.catalog.lookupTable_name_self]
        simp only
        rw [checkOptionalBool_complete filter (root ++ [ModelCheckPathSegment.viewFilter])]
        rw [checkViewValue_complete value root]
        cases name
        rfl

private theorem checkGroupKey_sound {Γ : TermContext} {raw checked index root}
    (success : checkGroupKey Γ raw index root = .ok checked) :
    GroupKeyWellFormed Γ raw checked ∧ checked.sourceOrdinal = index := by
  cases raw with
  | mk rawAttr rawBand =>
    simp only [checkGroupKey] at success
    split at success
    · contradiction
    · rename_i attr found
      have name := checkedAttributeLookup_name Γ.declarations Γ.current found
      split at success <;> rename_i shapeEq
      · cases rawBand with
        | none => simp [modelError] at success
        | some width =>
          simp only at success
          split at success
          · rename_i positive
            have same := Except.ok.inj success
            have attrSame := congrArg CheckedGroupKey.attr same
            exact ⟨.mk (by simpa [← attrSame] using name)
              (congrArg CheckedGroupKey.band same).symm,
              (congrArg CheckedGroupKey.sourceOrdinal same).symm⟩
          · simp [modelError] at success
      · cases rawBand with
        | none =>
          simp only at success
          have same := Except.ok.inj success
          have attrSame := congrArg CheckedGroupKey.attr same
          exact ⟨.mk (by simpa [← attrSame] using name)
            (congrArg CheckedGroupKey.band same).symm,
            (congrArg CheckedGroupKey.sourceOrdinal same).symm⟩
        | some width => simp [modelError] at success
      · cases rawBand with
        | none =>
          simp only at success
          have same := Except.ok.inj success
          have attrSame := congrArg CheckedGroupKey.attr same
          exact ⟨.mk (by simpa [← attrSame] using name)
            (congrArg CheckedGroupKey.band same).symm,
            (congrArg CheckedGroupKey.sourceOrdinal same).symm⟩
        | some width => simp [modelError] at success
      · simp [modelError] at success

private theorem checkGroupKey_complete {Γ : TermContext} {raw checked}
    (typed : GroupKeyWellFormed Γ raw checked)
    (root : List ModelCheckPathSegment) :
    checkGroupKey Γ raw checked.sourceOrdinal root = .ok checked := by
  cases checked with
  | mk sourceOrdinal attr band valid =>
    cases raw with
    | mk rawAttr rawBand =>
      cases typed with
      | mk name bandEq =>
        simp only at name bandEq
        subst rawAttr
        subst rawBand
        simp only [checkGroupKey]
        rw [Γ.model.schemaFor Γ.current |>.lookupAttribute_name_self attr]
        simp only
        split <;> rename_i shapeEq
        · cases band with
          | none => simp [GroupBandValid, shapeEq] at valid
          | some width =>
            simp only
            have positive : 0 < width := by simpa [GroupBandValid, shapeEq] using valid
            rw [dif_pos positive]
            congr
        · cases band with
          | none => simp only; congr
          | some width => simp [GroupBandValid, shapeEq] at valid
        · cases band with
          | none => simp only; congr
          | some width => simp [GroupBandValid, shapeEq] at valid
        · simp [GroupBandValid, shapeEq] at valid

private theorem checkGroupKeysAux_sound {Γ : TermContext} {root index raws checked}
    (success : checkGroupKeysAux Γ root index raws = .ok checked) :
    List.Forall₂ (GroupKeyWellFormed Γ) raws checked ∧
      checked.map CheckedGroupKey.sourceOrdinal = List.range' index raws.length := by
  induction raws generalizing index checked with
  | nil =>
      cases success
      exact ⟨.nil, rfl⟩
  | cons raw raws ih =>
      simp only [checkGroupKeysAux, Bind.bind, Except.bind] at success
      split at success
      · contradiction
      · rename_i head headOk
        split at success
        · contradiction
        · rename_i tail tailOk
          cases success
          have rest := ih tailOk
          have headTyped := checkGroupKey_sound headOk
          exact ⟨.cons headTyped.1 rest.1, by
            simp [headTyped.2, rest.2, List.range'_succ]⟩

private theorem checkGroupKeysAux_complete {Γ : TermContext} {root index raws checked}
    (typed : List.Forall₂ (GroupKeyWellFormed Γ) raws checked)
    (order : checked.map CheckedGroupKey.sourceOrdinal = List.range' index raws.length) :
    checkGroupKeysAux Γ root index raws = .ok checked := by
  induction typed generalizing index with
  | nil => rfl
  | cons head tail ih =>
      rename_i raw checked raws checkeds
      simp only [List.map_cons, List.length_cons, List.range'_succ] at order
      have headOrder := List.cons.inj order
      have ordinal : checked.sourceOrdinal = index := headOrder.1
      simp only [checkGroupKeysAux, Bind.bind, Except.bind]
      rw [← ordinal]
      rw [checkGroupKey_complete (root := root) head]
      simp only
      have tailOrder : List.map CheckedGroupKey.sourceOrdinal checkeds =
          List.range' (checked.sourceOrdinal + 1) raws.length := by
        simpa [ordinal] using headOrder.2
      rw [ih tailOrder]
      rfl

private theorem checkGroupedFilter_sound {Γ : TermContext} {raw checked root}
    (success : checkGroupedFilter Γ raw root = .ok checked) :
    OptionalRel (ExprChecks Γ (.table Γ.current) .bool) raw checked ∧
      AggregateFreeFilter raw := by
  cases raw with
  | none => cases success; exact ⟨.none, trivial⟩
  | some raw =>
      simp only [checkGroupedFilter] at success
      split at success
      · simp [modelError] at success
      · rename_i aggregateFree
        have free : rawExprContainsAggregate raw = false :=
          Bool.eq_false_of_not_eq_true aggregateFree
        simp only [Bind.bind, Except.bind] at success
        split at success
        · contradiction
        · rename_i value valueOk
          have rawOk := liftTerm_ok_iff.mp valueOk
          cases success
          exact ⟨.some (checkExpr_sound rawOk), free⟩

private theorem checkGroupedFilter_complete {Γ : TermContext} {raw checked}
    (typed : OptionalRel (ExprChecks Γ (.table Γ.current) .bool) raw checked)
    (aggregateFree : AggregateFreeFilter raw)
    (root : List ModelCheckPathSegment) :
    checkGroupedFilter Γ raw root = .ok checked := by
  cases typed with
  | none => rfl
  | some typed =>
      simp only [AggregateFreeFilter] at aggregateFree
      simp only [checkGroupedFilter, aggregateFree, Bool.false_eq_true, ↓reduceIte,
        Bind.bind, Except.bind]
      rw [liftTerm_ok_iff.mpr
        (checkExpr_complete (path := root ++ [ModelCheckPathSegment.viewFilter]) typed .bool)]
      rfl

private theorem checkGroupedView_sound {ctx : DeclarationContext}
    {box : BoxId ctx.modelSchema.catalog} {raw checked index root}
    (bound : index < (ctx.groupedViews box).length)
    (success : checkGroupedView ctx box raw index root = .ok checked) :
    GroupedViewWellFormed raw checked ∧ checked.sourceOrdinal = index := by
  simp only [checkGroupedView, Bind.bind, Except.bind] at success
  split at success
  · contradiction
  · rename_i target found
    split at success
    · rename_i keyCount
      split at success
      · contradiction
      · rename_i keys keysOk
        split at success
        · contradiction
        · rename_i filter filterOk
          split at success
          · rename_i checkedCount
            cases success
            have keysTyped := checkGroupKeysAux_sound keysOk
            have filterTyped := checkGroupedFilter_sound filterOk
            exact ⟨.mk bound rfl (checkedTableLookup_name ctx box found)
              keysTyped.1 (by simpa only [List.range_eq_range'] using keysTyped.2) keyCount
              filterTyped.1 filterTyped.2, rfl⟩
          · simp [modelError] at success
    · simp [modelError] at success

private theorem checkGroupedView_complete {ctx : DeclarationContext}
    {box : BoxId ctx.modelSchema.catalog} {raw checked}
    (typed : GroupedViewWellFormed raw checked)
    (root : List ModelCheckPathSegment) :
    checkGroupedView ctx box raw checked.sourceOrdinal root = .ok checked := by
  cases typed with
  | mk ordinal name target keys keyOrder keyCount filter aggregateFree =>
    cases raw with
    | mk rawName rawTable rawFilter rawKeys =>
      simp only at name target keys keyOrder keyCount filter aggregateFree
      simp only [checkGroupedView, Bind.bind, Except.bind]
      rw [← target, ctx.modelSchema.catalog.lookupTable_name_self]
      simp only
      rw [dif_pos keyCount]
      rw [checkGroupKeysAux_complete keys
        (by simpa only [List.range_eq_range'] using keyOrder)]
      simp only
      rw [checkGroupedFilter_complete filter aggregateFree root]
      simp only
      rw [dif_pos (by simpa using checked.keyCount)]
      cases name
      rfl

private theorem get_eq_head_of_append {α : Type} {source prior tail : List α}
    {head : α} {index : Nat} (sourceEq : source = prior ++ head :: tail)
    (indexEq : index = prior.length) (bound : index < source.length) :
    source.get ⟨index, bound⟩ = head := by
  have getSome : source.get? index = some head := by
    rw [sourceEq, indexEq]
    apply List.get?_eq_some.mpr
    refine ⟨by simp, ?_⟩
    rw [List.get_eq_getElem]
    calc
      (prior ++ head :: tail)[prior.length] =
          (head :: tail)[prior.length - prior.length] :=
        List.getElem_append_right (le_refl prior.length)
      _ = head := by simp
  obtain ⟨otherBound, exact⟩ := List.get?_eq_some.mp getSome
  simpa using exact

private theorem checkTransitionsAux_sound {ctx : DeclarationContext}
    {box : BoxId ctx.modelSchema.catalog} {boxIndex index raws checked}
    (prior : List IR.Transition)
    (source : ctx.transitions box = prior ++ raws)
    (indexEq : index = prior.length)
    (success : checkTransitionsAux ctx box boxIndex index raws = .ok checked) :
    List.Forall₂ TransitionPackWellFormed raws checked ∧
      checked.map (fun entry => entry.checked.sourceOrdinal) =
        List.range' index raws.length := by
  induction raws generalizing prior index checked with
  | nil => cases success; exact ⟨.nil, rfl⟩
  | cons raw raws ih =>
      have bound : index < (ctx.transitions box).length := by
        rw [source, indexEq]
        simp
      have rawAt : ctx.transitionAt box ⟨index, bound⟩ = raw := by
        exact get_eq_head_of_append source indexEq bound
      simp only [checkTransitionsAux, Bind.bind, Except.bind] at success
      rw [dif_pos bound] at success
      split at success
      · contradiction
      · rename_i terms termsOk
        split at success
        · contradiction
        · rename_i tail tailOk
          cases success
          have sourceTail : ctx.transitions box = (prior ++ [raw]) ++ raws := by
            simpa [List.append_assoc] using source
          have rest := ih (prior := prior ++ [raw]) (index := index + 1)
            sourceTail (by simp [indexEq]) tailOk
          have targetName := ctx.checkedTransition_target_name box ⟨index, bound⟩
          exact ⟨.cons (.mk bound rawAt.symm rfl rfl
              (by simpa [rawAt] using targetName)
              (checkTransitionTerms_sound (liftTerm_ok_iff.mp termsOk))) rest.1,
            by simp [rest.2, List.range'_succ]⟩

private theorem CheckedTransition.heq_mk {left right : TermContext}
    {leftOrdinal rightOrdinal : Nat} {leftName rightName : String}
    {leftTerms : TransitionTerms left.model left.current left.inputs}
    {rightTerms : TransitionTerms right.model right.current right.inputs}
    (context : left = right) (ordinal : leftOrdinal = rightOrdinal)
    (name : leftName = rightName) (terms : HEq leftTerms rightTerms) :
    HEq (CheckedTransition.mk (Γ := left) leftOrdinal leftName leftTerms)
      (CheckedTransition.mk (Γ := right) rightOrdinal rightName rightTerms) := by
  cases context
  cases ordinal
  cases name
  cases terms
  rfl

private theorem CheckedTransitionPack.eq_of_heq {ctx : DeclarationContext}
    {box : BoxId ctx.modelSchema.catalog} {left right : CheckedTransitionPack ctx box}
    (target : left.target = right.target) (checked : HEq left.checked right.checked) :
    left = right := by
  cases left
  cases right
  simp only at target
  cases target
  simp only at checked
  cases checked
  rfl

private theorem TransitionWellTyped.cast_target {ctx : DeclarationContext}
    {box : BoxId ctx.modelSchema.catalog} {left right : TableId ctx.modelSchema.catalog box}
    {raw : IR.Transition}
    {terms : TransitionTerms ctx.modelSchema ⟨box, left⟩
      (TermContext.inputs ⟨ctx, box, left⟩)}
    (same : left = right)
    (typed : TransitionWellTyped ⟨ctx, box, left⟩ raw terms) :
    TransitionWellTyped ⟨ctx, box, right⟩ raw
      (Eq.mp (congrArg (fun target =>
        TransitionTerms ctx.modelSchema ⟨box, target⟩
          (TermContext.inputs ⟨ctx, box, target⟩)) same) terms) := by
  cases same
  exact typed

private theorem checkTransitionsAux_complete {ctx : DeclarationContext}
    {box : BoxId ctx.modelSchema.catalog} {boxIndex index : Nat}
    {raws : List IR.Transition} {expected : List (CheckedTransitionPack ctx box)}
    (typed : List.Forall₂ (TransitionPackWellFormed (ctx := ctx) (box := box))
      raws expected)
    (order : expected.map (fun entry => entry.checked.sourceOrdinal) =
      List.range' index raws.length) :
    checkTransitionsAux ctx box boxIndex index raws = .ok expected := by
  induction typed generalizing index with
  | nil => rfl
  | cons head tail ih =>
      rename_i raw checked raws checkeds
      simp only [List.map_cons, List.length_cons, List.range'_succ] at order
      have parts := List.cons.inj order
      cases checked with
      | mk checkedTarget checkedTransition =>
        cases head with
        | mk ordinal source targetExact name target terms =>
          have indexEq : checkedTransition.sourceOrdinal = index := parts.1
          have bound : index < (ctx.transitions box).length := by simpa [← indexEq] using ordinal
          have finEq : (⟨index, bound⟩ : Fin (ctx.transitions box).length) =
              ⟨checkedTransition.sourceOrdinal, ordinal⟩ := Fin.ext indexEq.symm
          have targetAtIndex : checkedTarget =
              ctx.resolveTransitionTarget box ⟨index, bound⟩ :=
            targetExact.trans (congrArg (ctx.resolveTransitionTarget box) finEq.symm)
          let actualTerms := Eq.mp (congrArg (fun target =>
            TransitionTerms ctx.modelSchema ⟨box, target⟩
              (TermContext.inputs ⟨ctx, box, target⟩)) targetAtIndex) checkedTransition.terms
          have actualTyped := TransitionWellTyped.cast_target targetAtIndex terms
          have termsOk := checkTransitionTerms_complete
            (path := [.model, .box boxIndex, .transition index]) actualTyped
          let actualHead : CheckedTransitionPack ctx box :=
            ⟨ctx.resolveTransitionTarget box ⟨index, bound⟩,
              ⟨index, raw.name, actualTerms⟩⟩
          have headEq : actualHead = ⟨checkedTarget, checkedTransition⟩ := by
            apply CheckedTransitionPack.eq_of_heq targetAtIndex.symm
            cases checkedTransition with
            | mk expectedOrdinal expectedName expectedTerms =>
              simp only at indexEq name targetAtIndex actualTerms actualHead ⊢
              cases indexEq
              cases name
              exact CheckedTransition.heq_mk
                (congrArg (fun target : TableId ctx.modelSchema.catalog box =>
                  (⟨ctx, box, target⟩ : TermContext)) targetAtIndex.symm)
                rfl rfl (cast_heq _ _)
          have tailOk := ih (index := index + 1) (by
            simpa [indexEq] using parts.2)
          simp only [checkTransitionsAux, Bind.bind, Except.bind]
          rw [dif_pos bound]
          rw [liftTerm_ok_iff.mpr termsOk]
          simp only
          rw [tailOk]
          simp only
          change Except.ok (actualHead :: checkeds) = _
          rw [headEq]

private theorem checkSummary_sound {ctx : DeclarationContext} {raw checked index}
    (bound : index < ctx.summaries.length)
    (success : checkSummary ctx raw index = .ok checked) :
    SummaryWellFormed raw checked ∧ checked.sourceOrdinal = index := by
  simp only [checkSummary] at success
  split at success
  · simp [modelError] at success
  · rename_i box boxFound
    split at success
    · simp [modelError] at success
    · rename_i viewIndex viewFound
      split at success
      · rename_i viewBound
        have boxName := checkedBoxLookup_name ctx boxFound
        have viewName : ((ctx.views box).get ⟨viewIndex, viewBound⟩).name = raw.view := by
          obtain ⟨otherBound, matched, first⟩ :=
            List.findIdx?_eq_some_iff_getElem.mp viewFound
          simpa [List.get_eq_getElem, beq_iff_eq] using matched
        cases success
        exact ⟨.mk bound rfl boxName viewName viewFound rfl, rfl⟩
      · simp [modelError] at success

private theorem checkSummary_complete {ctx : DeclarationContext} {raw checked}
    (typed : SummaryWellFormed raw checked) :
    checkSummary ctx raw checked.sourceOrdinal = .ok checked := by
  cases typed with
  | mk ordinal name box view viewFound reduce =>
    cases raw with
    | mk rawName rawBox rawView rawReduce =>
      simp only at name box view viewFound reduce
      simp only [checkSummary]
      rw [← box, ctx.modelSchema.catalog.lookupBox_name_self]
      simp only
      rw [viewFound]
      simp only
      rw [dif_pos checked.viewOrdinal.isLt]
      cases name
      cases reduce
      rfl

private theorem checkSummariesAux_sound {ctx : DeclarationContext}
    {index raws checked}
    (bound : index + raws.length ≤ ctx.summaries.length)
    (success : checkSummariesAux ctx index raws = .ok checked) :
    List.Forall₂ (SummaryWellFormed (ctx := ctx)) raws checked ∧
      checked.map CheckedSummaryDecl.sourceOrdinal = List.range' index raws.length := by
  induction raws generalizing index checked with
  | nil => cases success; exact ⟨.nil, rfl⟩
  | cons raw raws ih =>
      simp only [checkSummariesAux, Bind.bind, Except.bind] at success
      split at success
      · contradiction
      · rename_i head headOk
        split at success
        · contradiction
        · rename_i tail tailOk
          cases success
          have headTyped := checkSummary_sound (by simp at bound; omega) headOk
          have rest := ih (index := index + 1) (by simp at bound ⊢; omega) tailOk
          exact ⟨.cons headTyped.1 rest.1,
            by simp [headTyped.2, rest.2, List.range'_succ]⟩

private theorem checkSummariesAux_complete {ctx : DeclarationContext}
    {index raws checked}
    (typed : List.Forall₂ (SummaryWellFormed (ctx := ctx)) raws checked)
    (order : checked.map CheckedSummaryDecl.sourceOrdinal = List.range' index raws.length) :
    checkSummariesAux ctx index raws = .ok checked := by
  induction typed generalizing index with
  | nil => rfl
  | cons head tail ih =>
      rename_i raw checked raws checkeds
      simp only [List.map_cons, List.length_cons, List.range'_succ] at order
      have parts := List.cons.inj order
      cases parts.1
      simp only [checkSummariesAux, Bind.bind, Except.bind]
      rw [checkSummary_complete head]
      simp only
      rw [ih (by simpa using parts.2)]
      rfl

private theorem checkViewsAux_sound {ctx : DeclarationContext}
    {box : BoxId ctx.modelSchema.catalog} {boxIndex index raws checked}
    (bound : index + raws.length ≤ (ctx.views box).length)
    (success : checkViewsAux ctx box boxIndex index raws = .ok checked) :
    List.Forall₂ ViewWellFormed raws checked ∧
      checked.map CheckedViewDecl.sourceOrdinal = List.range' index raws.length := by
  induction raws generalizing index checked with
  | nil => cases success; exact ⟨.nil, rfl⟩
  | cons raw raws ih =>
      simp only [checkViewsAux, Bind.bind, Except.bind] at success
      split at success
      · contradiction
      · rename_i head headOk
        split at success
        · contradiction
        · rename_i tail tailOk
          cases success
          have headTyped := checkView_sound (by simp at bound; omega) headOk
          have rest := ih (index := index + 1) (by simp at bound ⊢; omega) tailOk
          exact ⟨.cons headTyped.1 rest.1,
            by simp [headTyped.2, rest.2, List.range'_succ]⟩

private theorem checkViewsAux_complete {ctx : DeclarationContext}
    {box : BoxId ctx.modelSchema.catalog} {boxIndex index raws checked}
    (typed : List.Forall₂ ViewWellFormed raws checked)
    (order : checked.map CheckedViewDecl.sourceOrdinal = List.range' index raws.length) :
    checkViewsAux ctx box boxIndex index raws = .ok checked := by
  induction typed generalizing index with
  | nil => rfl
  | cons head tail ih =>
      rename_i raw checked raws checkeds
      simp only [List.map_cons, List.length_cons, List.range'_succ] at order
      have parts := List.cons.inj order
      simp only [checkViewsAux, Bind.bind, Except.bind]
      rw [← parts.1]
      rw [checkView_complete (root := [.model, .box boxIndex, .view checked.sourceOrdinal]) head]
      simp only
      have tailOrder : List.map CheckedViewDecl.sourceOrdinal checkeds =
          List.range' (checked.sourceOrdinal + 1) raws.length := by
        simpa [parts.1] using parts.2
      rw [ih tailOrder]
      rfl

private theorem checkGroupedViewsAux_sound {ctx : DeclarationContext}
    {box : BoxId ctx.modelSchema.catalog} {boxIndex index raws checked}
    (bound : index + raws.length ≤ (ctx.groupedViews box).length)
    (success : checkGroupedViewsAux ctx box boxIndex index raws = .ok checked) :
    List.Forall₂ GroupedViewWellFormed raws checked ∧
      checked.map CheckedGroupedViewDecl.sourceOrdinal = List.range' index raws.length := by
  induction raws generalizing index checked with
  | nil => cases success; exact ⟨.nil, rfl⟩
  | cons raw raws ih =>
      simp only [checkGroupedViewsAux, Bind.bind, Except.bind] at success
      split at success
      · contradiction
      · rename_i head headOk
        split at success
        · contradiction
        · rename_i tail tailOk
          cases success
          have headTyped := checkGroupedView_sound (by simp at bound; omega) headOk
          have rest := ih (index := index + 1) (by simp at bound ⊢; omega) tailOk
          exact ⟨.cons headTyped.1 rest.1,
            by simp [headTyped.2, rest.2, List.range'_succ]⟩

private theorem checkGroupedViewsAux_complete {ctx : DeclarationContext}
    {box : BoxId ctx.modelSchema.catalog} {boxIndex index raws checked}
    (typed : List.Forall₂ GroupedViewWellFormed raws checked)
    (order : checked.map CheckedGroupedViewDecl.sourceOrdinal =
      List.range' index raws.length) :
    checkGroupedViewsAux ctx box boxIndex index raws = .ok checked := by
  induction typed generalizing index with
  | nil => rfl
  | cons head tail ih =>
      rename_i raw checked raws checkeds
      simp only [List.map_cons, List.length_cons, List.range'_succ] at order
      have parts := List.cons.inj order
      simp only [checkGroupedViewsAux, Bind.bind, Except.bind]
      rw [← parts.1]
      rw [checkGroupedView_complete
        (root := [.model, .box boxIndex, .groupedView checked.sourceOrdinal]) head]
      simp only
      have tailOrder : List.map CheckedGroupedViewDecl.sourceOrdinal checkeds =
          List.range' (checked.sourceOrdinal + 1) raws.length := by
        simpa [parts.1] using parts.2
      rw [ih tailOrder]
      rfl

private theorem DeclarationContext.eraseTables_exact (ctx : DeclarationContext)
    (box : BoxId ctx.modelSchema.catalog) :
    ctx.modelSchema.eraseTables box = (sourceBox ctx.source ctx.wellFormed box).tables := by
  apply List.ext_get (by
    simp [ModelSchema.eraseTables, ctx.catalogTables_length box])
  intro index leftBound rightBound
  simp only [ModelSchema.eraseTables, List.get_ofFn]
  simpa [sourceTable] using
    ctx.modelSchema_eraseTable_exact
      ⟨box, ⟨index, by simpa [ctx.catalogTables_length box] using leftBound⟩⟩

private theorem checkBox_sound {ctx : DeclarationContext}
    {box : BoxId ctx.modelSchema.catalog} {boxIndex checked}
    (ordinal : boxIndex = box.ordinal.val)
    (success : checkBox ctx box boxIndex = .ok checked) :
    BoxTermsWellFormed (sourceBox ctx.source ctx.wellFormed box) checked := by
  simp only [checkBox, Bind.bind, Except.bind] at success
  split at success
  · contradiction
  · rename_i transitions transitionsOk
    split at success
    · contradiction
    · rename_i outputs outputsOk
      split at success
      · contradiction
      · rename_i views viewsOk
        split at success
        · contradiction
        · rename_i grouped groupedOk
          cases success
          have transitionTyped := checkTransitionsAux_sound [] rfl rfl transitionsOk
          have outputTyped := checkOutputsAux_sound [] rfl rfl outputsOk
          have viewTyped := checkViewsAux_sound (by simp) viewsOk
          have groupedTyped := checkGroupedViewsAux_sound (by simp) groupedOk
          exact .mk rfl ordinal rfl (ctx.eraseTables_exact box).symm rfl
            transitionTyped.1 (by simpa only [List.range_eq_range'] using transitionTyped.2)
            outputTyped.1 (by simpa only [List.range_eq_range'] using outputTyped.2)
            viewTyped.1 (by simpa only [List.range_eq_range'] using viewTyped.2)
            groupedTyped.1 (by simpa only [List.range_eq_range'] using groupedTyped.2)

private theorem checkBox_complete {ctx : DeclarationContext}
    {box : BoxId ctx.modelSchema.catalog} {boxIndex raw expected}
    (typed : BoxTermsWellFormed (ctx := ctx) (box := box) raw expected)
    (ordinal : expected.sourceOrdinal = boxIndex) :
    checkBox ctx box boxIndex = .ok expected := by
  cases typed with
  | mk sourceExact expectedOrdinal name tables inputs transitions transitionOrder outputs outputOrder
      views viewOrder grouped groupedOrder =>
    subst raw
    have transitionsOk :=
      checkTransitionsAux_complete (boxIndex := boxIndex) transitions
        (by simpa only [List.range_eq_range'] using transitionOrder)
    have outputsOk := checkOutputsAux_complete (boxIndex := boxIndex) outputs
      (by simpa only [List.range_eq_range'] using outputOrder)
    have viewsOk := checkViewsAux_complete (boxIndex := boxIndex) views
      (by simpa only [List.range_eq_range'] using viewOrder)
    have groupedOk := checkGroupedViewsAux_complete (boxIndex := boxIndex) grouped
      (by simpa only [List.range_eq_range'] using groupedOrder)
    have transitionsOk' : checkTransitionsAux ctx box boxIndex 0 (ctx.transitions box) =
        .ok expected.transitions := by simpa [DeclarationContext.transitions] using transitionsOk
    have outputsOk' : checkOutputsAux ctx box boxIndex 0 (ctx.outputs box) =
        .ok expected.outputs := by simpa [DeclarationContext.outputs] using outputsOk
    have viewsOk' : checkViewsAux ctx box boxIndex 0 (ctx.views box) =
        .ok expected.views := by simpa [DeclarationContext.views] using viewsOk
    have groupedOk' : checkGroupedViewsAux ctx box boxIndex 0 (ctx.groupedViews box) =
        .ok expected.groupedViews := by simpa [DeclarationContext.groupedViews] using groupedOk
    simp only [checkBox, Bind.bind, Except.bind]
    rw [transitionsOk', outputsOk', viewsOk', groupedOk']
    cases ordinal
    rfl

private theorem checkBoxesAux_sound {ctx : DeclarationContext}
    {index ordinals checked}
    (order : ordinals.map Fin.val = List.range' index ordinals.length)
    (success : checkBoxesAux ctx index ordinals = .ok checked) :
    List.Forall₂ (BoxPackWellFormed (ctx := ctx))
      (ordinals.map fun ordinal => sourceBox ctx.source ctx.wellFormed ⟨ordinal⟩) checked ∧
      checked.map (fun entry => entry.box.ordinal.val) = List.range' index ordinals.length := by
  induction ordinals generalizing index checked with
  | nil => cases success; exact ⟨.nil, rfl⟩
  | cons ordinal ordinals ih =>
      simp only [List.map_cons, List.length_cons, List.range'_succ] at order
      have parts := List.cons.inj order
      simp only [checkBoxesAux, Bind.bind, Except.bind] at success
      split at success
      · contradiction
      · rename_i head headOk
        split at success
        · contradiction
        · rename_i tail tailOk
          cases success
          have headTyped := checkBox_sound parts.1.symm headOk
          have rest := ih (index := index + 1) parts.2 tailOk
          exact ⟨.cons (.mk headTyped) rest.1,
            by simp [parts.1, rest.2, List.range'_succ]⟩

private theorem checkBoxesAux_complete {ctx : DeclarationContext}
    {index raws expected}
    (typed : List.Forall₂ (BoxPackWellFormed (ctx := ctx)) raws expected)
    (order : expected.map (fun entry => entry.box.ordinal.val) =
      List.range' index raws.length)
    (ordinals : List (Fin ctx.modelSchema.catalog.boxes.entries.length))
    (ordinalPacks : ordinals = expected.map (fun entry => entry.box.ordinal)) :
    checkBoxesAux ctx index ordinals = .ok expected := by
  subst ordinals
  induction typed generalizing index with
  | nil => rfl
  | cons head tail ih =>
      rename_i raw checked raws checkeds
      simp only [List.map_cons, List.length_cons, List.range'_succ] at order
      have parts := List.cons.inj order
      cases head with
      | mk boxTyped =>
        have checkedOrdinal : checked.checked.sourceOrdinal = index := by
          cases boxTyped
          exact ‹checked.checked.sourceOrdinal = checked.box.ordinal.val›.trans parts.1
        have boxOk := checkBox_complete boxTyped checkedOrdinal
        have tailOk := ih (index := index + 1) parts.2
        simp only [checkBoxesAux, Bind.bind, Except.bind]
        rw [boxOk]
        simp only
        rw [tailOk]
        rfl

private theorem canonicalBoxOrdinals (ctx : DeclarationContext) :
    (List.ofFn id : List (Fin ctx.modelSchema.catalog.boxes.entries.length)).map Fin.val =
      List.range ctx.source.boxes.length := by
  apply List.ext_get (by simp [ctx.catalogBoxes_length])
  intro index leftBound rightBound
  simp [List.get_ofFn]

private theorem sourceBoxes_from_ordinals (ctx : DeclarationContext) :
    (List.ofFn id : List (Fin ctx.modelSchema.catalog.boxes.entries.length)).map
      (fun ordinal => sourceBox ctx.source ctx.wellFormed ⟨ordinal⟩) =
      ctx.source.boxes := by
  apply List.ext_get (by simp [ctx.catalogBoxes_length])
  intro index leftBound rightBound
  simp [sourceBox]

private theorem checkBoxes_sound {ctx : DeclarationContext} {checked}
    (success : checkBoxes ctx = .ok checked) :
    List.Forall₂ (BoxPackWellFormed (ctx := ctx)) ctx.source.boxes checked ∧
      checked.map (fun entry => entry.box.ordinal.val) = List.range ctx.source.boxes.length := by
  have result := checkBoxesAux_sound
    (index := 0) (ordinals := (List.ofFn id :
      List (Fin ctx.modelSchema.catalog.boxes.entries.length)))
    (by simpa only [List.range_eq_range', List.length_ofFn,
      ctx.catalogBoxes_length] using canonicalBoxOrdinals ctx) success
  rw [sourceBoxes_from_ordinals ctx] at result
  exact ⟨result.1, by simpa only [List.range_eq_range', List.length_ofFn,
    ctx.catalogBoxes_length] using result.2⟩

private theorem checkBoxes_complete {ctx : DeclarationContext}
    {raws expected}
    (source : raws = ctx.source.boxes)
    (typed : List.Forall₂ (BoxPackWellFormed (ctx := ctx)) raws expected)
    (order : expected.map (fun entry => entry.box.ordinal.val) = List.range raws.length) :
    checkBoxes ctx = .ok expected := by
  have lengths := List.Forall₂.length_eq typed
  have valuesEq :
      (List.ofFn id : List (Fin ctx.modelSchema.catalog.boxes.entries.length)).map Fin.val =
        expected.map (fun entry => entry.box.ordinal.val) := by
    calc
      _ = List.range ctx.source.boxes.length := canonicalBoxOrdinals ctx
      _ = List.range raws.length := by rw [source]
      _ = _ := order.symm
  have ordinalListEq :
      (List.ofFn id : List (Fin ctx.modelSchema.catalog.boxes.entries.length)) =
        expected.map (fun entry => entry.box.ordinal) := by
    apply List.ext_get (by
      simpa only [List.length_ofFn] using (show
        ctx.modelSchema.catalog.boxes.entries.length =
          (expected.map fun entry => entry.box.ordinal).length from by
        calc
          ctx.modelSchema.catalog.boxes.entries.length = ctx.source.boxes.length :=
            ctx.catalogBoxes_length
          _ = raws.length := (congrArg List.length source).symm
          _ = expected.length := lengths
          _ = (expected.map fun entry => entry.box.ordinal).length := by simp))
    intro index leftBound rightBound
    apply Fin.ext
    have point := congrArg (fun values : List Nat => values.get? index) valuesEq
    simp only [List.get?_map] at point
    have expectedBound : index < expected.length := by simpa using rightBound
    rw [List.get?_eq_get leftBound, List.get?_eq_get expectedBound] at point
    simpa [List.get_map] using point
  exact checkBoxesAux_complete typed
    (by simpa only [List.range_eq_range'] using order)
    (List.ofFn id) ordinalListEq

/-! ## Judgment erasure -/

@[simp] theorem OptionalRel.map_exact {α β : Type} {relation : α → β → Prop}
    {erase : β → α} (exact : ∀ {left right}, relation left right → erase right = left)
    {left : Option α} {right : Option β} (related : OptionalRel relation left right) :
    right.map erase = left := by
  cases related with
  | none => rfl
  | some related => simp [exact related]

@[simp] theorem OutputFieldWellFormed.erase_exact {ctx : DeclarationContext}
    {box : BoxId ctx.modelSchema.catalog} {target : TableId ctx.modelSchema.catalog box}
    {portSchema : BoxPortSchema ctx.modelSchema.catalog box} {raw checked}
    (wellFormed : OutputFieldWellFormed (ctx := ctx) (box := box)
      (target := target) (portSchema := portSchema) raw checked) :
    checked.erase = raw := by
  cases wellFormed with
  | mk ordinal name schemaName _numeric _expectedOrigin _expected operation filter =>
      cases raw with
      | mk rawName rawOp rawFilter =>
          have filterExact := OptionalRel.map_exact
            (fun typed => ExprChecks.erase_exact typed) filter
          simp only [CheckedOutputField.erase, IR.OutputField.mk.injEq]
          exact ⟨name, AggOpSynthesizes.erase_exact operation, filterExact⟩

@[simp] theorem ViewValueWellFormed.erase_exact {Γ : TermContext} {reduce raw checked}
    (wellFormed : ViewValueWellFormed Γ reduce raw checked) :
    checked.erase = (reduce, raw) := by
  cases wellFormed with
  | count => rfl
  | sumInt typed | sumReal typed | minInt typed | minReal typed
  | maxInt typed | maxReal typed =>
      simp only [CheckedViewValue.erase, Prod.mk.injEq, true_and, Option.some.injEq]
      exact ExprSynthesizes.erase_exact typed

@[simp] theorem GroupKeyWellFormed.erase_exact {Γ : TermContext} {raw checked}
    (wellFormed : GroupKeyWellFormed Γ raw checked) : checked.erase = raw := by
  cases wellFormed
  simp [CheckedGroupKey.erase, *]

theorem forall₂_map_right_exact {α β : Type} {relation : α → β → Prop}
    {erase : β → α} (exact : ∀ {left right}, relation left right → erase right = left)
    {left : List α} {right : List β} (related : List.Forall₂ relation left right) :
    right.map erase = left := by
  induction related with
  | nil => rfl
  | cons head tail ih => simp [exact head, ih]

@[simp] theorem TransitionPackWellFormed.erase_exact {ctx : DeclarationContext}
    {box : BoxId ctx.modelSchema.catalog} {raw checked}
    (wellFormed : TransitionPackWellFormed (ctx := ctx) (box := box) raw checked) :
    checked.erase = raw := by
  cases wellFormed with
  | mk ordinal _source _targetExact name target terms =>
      cases terms with
      | mk guard hazard effects claims unique covered =>
          have effectsExact := forall₂_map_right_exact
            (fun typed => EffectWellTyped.erase_exact typed) effects
          have claimsExact := forall₂_map_right_exact
            (fun typed => ClaimWellTyped.erase_exact typed) claims
          cases raw with
          | mk rawName rawTable rawGuard rawHazard rawEffects rawClaims =>
              simp only [CheckedTransitionPack.erase, CheckedTransition.erase,
                IR.Transition.mk.injEq]
              exact ⟨name, target, ExprChecks.erase_exact guard,
                ExprChecks.erase_exact hazard, effectsExact, claimsExact⟩

@[simp] theorem OutputWellFormed.erase_exact {ctx : DeclarationContext}
    {box : BoxId ctx.modelSchema.catalog} {raw checked}
    (wellFormed : OutputWellFormed (ctx := ctx) (box := box) raw checked) :
    checked.erase = raw := by
  cases wellFormed with
  | perTable ordinal _portSchemaExact name _portName schema builder target unique schemaFieldCount fieldCount fieldOrder fieldsRelated =>
      have fieldsExact := forall₂_map_right_exact
        (fun typed => OutputFieldWellFormed.erase_exact typed) fieldsRelated
      cases raw with
      | mk rawName rawSchema rawBuilder =>
          simp only [CheckedOutputDecl.erase, IR.OutputDecl.mk.injEq]
          constructor
          · exact name
          constructor
          · exact schema
          · simpa [target, fieldsExact] using builder.symm

@[simp] theorem ViewWellFormed.erase_exact {ctx : DeclarationContext}
    {box : BoxId ctx.modelSchema.catalog} {raw checked}
    (wellFormed : ViewWellFormed (ctx := ctx) (box := box) raw checked) :
    checked.erase = raw := by
  cases wellFormed with
  | mk ordinal name target filter value =>
      have filterExact := OptionalRel.map_exact
        (fun typed => ExprChecks.erase_exact typed) filter
      have valueExact := ViewValueWellFormed.erase_exact value
      cases raw with
      | mk rawName rawTable rawFilter rawValue rawReduce =>
          simp only [CheckedViewDecl.erase, IR.ViewDecl.mk.injEq]
          exact ⟨name, target, filterExact,
            congrArg Prod.snd valueExact, congrArg Prod.fst valueExact⟩

@[simp] theorem GroupedViewWellFormed.erase_exact {ctx : DeclarationContext}
    {box : BoxId ctx.modelSchema.catalog} {raw checked}
    (wellFormed : GroupedViewWellFormed (ctx := ctx) (box := box) raw checked) :
    checked.erase = raw := by
  cases wellFormed with
  | mk ordinal name target keys keyOrder keyCount filter aggregateFree =>
      have keysExact := forall₂_map_right_exact
        (fun typed => GroupKeyWellFormed.erase_exact typed) keys
      have filterExact := OptionalRel.map_exact
        (fun typed => ExprChecks.erase_exact typed) filter
      cases raw with
      | mk rawName rawTable rawFilter rawKeys =>
          simp only [CheckedGroupedViewDecl.erase, IR.GroupedViewDecl.mk.injEq]
          exact ⟨name, target, filterExact, keysExact⟩

@[simp] theorem SummaryWellFormed.erase_exact {ctx : DeclarationContext} {raw checked}
    (wellFormed : SummaryWellFormed (ctx := ctx) raw checked) :
    checked.erase = raw := by
  cases wellFormed with
  | mk ordinal name box view _viewFound reduce =>
      cases raw with
      | mk rawName rawBox rawView rawReduce =>
          simp only [CheckedSummaryDecl.erase, IR.SummaryDecl.mk.injEq]
          exact ⟨name, box, view, reduce⟩

@[simp] theorem BoxTermsWellFormed.erase_exact {ctx : DeclarationContext}
    {box : BoxId ctx.modelSchema.catalog} {raw checked}
    (wellFormed : BoxTermsWellFormed (ctx := ctx) (box := box) raw checked) :
    checked.erase = raw := by
  cases wellFormed with
  | mk _sourceExact ordinal name tables inputs transitions transitionOrder outputs outputOrder
      views viewOrder grouped groupedOrder =>
      have transitionExact := forall₂_map_right_exact
        (fun typed => TransitionPackWellFormed.erase_exact typed) transitions
      have outputExact := forall₂_map_right_exact
        (fun typed => OutputWellFormed.erase_exact typed) outputs
      have viewExact := forall₂_map_right_exact
        (fun typed => ViewWellFormed.erase_exact typed) views
      have groupedExact := forall₂_map_right_exact
        (fun typed => GroupedViewWellFormed.erase_exact typed) grouped
      cases raw with
      | mk rawName rawTables rawTransitions rawInputs rawOutputs rawViews rawGrouped =>
          simp only [CheckedBox.erase, IR.Box.mk.injEq]
          exact ⟨name.symm, tables.symm, transitionExact, inputs.symm,
            outputExact, viewExact, groupedExact⟩

@[simp] theorem BoxPackWellFormed.erase_exact {ctx : DeclarationContext} {raw checked}
    (wellFormed : BoxPackWellFormed (ctx := ctx) raw checked) :
    checked.erase = raw := by
  cases wellFormed with
  | mk box => exact BoxTermsWellFormed.erase_exact box

/-- Structural elaboration reconstructs the related raw model exactly. -/
@[simp] theorem ModelElaborates.erase_exact {raw : IR.Model} {checked : Checked.Model}
    (elaborates : ModelElaborates raw checked) : checked.erase = raw := by
  cases elaborates with
  | mk source declarations boxTerms boxOrder summaryTerms summaryOrder wires =>
      have boxesExact := forall₂_map_right_exact
        (fun typed => BoxPackWellFormed.erase_exact typed) boxTerms
      have summariesExact := forall₂_map_right_exact
        (fun typed => SummaryWellFormed.erase_exact typed) summaryTerms
      cases checked with
      | mk ctx boxes summaries checkedWires =>
          simp only at source wires boxesExact summariesExact ⊢
          subst raw
          change
            { name := ctx.source.name, dt := ctx.source.dt,
              params := ctx.modelSchema.eraseParameters,
              boxes := boxes.map CheckedBoxPack.erase,
              wires := checkedWires,
              summaries := summaries.map CheckedSummaryDecl.erase } = ctx.source
          rw [boxesExact, summariesExact, ctx.modelSchema_eraseParameters_exact, wires]

/-- Independent model validity always has a reconstructive checked witness;
wire bytes are copied but never inspected. -/
theorem ModelWellFormed.has_checked_erasure {raw : IR.Model}
    (wellFormed : ModelWellFormed raw) :
    ∃ checked : Checked.Model, checked.erase = raw := by
  rcases wellFormed with ⟨checked, elaborates⟩
  exact ⟨checked, elaborates.erase_exact⟩

private theorem checkDeclarations_source_exact {raw : IR.Model} {ctx : DeclarationContext}
    (success : checkDeclarations raw = .ok ctx) : ctx.source = raw := by
  simp only [checkDeclarations] at success
  split at success
  · cases success
    rfl
  · contradiction

private theorem DeclarationContext.eq_of_source {left right : DeclarationContext}
    (same : left.source = right.source) : left = right := by
  cases left with
  | mk leftSource leftWellFormed =>
    cases right with
    | mk rightSource rightWellFormed =>
      simp only at same
      cases same
      congr

/-- Successful model checking yields the independent structural elaboration. -/
theorem checkModel_elaborates {raw : IR.Model} {checked : Checked.Model}
    (success : checkModel raw = .ok checked) : ModelElaborates raw checked := by
  simp only [checkModel] at success
  split at success
  · contradiction
  · rename_i ctx declarationsOk
    split at success
    · contradiction
    · rename_i boxes boxesOk
      split at success
      · contradiction
      · rename_i summaries summariesOk
        cases success
        have source := checkDeclarations_source_exact declarationsOk
        have boxesTyped := checkBoxes_sound boxesOk
        have summariesTyped := checkSummariesAux_sound (index := 0)
          (by simp [DeclarationContext.summaries]) summariesOk
        exact .mk source (source ▸ ctx.wellFormed)
          (by simpa [source] using boxesTyped.1)
          (by simpa [source] using boxesTyped.2)
          (by simpa [DeclarationContext.summaries, source] using summariesTyped.1)
          (by simpa [DeclarationContext.summaries, source,
            List.range_eq_range'] using summariesTyped.2) rfl

/-- Model checker soundness and reconstructive exact erasure. -/
theorem checkModel_sound {raw : IR.Model} {checked : Checked.Model}
    (success : checkModel raw = .ok checked) :
    ModelWellFormed raw ∧ checked.erase = raw := by
  have elaborates := checkModel_elaborates success
  exact ⟨⟨checked, elaborates⟩, elaborates.erase_exact⟩

/-- Every independent elaboration has a successful canonical checker result. -/
theorem ModelElaborates.checkModel_exists {raw : IR.Model} {checked : Checked.Model}
    (elaborates : ModelElaborates raw checked) :
    ∃ actual, checkModel raw = .ok actual := by
  cases checked with
  | mk ctx boxes summaries wires =>
    cases elaborates with
    | mk source declarations boxTerms boxOrder summaryTerms summaryOrder wireExact =>
      let canonicalCtx : DeclarationContext := ⟨raw, declarations⟩
      have contextEq : ctx = canonicalCtx := by
        apply DeclarationContext.eq_of_source
        simpa [canonicalCtx] using source
      let Package := fun c : DeclarationContext =>
        Σ canonicalBoxes : List (CheckedBoxPack c),
        Σ canonicalSummaries : List (CheckedSummaryDecl c),
          PLift (List.Forall₂ (BoxPackWellFormed (ctx := c)) raw.boxes canonicalBoxes ∧
          canonicalBoxes.map (fun entry => entry.box.ordinal.val) =
            List.range raw.boxes.length ∧
          List.Forall₂ (SummaryWellFormed (ctx := c))
            raw.summaries canonicalSummaries ∧
          canonicalSummaries.map CheckedSummaryDecl.sourceOrdinal =
            List.range raw.summaries.length)
      let original : Package ctx :=
        ⟨boxes, summaries, ⟨boxTerms, boxOrder, summaryTerms, summaryOrder⟩⟩
      let transported : Package canonicalCtx :=
        Eq.mp (congrArg Package contextEq) original
      rcases transported with ⟨canonicalBoxes, canonicalSummaries, proof⟩
      rcases proof.down with ⟨boxTerms', boxOrder', summaryTerms', summaryOrder'⟩
      have declarationsOk : checkDeclarations raw = .ok canonicalCtx := by
        simp [checkDeclarations, canonicalCtx, declarations]
      have boxesOk : checkBoxes canonicalCtx = .ok canonicalBoxes :=
        checkBoxes_complete (source := by rfl) boxTerms' boxOrder'
      have summariesOk : checkSummariesAux canonicalCtx 0 canonicalCtx.summaries =
          .ok canonicalSummaries := by
        apply checkSummariesAux_complete summaryTerms'
        simpa only [List.range_eq_range'] using summaryOrder'
      refine ⟨⟨canonicalCtx, canonicalBoxes, canonicalSummaries, raw.wires⟩, ?_⟩
      simp only [checkModel]
      rw [declarationsOk]
      simp only
      rw [boxesOk]
      simp only
      rw [summariesOk]

/-- Independent model validity is complete for the canonical checker. -/
theorem checkModel_complete {raw : IR.Model} (wellFormed : ModelWellFormed raw) :
    ∃ checked, checkModel raw = .ok checked := by
  rcases wellFormed with ⟨checked, elaborates⟩
  exact elaborates.checkModel_exists

/-- Model failure is exactly failure of the independent model judgment. -/
theorem checkModel_failure_iff (raw : IR.Model) :
    (∃ error, checkModel raw = .error error) ↔ ¬ ModelWellFormed raw := by
  constructor
  · rintro ⟨error, failed⟩ wellFormed
    obtain ⟨checked, succeeded⟩ := checkModel_complete wellFormed
    rw [failed] at succeeded
    contradiction
  · intro invalid
    cases result : checkModel raw with
    | error error => exact ⟨error, rfl⟩
    | ok checked => exact False.elim (invalid (checkModel_sound result).1)

/-! ## Structural checked-model canonicality and equivalence -/

namespace CheckedBox

def Canonical {ctx : DeclarationContext} {box : BoxId ctx.modelSchema.catalog}
    (checked : CheckedBox ctx box) : Prop :=
  checked.sourceOrdinal = box.ordinal.val ∧
  checked.transitions.map (fun entry => entry.checked.sourceOrdinal) =
    List.range checked.transitions.length ∧
  checked.outputs.map CheckedOutputDecl.sourceOrdinal = List.range checked.outputs.length ∧
  checked.views.map CheckedViewDecl.sourceOrdinal = List.range checked.views.length ∧
  checked.groupedViews.map CheckedGroupedViewDecl.sourceOrdinal =
    List.range checked.groupedViews.length

end CheckedBox

namespace Checked.Model

inductive CheckedViewValueShape where
  | count
  | sumInt (value : CheckedExprShape)
  | sumReal (value : CheckedExprShape)
  | minInt (value : CheckedExprShape)
  | minReal (value : CheckedExprShape)
  | maxInt (value : CheckedExprShape)
  | maxReal (value : CheckedExprShape)

private def viewValueShape {Γ : TermContext} :
    CheckedViewValue Γ → CheckedViewValueShape
  | .count => .count
  | .sumInt value => .sumInt (checkedExprShape (Γ := Γ) value)
  | .sumReal value => .sumReal (checkedExprShape (Γ := Γ) value)
  | .minInt value => .minInt (checkedExprShape (Γ := Γ) value)
  | .minReal value => .minReal (checkedExprShape (Γ := Γ) value)
  | .maxInt value => .maxInt (checkedExprShape (Γ := Γ) value)
  | .maxReal value => .maxReal (checkedExprShape (Γ := Γ) value)

inductive CheckedOrderingShape where
  | raceTime
  | keyReal (value : CheckedExprShape)
  | keyInt (value : CheckedExprShape)
  | keyEnum (ownerBox ownerTable attr : Nat) (value : CheckedExprShape)

private def orderingShape {Γ : TermContext}
    (claim : ResourceClaim Γ.model Γ.current Γ.inputs) : CheckedOrderingShape :=
  match claim with
  | ⟨_, _, _, _, ordering⟩ =>
      match ordering with
      | .raceTime => .raceTime
      | .key domain value =>
          match domain with
          | .real => .keyReal (checkedExprShape (Γ := Γ) value)
          | .int => .keyInt (checkedExprShape (Γ := Γ) value)
          | .enum (owner := owner) _ attr _ _ =>
              .keyEnum owner.box.ordinal.val owner.table.ordinal.val
                attr.ordinal.val (checkedExprShape (Γ := Γ) value)

private def declarationShape (ctx : DeclarationContext) :=
  (ctx.source.name, ctx.source.dt, ctx.source.params,
    ctx.source.boxes.map fun box =>
      (box.name, box.tables, box.inputs,
        box.outputs.map fun output => (output.name, output.schema),
        box.transitions.map fun transition => (transition.name, transition.table),
        box.views.map IR.ViewDecl.name,
        box.groupedViews.map IR.GroupedViewDecl.name))

/-- Intrinsic componentwise canonicality: every checked component is related by
the independent syntax-directed elaboration rules to its own reconstructive
erasure.  This mentions neither checker success nor equality of erasures. -/
def Canonical (checked : Checked.Model) : Prop :=
  ModelElaborates checked.erase checked

/-- Erasing a canonical checked model is independently model-well-formed. -/
theorem Canonical.erasure_valid {checked : Checked.Model}
    (canonical : checked.Canonical) : ModelWellFormed checked.erase :=
  ⟨checked, canonical⟩

/-- Every successful checker result satisfies structural canonicality. -/
theorem checkModel_canonical {raw : IR.Model} {checked : Checked.Model}
    (success : checkModel raw = .ok checked) : checked.Canonical := by
  have elaborates := checkModel_elaborates success
  simpa [Canonical, elaborates.erase_exact] using elaborates

/-- A proof-witness-free structural projection. This is not raw erasure: it
retains resolved owner ordinals and checked constructor shapes. -/
def structuralShape (checked : Checked.Model) :=
  (declarationShape checked.declarations,
    checked.wires,
    checked.boxes.map fun entry =>
      (entry.box.ordinal.val, entry.checked.sourceOrdinal,
        entry.checked.transitions.map fun transition =>
          (transition.target.ordinal.val, transition.checked.sourceOrdinal,
            transition.checked.name,
            checkedExprShape transition.checked.terms.guard,
            checkedExprShape transition.checked.terms.hazard,
            transition.checked.terms.effects.map fun effect =>
              (effect.destination.ordinal.val, checkedExprShape effect.value),
            transition.checked.terms.claims.map fun claim =>
              (claim.resourceTarget.box.ordinal.val,
                claim.resourceTarget.table.ordinal.val,
                checkedExprShape claim.resource, orderingShape claim)),
        entry.checked.outputs.map fun output =>
          (output.sourceOrdinal, output.target.ordinal.val, output.name,
            output.portSchema.source,
            output.fields.map fun field =>
              (field.sourceOrdinal, field.schemaAttribute.ordinal.val, field.name,
                checkedAggOpShape field.op,
                field.filter.map checkedExprShape)),
        entry.checked.views.map fun view =>
          (view.sourceOrdinal, view.target.ordinal.val, view.name,
            viewValueShape view.value,
            view.filter.map fun expr :
                Term checked.declarations.modelSchema ⟨entry.box, view.target⟩
                  (TermContext.inputs ⟨checked.declarations, entry.box, view.target⟩) .bool =>
              checkedExprShape
                (Γ := ⟨checked.declarations, entry.box, view.target⟩)
                (scope := .table ⟨entry.box, view.target⟩) expr),
        entry.checked.groupedViews.map fun view =>
          (view.sourceOrdinal, view.target.ordinal.val, view.name,
            view.keys.map fun key =>
              (key.sourceOrdinal, key.attr.ordinal.val, key.band),
            view.filter.map fun expr :
                Term checked.declarations.modelSchema ⟨entry.box, view.target⟩
                  (TermContext.inputs ⟨checked.declarations, entry.box, view.target⟩) .bool =>
              checkedExprShape
                (Γ := ⟨checked.declarations, entry.box, view.target⟩)
                (scope := .table ⟨entry.box, view.target⟩) expr)),
    checked.summaries.map fun summary =>
      (summary.sourceOrdinal, summary.box.ordinal.val,
        summary.viewOrdinal.val, summary.name, summary.reduce))

/-- Pointwise checked-model equivalence ignores proof witnesses and transparent
coercions while preserving all structural owner/order/term information above. -/
def Equivalent (left right : Checked.Model) : Prop :=
  left.structuralShape = right.structuralShape

@[refl] theorem Equivalent.refl (checked : Checked.Model) : Equivalent checked checked := rfl

@[symm] theorem Equivalent.symm {left right : Checked.Model}
    (same : Equivalent left right) : Equivalent right left := Eq.symm same

@[trans] theorem Equivalent.trans {first second third : Checked.Model}
    (left : Equivalent first second) (right : Equivalent second third) :
    Equivalent first third := Eq.trans left right

/-- Strong completeness: checking any independent elaboration returns a
structurally equivalent canonical checked model. -/
theorem checkModel_equivalent_of_elaborates {raw : IR.Model}
    {checked : Checked.Model} (elaborates : ModelElaborates raw checked) :
    ∃ actual, checkModel raw = .ok actual ∧ actual.Equivalent checked := by
  cases checked with
  | mk ctx boxes summaries wires =>
    cases elaborates with
    | mk source declarations boxTerms boxOrder summaryTerms summaryOrder wireExact =>
      let canonicalCtx : DeclarationContext := ⟨raw, declarations⟩
      have contextEq : ctx = canonicalCtx := by
        apply DeclarationContext.eq_of_source
        simpa [canonicalCtx] using source
      let Package := fun c : DeclarationContext =>
        Σ canonicalBoxes : List (CheckedBoxPack c),
        Σ canonicalSummaries : List (CheckedSummaryDecl c),
          PLift (List.Forall₂ (BoxPackWellFormed (ctx := c)) raw.boxes canonicalBoxes ∧
          canonicalBoxes.map (fun entry => entry.box.ordinal.val) =
            List.range raw.boxes.length ∧
          List.Forall₂ (SummaryWellFormed (ctx := c))
            raw.summaries canonicalSummaries ∧
          canonicalSummaries.map CheckedSummaryDecl.sourceOrdinal =
            List.range raw.summaries.length ∧
          structuralShape ⟨c, canonicalBoxes, canonicalSummaries, raw.wires⟩ =
            structuralShape ⟨ctx, boxes, summaries, wires⟩)
      have shapeOriginal :
          structuralShape ⟨ctx, boxes, summaries, raw.wires⟩ =
            structuralShape ⟨ctx, boxes, summaries, wires⟩ := by
        cases wireExact
        rfl
      let original : Package ctx :=
        ⟨boxes, summaries,
          ⟨boxTerms, boxOrder, summaryTerms, summaryOrder, shapeOriginal⟩⟩
      let transported : Package canonicalCtx :=
        Eq.mp (congrArg Package contextEq) original
      rcases transported with ⟨canonicalBoxes, canonicalSummaries, proof⟩
      rcases proof.down with ⟨boxTerms', boxOrder', summaryTerms', summaryOrder', shape⟩
      have declarationsOk : checkDeclarations raw = .ok canonicalCtx := by
        simp [checkDeclarations, canonicalCtx, declarations]
      have boxesOk : checkBoxes canonicalCtx = .ok canonicalBoxes :=
        checkBoxes_complete (source := by rfl) boxTerms' boxOrder'
      have summariesOk : checkSummariesAux canonicalCtx 0 canonicalCtx.summaries =
          .ok canonicalSummaries := by
        apply checkSummariesAux_complete summaryTerms'
        simpa only [List.range_eq_range'] using summaryOrder'
      let actual : Checked.Model :=
        ⟨canonicalCtx, canonicalBoxes, canonicalSummaries, raw.wires⟩
      refine ⟨actual, ?_, ?_⟩
      · simp only [checkModel]
        rw [declarationsOk]
        simp only
        rw [boxesOk]
        simp only
        rw [summariesOk]
      · exact shape

/-- Canonical checked models recheck to a structurally equivalent result. -/
theorem checkModel_checked_round_trip {checked : Checked.Model}
    (canonical : checked.Canonical) :
    ∃ rechecked, checkModel checked.erase = .ok rechecked ∧
      rechecked.Equivalent checked :=
  checkModel_equivalent_of_elaborates canonical

end Checked.Model

end Sembla.Semantics
