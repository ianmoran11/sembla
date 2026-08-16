import Sembla.Frontend.Builders.Transition

/-!
Pure, syntax-independent builders for model-local observations and the one
complete raw-model assembly boundary.

The complete specification embeds exactly one `TransitionOverlaySpec` and
attaches observations by `Fin` source ordinal.  Raw wires are retained opaquely;
this module makes no wire-validity or composition claim.
-/
namespace Sembla.Frontend.Builders

open Sembla
open Sembla.Semantics

/-! ## Exact raw observation constructors -/

namespace ObservationRaw

def input (name : String) (schema : List IR.Attr) : IR.PortDecl :=
  { name := name, schema := schema }

def outputField (name : String) (op : IR.AggOp)
    (filter : Option IR.Expr := none) : IR.OutputField :=
  { name := name, op := op, filter := filter }

def perTable (table : String) (fields : List IR.OutputField) : IR.OutputBuilder :=
  .perTable table fields

def output (name : String) (schema : List IR.Attr)
    (builder : IR.OutputBuilder) : IR.OutputDecl :=
  { name := name, schema := schema, builder := builder }

def view (name table : String) (filter value : Option IR.Expr)
    (reduce : IR.ViewReduce) : IR.ViewDecl :=
  { name := name, table := table, filter := filter, value := value, reduce := reduce }

def groupKey (attributeName : String) (bandWidth : Option Nat := none) : IR.GroupKey :=
  { attr := attributeName, bandWidth := bandWidth }

def groupedView (name table : String) (filter : Option IR.Expr)
    (keys : List IR.GroupKey) : IR.GroupedViewDecl :=
  { name := name, table := table, filter := filter, keys := keys }

def summary (name box view : String) (reduce : IR.SummaryReduce) : IR.SummaryDecl :=
  { name := name, box := box, view := view, reduce := reduce }

@[simp] theorem input_exact (name schema) :
    input name schema = { name := name, schema := schema } := rfl
@[simp] theorem input_name (name schema) : (input name schema).name = name := rfl
@[simp] theorem input_schema (name schema) : (input name schema).schema = schema := rfl

@[simp] theorem outputField_exact (name op filter) :
    outputField name op filter = { name := name, op := op, filter := filter } := rfl
@[simp] theorem outputField_name (name op filter) :
    (outputField name op filter).name = name := rfl
@[simp] theorem outputField_op (name op filter) :
    (outputField name op filter).op = op := rfl
@[simp] theorem outputField_filter (name op filter) :
    (outputField name op filter).filter = filter := rfl

@[simp] theorem perTable_exact (table fields) :
    perTable table fields = .perTable table fields := rfl
@[simp] theorem output_exact (name schema builder) :
    output name schema builder = { name := name, schema := schema, builder := builder } := rfl
@[simp] theorem output_schema (name schema builder) :
    (output name schema builder).schema = schema := rfl
@[simp] theorem output_builder (name schema builder) :
    (output name schema builder).builder = builder := rfl

@[simp] theorem view_exact (name table filter value reduce) :
    view name table filter value reduce =
      { name := name, table := table, filter := filter, value := value, reduce := reduce } := rfl
@[simp] theorem groupKey_exact (attributeName bandWidth) :
    groupKey attributeName bandWidth =
      { attr := attributeName, bandWidth := bandWidth } := rfl
@[simp] theorem groupedView_exact (name table filter keys) :
    groupedView name table filter keys =
      { name := name, table := table, filter := filter, keys := keys } := rfl
@[simp] theorem summary_exact (name box viewName reduce) :
    summary name box viewName reduce =
      { name := name, box := box, view := viewName, reduce := reduce } := rfl

/-- The general raw output constructor retains the supplied field order exactly. -/
theorem output_fields_preserve_supplied_order (name schema table fields) :
    (output name schema (perTable table fields)).builder = .perTable table fields := rfl

end ObservationRaw

/-! ## Current-surface output lowering -/

/-- Syntax-independent authored output field.  The output schema supplies the
canonical emitted name/order while this record retains the authored operation
and filter exactly. -/
structure SurfaceOutputFieldSpec where
  name : String
  op : IR.AggOp
  filter : Option IR.Expr
  deriving Repr, BEq

inductive ObservationLoweringErrorCategory where
  | duplicateOutputField
  | extraOutputField
  | missingOutputField
  deriving Repr, BEq, DecidableEq

/-- Builder-owned syntax positions.  These paths are used only by surface
lowering and token mapping; semantic checker failures keep
`ModelCheckPathSegment` unchanged. -/
inductive ObservationSurfacePathSegment where
  | box (index : Nat)
  | input (index : Nat)
  | output (index : Nat)
  | outputSchema (index : Nat)
  | outputField (index : Nat)
  | view (index : Nat)
  | groupedView (index : Nat)
  | groupedKey (index : Nat)
  | groupedBand
  | summary (index : Nat)
  | summaryBox
  | summaryView
  deriving Repr, BEq, DecidableEq

structure ObservationLoweringError where
  category : ObservationLoweringErrorCategory
  path : List ObservationSurfacePathSegment
  deriving Repr, BEq, DecidableEq

private def firstDuplicateIndexAux (seen : List String) :
    Nat → List String → Option Nat
  | _, [] => none
  | index, name :: names =>
      if name ∈ seen then some index
      else firstDuplicateIndexAux (name :: seen) (index + 1) names

private def firstDuplicateIndex (names : List String) : Option Nat :=
  firstDuplicateIndexAux [] 0 names

private def firstExtraIndexAux (schemaNames : List String) :
    Nat → List SurfaceOutputFieldSpec → Option Nat
  | _, [] => none
  | index, field :: fields =>
      if field.name ∈ schemaNames then firstExtraIndexAux schemaNames (index + 1) fields
      else some index

private def firstExtraIndex (schemaNames : List String)
    (fields : List SurfaceOutputFieldSpec) : Option Nat :=
  firstExtraIndexAux schemaNames 0 fields

private def firstMissingIndexAux (fieldNames : List String) :
    Nat → List IR.Attr → Option Nat
  | _, [] => none
  | index, attr :: attributes =>
      if attr.name ∈ fieldNames then firstMissingIndexAux fieldNames (index + 1) attributes
      else some index

private def firstMissingIndex (schema : List IR.Attr)
    (fields : List SurfaceOutputFieldSpec) : Option Nat :=
  firstMissingIndexAux (fields.map SurfaceOutputFieldSpec.name) 0 schema

private def findSurfaceField (name : String) :
    List SurfaceOutputFieldSpec → Option SurfaceOutputFieldSpec
  | [] => none
  | field :: fields => if field.name = name then some field else findSurfaceField name fields

/-- Project authored fields into schema order.  This helper is total because it
is used only after exact duplicate/extra/missing coverage validation. -/
private def lowerSurfaceOutputFieldsUnchecked (schema : List IR.Attr)
    (fields : List SurfaceOutputFieldSpec) : List IR.OutputField :=
  schema.map fun attr =>
    match findSurfaceField attr.name fields with
    | some field => ObservationRaw.outputField attr.name field.op field.filter
    | none => ObservationRaw.outputField attr.name .count none

/-- Current macro-compatible output lowering: validate exact authored coverage,
then emit fields in declared schema order. -/
def lowerSurfaceOutputFields (schema : List IR.Attr)
    (fields : List SurfaceOutputFieldSpec) (boxIndex : Nat := 0)
    (outputIndex : Nat := 0) : Except ObservationLoweringError (List IR.OutputField) :=
  match firstDuplicateIndex (fields.map SurfaceOutputFieldSpec.name) with
  | some index => .error ⟨.duplicateOutputField,
      [.box boxIndex, .output outputIndex, .outputField index]⟩
  | none =>
      match firstExtraIndex (schema.map IR.Attr.name) fields with
      | some index => .error ⟨.extraOutputField,
          [.box boxIndex, .output outputIndex, .outputField index]⟩
      | none =>
          match firstMissingIndex schema fields with
          | some index => .error ⟨.missingOutputField,
              [.box boxIndex, .output outputIndex, .outputSchema index]⟩
          | none => .ok (lowerSurfaceOutputFieldsUnchecked schema fields)

/-- Pure current-surface output boundary. It owns exact authored coverage,
schema-order projection, and final raw output construction. -/
def lowerSurfaceOutput (name : String) (schema : List IR.Attr) (table : String)
    (fields : List SurfaceOutputFieldSpec) (boxIndex : Nat := 0)
    (outputIndex : Nat := 0) : Except ObservationLoweringError IR.OutputDecl := do
  let lowered ← lowerSurfaceOutputFields schema fields boxIndex outputIndex
  pure (ObservationRaw.output name schema (ObservationRaw.perTable table lowered))

/-- Successful surface lowering always emits exact schema-name order. -/
theorem lowerSurfaceOutputFields_schema_order {schema fields boxIndex outputIndex lowered}
    (success : lowerSurfaceOutputFields schema fields boxIndex outputIndex = .ok lowered) :
    lowered.map IR.OutputField.name = schema.map IR.Attr.name := by
  unfold lowerSurfaceOutputFields at success
  split at success <;> try contradiction
  split at success <;> try contradiction
  split at success <;> try contradiction
  cases success
  simp only [lowerSurfaceOutputFieldsUnchecked, List.map_map]
  apply List.map_congr_left
  intro attr member
  cases found : findSurfaceField attr.name fields <;>
    simp [found, Function.comp_apply, ObservationRaw.outputField]

@[simp] theorem lowerSurfaceOutput_exact {name schema table fields boxIndex outputIndex raw}
    (success : lowerSurfaceOutput name schema table fields boxIndex outputIndex = .ok raw) :
    ∃ lowered, lowerSurfaceOutputFields schema fields boxIndex outputIndex = .ok lowered ∧
      raw = ObservationRaw.output name schema (ObservationRaw.perTable table lowered) := by
  simp only [lowerSurfaceOutput, Bind.bind, Except.bind] at success
  split at success <;> try contradiction
  rename_i lowered loweredOk
  cases success
  exact ⟨lowered, loweredOk, rfl⟩

/-! ## Ordinal-indexed complete assembly -/

structure BoxObservationPayload where
  inputs : List IR.PortDecl
  outputs : List IR.OutputDecl
  views : List IR.ViewDecl
  groupedViews : List IR.GroupedViewDecl
  deriving Repr, BEq

/-- Exactly one observation payload for each existing core box ordinal. -/
abbrev BoxObservationSpec (overlay : TransitionOverlaySpec) :=
  (ordinal : Fin overlay.core.boxes.length) → BoxObservationPayload

/-- One current-surface output before schema-order projection. -/
structure SurfaceOutputSpec where
  name : String
  schema : List IR.Attr
  table : String
  fields : List SurfaceOutputFieldSpec
  deriving Repr, BEq

/-- Later-owned current-surface observations for one existing overlay ordinal. -/
structure SurfaceBoxObservationPayload where
  inputs : List IR.PortDecl
  outputs : List SurfaceOutputSpec
  views : List IR.ViewDecl
  groupedViews : List IR.GroupedViewDecl
  deriving Repr, BEq

abbrev SurfaceBoxObservationSpec (overlay : TransitionOverlaySpec) :=
  (ordinal : Fin overlay.core.boxes.length) → SurfaceBoxObservationPayload

/-- The only complete frontend specification.  Core and transition ownership is
embedded once through `overlay`; wires are opaque raw preservation data. -/
structure CompleteModelSpec where
  overlay : TransitionOverlaySpec
  observations : BoxObservationSpec overlay
  summaries : List IR.SummaryDecl
  wires : List IR.Wire

/-- Macro-facing current-surface specification. It embeds the sole core and
transition owner and adds only later-owned observation syntax data. -/
structure SurfaceCompleteModelSpec where
  overlay : TransitionOverlaySpec
  observations : SurfaceBoxObservationSpec overlay
  summaries : List IR.SummaryDecl
  wires : List IR.Wire

private def lowerSurfaceOutputs (boxIndex : Nat) :
    Nat → List SurfaceOutputSpec →
      Except ObservationLoweringError (List IR.OutputDecl)
  | _, [] => .ok []
  | outputIndex, output :: outputs => do
      let head ← lowerSurfaceOutput output.name output.schema output.table
        output.fields boxIndex outputIndex
      let tail ← lowerSurfaceOutputs boxIndex (outputIndex + 1) outputs
      pure (head :: tail)

private def lowerSurfaceBoxObservation (boxIndex : Nat)
    (source : SurfaceBoxObservationPayload) :
    Except ObservationLoweringError BoxObservationPayload := do
  let outputs ← lowerSurfaceOutputs boxIndex 0 source.outputs
  pure (BoxObservationPayload.mk source.inputs outputs source.views source.groupedViews)

private def lowerSurfaceObservations :
    (n boxIndex : Nat) →
    ((ordinal : Fin n) → SurfaceBoxObservationPayload) →
    Except ObservationLoweringError ((ordinal : Fin n) → BoxObservationPayload)
  | 0, _, _ => .ok Fin.elim0
  | n + 1, boxIndex, source => do
      let head ← lowerSurfaceBoxObservation boxIndex (source ⟨0, Nat.zero_lt_succ n⟩)
      let tail ← lowerSurfaceObservations n (boxIndex + 1) (fun ordinal => source ordinal.succ)
      pure (Fin.cases head tail)

namespace SurfaceCompleteModelSpec

/-- Pure, ordinal-preserving lowering of every current-surface output. -/
def toCompleteModelSpec (source : SurfaceCompleteModelSpec) :
    Except ObservationLoweringError CompleteModelSpec := do
  let observations ← lowerSurfaceObservations source.overlay.core.boxes.length 0
    source.observations
  pure (CompleteModelSpec.mk source.overlay observations source.summaries source.wires)

end SurfaceCompleteModelSpec

namespace CompleteModelSpec

/-- Exact complete box at one existing source ordinal. -/
def rawBox (spec : CompleteModelSpec)
    (ordinal : Fin spec.overlay.core.boxes.length) : IR.Box :=
  { spec.overlay.rawBox ordinal with
    inputs := (spec.observations ordinal).inputs
    outputs := (spec.observations ordinal).outputs
    views := (spec.observations ordinal).views
    groupedViews := (spec.observations ordinal).groupedViews }

/-- Source-ordered complete boxes, generated from every and only core ordinal. -/
def rawBoxes (spec : CompleteModelSpec) : List IR.Box :=
  List.ofFn spec.rawBox

/-- Sole complete raw-model assembly boundary. -/
def toRaw (spec : CompleteModelSpec) : IR.Model :=
  { spec.overlay.toRaw with
    boxes := spec.rawBoxes
    wires := spec.wires
    summaries := spec.summaries }

/-- Named alias used by frontend adapters. -/
def assembleRaw (spec : CompleteModelSpec) : IR.Model := spec.toRaw

@[simp] theorem rawBox_name (spec : CompleteModelSpec) ordinal :
    (spec.rawBox ordinal).name = (spec.overlay.rawBox ordinal).name := rfl
@[simp] theorem rawBox_tables (spec : CompleteModelSpec) ordinal :
    (spec.rawBox ordinal).tables = (spec.overlay.rawBox ordinal).tables := rfl
@[simp] theorem rawBox_transitions (spec : CompleteModelSpec) ordinal :
    (spec.rawBox ordinal).transitions = spec.overlay.transitions ordinal := rfl
@[simp] theorem rawBox_inputs (spec : CompleteModelSpec) ordinal :
    (spec.rawBox ordinal).inputs = (spec.observations ordinal).inputs := rfl
@[simp] theorem rawBox_outputs (spec : CompleteModelSpec) ordinal :
    (spec.rawBox ordinal).outputs = (spec.observations ordinal).outputs := rfl
@[simp] theorem rawBox_views (spec : CompleteModelSpec) ordinal :
    (spec.rawBox ordinal).views = (spec.observations ordinal).views := rfl
@[simp] theorem rawBox_groupedViews (spec : CompleteModelSpec) ordinal :
    (spec.rawBox ordinal).groupedViews = (spec.observations ordinal).groupedViews := rfl

@[simp] theorem rawBoxes_length (spec : CompleteModelSpec) :
    spec.rawBoxes.length = spec.overlay.core.boxes.length := by
  simp [rawBoxes]

/-- No observation payload can be omitted, truncated, or reassociated. -/
theorem rawBoxes_get (spec : CompleteModelSpec)
    (ordinal : Fin spec.overlay.core.boxes.length) :
    spec.rawBoxes.get ⟨ordinal.val, by
      rw [rawBoxes_length]
      exact ordinal.isLt⟩ = spec.rawBox ordinal := by
  simp [rawBoxes]

@[simp] theorem toRaw_name (spec : CompleteModelSpec) :
    spec.toRaw.name = spec.overlay.core.name := rfl
@[simp] theorem toRaw_dt (spec : CompleteModelSpec) :
    spec.toRaw.dt = spec.overlay.core.dt := rfl
@[simp] theorem toRaw_params (spec : CompleteModelSpec) :
    spec.toRaw.params = spec.overlay.core.params := rfl
@[simp] theorem toRaw_boxes (spec : CompleteModelSpec) :
    spec.toRaw.boxes = spec.rawBoxes := rfl
@[simp] theorem toRaw_summaries (spec : CompleteModelSpec) :
    spec.toRaw.summaries = spec.summaries := rfl
@[simp] theorem toRaw_wires (spec : CompleteModelSpec) :
    spec.toRaw.wires = spec.wires := rfl
@[simp] theorem assembleRaw_exact (spec : CompleteModelSpec) :
    spec.assembleRaw = spec.toRaw := rfl

/-- Core box names remain in exact source order. -/
theorem toRaw_core_box_names_exact (spec : CompleteModelSpec) :
    spec.toRaw.boxes.map IR.Box.name = spec.overlay.core.boxNames := by
  apply List.ext_get
  · simp [CoreModelShell.boxNames]
  · intro index leftBound rightBound
    simp [rawBoxes, rawBox, CoreModelShell.boxNames]

/-- Every core table list remains at the same box/source ordinal. -/
theorem toRaw_core_tables_exact (spec : CompleteModelSpec) :
    spec.toRaw.boxes.map IR.Box.tables =
      spec.overlay.core.boxes.map CoreBoxShell.tables := by
  apply List.ext_get
  · simp
  · intro index leftBound rightBound
    simp [rawBoxes, rawBox]

/-- Every transition list remains at the same box/source ordinal. -/
theorem toRaw_transitions_exact (spec : CompleteModelSpec)
    (ordinal : Fin spec.overlay.core.boxes.length) :
    (spec.rawBoxes.get ⟨ordinal.val, by simpa using ordinal.isLt⟩).transitions =
      spec.overlay.transitions ordinal := by
  rw [rawBoxes_get]
  rfl

/-- Inputs attach to the supplied existing box ordinal exactly. -/
theorem toRaw_inputs_exact (spec : CompleteModelSpec)
    (ordinal : Fin spec.overlay.core.boxes.length) :
    (spec.rawBoxes.get ⟨ordinal.val, by simpa using ordinal.isLt⟩).inputs =
      (spec.observations ordinal).inputs := by
  rw [rawBoxes_get]
  rfl

/-- Outputs attach to the supplied existing box ordinal exactly. -/
theorem toRaw_outputs_exact (spec : CompleteModelSpec)
    (ordinal : Fin spec.overlay.core.boxes.length) :
    (spec.rawBoxes.get ⟨ordinal.val, by simpa using ordinal.isLt⟩).outputs =
      (spec.observations ordinal).outputs := by
  rw [rawBoxes_get]
  rfl

/-- Ordinary views attach to the supplied existing box ordinal exactly. -/
theorem toRaw_views_exact (spec : CompleteModelSpec)
    (ordinal : Fin spec.overlay.core.boxes.length) :
    (spec.rawBoxes.get ⟨ordinal.val, by simpa using ordinal.isLt⟩).views =
      (spec.observations ordinal).views := by
  rw [rawBoxes_get]
  rfl

/-- Grouped views attach to the supplied existing box ordinal exactly. -/
theorem toRaw_groupedViews_exact (spec : CompleteModelSpec)
    (ordinal : Fin spec.overlay.core.boxes.length) :
    (spec.rawBoxes.get ⟨ordinal.val, by simpa using ordinal.isLt⟩).groupedViews =
      (spec.observations ordinal).groupedViews := by
  rw [rawBoxes_get]
  rfl

/-- Model summaries retain exact supplied source order. -/
theorem toRaw_summaries_exact (spec : CompleteModelSpec) :
    spec.toRaw.summaries = spec.summaries := rfl

/-- Raw wires retain exact supplied source order, without a validity claim. -/
theorem toRaw_wires_exact (spec : CompleteModelSpec) :
    spec.toRaw.wires = spec.wires := rfl

end CompleteModelSpec

/-! ## Structured final failure and checker adapters -/

inductive ObservationBuilderError where
  | core (error : CoreBuilderError)
  | transition (error : TransitionBuilderError)
  | lowering (error : ObservationLoweringError)
  | modelCheck (error : ModelCheckError)
  deriving Repr, BEq

private def liftModelResult :
    Except ModelCheckError Checked.Model → Except ObservationBuilderError Checked.Model
  | .ok checked => .ok checked
  | .error error => .error (.modelCheck error)

/-- Lower surface fields while retaining the final builder's exact error sum. -/
def buildSurfaceOutputFields (schema : List IR.Attr)
    (fields : List SurfaceOutputFieldSpec) (boxIndex : Nat := 0)
    (outputIndex : Nat := 0) :
    Except ObservationBuilderError (List IR.OutputField) :=
  match lowerSurfaceOutputFields schema fields boxIndex outputIndex with
  | .ok lowered => .ok lowered
  | .error error => .error (.lowering error)

/-- Structural, checker-independent transition obligation for a complete
candidate.  It is stated only through the authoritative declaration and term
judgments and contains no executable-success premise. -/
def CompleteTransitionsWellTyped (spec : CompleteModelSpec) : Prop :=
  ∀ (declarations : DeclarationsWellFormed spec.toRaw),
    let ctx : DeclarationContext := ⟨spec.toRaw, declarations⟩
    ∀ (box : BoxId ctx.modelSchema.catalog)
      (ordinal : Fin (ctx.transitions box).length),
      ∃ checked,
        TransitionWellTyped
          { declarations := ctx
            box := box
            currentTable := ctx.resolveTransitionTarget box ordinal }
          (ctx.transitionAt box ordinal) checked

private def validateTransitionOrdinals (surface : Bool) (ctx : DeclarationContext)
    (box : BoxId ctx.modelSchema.catalog) (boxIndex : Nat) :
    List (Fin (ctx.transitions box).length) → Except ObservationBuilderError Unit
  | [] => .ok ()
  | ordinal :: ordinals =>
      let target := ctx.resolveTransitionTarget box ordinal
      let Γ : TermContext :=
        { declarations := ctx, box := box, currentTable := target }
      let raw := ctx.transitionAt box ordinal
      let path : List ModelCheckPathSegment :=
        [.model, .box boxIndex, .transition ordinal.val]
      let result := if surface then
          buildSurfaceTransition Γ raw boxIndex ordinal.val path
        else buildTransition Γ raw path
      match result with
      | .ok _ => validateTransitionOrdinals surface ctx box boxIndex ordinals
      | .error error => .error (.transition error)

private def validateBoxOrdinals (surface : Bool) (ctx : DeclarationContext) :
    List (Fin ctx.modelSchema.catalog.boxes.entries.length) →
      Except ObservationBuilderError Unit
  | [] => .ok ()
  | boxOrdinal :: boxOrdinals =>
      let box : BoxId ctx.modelSchema.catalog := ⟨boxOrdinal⟩
      match validateTransitionOrdinals surface ctx box boxOrdinal.val
          (List.finRange (ctx.transitions box).length) with
      | .ok _ => validateBoxOrdinals surface ctx boxOrdinals
      | .error error => .error error

/-- Validate every transition against a declaration context derived from the
complete input-bearing candidate.  The current surface selects the race-only
adapter; the general raw path retains every PRD 0006-valid key ordering. -/
private def validateCompleteTransitions (_spec : CompleteModelSpec)
    (ctx : DeclarationContext) (surface : Bool) :
    Except ObservationBuilderError Unit :=
  validateBoxOrdinals surface ctx
    (List.finRange ctx.modelSchema.catalog.boxes.entries.length)

private theorem validateTransitionOrdinals_complete (ctx : DeclarationContext)
    (box : BoxId ctx.modelSchema.catalog) (boxIndex : Nat)
    (accepted : ∀ ordinal, ∃ checked,
      TransitionWellTyped
        { declarations := ctx
          box := box
          currentTable := ctx.resolveTransitionTarget box ordinal }
        (ctx.transitionAt box ordinal) checked)
    (ordinals : List (Fin (ctx.transitions box).length)) :
    validateTransitionOrdinals false ctx box boxIndex ordinals = .ok () := by
  induction ordinals with
  | nil => rfl
  | cons ordinal ordinals ih =>
      obtain ⟨checked, typed⟩ := accepted ordinal
      have built := buildTransition_complete typed
        [.model, .box boxIndex, .transition ordinal.val]
      simp [validateTransitionOrdinals, built, ih]

private theorem validateBoxOrdinals_complete (spec : CompleteModelSpec)
    (declarations : DeclarationsWellFormed spec.toRaw)
    (accepted : CompleteTransitionsWellTyped spec)
    (ordinals : List (Fin
      (DeclarationContext.modelSchema ⟨spec.toRaw, declarations⟩).catalog.boxes.entries.length)) :
    validateBoxOrdinals false ⟨spec.toRaw, declarations⟩ ordinals = .ok () := by
  induction ordinals with
  | nil => rfl
  | cons boxOrdinal boxOrdinals ih =>
      let ctx : DeclarationContext := ⟨spec.toRaw, declarations⟩
      let box : BoxId ctx.modelSchema.catalog := ⟨boxOrdinal⟩
      have transitionsOk := validateTransitionOrdinals_complete ctx box boxOrdinal.val
        (accepted declarations box) (List.finRange (ctx.transitions box).length)
      simp [validateBoxOrdinals, ctx, box, transitionsOk, ih]

private theorem validateCompleteTransitions_complete (spec : CompleteModelSpec)
    (declarations : DeclarationsWellFormed spec.toRaw)
    (accepted : CompleteTransitionsWellTyped spec) :
    validateCompleteTransitions spec ⟨spec.toRaw, declarations⟩ false = .ok () := by
  exact validateBoxOrdinals_complete spec declarations accepted _

private def buildCompleteModelWith (surface : Bool) (spec : CompleteModelSpec) :
    Except ObservationBuilderError Checked.Model :=
  match buildModelShell spec.overlay.core with
  | .error error => .error (.core error)
  | .ok _ =>
      match checkDeclarations spec.toRaw with
      | .error error => .error (.modelCheck (.declaration error))
      | .ok ctx =>
          match validateCompleteTransitions spec ctx surface with
          | .error error => .error error
          | .ok _ => liftModelResult (checkModel spec.toRaw)

/-- General complete raw-model builder. -/
def buildCompleteModel (spec : CompleteModelSpec) :
    Except ObservationBuilderError Checked.Model :=
  buildCompleteModelWith false spec

/-- Current race-time-only surface entry. It first owns all observation
surface lowering, then shares the complete input-bearing checking boundary. -/
def buildSurfaceCompleteModel (source : SurfaceCompleteModelSpec) :
    Except ObservationBuilderError Checked.Model :=
  match source.toCompleteModelSpec with
  | .error error => .error (.lowering error)
  | .ok spec => buildCompleteModelWith true spec

/-- Successful surface construction names the exact lowered candidate and retains
its authoritative checker acceptance and erasure. -/
theorem buildSurfaceCompleteModel_sound {source : SurfaceCompleteModelSpec} {checked}
    (success : buildSurfaceCompleteModel source = .ok checked) :
    ∃ spec, source.toCompleteModelSpec = .ok spec ∧
      ModelWellFormed spec.toRaw ∧ checkModel spec.toRaw = .ok checked ∧
      checked.erase = spec.toRaw := by
  unfold buildSurfaceCompleteModel at success
  cases lowering : source.toCompleteModelSpec with
  | error error => simp [lowering] at success
  | ok spec =>
      simp only [lowering] at success
      refine ⟨spec, rfl, ?_⟩
      unfold buildCompleteModelWith at success
      cases coreResult : buildModelShell spec.overlay.core with
      | error error => simp [coreResult] at success
      | ok raw =>
          cases declarationResult : checkDeclarations spec.toRaw with
          | error error => simp [coreResult, declarationResult] at success
          | ok ctx =>
              cases transitionResult : validateCompleteTransitions spec ctx true with
              | error error => simp [coreResult, declarationResult, transitionResult] at success
              | ok unit =>
                  cases modelResult : checkModel spec.toRaw with
                  | error error => simp [coreResult, declarationResult, transitionResult,
                      modelResult, liftModelResult] at success
                  | ok actual =>
                      have certified := checkModel_sound modelResult
                      simp [coreResult, declarationResult, transitionResult, modelResult,
                        liftModelResult] at success
                      cases success
                      exact ⟨certified.1, rfl, certified.2⟩

/-- Public surface-model acceptance and exact-erasure bridge. -/
theorem buildSurfaceCompleteModel_model_acceptance_and_erasure
    {source : SurfaceCompleteModelSpec} {checked}
    (success : buildSurfaceCompleteModel source = .ok checked) :
    ∃ spec actual, source.toCompleteModelSpec = .ok spec ∧
      checkModel spec.toRaw = .ok actual ∧ actual.erase = spec.toRaw := by
  obtain ⟨spec, lowered, _, checkedOk, erased⟩ :=
    buildSurfaceCompleteModel_sound success
  exact ⟨spec, checked, lowered, checkedOk, erased⟩

private theorem checkModel_error_of_checkDeclarations_error (raw : IR.Model)
    (error : CheckError) (failed : checkDeclarations raw = .error error) :
    checkModel raw = .error (.declaration error) := by
  simp [checkModel, failed]

private theorem buildCompleteModelWith_failure_iff (surface : Bool)
    (spec : CompleteModelSpec) :
    (∃ error, buildCompleteModelWith surface spec = .error error) ↔
      (∃ coreError, buildModelShell spec.overlay.core = .error coreError) ∨
      (∃ coreRaw ctx transitionError,
        buildModelShell spec.overlay.core = .ok coreRaw ∧
        checkDeclarations spec.toRaw = .ok ctx ∧
        validateCompleteTransitions spec ctx surface = .error transitionError) ∨
      (∃ coreRaw modelError,
        buildModelShell spec.overlay.core = .ok coreRaw ∧
        checkModel spec.toRaw = .error modelError) := by
  cases coreResult : buildModelShell spec.overlay.core with
  | error coreError => simp [buildCompleteModelWith, coreResult]
  | ok coreRaw =>
      cases declarationResult : checkDeclarations spec.toRaw with
      | error declarationError =>
          have modelFailed := checkModel_error_of_checkDeclarations_error spec.toRaw
            declarationError declarationResult
          simp [buildCompleteModelWith, coreResult, declarationResult, modelFailed]
      | ok ctx =>
          cases transitionResult : validateCompleteTransitions spec ctx surface with
          | error transitionError =>
              simp [buildCompleteModelWith, coreResult, declarationResult, transitionResult]
          | ok unit =>
              cases modelResult : checkModel spec.toRaw with
              | ok checked => simp [buildCompleteModelWith, coreResult, declarationResult,
                  transitionResult, modelResult, liftModelResult]
              | error modelError => simp [buildCompleteModelWith, coreResult,
                  declarationResult, transitionResult, modelResult, liftModelResult]

/-- Surface failure first identifies exact lowering failure, then exposes
one of the explicit shared complete-builder phases. -/
theorem buildSurfaceCompleteModel_failure_iff (source : SurfaceCompleteModelSpec) :
    (∃ error, buildSurfaceCompleteModel source = .error error) ↔
      (∃ loweringError, source.toCompleteModelSpec = .error loweringError) ∨
      (∃ spec, source.toCompleteModelSpec = .ok spec ∧
        ((∃ coreError, buildModelShell spec.overlay.core = .error coreError) ∨
        (∃ coreRaw ctx transitionError,
          buildModelShell spec.overlay.core = .ok coreRaw ∧
          checkDeclarations spec.toRaw = .ok ctx ∧
          validateCompleteTransitions spec ctx true = .error transitionError) ∨
        (∃ coreRaw modelError,
          buildModelShell spec.overlay.core = .ok coreRaw ∧
          checkModel spec.toRaw = .error modelError))) := by
  cases lowering : source.toCompleteModelSpec with
  | error loweringError => simp [buildSurfaceCompleteModel, lowering]
  | ok spec =>
      constructor
      · intro failed
        right
        refine ⟨spec, rfl, ?_⟩
        exact (buildCompleteModelWith_failure_iff true spec).mp (by
          simpa [buildSurfaceCompleteModel, lowering] using failed)
      · intro characterized
        rcases characterized with loweringFailed | ⟨other, otherLowering, phases⟩
        · obtain ⟨error, errorEq⟩ := loweringFailed
          contradiction
        · have same : other = spec := (Except.ok.inj otherLowering).symm
          subst other
          obtain ⟨error, failed⟩ :=
            (buildCompleteModelWith_failure_iff true spec).mpr phases
          exact ⟨error, by simpa [buildSurfaceCompleteModel, lowering] using failed⟩

/-- Exact observation-lowering errors and paths are retained unchanged. -/
theorem buildSurfaceOutputFields_error_iff (schema fields error boxIndex outputIndex) :
    buildSurfaceOutputFields schema fields boxIndex outputIndex = .error (.lowering error) ↔
      lowerSurfaceOutputFields schema fields boxIndex outputIndex = .error error := by
  cases result : lowerSurfaceOutputFields schema fields boxIndex outputIndex <;>
    simp [buildSurfaceOutputFields, result]

/-- Successful final construction is accepted by the authoritative checker and
its checked erasure is the exact complete candidate. -/
theorem buildCompleteModel_sound {spec : CompleteModelSpec} {checked}
    (success : buildCompleteModel spec = .ok checked) :
    ModelWellFormed spec.toRaw ∧
      checkModel spec.toRaw = .ok checked ∧ checked.erase = spec.toRaw := by
  unfold buildCompleteModel buildCompleteModelWith at success
  cases coreResult : buildModelShell spec.overlay.core with
  | error error => simp [coreResult] at success
  | ok raw =>
      cases declarationResult : checkDeclarations spec.toRaw with
      | error error => simp [coreResult, declarationResult] at success
      | ok ctx =>
          cases transitionResult : validateCompleteTransitions spec ctx false with
          | error error => simp [coreResult, declarationResult, transitionResult] at success
          | ok unit =>
              cases modelResult : checkModel spec.toRaw with
              | error error => simp [coreResult, declarationResult, transitionResult,
                  modelResult, liftModelResult] at success
              | ok actual =>
                  have certified := checkModel_sound modelResult
                  simp [coreResult, declarationResult, transitionResult, modelResult,
                    liftModelResult] at success
                  cases success
                  exact ⟨certified.1, rfl, certified.2⟩

/-- Public complete-model acceptance and exact-erasure bridge. -/
theorem buildCompleteModel_model_acceptance_and_erasure
    {spec : CompleteModelSpec} {checked}
    (success : buildCompleteModel spec = .ok checked) :
    ∃ actual, checkModel spec.toRaw = .ok actual ∧ actual.erase = spec.toRaw := by
  exact ⟨checked, (buildCompleteModel_sound success).2.1,
    (buildCompleteModel_sound success).2.2⟩

private theorem modelWellFormed_declarations {raw : IR.Model}
    (accepted : ModelWellFormed raw) : DeclarationsWellFormed raw := by
  obtain ⟨checked, elaborates⟩ := accepted
  cases elaborates with
  | mk source declarations boxTerms boxOrder summaryTerms summaryOrder wires =>
      exact declarations

/-- Every independently valid complete candidate is reproduced exactly from
structural authoritative core, transition, and whole-model judgments. -/
theorem buildCompleteModel_complete (spec : CompleteModelSpec)
    (coreAccepted : DeclarationsWellFormed spec.overlay.core.toRaw)
    (transitionsAccepted : CompleteTransitionsWellTyped spec)
    (accepted : ModelWellFormed spec.toRaw) :
    ∃ checked, buildCompleteModel spec = .ok checked := by
  have coreOk := buildModelShell_complete coreAccepted
  let declarations : DeclarationsWellFormed spec.toRaw :=
    modelWellFormed_declarations accepted
  let ctx : DeclarationContext := ⟨spec.toRaw, declarations⟩
  have declarationOk : checkDeclarations spec.toRaw = .ok ctx := by
    simp [checkDeclarations, ctx, declarations]
  have transitionOk : validateCompleteTransitions spec ctx false = .ok () := by
    exact validateCompleteTransitions_complete spec declarations transitionsAccepted
  obtain ⟨checked, checkedOk⟩ := checkModel_complete accepted
  exact ⟨checked, by
    simp [buildCompleteModel, buildCompleteModelWith, coreOk, declarationOk,
      transitionOk, checkedOk, liftModelResult]⟩

/-- Builder failure is characterized only by a direct core operation, a PRD
0008 transition adapter, or an exact final model-check failure. Declaration
failures are included in the final `checkModel` alternative. -/
theorem buildCompleteModel_failure_iff (spec : CompleteModelSpec) :
    (∃ error, buildCompleteModel spec = .error error) ↔
      (∃ coreError, buildModelShell spec.overlay.core = .error coreError) ∨
      (∃ coreRaw ctx transitionError,
        buildModelShell spec.overlay.core = .ok coreRaw ∧
        checkDeclarations spec.toRaw = .ok ctx ∧
        validateCompleteTransitions spec ctx false = .error transitionError) ∨
      (∃ coreRaw modelError,
        buildModelShell spec.overlay.core = .ok coreRaw ∧
        checkModel spec.toRaw = .error modelError) := by
  exact buildCompleteModelWith_failure_iff false spec

/-- Under the preceding core/context/transition phase results, the final builder
wraps exactly the `ModelCheckError` returned by `checkModel`. -/
theorem buildCompleteModel_model_check_error_iff (spec : CompleteModelSpec)
    (modelError : ModelCheckError) (coreRaw : IR.Model) (ctx : DeclarationContext)
    (coreOk : buildModelShell spec.overlay.core = .ok coreRaw)
    (declarationsOk : checkDeclarations spec.toRaw = .ok ctx)
    (transitionsOk : validateCompleteTransitions spec ctx false = .ok ()) :
    buildCompleteModel spec = .error (.modelCheck modelError) ↔
      checkModel spec.toRaw = .error modelError := by
  cases modelResult : checkModel spec.toRaw with
  | ok checked =>
      simp [buildCompleteModel, buildCompleteModelWith, coreOk, declarationsOk,
        transitionsOk, modelResult, liftModelResult]
  | error actualError =>
      simp [buildCompleteModel, buildCompleteModelWith, coreOk, declarationsOk,
        transitionsOk, modelResult, liftModelResult]

end Sembla.Frontend.Builders
