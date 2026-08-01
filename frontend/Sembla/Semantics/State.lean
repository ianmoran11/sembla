import Sembla.Semantics.Types

/-!
Schema-indexed finite rows and valid model state, plus the explicitly
unvalidated supplied-state boundary reserved for PRD 0011.

A valid table is a dependent function over its owner-indexed row ordinals, so
its shape is exactly the table `sizeHint`. No constructor here initializes rows,
validates supplied data, performs lookup, or introduces row liveness.
-/
namespace Sembla.Semantics

/-- A row is exactly its schema-indexed family of typed attr projections. -/
structure TypedRow (model : ModelSchema) (target : TableTarget model.catalog) where
  project : (attr : AttributeId (model.schemaFor target)) →
    ScalarValue model.catalog ((model.schemaFor target).attributeSort attr)

namespace TypedRow

/-- Build a typed row from all and only its typed projections. -/
def ofProjections {model : ModelSchema} {target : TableTarget model.catalog}
    (values : (attr : AttributeId (model.schemaFor target)) →
      ScalarValue model.catalog ((model.schemaFor target).attributeSort attr)) :
    TypedRow model target := ⟨values⟩

@[simp] theorem reconstruct {model : ModelSchema} {target : TableTarget model.catalog}
    (row : TypedRow model target) : ofProjections row.project = row := by
  cases row
  rfl

@[ext] theorem ext {model : ModelSchema} {target : TableTarget model.catalog}
    {left right : TypedRow model target}
    (equal : ∀ attr, left.project attr = right.project attr) : left = right := by
  cases left with
  | mk leftProject =>
      cases right with
      | mk rightProject =>
          congr
          funext attr
          exact equal attr

end TypedRow

/-- Exactly one typed row for every valid row ordinal of the owning table. -/
structure ValidTableState (model : ModelSchema) (target : TableTarget model.catalog) where
  row : RowId model.catalog target → TypedRow model target

namespace ValidTableState

/-- Build valid table state from its complete row projection family. -/
def ofProjections {model : ModelSchema} {target : TableTarget model.catalog}
    (rows : RowId model.catalog target → TypedRow model target) :
    ValidTableState model target := ⟨rows⟩

@[simp] theorem reconstruct {model : ModelSchema} {target : TableTarget model.catalog}
    (tableState : ValidTableState model target) :
    ofProjections tableState.row = tableState := by
  cases tableState
  rfl

@[ext] theorem ext {model : ModelSchema} {target : TableTarget model.catalog}
    {left right : ValidTableState model target}
    (equal : ∀ row attr, (left.row row).project attr = (right.row row).project attr) :
    left = right := by
  cases left with
  | mk leftRows =>
      cases right with
      | mk rightRows =>
          congr
          funext row
          apply TypedRow.ext
          exact equal row

end ValidTableState

/-- A valid model state is the complete dependent family of its table states. -/
structure ValidModelState (model : ModelSchema) where
  table : (target : TableTarget model.catalog) → ValidTableState model target

namespace ValidModelState

/-- Build valid model state from all schema-indexed table projections. -/
def ofProjections {model : ModelSchema}
    (tables : (target : TableTarget model.catalog) → ValidTableState model target) :
    ValidModelState model := ⟨tables⟩

/-- Fully indexed scalar projection from a valid state. -/
def project {model : ModelSchema} (state : ValidModelState model)
    (target : TableTarget model.catalog) (row : RowId model.catalog target)
    (attr : AttributeId (model.schemaFor target)) :
    ScalarValue model.catalog ((model.schemaFor target).attributeSort attr) :=
  ((state.table target).row row).project attr

@[simp] theorem reconstruct {model : ModelSchema} (state : ValidModelState model) :
    ofProjections state.table = state := by
  cases state
  rfl

@[ext] theorem ext {model : ModelSchema} {left right : ValidModelState model}
    (equal : ∀ target row attr,
      left.project target row attr = right.project target row attr) : left = right := by
  cases left with
  | mk leftTables =>
      cases right with
      | mk rightTables =>
          congr
          funext target
          apply ValidTableState.ext
          exact equal target

end ValidModelState

/-- Supplied scalar data before schema validation. Enum and reference ordinals
are ordinary naturals and references retain unresolved names. -/
inductive SuppliedValue where
  | real (value : ℝ)
  | int (value : Int)
  | bool (value : Bool)
  | enum (ordinal : Nat)
  | ref (box table : String) (row : Nat)

/-- Unvalidated named cell. Names may be missing, duplicated or unexpected. -/
structure SuppliedCell where
  name : String
  value : SuppliedValue

/-- Unvalidated row with arbitrary column layout and value types. -/
structure SuppliedRow where
  cells : List SuppliedCell

/-- Unvalidated table with arbitrary names and row count. -/
structure SuppliedTable where
  box : String
  table : String
  rows : List SuppliedRow

/-- Unvalidated model state. Duplicate or missing supplied tables remain
representable until PRD 0011 defines validation and error behavior. -/
structure SuppliedState where
  tables : List SuppliedTable

end Sembla.Semantics
