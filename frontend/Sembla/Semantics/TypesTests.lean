import Sembla.Semantics.State

/-!
Positive and checked-failure fixtures for PRD 0003. Every box/table header is
built before owner-indexed attr schemas. The fixture includes same-named and
equal-cardinality tables in distinct scopes, a zero-sized table, equal-looking
enums, and box-local forward/mutual references.
-/
namespace Sembla.Semantics.TypesTests

open Sembla
open Sembla.Semantics

noncomputable section

private def realParam : CheckedParamDecl :=
  { name := "rate"
    sort := .real
    default := ⟨⟨1, -1⟩⟩
    prior := some { family := .uniform, args := [⟨0, 0⟩, ⟨10, -1⟩] } }

private def intParam : CheckedParamDecl :=
  { name := "seed", sort := .int, default := (7 : Int), prior := none }

private def params : ParamContext :=
  { scope := "model:fixture"
    entries := [realParam, intParam]
    uniqueNames := by decide }

private def clonedParams : ParamContext :=
  { scope := "model:clone"
    entries := [realParam, intParam]
    uniqueNames := by decide }

private def clonedSeed : ParameterId clonedParams := ⟨⟨1, by decide⟩⟩

private def northTables : OrderedContext TableHeader TableHeader.name :=
  { scope := "box:north"
    entries :=
      [ { name := "empty", sizeHint := 0 }
      , { name := "people", sizeHint := 2 }
      , { name := "regions", sizeHint := 1 }
      , { name := "twin", sizeHint := 2 } ]
    uniqueNames := by decide }

private def southTables : OrderedContext TableHeader TableHeader.name :=
  { scope := "box:south"
    entries := [{ name := "people", sizeHint := 2 }]
    uniqueNames := by decide }

private def catalog : SchemaUniverse :=
  { boxes :=
      { scope := "model:fixture:boxes"
        entries :=
          [ { name := "north", tables := northTables }
          , { name := "south", tables := southTables } ]
        uniqueNames := by decide } }

private def clonedCatalog : SchemaUniverse :=
  { boxes :=
      { scope := "model:clone:boxes"
        entries := catalog.boxes.entries
        uniqueNames := catalog.boxes.uniqueNames } }

private def clonedNorth : BoxId clonedCatalog := ⟨⟨0, by decide⟩⟩

private def north : BoxId catalog := ⟨⟨0, by decide⟩⟩
private def south : BoxId catalog := ⟨⟨1, by decide⟩⟩
private def emptyTable : TableId catalog north := ⟨⟨0, by decide⟩⟩
private def peopleTable : TableId catalog north := ⟨⟨1, by decide⟩⟩
private def regionsTable : TableId catalog north := ⟨⟨2, by decide⟩⟩
private def twinTable : TableId catalog north := ⟨⟨3, by decide⟩⟩
private def southPeopleTable : TableId catalog south := ⟨⟨0, by decide⟩⟩

private def emptyTarget : TableTarget catalog := ⟨north, emptyTable⟩
private def peopleTarget : TableTarget catalog := ⟨north, peopleTable⟩
private def regionsTarget : TableTarget catalog := ⟨north, regionsTable⟩
private def twinTarget : TableTarget catalog := ⟨north, twinTable⟩
private def southPeopleTarget : TableTarget catalog := ⟨south, southPeopleTable⟩

private def statusEnum : EnumSchema :=
  { variants := ["susceptible", "infected"]
    nonempty := by decide
    uniqueVariants := by decide }

private def sameLookingEnum : EnumSchema :=
  { variants := ["susceptible", "infected"]
    nonempty := by decide
    uniqueVariants := by decide }

/-- `people` resolves a forward reference to the later `regions` header. -/
private def peopleSchema : TableSchema catalog peopleTarget :=
  { attributes :=
      { scope := "north.people:attrs"
        entries :=
          [ { name := "age", shape := .int }
          , { name := "status", shape := .enum statusEnum }
          , { name := "region", shape := .ref regionsTable } ]
        uniqueNames := by decide } }

/-- `regions` points back to `people`, making the schemas mutually referential. -/
private def regionsSchema : TableSchema catalog regionsTarget :=
  { attributes :=
      { scope := "north.regions:attrs"
        entries :=
          [ { name := "weight", shape := .real }
          , { name := "resident", shape := .ref peopleTable } ]
        uniqueNames := by decide } }

private def twinSchema : TableSchema catalog twinTarget :=
  { attributes :=
      { scope := "north.twin:attrs"
        entries := [{ name := "value", shape := .real }]
        uniqueNames := by decide } }

private def southPeopleSchema : TableSchema catalog southPeopleTarget :=
  { attributes :=
      { scope := "south.people:attrs"
        entries := [{ name := "status", shape := .enum sameLookingEnum }]
        uniqueNames := by decide } }

/-- A total phase-two family. Each target receives attrs whose reference target
is its own box-local table; the richer forward/mutual schemas above exercise the
same constructors independently of this state fixture. -/
private def stateSchemas (target : TableTarget catalog) : TableSchema catalog target :=
  { attributes :=
      { scope := catalog.boxName target.box ++ "." ++ catalog.tableName target ++ ":state-attrs"
        entries :=
          [ { name := "measure", shape := .real }
          , { name := "count", shape := .int }
          , { name := "status", shape := .enum statusEnum }
          , { name := "self", shape := .ref target.table } ]
        uniqueNames := by simp } }

private def model : ModelSchema :=
  { params := params, catalog := catalog, tableSchemas := stateSchemas }

private def ageAttr : AttributeId peopleSchema := ⟨⟨0, by decide⟩⟩
private def statusAttr : AttributeId peopleSchema := ⟨⟨1, by decide⟩⟩
private def southStatusAttr : AttributeId southPeopleSchema := ⟨⟨0, by decide⟩⟩
private def statusShape : (peopleSchema.attr statusAttr).shape = .enum statusEnum := rfl
private def southStatusShape :
    (southPeopleSchema.attr southStatusAttr).shape = .enum sameLookingEnum := rfl

private def susceptible : VariantId peopleSchema statusAttr statusEnum :=
  ⟨statusShape, ⟨0, by decide⟩⟩
private def southernSusceptible :
    VariantId southPeopleSchema southStatusAttr sameLookingEnum :=
  ⟨southStatusShape, ⟨0, by decide⟩⟩

private def regionRow : RowId catalog regionsTarget := ⟨⟨0, by decide⟩⟩
private def peopleRow : RowId catalog peopleTarget := ⟨⟨1, by decide⟩⟩
private def twinRow : RowId catalog twinTarget := ⟨⟨1, by decide⟩⟩
private def southPeopleRow : RowId catalog southPeopleTarget := ⟨⟨1, by decide⟩⟩

private def measureAttr : AttributeId (model.schemaFor twinTarget) := ⟨⟨0, by decide⟩⟩
private def countAttr : AttributeId (model.schemaFor twinTarget) := ⟨⟨1, by decide⟩⟩
private def modelStatusAttr : AttributeId (model.schemaFor twinTarget) := ⟨⟨2, by decide⟩⟩
private def selfAttr : AttributeId (model.schemaFor twinTarget) := ⟨⟨3, by decide⟩⟩
private def modelStatusShape :
    ((model.schemaFor twinTarget).attr modelStatusAttr).shape = .enum statusEnum := rfl
private def modelSusceptible :
    VariantId (model.schemaFor twinTarget) modelStatusAttr statusEnum :=
  ⟨modelStatusShape, ⟨0, by decide⟩⟩

private noncomputable def realValue : ScalarValue catalog .real := .real (3 / 2 : ℝ)
private def intValue : ScalarValue catalog .int := .int 4
private def boolValue : ScalarValue catalog .bool := .bool true
private def enumValue : ScalarValue catalog
    (.enum peopleSchema statusAttr statusEnum susceptible.shapeEq) := .enum susceptible
private def refValue : ScalarValue catalog (.ref regionsTarget) := .ref regionRow

private noncomputable def twinTypedRow : TypedRow model twinTarget :=
  { project := fun ⟨ordinal⟩ =>
      Fin.cases (ScalarValue.real (1 : ℝ))
        (fun afterReal => Fin.cases (ScalarValue.int 2)
          (fun afterInt => Fin.cases (ScalarValue.enum modelSusceptible)
            (fun afterEnum => Fin.cases (ScalarValue.ref twinRow)
              (fun impossible => Fin.elim0 impossible) afterEnum)
            afterInt)
          afterReal)
        ordinal }

private noncomputable def twinTableState : ValidTableState model twinTarget :=
  { row := fun _ => twinTypedRow }

private def malformedSupplied : SuppliedState :=
  { tables :=
      [ { box := "north"
          table := "people"
          rows :=
            [ { cells :=
                  [ { name := "age", value := .bool true }
                  , { name := "status", value := .enum 99 }
                  , { name := "region", value := .ref "south" "people" 99 }
                  , { name := "extra", value := .int 0 } ] } ] }
      , { box := "north", table := "people", rows := [] } ] }

#guard (params.lookup "seed").map (fun id => id.ordinal.val) == some 1
#guard (catalog.lookupBox "south").map (fun id => id.ordinal.val) == some 1
#guard (catalog.lookupTable north "regions").map (fun id => id.ordinal.val) == some 2
#guard (peopleSchema.lookupAttribute "status").map (fun id => id.ordinal.val) == some 1
#guard (statusEnum.lookup peopleSchema statusAttr statusShape "infected").map
  (fun id => id.ordinal.val) == some 1

#guard (model.eraseParameters ==
  [ { name := "rate"
      ty := .real
      default := .real ⟨1, -1⟩
      prior := some { family := .uniform, args := [⟨0, 0⟩, ⟨10, -1⟩] } }
  , { name := "seed", ty := .int, default := .int 7, prior := none } ])
#guard (peopleSchema.eraseTable ==
  { name := "people"
    sizeHint := 2
    attrs :=
      [ { name := "age", ty := .int }
      , { name := "status", ty := .enum ["susceptible", "infected"] }
      , { name := "region", ty := .ref "regions" } ] })
#guard (regionsSchema.eraseTable ==
  { name := "regions"
    sizeHint := 1
    attrs :=
      [ { name := "weight", ty := .real }
      , { name := "resident", ty := .ref "people" } ] })
#guard (catalog.tableName peopleTarget == catalog.tableName southPeopleTarget)
#guard (catalog.tableSize peopleTarget == catalog.tableSize twinTarget)
#guard (malformedSupplied.tables.length == 2)
#guard (ScientificLiteral.erase ⟨⟨1, 0⟩⟩ == (⟨1, 0⟩ : IR.Scientific))
#guard (ScientificLiteral.erase ⟨⟨10, -1⟩⟩ == (⟨10, -1⟩ : IR.Scientific))

example : scientificDenote ⟨1, 0⟩ = scientificDenote ⟨10, -1⟩ :=
  scientific_one_eq_ten_tenth
example (row : RowId catalog emptyTarget) : False := RowId.zero_size_elim (by decide) row
example : ScalarValue catalog .real := realValue
example : ScalarValue catalog .int := intValue
example : ScalarValue catalog .bool := boolValue
example : ScalarValue catalog
    (.enum peopleSchema statusAttr statusEnum susceptible.shapeEq) := enumValue
example : ScalarValue catalog (.ref regionsTarget) := refValue
example : ScalarValue catalog .real := twinTypedRow.project measureAttr
example : ScalarValue catalog .int := twinTypedRow.project countAttr
example : ScalarValue catalog
    ((model.schemaFor twinTarget).attributeSort modelStatusAttr) :=
  twinTypedRow.project modelStatusAttr
example : ScalarValue catalog (.ref twinTarget) := twinTypedRow.project selfAttr
example (state : ValidModelState model) :
    ValidModelState.ofProjections state.table = state := ValidModelState.reconstruct state
example (state : ValidTableState model twinTarget) :
    ValidTableState.ofProjections state.row = state := ValidTableState.reconstruct state
example (row : TypedRow model twinTarget) :
    TypedRow.ofProjections row.project = row := TypedRow.reconstruct row

/--
error: type mismatch
  Sembla.Semantics.TypesTests.realValue
has type
  ScalarValue Sembla.Semantics.TypesTests.catalog ScalarSort.real : Type
but is expected to have type
  ScalarValue Sembla.Semantics.TypesTests.catalog ScalarSort.int : Type
-/
#guard_msgs (error) in
example : ScalarValue catalog .int := realValue

/--
error: type mismatch
  ScalarValue.enum Sembla.Semantics.TypesTests.southernSusceptible
has type
  ScalarValue Sembla.Semantics.TypesTests.catalog
    (ScalarSort.enum Sembla.Semantics.TypesTests.southPeopleSchema Sembla.Semantics.TypesTests.southStatusAttr
      Sembla.Semantics.TypesTests.sameLookingEnum ⋯) : Type
but is expected to have type
  ScalarValue Sembla.Semantics.TypesTests.catalog
    (ScalarSort.enum Sembla.Semantics.TypesTests.peopleSchema Sembla.Semantics.TypesTests.statusAttr
      Sembla.Semantics.TypesTests.statusEnum ⋯) : Type
-/
#guard_msgs (error) in
example : ScalarValue catalog
    (.enum peopleSchema statusAttr statusEnum susceptible.shapeEq) :=
  .enum southernSusceptible

/--
error: type mismatch
  Sembla.Semantics.TypesTests.southPeopleRow
has type
  RowId Sembla.Semantics.TypesTests.catalog Sembla.Semantics.TypesTests.southPeopleTarget : Type
but is expected to have type
  RowId Sembla.Semantics.TypesTests.catalog Sembla.Semantics.TypesTests.peopleTarget : Type
-/
#guard_msgs (error) in
example : RowId catalog peopleTarget := southPeopleRow

/--
error: type mismatch
  Sembla.Semantics.TypesTests.twinRow
has type
  RowId Sembla.Semantics.TypesTests.catalog Sembla.Semantics.TypesTests.twinTarget : Type
but is expected to have type
  RowId Sembla.Semantics.TypesTests.catalog Sembla.Semantics.TypesTests.peopleTarget : Type
-/
#guard_msgs (error) in
example : RowId catalog peopleTarget := twinRow

/--
error: application type mismatch
  ScalarValue.ref Sembla.Semantics.TypesTests.twinRow
argument
  Sembla.Semantics.TypesTests.twinRow
has type
  RowId Sembla.Semantics.TypesTests.catalog Sembla.Semantics.TypesTests.twinTarget : Type
but is expected to have type
  RowId Sembla.Semantics.TypesTests.catalog Sembla.Semantics.TypesTests.peopleTarget : Type
-/
#guard_msgs (error) in
example : ScalarValue catalog (.ref peopleTarget) := .ref twinRow

/--
error: tactic 'rfl' failed, the left-hand side
  (Sembla.Semantics.TypesTests.peopleSchema.attr Sembla.Semantics.TypesTests.ageAttr).shape
is not definitionally equal to the right-hand side
  AttrShape.enum Sembla.Semantics.TypesTests.statusEnum
⊢ (Sembla.Semantics.TypesTests.peopleSchema.attr Sembla.Semantics.TypesTests.ageAttr).shape =
    AttrShape.enum Sembla.Semantics.TypesTests.statusEnum
-/
#guard_msgs (error) in
example : VariantId peopleSchema ageAttr statusEnum :=
  ⟨by rfl, ⟨0, by decide⟩⟩

/--
error: application type mismatch
  AttrShape.ref Sembla.Semantics.TypesTests.southPeopleTable
argument
  Sembla.Semantics.TypesTests.southPeopleTable
has type
  TableId Sembla.Semantics.TypesTests.catalog Sembla.Semantics.TypesTests.south : Type
but is expected to have type
  TableId Sembla.Semantics.TypesTests.catalog Sembla.Semantics.TypesTests.peopleTarget.box : Type
-/
#guard_msgs (error) in
example : AttrShape catalog peopleTarget := .ref southPeopleTable

/--
error: type mismatch
  Sembla.Semantics.TypesTests.clonedSeed
has type
  ParameterId Sembla.Semantics.TypesTests.clonedParams : Type
but is expected to have type
  ParameterId Sembla.Semantics.TypesTests.params : Type
-/
#guard_msgs (error) in
example : ParameterId params := clonedSeed

/--
error: type mismatch
  Sembla.Semantics.TypesTests.clonedNorth
has type
  BoxId Sembla.Semantics.TypesTests.clonedCatalog : Type
but is expected to have type
  BoxId Sembla.Semantics.TypesTests.catalog : Type
-/
#guard_msgs (error) in
example : BoxId catalog := clonedNorth

end

end Sembla.Semantics.TypesTests
