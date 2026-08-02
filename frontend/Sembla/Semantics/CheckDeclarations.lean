import Sembla.Semantics.Syntax

/-!
Executable declaration checking for the V1 IR.

This module deliberately separates the structural `DeclarationsWellFormed`
judgment from diagnostics and from the canonical checker.  The checked context
retains the raw source and derives all checked catalogs from that source, so its
exact declaration erasure cannot drift from the accepted input.
-/
namespace Sembla.Semantics

open Sembla

/-! ## Exact executable scientific comparisons -/

/-- Exact rational value of a raw finite decimal. -/
def scientificRat (value : IR.Scientific) : ℚ :=
  (value.coefficient : ℚ) * (10 : ℚ) ^ value.exponent

/-- Executable exact strict order; no floating-point conversion is involved. -/
def scientificLt (left right : IR.Scientific) : Bool :=
  decide (scientificRat left < scientificRat right)

/-- Executable exact positivity. Powers of ten are positive, so the coefficient
alone determines the sign. -/
def scientificPositive (value : IR.Scientific) : Bool :=
  decide (0 < value.coefficient)

@[simp] theorem scientificRat_cast (value : IR.Scientific) :
    (scientificRat value : ℝ) = scientificDenote value := by
  simp [scientificRat, scientificDenote]

@[simp] theorem scientificLt_iff_denote_lt (left right : IR.Scientific) :
    scientificLt left right = true ↔
      scientificDenote left < scientificDenote right := by
  simp [scientificLt, ← scientificRat_cast]

@[simp] theorem scientificPositive_iff_denote_pos (value : IR.Scientific) :
    scientificPositive value = true ↔ 0 < scientificDenote value := by
  simp only [scientificPositive, decide_eq_true_eq, scientificDenote]
  have powerPositive : 0 < (10 : ℝ) ^ value.exponent := by positivity
  constructor
  · intro positive
    exact mul_pos (by exact_mod_cast positive) powerPositive
  · intro positive
    by_contra notPositive
    have coefficientNonpositive : (value.coefficient : ℝ) ≤ 0 := by
      exact_mod_cast (le_of_not_gt notPositive)
    exact (not_lt_of_ge (mul_nonpos_of_nonpos_of_nonneg coefficientNonpositive
      powerPositive.le)) positive

/-! ## Independent structural judgment -/

/-- Source names are unique in their one declared namespace. -/
def NamesUnique {α : Type} (entries : List α) (nameOf : α → String) : Prop :=
  (entries.map nameOf).Nodup

/-- Executable structural prior predicate accepted by V1. -/
def priorWellFormedBool (prior : IR.Prior) : Bool :=
  match prior.args with
  | [lower, upper] =>
      match prior.family with
      | .uniform => scientificLt lower upper
      | .normal | .logNormal => true
  | _ => false

/-- Declarative prior judgment, independent of the declaration checker. -/
def PriorWellFormed (prior : IR.Prior) : Prop := priorWellFormedBool prior = true

/-- Executable default and prior predicate for one raw parameter. -/
def parameterWellFormedBool (parameter : IR.ParamDecl) : Bool :=
  match parameter.ty, parameter.default with
  | .real, .real _ =>
      match parameter.prior with
      | none => true
      | some prior => priorWellFormedBool prior
  | .int, .int _ => parameter.prior.isNone
  | _, _ => false

/-- Declarative parameter judgment, independent of the declaration checker. -/
def ParameterWellFormed (parameter : IR.ParamDecl) : Prop :=
  parameterWellFormedBool parameter = true

/-- Executable attribute predicate relative to the complete box table catalog. -/
def attributeTypeWellFormedBool (tableNames : List String)
    (attrType : IR.AttrType) : Bool :=
  match attrType with
  | .real | .int => true
  | .enum variants => !variants.isEmpty && decide variants.Nodup
  | .ref table => decide (table ∈ tableNames)

/-- Declarative attribute declaration conditions. -/
def AttributeTypeWellFormed (tableNames : List String) (attrType : IR.AttrType) : Prop :=
  attributeTypeWellFormedBool tableNames attrType = true

/-- A schema has one ordered attribute namespace and all references resolve
against the already complete table-name catalog. -/
def SchemaWellFormed (tableNames : List String) (attrs : List IR.Attr) : Prop :=
  NamesUnique attrs IR.Attr.name ∧
    attrs.Forall (fun attr => AttributeTypeWellFormed tableNames attr.ty)

namespace SchemaWellFormed

theorem uniqueAttributes {tableNames attrs} (h : SchemaWellFormed tableNames attrs) :
    NamesUnique attrs IR.Attr.name := h.1

theorem attributeTypes {tableNames attrs} (h : SchemaWellFormed tableNames attrs) :
    attrs.Forall (fun attr => AttributeTypeWellFormed tableNames attr.ty) := h.2

end SchemaWellFormed

/-- All declaration-only obligations owned inside one raw box. -/
def BoxDeclarationsWellFormed (box : IR.Box) : Prop :=
  NamesUnique box.tables IR.Table.name ∧
  NamesUnique box.transitions IR.Transition.name ∧
  NamesUnique box.inputs IR.PortDecl.name ∧
  NamesUnique box.outputs IR.OutputDecl.name ∧
  ((box.views.map IR.ViewDecl.name) ++
      (box.groupedViews.map IR.GroupedViewDecl.name)).Nodup ∧
  box.tables.Forall
    (fun table => SchemaWellFormed (box.tables.map IR.Table.name) table.attrs) ∧
  box.inputs.Forall
    (fun input => SchemaWellFormed (box.tables.map IR.Table.name) input.schema) ∧
  box.outputs.Forall
    (fun output => SchemaWellFormed (box.tables.map IR.Table.name) output.schema) ∧
  box.transitions.Forall
    (fun transition => transition.table ∈ box.tables.map IR.Table.name)

namespace BoxDeclarationsWellFormed

theorem uniqueTables {box} (h : BoxDeclarationsWellFormed box) :
    NamesUnique box.tables IR.Table.name := h.1
theorem uniqueTransitions {box} (h : BoxDeclarationsWellFormed box) :
    NamesUnique box.transitions IR.Transition.name := h.2.1
theorem uniqueInputs {box} (h : BoxDeclarationsWellFormed box) :
    NamesUnique box.inputs IR.PortDecl.name := h.2.2.1
theorem uniqueOutputs {box} (h : BoxDeclarationsWellFormed box) :
    NamesUnique box.outputs IR.OutputDecl.name := h.2.2.2.1
theorem uniqueViews {box} (h : BoxDeclarationsWellFormed box) :
    ((box.views.map IR.ViewDecl.name) ++
      (box.groupedViews.map IR.GroupedViewDecl.name)).Nodup := h.2.2.2.2.1
theorem tableSchemas {box} (h : BoxDeclarationsWellFormed box) :
    box.tables.Forall
      (fun table => SchemaWellFormed (box.tables.map IR.Table.name) table.attrs) :=
  h.2.2.2.2.2.1
theorem inputSchemas {box} (h : BoxDeclarationsWellFormed box) :
    box.inputs.Forall
      (fun input => SchemaWellFormed (box.tables.map IR.Table.name) input.schema) :=
  h.2.2.2.2.2.2.1
theorem outputSchemas {box} (h : BoxDeclarationsWellFormed box) :
    box.outputs.Forall
      (fun output => SchemaWellFormed (box.tables.map IR.Table.name) output.schema) :=
  h.2.2.2.2.2.2.2.1
theorem transitionTargets {box} (h : BoxDeclarationsWellFormed box) :
    box.transitions.Forall
      (fun transition => transition.table ∈ box.tables.map IR.Table.name) :=
  h.2.2.2.2.2.2.2.2

end BoxDeclarationsWellFormed

/-- Structural declaration judgment.  It contains no reference to the checker,
its diagnostic function or the existence of a checked context. -/
def DeclarationsWellFormed (raw : IR.Model) : Prop :=
  0 < raw.dt.coefficient ∧
  NamesUnique raw.params IR.ParamDecl.name ∧
  raw.params.Forall ParameterWellFormed ∧
  NamesUnique raw.boxes IR.Box.name ∧
  raw.boxes.Forall BoxDeclarationsWellFormed ∧
  NamesUnique raw.summaries IR.SummaryDecl.name

namespace DeclarationsWellFormed

theorem dtPositive {raw} (h : DeclarationsWellFormed raw) :
    0 < raw.dt.coefficient := h.1
theorem uniqueParameters {raw} (h : DeclarationsWellFormed raw) :
    NamesUnique raw.params IR.ParamDecl.name := h.2.1
theorem parameters {raw} (h : DeclarationsWellFormed raw) :
    raw.params.Forall ParameterWellFormed := h.2.2.1
theorem uniqueBoxes {raw} (h : DeclarationsWellFormed raw) :
    NamesUnique raw.boxes IR.Box.name := h.2.2.2.1
theorem boxes {raw} (h : DeclarationsWellFormed raw) :
    raw.boxes.Forall BoxDeclarationsWellFormed := h.2.2.2.2.1
theorem uniqueSummaries {raw} (h : DeclarationsWellFormed raw) :
    NamesUnique raw.summaries IR.SummaryDecl.name := h.2.2.2.2.2

end DeclarationsWellFormed

instance priorWellFormedDecidable (prior : IR.Prior) : Decidable (PriorWellFormed prior) :=
  inferInstanceAs (Decidable (priorWellFormedBool prior = true))

instance parameterWellFormedDecidable (parameter : IR.ParamDecl) :
    Decidable (ParameterWellFormed parameter) :=
  inferInstanceAs (Decidable (parameterWellFormedBool parameter = true))

instance attributeTypeWellFormedDecidable (tables : List String)
    (attrType : IR.AttrType) : Decidable (AttributeTypeWellFormed tables attrType) :=
  inferInstanceAs (Decidable (attributeTypeWellFormedBool tables attrType = true))

instance decidableListForall {α : Type} (predicate : α → Prop)
    [decision : (entry : α) → Decidable (predicate entry)] :
    (entries : List α) → Decidable (entries.Forall predicate)
  | [] => isTrue (by simp)
  | entry :: entries =>
      decidable_of_iff (predicate entry ∧ entries.Forall predicate)
        ((List.forall_cons predicate entry entries).symm)

instance schemaWellFormedDecidable (tables : List String) (attrs : List IR.Attr) :
    Decidable (SchemaWellFormed tables attrs) := by
  unfold SchemaWellFormed NamesUnique
  infer_instance

instance boxDeclarationsWellFormedDecidable (box : IR.Box) :
    Decidable (BoxDeclarationsWellFormed box) := by
  unfold BoxDeclarationsWellFormed NamesUnique
  infer_instance

instance declarationsWellFormedDecidable (raw : IR.Model) :
    Decidable (DeclarationsWellFormed raw) := by
  unfold DeclarationsWellFormed NamesUnique
  infer_instance

/-! ## Stable structured diagnostics -/

inductive CheckErrorCategory where
  | nonpositiveDt
  | duplicateName
  | parameterDefaultMismatch
  | integerPrior
  | priorArity
  | unorderedUniform
  | emptyEnum
  | duplicateEnumVariant
  | unresolvedTableReference
  | unresolvedTransitionTable
  deriving Repr, BEq, DecidableEq

inductive CheckPathSegment where
  | dt
  | parameters
  | parameter (index : Nat)
  | boxes
  | box (index : Nat)
  | tables
  | table (index : Nat)
  | transitions
  | transition (index : Nat)
  | inputs
  | input (index : Nat)
  | outputs
  | output (index : Nat)
  | views
  | view (index : Nat)
  | groupedViews
  | groupedView (index : Nat)
  | summaries
  | summary (index : Nat)
  | schema
  | attribute (index : Nat)
  | prior
  | argument (index : Nat)
  | enumVariant (index : Nat)
  | name
  | ty
  | default
  | tableTarget
  deriving Repr, BEq, DecidableEq

structure CheckError where
  category : CheckErrorCategory
  path : List CheckPathSegment
  deriving Repr, BEq, DecidableEq

private def firstDuplicateIndexAux (seen : List String) :
    Nat → List String → Option Nat
  | _, [] => none
  | index, name :: names =>
      if name ∈ seen then some index
      else firstDuplicateIndexAux (name :: seen) (index + 1) names

/-- Index of the second occurrence of the first duplicate source name. -/
def firstDuplicateIndex? (names : List String) : Option Nat :=
  firstDuplicateIndexAux [] 0 names

private def duplicateAt (pathPrefix : List CheckPathSegment)
    (atIndex : Nat → CheckPathSegment) (names : List String) : Option CheckError :=
  (firstDuplicateIndex? names).map fun index =>
    { category := .duplicateName, path := pathPrefix ++ [atIndex index, .name] }

private def firstIndexedErrorAux {α : Type} (check : Nat → α → Option CheckError) :
    Nat → List α → Option CheckError
  | _, [] => none
  | index, entry :: entries =>
      match check index entry with
      | some err => some err
      | none => firstIndexedErrorAux check (index + 1) entries

private def firstIndexedError {α : Type} (entries : List α)
    (check : Nat → α → Option CheckError) : Option CheckError :=
  firstIndexedErrorAux check 0 entries

private def parameterError (index : Nat) (parameter : IR.ParamDecl) : Option CheckError :=
  let pathRoot := [.parameters, .parameter index]
  match parameter.ty, parameter.default with
  | .real, .int _ | .int, .real _ =>
      some { category := .parameterDefaultMismatch, path := pathRoot ++ [.default] }
  | .int, .int _ =>
      match parameter.prior with
      | some _ => some { category := .integerPrior, path := pathRoot ++ [.prior] }
      | none => none
  | .real, .real _ =>
      match parameter.prior with
      | none => none
      | some prior =>
          match prior.args with
          | [lower, upper] =>
              match prior.family with
              | .uniform =>
                  if scientificLt lower upper then none
                  else some { category := .unorderedUniform, path := pathRoot ++ [.prior, .argument 1] }
              | .normal | .logNormal => none
          | _ => some { category := .priorArity, path := pathRoot ++ [.prior] }

private def attributeTypeError (tableNames : List String)
    (pathRoot : List CheckPathSegment) (attrType : IR.AttrType) : Option CheckError :=
  match attrType with
  | .real | .int => none
  | .enum [] => some { category := .emptyEnum, path := pathRoot }
  | .enum variants =>
      (firstDuplicateIndex? variants).map fun index =>
        { category := .duplicateEnumVariant, path := pathRoot ++ [.enumVariant index] }
  | .ref table =>
      if table ∈ tableNames then none
      else some { category := .unresolvedTableReference, path := pathRoot ++ [.tableTarget] }

private def schemaError (tableNames : List String) (attrs : List IR.Attr)
    (pathRoot : List CheckPathSegment) : Option CheckError :=
  match duplicateAt (pathRoot ++ [.schema]) .attribute (attrs.map IR.Attr.name) with
  | some err => some err
  | none => firstIndexedError attrs fun index attr =>
      attributeTypeError tableNames (pathRoot ++ [.schema, .attribute index]) attr.ty

private def boxError (boxIndex : Nat) (box : IR.Box) : Option CheckError :=
  let root := [.boxes, .box boxIndex]
  let tableNames := box.tables.map IR.Table.name
  match duplicateAt (root ++ [.tables]) .table tableNames with
  | some err => some err
  | none =>
    match duplicateAt (root ++ [.transitions]) .transition
        (box.transitions.map IR.Transition.name) with
    | some err => some err
    | none =>
      match duplicateAt (root ++ [.inputs]) .input
          (box.inputs.map IR.PortDecl.name) with
      | some err => some err
      | none =>
        match duplicateAt (root ++ [.outputs]) .output
            (box.outputs.map IR.OutputDecl.name) with
        | some err => some err
        | none =>
          let viewNames := box.views.map IR.ViewDecl.name
          let allViewNames := viewNames ++ box.groupedViews.map IR.GroupedViewDecl.name
          match firstDuplicateIndex? allViewNames with
          | some index =>
              if index < viewNames.length then
                some { category := .duplicateName, path := root ++ [.views, .view index, .name] }
              else
                some { category := .duplicateName, path := root ++ [.groupedViews,
                    .groupedView (index - viewNames.length), .name] }
          | none =>
            match firstIndexedError box.tables fun index table =>
                schemaError tableNames table.attrs (root ++ [.tables, .table index]) with
            | some err => some err
            | none =>
              match firstIndexedError box.inputs fun index input =>
                  schemaError tableNames input.schema (root ++ [.inputs, .input index]) with
              | some err => some err
              | none =>
                match firstIndexedError box.outputs fun index output =>
                    schemaError tableNames output.schema
                      (root ++ [.outputs, .output index]) with
                | some err => some err
                | none => firstIndexedError box.transitions fun index transition =>
                    if transition.table ∈ tableNames then none
                    else some { category := .unresolvedTransitionTable, path := root ++ [.transitions, .transition index, .tableTarget] }

/-- Deterministic diagnostics.  The checker decision itself remains the separate
structural judgment below. -/
def diagnoseDeclarationError (raw : IR.Model) : Option CheckError :=
  if !scientificPositive raw.dt then
    some { category := .nonpositiveDt, path := [.dt] }
  else
    match duplicateAt [.parameters] .parameter (raw.params.map IR.ParamDecl.name) with
    | some err => some err
    | none =>
      match duplicateAt [.boxes] .box (raw.boxes.map IR.Box.name) with
      | some err => some err
      | none =>
        match duplicateAt [.summaries] .summary
            (raw.summaries.map IR.SummaryDecl.name) with
        | some err => some err
        | none =>
          match firstIndexedError raw.params parameterError with
          | some err => some err
          | none => firstIndexedError raw.boxes boxError

private def fallbackError : CheckError :=
  { category := .nonpositiveDt, path := [.dt] }

/-- Stable first error; its absence is definitionally controlled by the
independent structural judgment. -/
def firstDeclarationError (raw : IR.Model) : Option CheckError :=
  if DeclarationsWellFormed raw then none
  else some ((diagnoseDeclarationError raw).getD fallbackError)

@[simp] theorem firstDeclarationError_none_iff (raw : IR.Model) :
    firstDeclarationError raw = none ↔ DeclarationsWellFormed raw := by
  simp [firstDeclarationError]

/-! ## Canonical checked contexts -/

private def checkedParam (parameter : IR.ParamDecl) : CheckedParamDecl :=
  match parameter.ty, parameter.default with
  | .real, .real value =>
      { name := parameter.name, sort := .real, default := ⟨value⟩,
        prior := parameter.prior }
  | .int, .int value =>
      { name := parameter.name, sort := .int, default := value,
        prior := parameter.prior }
  | .real, .int _ =>
      { name := parameter.name, sort := .real, default := ⟨⟨0, 0⟩⟩,
        prior := parameter.prior }
  | .int, .real _ =>
      { name := parameter.name, sort := .int, default := (0 : Int),
        prior := parameter.prior }

private theorem checkedParam_erase_of_wellFormed (parameter : IR.ParamDecl)
    (wellFormed : ParameterWellFormed parameter) :
    (checkedParam parameter).erase = parameter := by
  rcases parameter with ⟨name, ty, default, prior⟩
  cases ty <;> cases default <;>
    simp [ParameterWellFormed, parameterWellFormedBool, checkedParam,
      CheckedParamDecl.erase, ParamSort.erase, ParamLiteral.erase] at wellFormed ⊢

private def checkedParams (parameters : List IR.ParamDecl) : List CheckedParamDecl :=
  parameters.map checkedParam

private theorem checkedParams_names (parameters : List IR.ParamDecl) :
    (checkedParams parameters).map CheckedParamDecl.name =
      parameters.map IR.ParamDecl.name := by
  induction parameters with
  | nil => rfl
  | cons parameter parameters ih =>
      change (checkedParam parameter).name ::
        (checkedParams parameters).map CheckedParamDecl.name =
        parameter.name :: parameters.map IR.ParamDecl.name
      rw [ih]
      rcases parameter with ⟨name, ty, default, prior⟩
      cases ty <;> cases default <;> rfl

private theorem checkedParams_erase (parameters : List IR.ParamDecl)
    (wellFormed : parameters.Forall ParameterWellFormed) :
    (checkedParams parameters).map CheckedParamDecl.erase = parameters := by
  induction parameters with
  | nil => rfl
  | cons parameter parameters ih =>
      simp only [List.forall_cons] at wellFormed
      change (checkedParam parameter).erase ::
        (checkedParams parameters).map CheckedParamDecl.erase =
        parameter :: parameters
      rw [checkedParam_erase_of_wellFormed parameter wellFormed.1, ih wellFormed.2]

private def buildParamContext (raw : IR.Model) (wellFormed : DeclarationsWellFormed raw) :
    ParamContext :=
  { scope := raw.name
    entries := checkedParams raw.params
    uniqueNames := by
      rw [checkedParams_names]
      exact wellFormed.uniqueParameters }

private def buildTableHeader (table : IR.Table) : TableHeader :=
  { name := table.name, sizeHint := table.sizeHint }

private def buildBoxHeader (box : IR.Box) (wellFormed : BoxDeclarationsWellFormed box) :
    BoxHeader :=
  { name := box.name
    tables :=
      { scope := box.name
        entries := box.tables.map buildTableHeader
        uniqueNames := by
          simpa [buildTableHeader, NamesUnique] using wellFormed.uniqueTables } }

private def buildBoxHeaders (boxes : List IR.Box)
    (wellFormed : boxes.Forall BoxDeclarationsWellFormed) : List BoxHeader :=
  boxes.attach.map fun boxed => buildBoxHeader boxed.1
    ((List.forall_iff_forall_mem.mp wellFormed) boxed.1 boxed.2)

private theorem buildBoxHeaders_names (boxes : List IR.Box)
    (wellFormed : boxes.Forall BoxDeclarationsWellFormed) :
    (buildBoxHeaders boxes wellFormed).map BoxHeader.name = boxes.map IR.Box.name := by
  simp [buildBoxHeaders, buildBoxHeader]

private theorem buildBoxHeaders_length (boxes : List IR.Box)
    (wellFormed : boxes.Forall BoxDeclarationsWellFormed) :
    (buildBoxHeaders boxes wellFormed).length = boxes.length := by
  simp [buildBoxHeaders]

private def buildCatalog (raw : IR.Model) (wellFormed : DeclarationsWellFormed raw) :
    SchemaUniverse :=
  { boxes :=
      { scope := raw.name
        entries := buildBoxHeaders raw.boxes wellFormed.boxes
        uniqueNames := by
          rw [buildBoxHeaders_names]
          exact wellFormed.uniqueBoxes } }

namespace OrderedContext

/-- Every member of an ordered name list is found by the executable scanner. -/
theorem findNameIndex_complete {wanted : String} {names : List String}
    (member : wanted ∈ names) :
    ∃ index, findNameIndex wanted names = some index := by
  induction names with
  | nil => simp at member
  | cons head tail ih =>
      simp only [List.mem_cons] at member
      rcases member with same | member
      · subst wanted
        refine ⟨⟨0, by simp⟩, ?_⟩
        simp [findNameIndex]
      · obtain ⟨index, found⟩ := ih member
        by_cases same : head = wanted
        · subst wanted
          refine ⟨⟨0, by simp⟩, ?_⟩
          simp [findNameIndex]
        · refine ⟨index.succ, ?_⟩
          simp [findNameIndex, same, found]

/-- Membership completeness lifted to every accepted ordered context. -/
theorem lookupOrdinal_complete {α : Type} {nameOf : α → String}
    (ctx : OrderedContext α nameOf) {wanted : String}
    (member : wanted ∈ ctx.entries.map nameOf) :
    ∃ index, ctx.lookupOrdinal wanted = some index := by
  obtain ⟨rawIndex, found⟩ := findNameIndex_complete member
  refine ⟨Fin.cast (by simp) rawIndex, ?_⟩
  simp [lookupOrdinal, found]

end OrderedContext

namespace SchemaUniverse

/-- Complete box-local table lookup for a name known to the table catalog. -/
theorem lookupTable_complete (catalog : SchemaUniverse) (box : BoxId catalog)
    {wanted : String}
    (member : wanted ∈ (catalog.boxHeader box).tables.entries.map TableHeader.name) :
    ∃ table, catalog.lookupTable box wanted = some table := by
  obtain ⟨index, found⟩ := (catalog.boxHeader box).tables.lookupOrdinal_complete member
  refine ⟨⟨index⟩, ?_⟩
  simp [lookupTable, found]

end SchemaUniverse

/-- The source box selected by a checked box identifier. -/
def sourceBox (raw : IR.Model) (wellFormed : DeclarationsWellFormed raw)
    (box : BoxId (buildCatalog raw wellFormed)) : IR.Box :=
  raw.boxes.get ⟨box.ordinal.val, by
    simpa [buildCatalog, buildBoxHeaders_length] using box.ordinal.isLt⟩

private def sourceBoxIndex (raw : IR.Model) (wellFormed : DeclarationsWellFormed raw)
    (box : BoxId (buildCatalog raw wellFormed)) : Fin raw.boxes.length :=
  ⟨box.ordinal.val, by
    simpa [buildCatalog, buildBoxHeaders_length] using box.ordinal.isLt⟩

private def sourceBoxWellFormed (raw : IR.Model)
    (wellFormed : DeclarationsWellFormed raw)
    (box : BoxId (buildCatalog raw wellFormed)) :
    BoxDeclarationsWellFormed (sourceBox raw wellFormed box) := by
  apply (List.forall_iff_forall_mem.mp wellFormed.boxes)
  simp [sourceBox, sourceBoxIndex]

private theorem catalogTableLength (raw : IR.Model)
    (wellFormed : DeclarationsWellFormed raw)
    (box : BoxId (buildCatalog raw wellFormed)) :
    ((buildCatalog raw wellFormed).boxHeader box).tables.entries.length =
      (sourceBox raw wellFormed box).tables.length := by
  simp [SchemaUniverse.boxHeader, buildCatalog, buildBoxHeaders, sourceBox,
    sourceBoxIndex, buildBoxHeader]

private theorem catalogTableNames (raw : IR.Model)
    (wellFormed : DeclarationsWellFormed raw)
    (box : BoxId (buildCatalog raw wellFormed)) :
    ((buildCatalog raw wellFormed).boxHeader box).tables.entries.map TableHeader.name =
      (sourceBox raw wellFormed box).tables.map IR.Table.name := by
  simp [SchemaUniverse.boxHeader, buildCatalog, buildBoxHeaders, sourceBox,
    sourceBoxIndex, buildBoxHeader, buildTableHeader]

private theorem buildBoxHeader_tables_length (box : IR.Box)
    (wellFormed : BoxDeclarationsWellFormed box) :
    (buildBoxHeader box wellFormed).tables.entries.length = box.tables.length := by
  simp [buildBoxHeader]

/-- The source table selected by a checked table target. -/
def sourceTable (raw : IR.Model) (wellFormed : DeclarationsWellFormed raw)
    (target : TableTarget (buildCatalog raw wellFormed)) : IR.Table :=
  (sourceBox raw wellFormed target.box).tables.get
    ⟨target.table.ordinal.val, by
      have bound := target.table.ordinal.isLt
      have lengths := catalogTableLength raw wellFormed target.box
      omega⟩

def attrShapeFromRaw (catalog : SchemaUniverse) (owner : TableTarget catalog) :
    IR.AttrType → AttrShape catalog owner
  | .real => .real
  | .int => .int
  | .enum variants =>
      if nonempty : variants ≠ [] then
        if unique : variants.Nodup then .enum ⟨variants, nonempty, unique⟩
        else .enum ⟨["invalid"], by decide, by decide⟩
      else .enum ⟨["invalid"], by decide, by decide⟩
  | .ref table => .ref ((catalog.lookupTable owner.box table).getD owner.table)

/-- Successful reference resolution selects exactly the resolved owner-indexed
identifier and erases back to its source spelling. -/
@[simp] theorem resolvedReference_erases_name (catalog : SchemaUniverse)
    (owner : TableTarget catalog) (name : String)
    (target : TableId catalog owner.box)
    (found : catalog.lookupTable owner.box name = some target) :
    (attrShapeFromRaw catalog owner (.ref name)).erase = .ref name := by
  have targetName := catalog.tableLookup_name owner.box found
  simp [attrShapeFromRaw, found, AttrShape.erase, targetName]

/-- Valid enum declarations retain variant spelling and source order exactly. -/
@[simp] theorem resolvedEnum_erases_exact (catalog : SchemaUniverse)
    (owner : TableTarget catalog) (variants : List String)
    (nonempty : variants ≠ []) (unique : variants.Nodup) :
    (attrShapeFromRaw catalog owner (.enum variants)).erase = .enum variants := by
  simp [attrShapeFromRaw, nonempty, unique, AttrShape.erase]

private def checkedAttributeFromRaw (catalog : SchemaUniverse) (owner : TableTarget catalog)
    (attr : IR.Attr) : CheckedAttribute catalog owner :=
  { name := attr.name, shape := attrShapeFromRaw catalog owner attr.ty }

private def buildTableSchema (raw : IR.Model) (wellFormed : DeclarationsWellFormed raw)
    (target : TableTarget (buildCatalog raw wellFormed)) :
    TableSchema (buildCatalog raw wellFormed) target :=
  let table := sourceTable raw wellFormed target
  let boxWellFormed := sourceBoxWellFormed raw wellFormed target.box
  let tableIndex : Fin (sourceBox raw wellFormed target.box).tables.length :=
    ⟨target.table.ordinal.val, by
      have bound := target.table.ordinal.isLt
      have lengths := catalogTableLength raw wellFormed target.box
      omega⟩
  let tableWellFormed := (List.forall_iff_forall_mem.mp boxWellFormed.tableSchemas) _
    (List.get_mem (sourceBox raw wellFormed target.box).tables tableIndex.val tableIndex.isLt)
  { attributes :=
      { scope := (sourceBox raw wellFormed target.box).name ++ "." ++ table.name
        entries := table.attrs.map (checkedAttributeFromRaw (buildCatalog raw wellFormed) target)
        uniqueNames := by
          simpa [checkedAttributeFromRaw, NamesUnique] using tableWellFormed.uniqueAttributes } }

/-- Canonical accepted schema, derived from exactly the checked raw source. -/
private def buildModelSchema (raw : IR.Model) (wellFormed : DeclarationsWellFormed raw) : ModelSchema :=
  { params := buildParamContext raw wellFormed
    catalog := buildCatalog raw wellFormed
    tableSchemas := fun target => buildTableSchema raw wellFormed target }

/-- A box-owned port schema is valid even when the owning box has no tables. -/
structure BoxPortSchema (catalog : SchemaUniverse) (box : BoxId catalog) where
  name : String
  source : List IR.Attr
  tableNames : List String
  tableNamesMatch :
    (catalog.boxHeader box).tables.entries.map TableHeader.name = tableNames
  sourceWellFormed : SchemaWellFormed tableNames source

namespace BoxPortSchema

/-- Instantiate a box-owned port schema at a later transition's current table. -/
def instantiate {catalog : SchemaUniverse} {box : BoxId catalog}
    (schema : BoxPortSchema catalog box) (current : TableId catalog box) :
    TableSchema catalog ⟨box, current⟩ :=
  { attributes :=
      { scope := schema.name
        entries := schema.source.map
          (checkedAttributeFromRaw catalog ⟨box, current⟩)
        uniqueNames := by
          simpa [checkedAttributeFromRaw, NamesUnique] using
            schema.sourceWellFormed.uniqueAttributes } }

@[simp] theorem instantiate_attribute_names {catalog : SchemaUniverse}
    {box : BoxId catalog} (schema : BoxPortSchema catalog box)
    (current : TableId catalog box) :
    ((schema.instantiate current).attributes.entries.map CheckedAttribute.name) =
      schema.source.map IR.Attr.name := by
  simp [instantiate, checkedAttributeFromRaw]

end BoxPortSchema

private def buildInputPortSchemas (raw : IR.Model)
    (wellFormed : DeclarationsWellFormed raw)
    (box : BoxId (buildCatalog raw wellFormed)) :
    List (BoxPortSchema (buildCatalog raw wellFormed) box) :=
  let source := sourceBox raw wellFormed box
  let boxWellFormed := sourceBoxWellFormed raw wellFormed box
  source.inputs.attach.map fun attached =>
    let schemaWellFormed :=
      (List.forall_iff_forall_mem.mp boxWellFormed.inputSchemas)
        attached.1 attached.2
    { name := attached.1.name
      source := attached.1.schema
      tableNames := source.tables.map IR.Table.name
      tableNamesMatch := catalogTableNames raw wellFormed box
      sourceWellFormed := schemaWellFormed }

private def buildOutputPortSchemas (raw : IR.Model)
    (wellFormed : DeclarationsWellFormed raw)
    (box : BoxId (buildCatalog raw wellFormed)) :
    List (BoxPortSchema (buildCatalog raw wellFormed) box) :=
  let source := sourceBox raw wellFormed box
  let boxWellFormed := sourceBoxWellFormed raw wellFormed box
  source.outputs.attach.map fun attached =>
    let schemaWellFormed :=
      (List.forall_iff_forall_mem.mp boxWellFormed.outputSchemas)
        attached.1 attached.2
    { name := attached.1.name
      source := attached.1.schema
      tableNames := source.tables.map IR.Table.name
      tableNamesMatch := catalogTableNames raw wellFormed box
      sourceWellFormed := schemaWellFormed }

/-- Exact shallow source projection owned by this increment. -/
structure BoxDeclarationProjection where
  name : String
  tables : List IR.Table
  transitions : List IR.Transition
  inputs : List IR.PortDecl
  outputs : List IR.OutputDecl
  views : List IR.ViewDecl
  groupedViews : List IR.GroupedViewDecl
  deriving Repr, BEq

structure DeclarationProjection where
  name : String
  dt : IR.Scientific
  params : List IR.ParamDecl
  boxes : List BoxDeclarationProjection
  summaries : List IR.SummaryDecl
  deriving Repr, BEq

private def projectBoxDeclarations (box : IR.Box) : BoxDeclarationProjection :=
  { name := box.name
    tables := box.tables
    transitions := box.transitions
    inputs := box.inputs
    outputs := box.outputs
    views := box.views
    groupedViews := box.groupedViews }

/-- Drop wires and retain every declaration field in exact source order. -/
def projectDeclarations (raw : IR.Model) : DeclarationProjection :=
  { name := raw.name
    dt := raw.dt
    params := raw.params
    boxes := raw.boxes.map projectBoxDeclarations
    summaries := raw.summaries }

/-- Canonical checked declaration context.  All dependent catalogs are derived
from `source` and `wellFormed`, rather than stored beside unrelated data. -/
structure DeclarationContext where
  source : IR.Model
  wellFormed : DeclarationsWellFormed source

namespace DeclarationContext

def name (ctx : DeclarationContext) : String := ctx.source.name

def dt (ctx : DeclarationContext) : ScientificLiteral := ⟨ctx.source.dt⟩

@[simp] theorem dt_positive (ctx : DeclarationContext) : 0 < ctx.dt.denote := by
  exact (scientificPositive_iff_denote_pos ctx.source.dt).mp (by
    simp [scientificPositive, ctx.wellFormed.dtPositive])

/-- Accepted model schema canonically derived from the checked source. -/
def modelSchema (ctx : DeclarationContext) : ModelSchema :=
  buildModelSchema ctx.source ctx.wellFormed

/-- Source-ordered raw transition headers; their table targets are guaranteed by
`wellFormed` and exposed through `resolveTransitionTarget` below. -/
def transitions (ctx : DeclarationContext)
    (box : BoxId ctx.modelSchema.catalog) : List IR.Transition :=
  (sourceBox ctx.source ctx.wellFormed box).transitions

/-- Separate source-ordered input headers. -/
def inputs (ctx : DeclarationContext)
    (box : BoxId ctx.modelSchema.catalog) : List IR.PortDecl :=
  (sourceBox ctx.source ctx.wellFormed box).inputs

/-- Checked box-owned input schemas, valid even for a zero-table box. -/
def inputPortSchemas (ctx : DeclarationContext)
    (box : BoxId ctx.modelSchema.catalog) :
    List (BoxPortSchema ctx.modelSchema.catalog box) :=
  buildInputPortSchemas ctx.source ctx.wellFormed box

/-- Separate source-ordered output headers, retaining deferred builders exactly. -/
def outputs (ctx : DeclarationContext)
    (box : BoxId ctx.modelSchema.catalog) : List IR.OutputDecl :=
  (sourceBox ctx.source ctx.wellFormed box).outputs

/-- Checked box-owned output schemas with builders retained only in `outputs`. -/
def outputPortSchemas (ctx : DeclarationContext)
    (box : BoxId ctx.modelSchema.catalog) :
    List (BoxPortSchema ctx.modelSchema.catalog box) :=
  buildOutputPortSchemas ctx.source ctx.wellFormed box

/-- Ordinary and grouped view payloads remain shallow and source ordered. -/
def views (ctx : DeclarationContext)
    (box : BoxId ctx.modelSchema.catalog) : List IR.ViewDecl :=
  (sourceBox ctx.source ctx.wellFormed box).views

def groupedViews (ctx : DeclarationContext)
    (box : BoxId ctx.modelSchema.catalog) : List IR.GroupedViewDecl :=
  (sourceBox ctx.source ctx.wellFormed box).groupedViews

/-- Model-global summary headers remain source ordered and otherwise deferred. -/
def summaries (ctx : DeclarationContext) : List IR.SummaryDecl := ctx.source.summaries

/-- The source transition selected from the ordered shallow catalog. -/
def transitionAt (ctx : DeclarationContext)
    (box : BoxId ctx.modelSchema.catalog)
    (index : Fin (ctx.transitions box).length) : IR.Transition :=
  (ctx.transitions box).get index

private theorem transitionTargetIsSome (ctx : DeclarationContext)
    (box : BoxId ctx.modelSchema.catalog)
    (index : Fin (ctx.transitions box).length) :
    (ctx.modelSchema.catalog.lookupTable box (ctx.transitionAt box index).table).isSome = true := by
  let source := sourceBox ctx.source ctx.wellFormed box
  have transitionMember : ctx.transitionAt box index ∈ source.transitions := by
    exact List.get_mem source.transitions index.val index.isLt
  have sourceNameMember :
      (ctx.transitionAt box index).table ∈ source.tables.map IR.Table.name :=
    (List.forall_iff_forall_mem.mp
      (sourceBoxWellFormed ctx.source ctx.wellFormed box).transitionTargets)
      _ transitionMember
  have catalogNameMember :
      (ctx.transitionAt box index).table ∈
        (ctx.modelSchema.catalog.boxHeader box).tables.entries.map TableHeader.name := by
    change (ctx.transitionAt box index).table ∈
      ((buildCatalog ctx.source ctx.wellFormed).boxHeader box).tables.entries.map
        TableHeader.name
    rw [catalogTableNames ctx.source ctx.wellFormed box]
    exact sourceNameMember
  obtain ⟨target, found⟩ :=
    ctx.modelSchema.catalog.lookupTable_complete box catalogNameMember
  exact Option.isSome_iff_exists.mpr ⟨target, found⟩

/-- Resolved table identifier for a transition header. -/
def resolveTransitionTarget (ctx : DeclarationContext)
    (box : BoxId ctx.modelSchema.catalog)
    (index : Fin (ctx.transitions box).length) :
    TableId ctx.modelSchema.catalog box :=
  (ctx.modelSchema.catalog.lookupTable box (ctx.transitionAt box index).table).get
    (ctx.transitionTargetIsSome box index)

/-- Transition resolution preserves the exact raw target spelling. -/
theorem checkedTransition_target_name (ctx : DeclarationContext)
    (box : BoxId ctx.modelSchema.catalog)
    (index : Fin (ctx.transitions box).length) :
    ctx.modelSchema.catalog.tableName ⟨box, ctx.resolveTransitionTarget box index⟩ =
      (ctx.transitionAt box index).table := by
  apply ctx.modelSchema.catalog.tableLookup_name box
  exact (Option.some_get (ctx.transitionTargetIsSome box index)).symm

/-- Parameter construction is coherent with the source projection. -/
@[simp] theorem modelSchema_eraseParameters_exact (ctx : DeclarationContext) :
    ctx.modelSchema.eraseParameters = ctx.source.params := by
  change (checkedParams ctx.source.params).map CheckedParamDecl.erase = ctx.source.params
  exact checkedParams_erase ctx.source.params ctx.wellFormed.parameters

end DeclarationContext

/-- Projection erasure deliberately stops before wires and whole-model payload
checking, which PRD 0006 owns. -/
def eraseDeclarations (ctx : DeclarationContext) : DeclarationProjection :=
  projectDeclarations ctx.source

/-- Canonical terminating declaration checker. -/
def checkDeclarations (raw : IR.Model) : Except CheckError DeclarationContext :=
  if wellFormed : DeclarationsWellFormed raw then
    .ok ⟨raw, wellFormed⟩
  else
    .error ((diagnoseDeclarationError raw).getD fallbackError)

@[simp] theorem checkDeclarations_sound {raw : IR.Model} {ctx : DeclarationContext}
    (checked : checkDeclarations raw = .ok ctx) :
    DeclarationsWellFormed raw ∧
      eraseDeclarations ctx = projectDeclarations raw := by
  simp only [checkDeclarations] at checked
  split at checked <;> rename_i wellFormed
  · cases checked
    exact ⟨wellFormed, rfl⟩
  · contradiction

@[simp] theorem checkDeclarations_complete {raw : IR.Model}
    (wellFormed : DeclarationsWellFormed raw) :
    ∃ ctx, checkDeclarations raw = .ok ctx := by
  refine ⟨⟨raw, wellFormed⟩, ?_⟩
  simp [checkDeclarations, wellFormed]

@[simp] theorem checkDeclarations_failure_iff (raw : IR.Model) :
    (∃ err, checkDeclarations raw = .error err) ↔
      ¬ DeclarationsWellFormed raw := by
  constructor
  · rintro ⟨err, checked⟩ wellFormed
    simp [checkDeclarations, wellFormed] at checked
  · intro notWellFormed
    refine ⟨(diagnoseDeclarationError raw).getD fallbackError, ?_⟩
    simp [checkDeclarations, notWellFormed]

@[simp] theorem checkDeclarations_erases_exact {raw : IR.Model}
    {ctx : DeclarationContext} (checked : checkDeclarations raw = .ok ctx) :
    eraseDeclarations ctx = projectDeclarations raw :=
  (checkDeclarations_sound checked).2

/-- Checked parameter lookup preserves the accepted source spelling. -/
theorem checkedParameterLookup_name (ctx : DeclarationContext) {wanted : String}
    {id : ParameterId ctx.modelSchema.params}
    (found : ctx.modelSchema.params.lookup wanted = some id) :
    ctx.modelSchema.params.name id = wanted :=
  ctx.modelSchema.params.parameterLookup_name found

/-- Checked box lookup preserves the accepted source spelling. -/
theorem checkedBoxLookup_name (ctx : DeclarationContext) {wanted : String}
    {box : BoxId ctx.modelSchema.catalog}
    (found : ctx.modelSchema.catalog.lookupBox wanted = some box) :
    ctx.modelSchema.catalog.boxName box = wanted :=
  ctx.modelSchema.catalog.boxLookup_name found

/-- Checked table lookup preserves the accepted source spelling. -/
theorem checkedTableLookup_name (ctx : DeclarationContext)
    (box : BoxId ctx.modelSchema.catalog) {wanted : String}
    {table : TableId ctx.modelSchema.catalog box}
    (found : ctx.modelSchema.catalog.lookupTable box wanted = some table) :
    ctx.modelSchema.catalog.tableName ⟨box, table⟩ = wanted :=
  ctx.modelSchema.catalog.tableLookup_name box found

/-- Checked attribute lookup preserves the accepted source spelling. -/
theorem checkedAttributeLookup_name (ctx : DeclarationContext)
    (target : TableTarget ctx.modelSchema.catalog) {wanted : String}
    {attr : AttributeId (ctx.modelSchema.schemaFor target)}
    (found : (ctx.modelSchema.schemaFor target).lookupAttribute wanted = some attr) :
    (ctx.modelSchema.schemaFor target).attributeName attr = wanted :=
  (ctx.modelSchema.schemaFor target).attributeLookup_name found

/-- Checked enum lookup preserves the accepted source spelling. -/
theorem checkedEnumLookup_name (ctx : DeclarationContext)
    (target : TableTarget ctx.modelSchema.catalog)
    (attr : AttributeId (ctx.modelSchema.schemaFor target))
    (enumSchema : EnumSchema)
    (shapeEq : ((ctx.modelSchema.schemaFor target).attr attr).shape = .enum enumSchema)
    {wanted : String} {variant : VariantId (ctx.modelSchema.schemaFor target) attr enumSchema}
    (found : enumSchema.lookup (ctx.modelSchema.schemaFor target) attr shapeEq wanted =
      some variant) :
    enumSchema.variantName (ctx.modelSchema.schemaFor target) attr variant = wanted :=
  enumSchema.enumLookup_name (ctx.modelSchema.schemaFor target) attr shapeEq found

end Sembla.Semantics
