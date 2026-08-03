import Sembla.Semantics.CheckModel

/-!
Pure, syntax-independent builders for the core declaration fragment.  The
builders retain raw values and source order exactly and validate with the
accepted PRD 0005 predicates.
-/
namespace Sembla.Frontend.Builders

open Sembla
open Sembla.Semantics

/-- Stable failure categories for the declaration-only builder surface. -/
inductive CoreBuilderErrorCategory where
  | nonpositiveDt
  | duplicateParameterName
  | duplicateBoxName
  | duplicateTableName
  | duplicateAttributeName
  | parameterDefaultTypeMismatch
  | integerPrior
  | invalidPriorArity
  | unorderedUniformBounds
  | emptyEnum
  | duplicateEnumVariant
  | unresolvedTableReference
  deriving Repr, BEq

/-- Source-index path components, independent of parser syntax and positions. -/
inductive CoreBuilderPathSegment where
  | modelMetadata
  | dt
  | parameter (index : Nat)
  | prior
  | priorArgument (index : Nat)
  | box (index : Nat)
  | table (index : Nat)
  | attribute (index : Nat)
  | enumVariant (index : Nat)
  | name
  | defaultValue
  | tableReference
  deriving Repr, BEq

/-- Structured pure-builder failure. -/
structure CoreBuilderError where
  category : CoreBuilderErrorCategory
  path : List CoreBuilderPathSegment
  deriving Repr, BEq

/-- Public declaration-only box specification. -/
structure CoreBoxShell where
  name : String
  tables : List IR.Table
  deriving Repr, BEq

namespace CoreBoxShell

/-- Complete ordered table-name catalog used to resolve every schema. -/
def tableNames (shell : CoreBoxShell) : List String :=
  shell.tables.map IR.Table.name

/-- Exact raw declaration-only box represented by a shell. -/
def toRaw (shell : CoreBoxShell) : IR.Box :=
  { name := shell.name
    tables := shell.tables
    transitions := []
    inputs := []
    outputs := []
    views := []
    groupedViews := [] }

@[simp] theorem toRaw_name (shell : CoreBoxShell) : shell.toRaw.name = shell.name := rfl
@[simp] theorem toRaw_tables (shell : CoreBoxShell) : shell.toRaw.tables = shell.tables := rfl
@[simp] theorem toRaw_transitions (shell : CoreBoxShell) : shell.toRaw.transitions = [] := rfl
@[simp] theorem toRaw_inputs (shell : CoreBoxShell) : shell.toRaw.inputs = [] := rfl
@[simp] theorem toRaw_outputs (shell : CoreBoxShell) : shell.toRaw.outputs = [] := rfl
@[simp] theorem toRaw_views (shell : CoreBoxShell) : shell.toRaw.views = [] := rfl
@[simp] theorem toRaw_groupedViews (shell : CoreBoxShell) : shell.toRaw.groupedViews = [] := rfl

end CoreBoxShell

/-- Public declaration-only model specification. -/
structure CoreModelShell where
  name : String
  dt : IR.Scientific
  params : List IR.ParamDecl
  boxes : List CoreBoxShell
  deriving Repr, BEq

namespace CoreModelShell

/-- Ordered parameter names retained for later frontend builder slices. -/
def parameterNames (shell : CoreModelShell) : List String :=
  shell.params.map IR.ParamDecl.name

/-- Ordered box names retained for later frontend builder slices. -/
def boxNames (shell : CoreModelShell) : List String :=
  shell.boxes.map CoreBoxShell.name

/-- Exact raw declaration-only model represented by a shell. -/
def toRaw (shell : CoreModelShell) : IR.Model :=
  { name := shell.name
    dt := shell.dt
    params := shell.params
    boxes := shell.boxes.map CoreBoxShell.toRaw
    wires := []
    summaries := [] }

@[simp] theorem toRaw_name (shell : CoreModelShell) : shell.toRaw.name = shell.name := rfl
@[simp] theorem toRaw_dt (shell : CoreModelShell) : shell.toRaw.dt = shell.dt := rfl
@[simp] theorem toRaw_params (shell : CoreModelShell) : shell.toRaw.params = shell.params := rfl
@[simp] theorem toRaw_boxes (shell : CoreModelShell) :
    shell.toRaw.boxes = shell.boxes.map CoreBoxShell.toRaw := rfl
@[simp] theorem toRaw_wires (shell : CoreModelShell) : shell.toRaw.wires = [] := rfl
@[simp] theorem toRaw_summaries (shell : CoreModelShell) : shell.toRaw.summaries = [] := rfl

end CoreModelShell

/-! ## Exact raw candidates -/

def priorRaw (family : IR.PriorFamily) (args : List IR.Scientific) : IR.Prior :=
  { family := family, args := args }

def parameterRaw (name : String) (ty : IR.ParamType) (default : IR.ParamValue)
    (prior : Option IR.Prior) : IR.ParamDecl :=
  { name := name, ty := ty, default := default, prior := prior }

def attributeRaw (name : String) (ty : IR.AttrType) : IR.Attr :=
  { name := name, ty := ty }

def tableRaw (name : String) (sizeHint : Nat) (attrs : List IR.Attr) : IR.Table :=
  { name := name, sizeHint := sizeHint, attrs := attrs }

/-! ## Stable diagnostic selection -/

private def error (category : CoreBuilderErrorCategory)
    (path : List CoreBuilderPathSegment) : CoreBuilderError :=
  { category := category, path := path }

private def firstDuplicateIndexAux (seen : List String) :
    Nat → List String → Option Nat
  | _, [] => none
  | index, name :: names =>
      if name ∈ seen then some index
      else firstDuplicateIndexAux (name :: seen) (index + 1) names

private def firstDuplicateIndex (names : List String) : Option Nat :=
  firstDuplicateIndexAux [] 0 names

private def priorIssue (base : List CoreBuilderPathSegment)
    (prior : IR.Prior) : Option CoreBuilderError :=
  match prior.args with
  | [lower, upper] =>
      match prior.family with
      | .uniform =>
          if scientificLt lower upper then none
          else some (error .unorderedUniformBounds (base ++ [.priorArgument 1]))
      | .normal | .logNormal => none
  | _ => some (error .invalidPriorArity base)

private def parameterIssue (base : List CoreBuilderPathSegment)
    (parameter : IR.ParamDecl) : Option CoreBuilderError :=
  match parameter.ty, parameter.default with
  | .real, .real _ =>
      match parameter.prior with
      | none => none
      | some prior => priorIssue (base ++ [.prior]) prior
  | .int, .int _ =>
      match parameter.prior with
      | none => none
      | some _ => some (error .integerPrior (base ++ [.prior]))
  | _, _ => some (error .parameterDefaultTypeMismatch (base ++ [.defaultValue]))

private def attributeIssue (tableNames : List String)
    (base : List CoreBuilderPathSegment) (attr : IR.Attr) : Option CoreBuilderError :=
  match attr.ty with
  | .real | .int => none
  | .enum variants =>
      if variants.isEmpty then some (error .emptyEnum (base ++ [.enumVariant 0]))
      else
        match firstDuplicateIndex variants with
        | some index => some (error .duplicateEnumVariant (base ++ [.enumVariant index]))
        | none => none
  | .ref table =>
      if table ∈ tableNames then none
      else some (error .unresolvedTableReference (base ++ [.tableReference]))

private def firstAttributeIssue (tableNames : List String)
    (base : List CoreBuilderPathSegment) : Nat → List IR.Attr → Option CoreBuilderError
  | _, [] => none
  | index, attr :: attrs =>
      match attributeIssue tableNames (base ++ [.attribute index]) attr with
      | some issue => some issue
      | none => firstAttributeIssue tableNames base (index + 1) attrs

private def schemaIssue (tableNames : List String)
    (base : List CoreBuilderPathSegment) (attrs : List IR.Attr) : Option CoreBuilderError :=
  match firstDuplicateIndex (attrs.map IR.Attr.name) with
  | some index => some (error .duplicateAttributeName (base ++ [.attribute index, .name]))
  | none => firstAttributeIssue tableNames base 0 attrs

private def firstTableIssue (tableNames : List String)
    (base : List CoreBuilderPathSegment) : Nat → List IR.Table → Option CoreBuilderError
  | _, [] => none
  | index, table :: tables =>
      match schemaIssue tableNames (base ++ [.table index]) table.attrs with
      | some issue => some issue
      | none => firstTableIssue tableNames base (index + 1) tables

private def boxIssue (base : List CoreBuilderPathSegment)
    (shell : CoreBoxShell) : Option CoreBuilderError :=
  match firstDuplicateIndex shell.tableNames with
  | some index => some (error .duplicateTableName (base ++ [.table index, .name]))
  | none => firstTableIssue shell.tableNames base 0 shell.tables

private def firstParameterIssue : Nat → List IR.ParamDecl → Option CoreBuilderError
  | _, [] => none
  | index, parameter :: parameters =>
      match parameterIssue [.parameter index] parameter with
      | some issue => some issue
      | none => firstParameterIssue (index + 1) parameters

private def firstBoxIssue : Nat → List CoreBoxShell → Option CoreBuilderError
  | _, [] => none
  | index, box :: boxes =>
      match boxIssue [.box index] box with
      | some issue => some issue
      | none => firstBoxIssue (index + 1) boxes

private def modelIssue (shell : CoreModelShell) : Option CoreBuilderError :=
  if shell.dt.coefficient ≤ 0 then
    some (error .nonpositiveDt [.modelMetadata, .dt])
  else
    match firstDuplicateIndex shell.parameterNames with
    | some index => some (error .duplicateParameterName [.parameter index, .name])
    | none =>
        match firstParameterIssue 0 shell.params with
        | some issue => some issue
        | none =>
            match firstDuplicateIndex shell.boxNames with
            | some index => some (error .duplicateBoxName [.box index, .name])
            | none => firstBoxIssue 0 shell.boxes

private def fallbackError (path : List CoreBuilderPathSegment) : CoreBuilderError :=
  error .duplicateAttributeName path

/-! ## Pure builders -/

/-- Build an exact raw prior, accepting exactly `PriorWellFormed`. -/
def buildPrior (family : IR.PriorFamily) (args : List IR.Scientific) :
    Except CoreBuilderError IR.Prior :=
  let raw := priorRaw family args
  if _ : PriorWellFormed raw then .ok raw
  else .error ((priorIssue [.prior] raw).getD (fallbackError [.prior]))

/-- Build an exact raw parameter, accepting exactly `ParameterWellFormed`.
Standalone diagnostics use source index zero; enclosing shell builders replace it
with the actual source index. -/
def buildParameter (name : String) (ty : IR.ParamType) (default : IR.ParamValue)
    (prior : Option IR.Prior := none) : Except CoreBuilderError IR.ParamDecl :=
  let raw := parameterRaw name ty default prior
  if _ : ParameterWellFormed raw then .ok raw
  else .error ((parameterIssue [.parameter 0] raw).getD
    (fallbackError [.parameter 0]))

/-- Build one named attribute against the complete enclosing table catalog.
Standalone diagnostics use attribute index zero. -/
def buildAttribute (tableNames : List String) (name : String) (ty : IR.AttrType) :
    Except CoreBuilderError IR.Attr :=
  let raw := attributeRaw name ty
  if _ : AttributeTypeWellFormed tableNames raw.ty then .ok raw
  else .error ((attributeIssue tableNames [.attribute 0] raw).getD
    (fallbackError [.attribute 0]))

/-- Build one table against the complete enclosing ordered table-name catalog.
The catalog is supplied by the enclosing box and the standalone diagnostic uses
table index zero; catalog membership of the table's own name is intentionally a
box-level namespace obligation. -/
def buildTable (tableNames : List String) (name : String) (sizeHint : Nat)
    (attrs : List IR.Attr) : Except CoreBuilderError IR.Table :=
  let raw := tableRaw name sizeHint attrs
  if _ : SchemaWellFormed tableNames raw.attrs then .ok raw
  else .error ((schemaIssue tableNames [.table 0] attrs).getD
    (fallbackError [.table 0]))

/-- Build a declaration-only box after collecting its complete table catalog. -/
def buildBoxShell (shell : CoreBoxShell) : Except CoreBuilderError IR.Box :=
  if _ : BoxDeclarationsWellFormed shell.toRaw then .ok shell.toRaw
  else .error ((boxIssue [] shell).getD (fallbackError []))

/-- Build a declaration-only model shell with all later-owned lists empty. -/
def buildModelShell (shell : CoreModelShell) : Except CoreBuilderError IR.Model :=
  if _ : DeclarationsWellFormed shell.toRaw then .ok shell.toRaw
  else .error ((modelIssue shell).getD (fallbackError []))

/-! ## Soundness, exact retention, completeness and failure -/

theorem buildPrior_sound {family args raw}
    (success : buildPrior family args = .ok raw) :
    PriorWellFormed raw ∧ raw = priorRaw family args ∧
      raw.family = family ∧ raw.args = args := by
  simp only [buildPrior] at success
  split at success <;> rename_i accepted
  · cases success
    exact ⟨accepted, rfl, rfl, rfl⟩
  · contradiction

theorem buildPrior_complete {raw : IR.Prior}
    (accepted : PriorWellFormed raw) :
    buildPrior raw.family raw.args = .ok raw := by
  simp [buildPrior, priorRaw, accepted]

theorem buildPrior_failure_iff (family args) :
    (∃ issue, buildPrior family args = .error issue) ↔
      ¬ PriorWellFormed (priorRaw family args) := by
  constructor
  · rintro ⟨issue, failed⟩ accepted
    simp [buildPrior, accepted] at failed
  · intro rejected
    refine ⟨(priorIssue [.prior] (priorRaw family args)).getD
      (fallbackError [.prior]), ?_⟩
    simp [buildPrior, rejected]

theorem buildParameter_sound {name ty default prior raw}
    (success : buildParameter name ty default prior = .ok raw) :
    ParameterWellFormed raw ∧ raw = parameterRaw name ty default prior ∧
      raw.name = name ∧ raw.ty = ty ∧ raw.default = default ∧ raw.prior = prior := by
  simp only [buildParameter] at success
  split at success <;> rename_i accepted
  · cases success
    exact ⟨accepted, rfl, rfl, rfl, rfl, rfl⟩
  · contradiction

theorem buildParameter_complete {raw : IR.ParamDecl}
    (accepted : ParameterWellFormed raw) :
    buildParameter raw.name raw.ty raw.default raw.prior = .ok raw := by
  simp [buildParameter, parameterRaw, accepted]

theorem buildParameter_failure_iff (name ty default prior) :
    (∃ issue, buildParameter name ty default prior = .error issue) ↔
      ¬ ParameterWellFormed (parameterRaw name ty default prior) := by
  constructor
  · rintro ⟨issue, failed⟩ accepted
    simp [buildParameter, accepted] at failed
  · intro rejected
    refine ⟨(parameterIssue [.parameter 0] (parameterRaw name ty default prior)).getD
      (fallbackError [.parameter 0]), ?_⟩
    simp [buildParameter, rejected]

theorem buildAttribute_sound {tableNames name ty raw}
    (success : buildAttribute tableNames name ty = .ok raw) :
    AttributeTypeWellFormed tableNames raw.ty ∧ raw = attributeRaw name ty ∧
      raw.name = name ∧ raw.ty = ty := by
  simp only [buildAttribute] at success
  split at success <;> rename_i accepted
  · cases success
    exact ⟨accepted, rfl, rfl, rfl⟩
  · contradiction

theorem buildAttribute_complete {tableNames : List String} {raw : IR.Attr}
    (accepted : AttributeTypeWellFormed tableNames raw.ty) :
    buildAttribute tableNames raw.name raw.ty = .ok raw := by
  simp [buildAttribute, attributeRaw, accepted]

theorem buildAttribute_failure_iff (tableNames name ty) :
    (∃ issue, buildAttribute tableNames name ty = .error issue) ↔
      ¬ AttributeTypeWellFormed tableNames (attributeRaw name ty).ty := by
  constructor
  · rintro ⟨issue, failed⟩ accepted
    simp [buildAttribute, accepted] at failed
  · intro rejected
    refine ⟨(attributeIssue tableNames [.attribute 0] (attributeRaw name ty)).getD
      (fallbackError [.attribute 0]), ?_⟩
    simp [buildAttribute, rejected]

theorem buildTable_sound {tableNames name sizeHint attrs raw}
    (success : buildTable tableNames name sizeHint attrs = .ok raw) :
    SchemaWellFormed tableNames raw.attrs ∧ raw = tableRaw name sizeHint attrs ∧
      raw.name = name ∧ raw.sizeHint = sizeHint ∧ raw.attrs = attrs := by
  simp only [buildTable] at success
  split at success <;> rename_i accepted
  · cases success
    exact ⟨accepted, rfl, rfl, rfl, rfl⟩
  · contradiction

theorem buildTable_complete {tableNames : List String} {raw : IR.Table}
    (accepted : SchemaWellFormed tableNames raw.attrs) :
    buildTable tableNames raw.name raw.sizeHint raw.attrs = .ok raw := by
  simp [buildTable, tableRaw, accepted]

theorem buildTable_failure_iff (tableNames name sizeHint attrs) :
    (∃ issue, buildTable tableNames name sizeHint attrs = .error issue) ↔
      ¬ SchemaWellFormed tableNames (tableRaw name sizeHint attrs).attrs := by
  constructor
  · rintro ⟨issue, failed⟩ accepted
    simp [buildTable, accepted] at failed
  · intro rejected
    refine ⟨(schemaIssue tableNames [.table 0] attrs).getD
      (fallbackError [.table 0]), ?_⟩
    simp [buildTable, rejected]

theorem buildBoxShell_sound {shell raw}
    (success : buildBoxShell shell = .ok raw) :
    BoxDeclarationsWellFormed raw ∧ raw = shell.toRaw ∧
      raw.name = shell.name ∧ raw.tables = shell.tables ∧
      raw.transitions = [] ∧ raw.inputs = [] ∧ raw.outputs = [] ∧
      raw.views = [] ∧ raw.groupedViews = [] := by
  simp only [buildBoxShell] at success
  split at success <;> rename_i accepted
  · cases success
    exact ⟨accepted, rfl, rfl, rfl, rfl, rfl, rfl, rfl, rfl⟩
  · contradiction

theorem buildBoxShell_complete {shell : CoreBoxShell}
    (accepted : BoxDeclarationsWellFormed shell.toRaw) :
    buildBoxShell shell = .ok shell.toRaw := by
  simp [buildBoxShell, accepted]

theorem buildBoxShell_failure_iff (shell : CoreBoxShell) :
    (∃ issue, buildBoxShell shell = .error issue) ↔
      ¬ BoxDeclarationsWellFormed shell.toRaw := by
  constructor
  · rintro ⟨issue, failed⟩ accepted
    simp [buildBoxShell, accepted] at failed
  · intro rejected
    refine ⟨(boxIssue [] shell).getD (fallbackError []), ?_⟩
    simp [buildBoxShell, rejected]

theorem buildModelShell_sound {shell raw}
    (success : buildModelShell shell = .ok raw) :
    DeclarationsWellFormed raw ∧ raw = shell.toRaw ∧
      raw.name = shell.name ∧ raw.dt = shell.dt ∧ raw.params = shell.params ∧
      raw.boxes = shell.boxes.map CoreBoxShell.toRaw ∧
      raw.wires = [] ∧ raw.summaries = [] := by
  simp only [buildModelShell] at success
  split at success <;> rename_i accepted
  · cases success
    exact ⟨accepted, rfl, rfl, rfl, rfl, rfl, rfl, rfl⟩
  · contradiction

theorem buildModelShell_complete {shell : CoreModelShell}
    (accepted : DeclarationsWellFormed shell.toRaw) :
    buildModelShell shell = .ok shell.toRaw := by
  simp [buildModelShell, accepted]

theorem buildModelShell_failure_iff (shell : CoreModelShell) :
    (∃ issue, buildModelShell shell = .error issue) ↔
      ¬ DeclarationsWellFormed shell.toRaw := by
  constructor
  · rintro ⟨issue, failed⟩ accepted
    simp [buildModelShell, accepted] at failed
  · intro rejected
    refine ⟨(modelIssue shell).getD (fallbackError []), ?_⟩
    simp [buildModelShell, rejected]

/-! ## Declaration-only whole-model bridge -/

private structure CoreBoxShape (box : IR.Box) : Prop where
  transitions : box.transitions = []
  inputs : box.inputs = []
  outputs : box.outputs = []
  views : box.views = []
  groupedViews : box.groupedViews = []

private structure CoreModelShape (raw : IR.Model) : Prop where
  boxes : raw.boxes.Forall CoreBoxShape
  wires : raw.wires = []
  summaries : raw.summaries = []

private theorem coreEraseTablesExact (ctx : DeclarationContext)
    (box : BoxId ctx.modelSchema.catalog) :
    ctx.modelSchema.eraseTables box =
      (sourceBox ctx.source ctx.wellFormed box).tables := by
  apply List.ext_get (by
    simp [ModelSchema.eraseTables, ctx.catalogTables_length box])
  intro index leftBound rightBound
  simp only [ModelSchema.eraseTables, List.get_ofFn]
  simpa [sourceTable] using
    ctx.modelSchema_eraseTable_exact
      ⟨box, ⟨index, by simpa [ctx.catalogTables_length box] using leftBound⟩⟩

private def emptyCheckedBox {ctx : DeclarationContext}
    (box : BoxId ctx.modelSchema.catalog) : CheckedBox ctx box :=
  ⟨box.ordinal.val, [], [], [], []⟩

private def emptyCheckedBoxPack (ctx : DeclarationContext)
    (ordinal : Fin ctx.modelSchema.catalog.boxes.entries.length) :
    CheckedBoxPack ctx :=
  let box : BoxId ctx.modelSchema.catalog := ⟨ordinal⟩
  ⟨box, emptyCheckedBox box⟩

private theorem emptyBoxWellFormed (ctx : DeclarationContext)
    (box : BoxId ctx.modelSchema.catalog)
    (shape : CoreBoxShape (sourceBox ctx.source ctx.wellFormed box)) :
    BoxPackWellFormed (ctx := ctx)
      (sourceBox ctx.source ctx.wellFormed box)
      (emptyCheckedBoxPack ctx box.ordinal) := by
  apply BoxPackWellFormed.mk
  apply BoxTermsWellFormed.mk
  · rfl
  · rfl
  · rfl
  · exact (coreEraseTablesExact ctx box).symm
  · rfl
  · simp [emptyCheckedBoxPack, emptyCheckedBox, shape.transitions]
  · simp [emptyCheckedBoxPack, emptyCheckedBox, shape.transitions]
  · simp [emptyCheckedBoxPack, emptyCheckedBox, shape.outputs]
  · simp [emptyCheckedBoxPack, emptyCheckedBox, shape.outputs]
  · simp [emptyCheckedBoxPack, emptyCheckedBox, shape.views]
  · simp [emptyCheckedBoxPack, emptyCheckedBox, shape.views]
  · simp [emptyCheckedBoxPack, emptyCheckedBox, shape.groupedViews]
  · simp [emptyCheckedBoxPack, emptyCheckedBox, shape.groupedViews]

private theorem coreCanonicalBoxOrdinals (ctx : DeclarationContext) :
    (List.ofFn id :
      List (Fin ctx.modelSchema.catalog.boxes.entries.length)).map Fin.val =
      List.range ctx.source.boxes.length := by
  apply List.ext_get (by simp [ctx.catalogBoxes_length])
  intro index leftBound rightBound
  simp [List.get_ofFn]

private theorem coreSourceBoxesFromOrdinals (ctx : DeclarationContext) :
    (List.ofFn id :
      List (Fin ctx.modelSchema.catalog.boxes.entries.length)).map
        (fun ordinal => sourceBox ctx.source ctx.wellFormed ⟨ordinal⟩) =
      ctx.source.boxes := by
  apply List.ext_get (by simp [ctx.catalogBoxes_length])
  intro index leftBound rightBound
  simp [sourceBox]

private theorem coreSourceBoxShape (ctx : DeclarationContext)
    (shapes : ctx.source.boxes.Forall CoreBoxShape)
    (ordinal : Fin ctx.modelSchema.catalog.boxes.entries.length) :
    CoreBoxShape
      (sourceBox ctx.source ctx.wellFormed (⟨ordinal⟩ : BoxId _)) := by
  apply (List.forall_iff_forall_mem.mp shapes)
  exact List.get_mem _ _ _

private theorem coreBoxesWellFormedAux (ctx : DeclarationContext)
    (shapes : ctx.source.boxes.Forall CoreBoxShape)
    (ordinals : List (Fin ctx.modelSchema.catalog.boxes.entries.length)) :
    List.Forall₂ (BoxPackWellFormed (ctx := ctx))
      (ordinals.map
        (fun ordinal => sourceBox ctx.source ctx.wellFormed ⟨ordinal⟩))
      (ordinals.map (emptyCheckedBoxPack ctx)) := by
  induction ordinals with
  | nil => simp
  | cons ordinal ordinals ih =>
      simp only [List.map_cons]
      apply List.Forall₂.cons
      · exact emptyBoxWellFormed ctx ⟨ordinal⟩
          (coreSourceBoxShape ctx shapes ordinal)
      · exact ih

private theorem coreBoxesWellFormed (ctx : DeclarationContext)
    (shapes : ctx.source.boxes.Forall CoreBoxShape) :
    List.Forall₂ (BoxPackWellFormed (ctx := ctx)) ctx.source.boxes
      ((List.ofFn id :
        List (Fin ctx.modelSchema.catalog.boxes.entries.length)).map
          (emptyCheckedBoxPack ctx)) := by
  rw [← coreSourceBoxesFromOrdinals ctx]
  exact coreBoxesWellFormedAux ctx shapes _

private theorem coreBoxesOrder (ctx : DeclarationContext) :
    ((List.ofFn id :
      List (Fin ctx.modelSchema.catalog.boxes.entries.length)).map
        (emptyCheckedBoxPack ctx)).map
          (fun entry => entry.box.ordinal.val) =
      List.range ctx.source.boxes.length := by
  simpa [emptyCheckedBoxPack] using coreCanonicalBoxOrdinals ctx

private theorem CoreBoxShell.toRaw_shape (shell : CoreBoxShell) :
    CoreBoxShape shell.toRaw := ⟨rfl, rfl, rfl, rfl, rfl⟩

private theorem CoreModelShell.toRaw_shape (shell : CoreModelShell) :
    CoreModelShape shell.toRaw := by
  constructor
  · change (shell.boxes.map CoreBoxShell.toRaw).Forall CoreBoxShape
    induction shell.boxes with
    | nil => simp
    | cons head tail ih =>
        rw [List.map_cons, List.forall_cons]
        exact ⟨head.toRaw_shape, ih⟩
  · rfl
  · rfl

private theorem coreModelShell_modelWellFormed (shell : CoreModelShell)
    (declarations : DeclarationsWellFormed shell.toRaw) :
    ModelWellFormed shell.toRaw := by
  let raw := shell.toRaw
  let ctx : DeclarationContext := ⟨raw, declarations⟩
  let boxes :=
    (List.ofFn id :
      List (Fin ctx.modelSchema.catalog.boxes.entries.length)).map
        (emptyCheckedBoxPack ctx)
  let checked : Checked.Model := ⟨ctx, boxes, [], []⟩
  refine ⟨checked, ?_⟩
  apply ModelElaborates.mk
  · rfl
  · exact declarations
  · exact coreBoxesWellFormed ctx shell.toRaw_shape.boxes
  · exact coreBoxesOrder ctx
  · simp
  · simp
  · rfl

/-- Successful model-shell construction is accepted by the declaration checker. -/
theorem buildModelShell_declaration_acceptance {shell raw}
    (success : buildModelShell shell = .ok raw) :
    ∃ ctx, checkDeclarations raw = .ok ctx :=
  checkDeclarations_complete (buildModelShell_sound success).1

/-- A successful complete shell is accepted by the whole-model checker and
its checked result erases to the exact constructed raw model. -/
theorem buildModelShell_model_acceptance_and_erasure {shell raw}
    (success : buildModelShell shell = .ok raw) :
    ∃ checked, checkModel raw = .ok checked ∧ checked.erase = raw := by
  have sound := buildModelShell_sound success
  have declarations : DeclarationsWellFormed shell.toRaw := by
    rw [← sound.2.1]
    exact sound.1
  rw [sound.2.1]
  obtain ⟨checked, checkedOk⟩ :=
    checkModel_complete (coreModelShell_modelWellFormed shell declarations)
  exact ⟨checked, checkedOk, (checkModel_sound checkedOk).2⟩

end Sembla.Frontend.Builders
