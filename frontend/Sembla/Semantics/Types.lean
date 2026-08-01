import Mathlib.Data.Real.Basic
import Mathlib.Tactic
import Sembla.IR

/-!
Foundational checked scalar and schema domains for the V1 IR.

The ordered contexts below are already valid, proof-carrying values. This module
provides their constructors and dependent identifiers; PRD 0005 owns checking
raw declaration lists, name resolution, and structured diagnostics. Table names
and sizes are established before attr schemas, so the second phase can
resolve forward and mutual references against the complete table catalog.
-/
namespace Sembla.Semantics

open Sembla

/-- Exact mathematical denotation of a raw finite decimal. Integer powers make
negative decimal exponents exact rather than approximate. -/
noncomputable def scientificDenote (value : IR.Scientific) : ℝ :=
  (value.coefficient : ℝ) * (10 : ℝ) ^ value.exponent

/-- The denotation equation is exposed as a named proof obligation. -/
theorem scientificDenote_equation (value : IR.Scientific) :
    scientificDenote value = (value.coefficient : ℝ) * (10 : ℝ) ^ value.exponent := rfl

/-- Any two raw encodings that are numerically equal have equal denotation. -/
theorem scientificDenote_congruent {left right : IR.Scientific}
    (equal : (left.coefficient : ℝ) * (10 : ℝ) ^ left.exponent =
      (right.coefficient : ℝ) * (10 : ℝ) ^ right.exponent) :
    scientificDenote left = scientificDenote right := by
  simpa [scientificDenote] using equal

/-- A raw-origin real literal keeps the exact syntax used to create it. -/
structure ScientificLiteral where
  source : IR.Scientific

namespace ScientificLiteral

noncomputable def denote (literal : ScientificLiteral) : ℝ := scientificDenote literal.source

def erase (literal : ScientificLiteral) : IR.Scientific := literal.source

@[simp] theorem erase_exact (literal : ScientificLiteral) : literal.erase = literal.source := rfl

end ScientificLiteral

/-- The required non-canonical decimal example, including a negative exponent. -/
theorem scientific_one_eq_ten_tenth :
    scientificDenote ⟨1, 0⟩ = scientificDenote ⟨10, -1⟩ := by
  norm_num [scientificDenote, zpow_neg]

/-- Parameter sorts are model-global and are distinct from attr-only sorts. -/
inductive ParamSort where
  | real
  | int

namespace ParamSort

def erase : ParamSort → IR.ParamType
  | .real => .real
  | .int => .int

end ParamSort

/-- A checked parameter default retains raw-origin scientific syntax. -/
def ParamLiteral : ParamSort → Type
  | .real => ScientificLiteral
  | .int => Int

namespace ParamLiteral

def erase : {sort : ParamSort} → ParamLiteral sort → IR.ParamValue
  | .real, value => .real (ScientificLiteral.erase value)
  | .int, value => .int value

@[simp] theorem erase_real_exact (value : ScientificLiteral) :
    ParamLiteral.erase (sort := .real) value = .real value.source := rfl

@[simp] theorem erase_int_exact (value : Int) :
    ParamLiteral.erase (sort := .int) value = .int value := rfl

end ParamLiteral

/-- Checked raw-origin parameter declaration. Prior payload is retained but not
interpreted here; its validation belongs to PRD 0005. -/
structure CheckedParamDecl where
  name : String
  sort : ParamSort
  default : ParamLiteral sort
  prior : Option IR.Prior

namespace CheckedParamDecl

def erase (decl : CheckedParamDecl) : IR.ParamDecl :=
  { name := decl.name
    ty := decl.sort.erase
    default := decl.default.erase
    prior := decl.prior }

@[simp] theorem erase_name (decl : CheckedParamDecl) : decl.erase.name = decl.name := rfl
@[simp] theorem erase_type (decl : CheckedParamDecl) : decl.erase.ty = decl.sort.erase := rfl
@[simp] theorem erase_prior (decl : CheckedParamDecl) : decl.erase.prior = decl.prior := rfl

end CheckedParamDecl

/-- A single source-ordered representation with a proof of unique names.
`scope` is the resolved owner identity: equal entries in a different scope do
not produce interchangeable identifiers. -/
structure OrderedContext (α : Type) (nameOf : α → String) where
  scope : String
  entries : List α
  uniqueNames : (entries.map nameOf).Nodup

namespace OrderedContext

/-- Executable ordered lookup, expressed once for every checked scope. -/
def findNameIndex (wanted : String) : (names : List String) → Option (Fin names.length)
  | [] => none
  | name :: names =>
      if name = wanted then
        some ⟨0, by simp⟩
      else
        (findNameIndex wanted names).map Fin.succ

theorem findNameIndex_sound {wanted : String} {names : List String}
    {index : Fin names.length} (found : findNameIndex wanted names = some index) :
    names.get index = wanted := by
  induction names with
  | nil => simp [findNameIndex] at found
  | cons head tail ih =>
      simp only [findNameIndex] at found
      split at found <;> rename_i same
      · cases found
        simpa using same
      · cases result : findNameIndex wanted tail with
        | none => simp [result] at found
        | some tailIndex =>
            simp [result] at found
            subst index
            simpa using ih result

/-- Lookup is derived by scanning the sole ordered representation. -/
def lookupOrdinal {α : Type} {nameOf : α → String}
    (ctx : OrderedContext α nameOf) (wanted : String) : Option (Fin ctx.entries.length) :=
  (findNameIndex wanted (ctx.entries.map nameOf)).map fun index =>
    Fin.cast (by simp) index

def nameAt {α : Type} {nameOf : α → String}
    (ctx : OrderedContext α nameOf) (index : Fin ctx.entries.length) : String :=
  nameOf (ctx.entries.get index)

theorem nameAt_injective {α : Type} {nameOf : α → String}
    (ctx : OrderedContext α nameOf) {left right : Fin ctx.entries.length}
    (same : ctx.nameAt left = ctx.nameAt right) : left = right := by
  have mapped :
      (ctx.entries.map nameOf).get (Fin.cast (by simp) left) =
      (ctx.entries.map nameOf).get (Fin.cast (by simp) right) := by
    simpa [nameAt] using same
  have castEqual := ctx.uniqueNames.get_inj_iff.mp mapped
  exact Fin.ext (by simpa using congrArg Fin.val castEqual)

theorem lookupOrdinal_name {α : Type} {nameOf : α → String}
    (ctx : OrderedContext α nameOf) {wanted : String} {index : Fin ctx.entries.length}
    (found : ctx.lookupOrdinal wanted = some index) : ctx.nameAt index = wanted := by
  unfold lookupOrdinal at found
  cases find : findNameIndex wanted (ctx.entries.map nameOf) with
  | none => simp [find] at found
  | some rawIndex =>
      simp [find] at found
      subst index
      have sound := findNameIndex_sound find
      simpa [nameAt] using sound

theorem lookupOrdinal_unique {α : Type} {nameOf : α → String}
    (ctx : OrderedContext α nameOf) {wanted : String}
    {left right : Fin ctx.entries.length}
    (leftFound : ctx.lookupOrdinal wanted = some left)
    (rightFound : ctx.lookupOrdinal wanted = some right) : left = right := by
  rw [leftFound] at rightFound
  exact Option.some.inj rightFound

end OrderedContext

/-- Model-global checked parameter scope. -/
abbrev ParamContext := OrderedContext CheckedParamDecl CheckedParamDecl.name

/-- A parameter identifier retains its exact owning model-global context. -/
structure ParameterId (ctx : ParamContext) where
  ordinal : Fin ctx.entries.length

namespace ParamContext

def lookup (ctx : ParamContext) (name : String) : Option (ParameterId ctx) :=
  (ctx.lookupOrdinal name).map ParameterId.mk

def name (ctx : ParamContext) (id : ParameterId ctx) : String := ctx.nameAt id.ordinal

def get (ctx : ParamContext) (id : ParameterId ctx) : CheckedParamDecl :=
  ctx.entries.get id.ordinal

def erase (ctx : ParamContext) : List IR.ParamDecl := ctx.entries.map CheckedParamDecl.erase

@[simp] theorem erase_exact (ctx : ParamContext) :
    ctx.erase = ctx.entries.map CheckedParamDecl.erase := rfl

theorem parameterLookup_name (ctx : ParamContext) {wanted : String} {id : ParameterId ctx}
    (found : ctx.lookup wanted = some id) : ctx.name id = wanted := by
  unfold lookup at found
  cases result : ctx.lookupOrdinal wanted with
  | none => simp [result] at found
  | some index =>
      simp [result] at found
      subst id
      exact ctx.lookupOrdinal_name result

theorem parameterLookup_deterministic (ctx : ParamContext) {wanted : String}
    {left right : ParameterId ctx} (hl : ctx.lookup wanted = some left)
    (hr : ctx.lookup wanted = some right) : left = right := by
  rw [hl] at hr
  exact Option.some.inj hr

theorem parameterLookup_unique (ctx : ParamContext) {wanted : String}
    {left right : ParameterId ctx} (hl : ctx.lookup wanted = some left)
    (hr : ctx.lookup wanted = some right) : left = right :=
  ctx.parameterLookup_deterministic hl hr

end ParamContext

/-- Phase-one table header. Attribute schemas are deliberately absent. -/
structure TableHeader where
  name : String
  sizeHint : Nat

/-- A box-local, ordered and unique table catalog. -/
structure BoxHeader where
  name : String
  tables : OrderedContext TableHeader TableHeader.name

/-- The complete ordered box/table catalog, established before attributes. -/
structure SchemaUniverse where
  boxes : OrderedContext BoxHeader BoxHeader.name

/-- A box identifier retains the complete owning catalog. -/
structure BoxId (catalog : SchemaUniverse) where
  ordinal : Fin catalog.boxes.entries.length

namespace SchemaUniverse

def lookupBox (catalog : SchemaUniverse) (name : String) : Option (BoxId catalog) :=
  (catalog.boxes.lookupOrdinal name).map BoxId.mk

def boxHeader (catalog : SchemaUniverse) (box : BoxId catalog) : BoxHeader :=
  catalog.boxes.entries.get box.ordinal

def boxName (catalog : SchemaUniverse) (box : BoxId catalog) : String :=
  (catalog.boxHeader box).name

theorem boxLookup_name (catalog : SchemaUniverse) {wanted : String} {box : BoxId catalog}
    (found : catalog.lookupBox wanted = some box) : catalog.boxName box = wanted := by
  unfold lookupBox at found
  cases result : catalog.boxes.lookupOrdinal wanted with
  | none => simp [result] at found
  | some index =>
      simp [result] at found
      subst box
      exact catalog.boxes.lookupOrdinal_name result

theorem boxLookup_deterministic (catalog : SchemaUniverse) {wanted : String}
    {left right : BoxId catalog} (hl : catalog.lookupBox wanted = some left)
    (hr : catalog.lookupBox wanted = some right) : left = right := by
  rw [hl] at hr
  exact Option.some.inj hr

theorem boxLookup_unique (catalog : SchemaUniverse) {wanted : String}
    {left right : BoxId catalog} (hl : catalog.lookupBox wanted = some left)
    (hr : catalog.lookupBox wanted = some right) : left = right :=
  catalog.boxLookup_deterministic hl hr

end SchemaUniverse

/-- A table identifier retains both its catalog and box-local owner. -/
structure TableId (catalog : SchemaUniverse) (box : BoxId catalog) where
  ordinal : Fin (catalog.boxHeader box).tables.entries.length

/-- Resolved box/table identity used by schemas, references, rows and state. -/
structure TableTarget (catalog : SchemaUniverse) where
  box : BoxId catalog
  table : TableId catalog box

namespace SchemaUniverse

def lookupTable (catalog : SchemaUniverse) (box : BoxId catalog)
    (name : String) : Option (TableId catalog box) :=
  ((catalog.boxHeader box).tables.lookupOrdinal name).map TableId.mk

def tableHeader (catalog : SchemaUniverse) (target : TableTarget catalog) : TableHeader :=
  (catalog.boxHeader target.box).tables.entries.get target.table.ordinal

def tableName (catalog : SchemaUniverse) (target : TableTarget catalog) : String :=
  (catalog.tableHeader target).name

def tableSize (catalog : SchemaUniverse) (target : TableTarget catalog) : Nat :=
  (catalog.tableHeader target).sizeHint

theorem tableLookup_name (catalog : SchemaUniverse) (box : BoxId catalog)
    {wanted : String} {table : TableId catalog box}
    (found : catalog.lookupTable box wanted = some table) :
    catalog.tableName ⟨box, table⟩ = wanted := by
  unfold lookupTable at found
  cases result : (catalog.boxHeader box).tables.lookupOrdinal wanted with
  | none => simp [result] at found
  | some index =>
      simp [result] at found
      subst table
      exact (catalog.boxHeader box).tables.lookupOrdinal_name result

theorem tableLookup_deterministic (catalog : SchemaUniverse) (box : BoxId catalog)
    {wanted : String} {left right : TableId catalog box}
    (hl : catalog.lookupTable box wanted = some left)
    (hr : catalog.lookupTable box wanted = some right) : left = right := by
  rw [hl] at hr
  exact Option.some.inj hr

theorem tableLookup_unique (catalog : SchemaUniverse) (box : BoxId catalog)
    {wanted : String} {left right : TableId catalog box}
    (hl : catalog.lookupTable box wanted = some left)
    (hr : catalog.lookupTable box wanted = some right) : left = right :=
  catalog.tableLookup_deterministic box hl hr

end SchemaUniverse

/-- Nonempty, duplicate-free enum variants in declaration order. -/
structure EnumSchema where
  variants : List String
  nonempty : variants ≠ []
  uniqueVariants : variants.Nodup

namespace EnumSchema

end EnumSchema

/-- Resolved attr shape. The owner fixes box-local reference scope. -/
inductive AttrShape (catalog : SchemaUniverse) (owner : TableTarget catalog) where
  | real
  | int
  | enum (schema : EnumSchema)
  | ref (target : TableId catalog owner.box)

namespace AttrShape

def erase {catalog : SchemaUniverse} {owner : TableTarget catalog} :
    AttrShape catalog owner → IR.AttrType
  | .real => .real
  | .int => .int
  | .enum schema => .enum schema.variants
  | .ref target => .ref (catalog.tableName ⟨owner.box, target⟩)

end AttrShape

/-- Raw-origin checked attr with its table owner retained in the type. -/
structure CheckedAttribute (catalog : SchemaUniverse) (owner : TableTarget catalog) where
  name : String
  shape : AttrShape catalog owner

namespace CheckedAttribute

def erase {catalog : SchemaUniverse} {owner : TableTarget catalog}
    (attr : CheckedAttribute catalog owner) : IR.Attr :=
  { name := attr.name, ty := attr.shape.erase }

@[simp] theorem erase_exact {catalog : SchemaUniverse} {owner : TableTarget catalog}
    (attr : CheckedAttribute catalog owner) :
    attr.erase = { name := attr.name, ty := attr.shape.erase } := rfl

end CheckedAttribute

/-- Phase-two table schema. Its owner remains a type index even when two tables
have structurally equal attrs. -/
structure TableSchema (catalog : SchemaUniverse) (owner : TableTarget catalog) where
  attributes : OrderedContext (CheckedAttribute catalog owner) CheckedAttribute.name

/-- Attr identifiers are table-local and owner-indexed. -/
structure AttributeId {catalog : SchemaUniverse} {owner : TableTarget catalog}
    (schema : TableSchema catalog owner) where
  ordinal : Fin schema.attributes.entries.length

namespace TableSchema

def lookupAttribute {catalog : SchemaUniverse} {owner : TableTarget catalog}
    (schema : TableSchema catalog owner) (name : String) : Option (AttributeId schema) :=
  (schema.attributes.lookupOrdinal name).map AttributeId.mk

def attr {catalog : SchemaUniverse} {owner : TableTarget catalog}
    (schema : TableSchema catalog owner) (id : AttributeId schema) :
    CheckedAttribute catalog owner :=
  schema.attributes.entries.get id.ordinal

def attributeName {catalog : SchemaUniverse} {owner : TableTarget catalog}
    (schema : TableSchema catalog owner) (id : AttributeId schema) : String :=
  (schema.attr id).name

def eraseAttributes {catalog : SchemaUniverse} {owner : TableTarget catalog}
    (schema : TableSchema catalog owner) : List IR.Attr :=
  schema.attributes.entries.map CheckedAttribute.erase

def eraseTable {catalog : SchemaUniverse} {owner : TableTarget catalog}
    (schema : TableSchema catalog owner) : IR.Table :=
  { name := catalog.tableName owner
    sizeHint := catalog.tableSize owner
    attrs := schema.eraseAttributes }

@[simp] theorem eraseAttributes_exact {catalog : SchemaUniverse}
    {owner : TableTarget catalog} (schema : TableSchema catalog owner) :
    schema.eraseAttributes = schema.attributes.entries.map CheckedAttribute.erase := rfl

@[simp] theorem eraseTable_exact {catalog : SchemaUniverse}
    {owner : TableTarget catalog} (schema : TableSchema catalog owner) :
    schema.eraseTable =
      { name := catalog.tableName owner
        sizeHint := catalog.tableSize owner
        attrs := schema.attributes.entries.map CheckedAttribute.erase } := rfl

theorem attributeLookup_name {catalog : SchemaUniverse} {owner : TableTarget catalog}
    (schema : TableSchema catalog owner) {wanted : String} {attr : AttributeId schema}
    (found : schema.lookupAttribute wanted = some attr) :
    schema.attributeName attr = wanted := by
  unfold lookupAttribute at found
  cases result : schema.attributes.lookupOrdinal wanted with
  | none => simp [result] at found
  | some index =>
      simp [result] at found
      subst attr
      exact schema.attributes.lookupOrdinal_name result

theorem attributeLookup_deterministic {catalog : SchemaUniverse}
    {owner : TableTarget catalog} (schema : TableSchema catalog owner) {wanted : String}
    {left right : AttributeId schema} (hl : schema.lookupAttribute wanted = some left)
    (hr : schema.lookupAttribute wanted = some right) : left = right := by
  rw [hl] at hr
  exact Option.some.inj hr

theorem attributeLookup_unique {catalog : SchemaUniverse}
    {owner : TableTarget catalog} (schema : TableSchema catalog owner) {wanted : String}
    {left right : AttributeId schema} (hl : schema.lookupAttribute wanted = some left)
    (hr : schema.lookupAttribute wanted = some right) : left = right :=
  schema.attributeLookup_deterministic hl hr

end TableSchema

/-- Enum identifiers require evidence that the selected attr owns exactly this
enum domain. Equal-looking enum lists on another attr cannot supply it. -/
structure VariantId {catalog : SchemaUniverse} {owner : TableTarget catalog}
    (tableSchema : TableSchema catalog owner) (attr : AttributeId tableSchema)
    (enumSchema : EnumSchema) where
  shapeEq : (tableSchema.attr attr).shape = .enum enumSchema
  ordinal : Fin enumSchema.variants.length

namespace EnumSchema

def lookup {catalog : SchemaUniverse} {owner : TableTarget catalog}
    (tableSchema : TableSchema catalog owner) (attr : AttributeId tableSchema)
    (enumSchema : EnumSchema) (shapeEq : (tableSchema.attr attr).shape = .enum enumSchema)
    (name : String) : Option (VariantId tableSchema attr enumSchema) :=
  (OrderedContext.findNameIndex name enumSchema.variants).map fun index => ⟨shapeEq, index⟩

def variantName {catalog : SchemaUniverse} {owner : TableTarget catalog}
    (tableSchema : TableSchema catalog owner) (attr : AttributeId tableSchema)
    (enumSchema : EnumSchema) (variant : VariantId tableSchema attr enumSchema) : String :=
  enumSchema.variants.get variant.ordinal

theorem variantName_injective {catalog : SchemaUniverse} {owner : TableTarget catalog}
    (tableSchema : TableSchema catalog owner) (attr : AttributeId tableSchema)
    (enumSchema : EnumSchema) {left right : VariantId tableSchema attr enumSchema}
    (same : enumSchema.variantName tableSchema attr left =
      enumSchema.variantName tableSchema attr right) : left = right := by
  cases left with
  | mk leftShape leftOrdinal =>
      cases right with
      | mk rightShape rightOrdinal =>
          simp only [variantName] at same
          have ordinalEqual := enumSchema.uniqueVariants.get_inj_iff.mp same
          cases ordinalEqual
          rfl

theorem enumLookup_name {catalog : SchemaUniverse} {owner : TableTarget catalog}
    (tableSchema : TableSchema catalog owner) (attr : AttributeId tableSchema)
    (enumSchema : EnumSchema) (shapeEq : (tableSchema.attr attr).shape = .enum enumSchema)
    {wanted : String} {variant : VariantId tableSchema attr enumSchema}
    (found : enumSchema.lookup tableSchema attr shapeEq wanted = some variant) :
    enumSchema.variantName tableSchema attr variant = wanted := by
  unfold lookup at found
  cases result : OrderedContext.findNameIndex wanted enumSchema.variants with
  | none => simp [result] at found
  | some index =>
      simp [result] at found
      subst variant
      exact OrderedContext.findNameIndex_sound result

theorem enumLookup_deterministic {catalog : SchemaUniverse} {owner : TableTarget catalog}
    (tableSchema : TableSchema catalog owner) (attr : AttributeId tableSchema)
    (enumSchema : EnumSchema) (shapeEq : (tableSchema.attr attr).shape = .enum enumSchema)
    {wanted : String} {left right : VariantId tableSchema attr enumSchema}
    (hl : enumSchema.lookup tableSchema attr shapeEq wanted = some left)
    (hr : enumSchema.lookup tableSchema attr shapeEq wanted = some right) : left = right := by
  rw [hl] at hr
  exact Option.some.inj hr

theorem enumLookup_unique {catalog : SchemaUniverse} {owner : TableTarget catalog}
    (tableSchema : TableSchema catalog owner) (attr : AttributeId tableSchema)
    (enumSchema : EnumSchema) (shapeEq : (tableSchema.attr attr).shape = .enum enumSchema)
    {wanted : String} {left right : VariantId tableSchema attr enumSchema}
    (hl : enumSchema.lookup tableSchema attr shapeEq wanted = some left)
    (hr : enumSchema.lookup tableSchema attr shapeEq wanted = some right) : left = right :=
  enumSchema.enumLookup_deterministic tableSchema attr shapeEq hl hr

@[simp] theorem variant_bound {catalog : SchemaUniverse} {owner : TableTarget catalog}
    {tableSchema : TableSchema catalog owner} {attr : AttributeId tableSchema}
    {enumSchema : EnumSchema} (variant : VariantId tableSchema attr enumSchema) :
    variant.ordinal.val < enumSchema.variants.length := variant.ordinal.isLt

end EnumSchema

/-- Row ordinals retain the owning box/table target. -/
structure RowId (catalog : SchemaUniverse) (target : TableTarget catalog) where
  ordinal : Fin (catalog.tableSize target)

namespace RowId

@[simp] theorem bound {catalog : SchemaUniverse} {target : TableTarget catalog}
    (row : RowId catalog target) :
    row.ordinal.val < catalog.tableSize target := row.ordinal.isLt

theorem zero_size_elim {catalog : SchemaUniverse} {target : TableTarget catalog}
    (empty : catalog.tableSize target = 0) (row : RowId catalog target) : False := by
  have below := row.bound
  simp [empty] at below

end RowId

/-- Runtime scalar sorts. Enum and reference constructors retain their resolved
owners. Arbitrary mathematical real values intentionally have no raw eraser. -/
inductive ScalarSort (catalog : SchemaUniverse) where
  | real
  | int
  | bool
  | enum {owner : TableTarget catalog} (tableSchema : TableSchema catalog owner)
      (attr : AttributeId tableSchema) (enumSchema : EnumSchema)
      (shapeEq : (tableSchema.attr attr).shape = .enum enumSchema)
  | ref (target : TableTarget catalog)

/-- The unique runtime sort assigned to a checked attr. -/
def TableSchema.attributeSort {catalog : SchemaUniverse} {owner : TableTarget catalog}
    (schema : TableSchema catalog owner) (attr : AttributeId schema) : ScalarSort catalog :=
  match shapeEq : (schema.attr attr).shape with
  | .real => .real
  | .int => .int
  | .enum enumSchema => .enum schema attr enumSchema shapeEq
  | .ref target => .ref ⟨owner.box, target⟩

/-- Intrinsically typed runtime scalar values. -/
inductive ScalarValue (catalog : SchemaUniverse) : ScalarSort catalog → Type
  | real (value : ℝ) : ScalarValue catalog .real
  | int (value : Int) : ScalarValue catalog .int
  | bool (value : Bool) : ScalarValue catalog .bool
  | enum {owner : TableTarget catalog} {tableSchema : TableSchema catalog owner}
      {attr : AttributeId tableSchema} {enumSchema : EnumSchema}
      (variant : VariantId tableSchema attr enumSchema) :
      ScalarValue catalog (.enum tableSchema attr enumSchema variant.shapeEq)
  | ref {target : TableTarget catalog} (row : RowId catalog target) :
      ScalarValue catalog (.ref target)

/-- Existential packaging records the one declared sort of a runtime value. -/
structure PackedScalar (catalog : SchemaUniverse) where
  sort : ScalarSort catalog
  value : ScalarValue catalog sort

namespace PackedScalar

def HasSort {catalog : SchemaUniverse} (value : PackedScalar catalog)
    (sort : ScalarSort catalog) : Prop := value.sort = sort

@[simp] theorem has_declared_sort {catalog : SchemaUniverse}
    (value : PackedScalar catalog) : value.HasSort value.sort := rfl

theorem sort_unique {catalog : SchemaUniverse} (value : PackedScalar catalog)
    {left right : ScalarSort catalog} (hl : value.HasSort left) (hr : value.HasSort right) :
    left = right := hl.symm.trans hr

end PackedScalar

/-- A resolved model schema combines the global parameter scope, complete
phase-one catalog, and phase-two schema family. -/
structure ModelSchema where
  params : ParamContext
  catalog : SchemaUniverse
  tableSchemas : (target : TableTarget catalog) → TableSchema catalog target

namespace ModelSchema

def schemaFor (model : ModelSchema) (target : TableTarget model.catalog) :
    TableSchema model.catalog target := model.tableSchemas target

def eraseParameters (model : ModelSchema) : List IR.ParamDecl := model.params.erase

def eraseTable (model : ModelSchema) (target : TableTarget model.catalog) : IR.Table :=
  { name := model.catalog.tableName target
    sizeHint := model.catalog.tableSize target
    attrs := (model.schemaFor target).eraseAttributes }

def eraseTables (model : ModelSchema) (box : BoxId model.catalog) : List IR.Table :=
  List.ofFn fun index => model.eraseTable ⟨box, ⟨index⟩⟩

@[simp] theorem eraseParameters_exact (model : ModelSchema) :
    model.eraseParameters = model.params.entries.map CheckedParamDecl.erase := rfl

@[simp] theorem eraseTable_exact (model : ModelSchema)
    (target : TableTarget model.catalog) :
    model.eraseTable target =
      { name := model.catalog.tableName target
        sizeHint := model.catalog.tableSize target
        attrs := (model.schemaFor target).attributes.entries.map CheckedAttribute.erase } := rfl

@[simp] theorem eraseTables_exact (model : ModelSchema) (box : BoxId model.catalog) :
    model.eraseTables box =
      List.ofFn (fun index => model.eraseTable ⟨box, ⟨index⟩⟩) := rfl

end ModelSchema

end Sembla.Semantics
