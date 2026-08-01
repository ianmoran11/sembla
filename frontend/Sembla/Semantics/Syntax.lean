import Sembla.Semantics.Types

/-!
Intrinsically typed, evaluator-independent term syntax for the V1 IR.

Every semantic identity is a dependent identifier from `Types`; raw spellings
are recovered only by the exact structural erasers in this module.  Input
signatures describe syntax scopes only.  They do not contain snapshot values,
row traversal, output builders, checking, or evaluation.
-/
namespace Sembla.Semantics

open Sembla

namespace ParamSort

/-- The runtime scalar sort denoted by a model-global parameter sort. -/
def scalarSort (sort : ParamSort) (catalog : SchemaUniverse) : ScalarSort catalog :=
  match sort with
  | .real => .real
  | .int => .int

@[simp] theorem scalarSort_real (catalog : SchemaUniverse) :
    ParamSort.real.scalarSort catalog = .real := rfl

@[simp] theorem scalarSort_int (catalog : SchemaUniverse) :
    ParamSort.int.scalarSort catalog = .int := rfl

end ParamSort

/-- Exactly the scalar sorts admitted by arithmetic and numeric comparison. -/
inductive NumericSort where
  | real
  | int

namespace NumericSort

/-- Embed a numeric syntax sort into the accepted runtime sort family. -/
def scalarSort (sort : NumericSort) (catalog : SchemaUniverse) : ScalarSort catalog :=
  match sort with
  | .real => .real
  | .int => .int

@[simp] theorem scalarSort_real (catalog : SchemaUniverse) :
    NumericSort.real.scalarSort catalog = .real := rfl

@[simp] theorem scalarSort_int (catalog : SchemaUniverse) :
    NumericSort.int.scalarSort catalog = .int := rfl

@[simp] theorem scalarSort_ne_bool (sort : NumericSort) (catalog : SchemaUniverse) :
    sort.scalarSort catalog ≠ .bool := by
  cases sort <;> simp [scalarSort]

@[simp] theorem scalarSort_ne_ref (sort : NumericSort) (catalog : SchemaUniverse)
    (target : TableTarget catalog) : sort.scalarSort catalog ≠ .ref target := by
  cases sort <;> simp [scalarSort]

end NumericSort

/-- A syntax-only input port declaration. `current` anchors box-local reference
ownership; the row schema is not a state-table snapshot. -/
structure InputDecl (model : ModelSchema) (current : TableTarget model.catalog) where
  name : String
  rowSchema : TableSchema model.catalog current

/-- Ordered, uniquely named input syntax scopes. -/
structure InputSignature (model : ModelSchema) (current : TableTarget model.catalog) where
  ports : OrderedContext (InputDecl model current) InputDecl.name

/-- A resolved input port identity tied to its exact signature. -/
structure InputId {model : ModelSchema} {current : TableTarget model.catalog}
    (inputs : InputSignature model current) where
  ordinal : Fin inputs.ports.entries.length

namespace InputSignature

variable {model : ModelSchema} {current : TableTarget model.catalog}

/-- Retrieve the declaration selected by an input identifier. -/
def get (inputs : InputSignature model current) (port : InputId inputs) :
    InputDecl model current := inputs.ports.entries.get port.ordinal

/-- Recover the exact raw input-port spelling. -/
def name (inputs : InputSignature model current) (port : InputId inputs) : String :=
  (inputs.get port).name

/-- The row schema inspected by the port's filter and aggregate value. -/
def schema (inputs : InputSignature model current) (port : InputId inputs) :
    TableSchema model.catalog current := (inputs.get port).rowSchema

end InputSignature

/-- The row inspected by a term. Input rows and model tables have distinct
constructors even when their schemas are structurally equal. -/
inductive RowScope (model : ModelSchema) (current : TableTarget model.catalog)
    (inputs : InputSignature model current) where
  | table (target : TableTarget model.catalog)
  | input (port : InputId inputs)

namespace RowScope

variable {model : ModelSchema} {current : TableTarget model.catalog}
    {inputs : InputSignature model current}

/-- Owner used by dependent schema and box-local reference indices. -/
def owner (scope : RowScope model current inputs) : TableTarget model.catalog :=
  match scope with
  | .table target => target
  | .input _ => current

/-- The exact row schema inspected by this scope. -/
def schema (scope : RowScope model current inputs) :
    TableSchema model.catalog scope.owner :=
  match scope with
  | .table target => model.schemaFor target
  | .input port => inputs.schema port

/-- Resolve a related table in the current row scope's box. -/
def relatedTarget (scope : RowScope model current inputs)
    (table : TableId model.catalog scope.owner.box) : TableTarget model.catalog :=
  ⟨scope.owner.box, table⟩

@[simp] theorem owner_table (target : TableTarget model.catalog) :
    (RowScope.table (model := model) (current := current) (inputs := inputs) target).owner = target := rfl

@[simp] theorem owner_input (port : InputId inputs) :
    (RowScope.input (model := model) (current := current) port).owner = current := rfl

@[simp] theorem schema_table (target : TableTarget model.catalog) :
    (RowScope.table (model := model) (current := current) (inputs := inputs) target).schema =
      model.schemaFor target := rfl

@[simp] theorem schema_input (port : InputId inputs) :
    (RowScope.input (model := model) (current := current) port).schema = inputs.schema port := rfl

end RowScope

/-- An attribute proven to be a reference to one particular box-local table. -/
structure ReferenceAttributeId {catalog : SchemaUniverse}
    {owner : TableTarget catalog} (schema : TableSchema catalog owner)
    (target : TableId catalog owner.box) where
  id : AttributeId schema
  shapeEq : (schema.attr id).shape = .ref target
  sortEq : schema.attributeSort id = .ref ⟨owner.box, target⟩

namespace ReferenceAttributeId

@[simp] theorem attributeSort_eq_ref {catalog : SchemaUniverse}
    {owner : TableTarget catalog} {schema : TableSchema catalog owner}
    {target : TableId catalog owner.box} (reference : ReferenceAttributeId schema target) :
    schema.attributeSort reference.id = .ref ⟨owner.box, target⟩ := reference.sortEq

end ReferenceAttributeId

mutual
  /-- Intrinsically typed expressions. The model, transition table, input
  signature, inspected row and result scalar sort are all retained as indices. -/
  inductive Expr (model : ModelSchema) (current : TableTarget model.catalog)
      (inputs : InputSignature model current) :
      RowScope model current inputs → ScalarSort model.catalog → Type where
    | real {scope : RowScope model current inputs} (value : ScientificLiteral) :
        Expr model current inputs scope .real
    | int {scope : RowScope model current inputs} (value : Int) :
        Expr model current inputs scope .int
    | bool {scope : RowScope model current inputs} (value : Bool) :
        Expr model current inputs scope .bool
    | enum {scope : RowScope model current inputs}
        (attr : AttributeId scope.schema) (enumSchema : EnumSchema)
        (variant : VariantId scope.schema attr enumSchema) :
        Expr model current inputs scope
          (.enum scope.schema attr enumSchema variant.shapeEq)
    | param {scope : RowScope model current inputs} (id : ParameterId model.params) :
        Expr model current inputs scope
          ((model.params.get id).sort.scalarSort model.catalog)
    | selfAttr {scope : RowScope model current inputs} (id : AttributeId scope.schema) :
        Expr model current inputs scope (scope.schema.attributeSort id)
    | intToReal {scope : RowScope model current inputs}
        (value : Expr model current inputs scope .int) :
        Expr model current inputs scope .real
    | add {scope : RowScope model current inputs} (numeric : NumericSort)
        (lhs rhs : Expr model current inputs scope (numeric.scalarSort model.catalog)) :
        Expr model current inputs scope (numeric.scalarSort model.catalog)
    | sub {scope : RowScope model current inputs} (numeric : NumericSort)
        (lhs rhs : Expr model current inputs scope (numeric.scalarSort model.catalog)) :
        Expr model current inputs scope (numeric.scalarSort model.catalog)
    | mul {scope : RowScope model current inputs} (numeric : NumericSort)
        (lhs rhs : Expr model current inputs scope (numeric.scalarSort model.catalog)) :
        Expr model current inputs scope (numeric.scalarSort model.catalog)
    | div {scope : RowScope model current inputs}
        (lhs rhs : Expr model current inputs scope .real) :
        Expr model current inputs scope .real
    | eq {scope : RowScope model current inputs} {sort : ScalarSort model.catalog}
        (lhs rhs : Expr model current inputs scope sort) : Expr model current inputs scope .bool
    | ne {scope : RowScope model current inputs} {sort : ScalarSort model.catalog}
        (lhs rhs : Expr model current inputs scope sort) : Expr model current inputs scope .bool
    | lt {scope : RowScope model current inputs} (numeric : NumericSort)
        (lhs rhs : Expr model current inputs scope (numeric.scalarSort model.catalog)) :
        Expr model current inputs scope .bool
    | le {scope : RowScope model current inputs} (numeric : NumericSort)
        (lhs rhs : Expr model current inputs scope (numeric.scalarSort model.catalog)) :
        Expr model current inputs scope .bool
    | gt {scope : RowScope model current inputs} (numeric : NumericSort)
        (lhs rhs : Expr model current inputs scope (numeric.scalarSort model.catalog)) :
        Expr model current inputs scope .bool
    | ge {scope : RowScope model current inputs} (numeric : NumericSort)
        (lhs rhs : Expr model current inputs scope (numeric.scalarSort model.catalog)) :
        Expr model current inputs scope .bool
    | and {scope : RowScope model current inputs}
        (lhs rhs : Expr model current inputs scope .bool) : Expr model current inputs scope .bool
    | or {scope : RowScope model current inputs}
        (lhs rhs : Expr model current inputs scope .bool) : Expr model current inputs scope .bool
    | not {scope : RowScope model current inputs}
        (value : Expr model current inputs scope .bool) : Expr model current inputs scope .bool
    | enumIs {scope : RowScope model current inputs}
        (attr : AttributeId scope.schema) (enumSchema : EnumSchema)
        (variant : VariantId scope.schema attr enumSchema) :
        Expr model current inputs scope .bool
    | input {scope : RowScope model current inputs} {sort : ScalarSort model.catalog}
        (port : InputId inputs)
        (aggregate : Aggregate model current inputs (.input port) sort) :
        Expr model current inputs scope sort
    | agg {scope : RowScope model current inputs} {sort : ScalarSort model.catalog}
        (table : TableId model.catalog scope.owner.box)
        (joinTarget : TableId model.catalog scope.owner.box)
        (fk : ReferenceAttributeId (model.schemaFor (scope.relatedTarget table)) joinTarget)
        (selfFk : ReferenceAttributeId scope.schema joinTarget)
        (op : AggOp model current inputs (.table (scope.relatedTarget table)) sort)
        (filter : Expr model current inputs (.table (scope.relatedTarget table)) .bool) :
        Expr model current inputs scope sort

  /-- Typed aggregate operators. Count is Int; sum retains its numeric sort. -/
  inductive AggOp (model : ModelSchema) (current : TableTarget model.catalog)
      (inputs : InputSignature model current) :
      RowScope model current inputs → ScalarSort model.catalog → Type where
    | count {scope : RowScope model current inputs} :
        AggOp model current inputs scope .int
    | sum {scope : RowScope model current inputs} (numeric : NumericSort)
        (value : Expr model current inputs scope (numeric.scalarSort model.catalog)) :
        AggOp model current inputs scope (numeric.scalarSort model.catalog)

  /-- An input aggregate whose filter and value inspect precisely its port row. -/
  inductive Aggregate (model : ModelSchema) (current : TableTarget model.catalog)
      (inputs : InputSignature model current) :
      RowScope model current inputs → ScalarSort model.catalog → Type where
    | unfiltered {scope : RowScope model current inputs} {sort : ScalarSort model.catalog}
        (op : AggOp model current inputs scope sort) :
        Aggregate model current inputs scope sort
    | filtered {scope : RowScope model current inputs} {sort : ScalarSort model.catalog}
        (op : AggOp model current inputs scope sort)
        (filter : Expr model current inputs scope .bool) :
        Aggregate model current inputs scope sort
end

/-- Top-level terms inspect the transition's current table. -/
abbrev Term (model : ModelSchema) (current : TableTarget model.catalog)
    (inputs : InputSignature model current) (sort : ScalarSort model.catalog) :=
  Expr model current inputs (.table current) sort

mutual
  /-- Exact structural erasure. Explicit Int-to-Real coercions are transparent. -/
  def Expr.erase {model : ModelSchema} {current : TableTarget model.catalog}
      {inputs : InputSignature model current} {scope : RowScope model current inputs}
      {sort : ScalarSort model.catalog} (expr : Expr model current inputs scope sort) : IR.Expr :=
    match expr with
    | .real value => .real value.erase
    | .int value => .int value
    | .bool value => .bool value
    | .enum attr enumSchema variant => .enum (enumSchema.variantName scope.schema attr variant)
    | .param id => .param (model.params.name id)
    | .selfAttr id => .selfAttr (scope.schema.attributeName id)
    | .intToReal value => value.erase
    | .add _ lhs rhs => .add lhs.erase rhs.erase
    | .sub _ lhs rhs => .sub lhs.erase rhs.erase
    | .mul _ lhs rhs => .mul lhs.erase rhs.erase
    | .div lhs rhs => .div lhs.erase rhs.erase
    | .eq lhs rhs => .eq lhs.erase rhs.erase
    | .ne lhs rhs => .ne lhs.erase rhs.erase
    | .lt _ lhs rhs => .lt lhs.erase rhs.erase
    | .le _ lhs rhs => .le lhs.erase rhs.erase
    | .gt _ lhs rhs => .gt lhs.erase rhs.erase
    | .ge _ lhs rhs => .ge lhs.erase rhs.erase
    | .and lhs rhs => .and lhs.erase rhs.erase
    | .or lhs rhs => .or lhs.erase rhs.erase
    | .not value => .not value.erase
    | .enumIs attr enumSchema variant =>
        .enumIs (scope.schema.attributeName attr)
          (enumSchema.variantName scope.schema attr variant)
    | .input port aggregate => .input (inputs.name port) aggregate.erase
    | .agg table _ fk selfFk op filter =>
        .agg op.erase (model.catalog.tableName (scope.relatedTarget table))
          ((model.schemaFor (scope.relatedTarget table)).attributeName fk.id)
          (scope.schema.attributeName selfFk.id) filter.erase

  /-- Exact aggregate-operator erasure. -/
  def AggOp.erase {model : ModelSchema} {current : TableTarget model.catalog}
      {inputs : InputSignature model current} {scope : RowScope model current inputs}
      {sort : ScalarSort model.catalog} (op : AggOp model current inputs scope sort) : IR.AggOp :=
    match op with
    | .count => .count
    | .sum _ value => .sum value.erase

  /-- Exact input-aggregate erasure, preserving optional-filter shape. -/
  def Aggregate.erase {model : ModelSchema} {current : TableTarget model.catalog}
      {inputs : InputSignature model current} {scope : RowScope model current inputs}
      {sort : ScalarSort model.catalog} (aggregate : Aggregate model current inputs scope sort) :
      IR.Aggregate :=
    match aggregate with
    | .unfiltered op => .mk op.erase none
    | .filtered op filter => .mk op.erase (some filter.erase)
end

/-- Effects require the exact scalar sort of their resolved destination. -/
inductive Effect (model : ModelSchema) (current : TableTarget model.catalog)
    (inputs : InputSignature model current) where
  | setAttr (destination : AttributeId (model.schemaFor current))
      (value : Term model current inputs ((model.schemaFor current).attributeSort destination)) :
      Effect model current inputs

namespace Effect

variable {model : ModelSchema} {current : TableTarget model.catalog}
    {inputs : InputSignature model current}

def destination : Effect model current inputs → AttributeId (model.schemaFor current)
  | .setAttr attr _ => attr

def value (effect : Effect model current inputs) :
    Term model current inputs ((model.schemaFor current).attributeSort effect.destination) :=
  match effect with
  | .setAttr _ value => value

def erase : Effect model current inputs → IR.Effect
  | .setAttr attr value =>
      .setAttr ((model.schemaFor current).attributeName attr) value.erase

@[simp] theorem erase_setAttr (attr : AttributeId (model.schemaFor current))
    (value : Term model current inputs ((model.schemaFor current).attributeSort attr)) :
    erase (.setAttr attr value) =
      .setAttr ((model.schemaFor current).attributeName attr) value.erase := rfl

end Effect

/-- Exact domains admitted for contest ordering. -/
inductive OrderingDomain (model : ModelSchema) where
  | real
  | int
  | enum {owner : TableTarget model.catalog}
      (schema : TableSchema model.catalog owner) (attr : AttributeId schema)
      (enumSchema : EnumSchema) (shapeEq : (schema.attr attr).shape = .enum enumSchema)

namespace OrderingDomain

/-- The scalar sort retained for later compatibility checking. -/
def scalarSort {model : ModelSchema} : OrderingDomain model → ScalarSort model.catalog
  | .real => .real
  | .int => .int
  | .enum schema attr enumSchema shapeEq => .enum schema attr enumSchema shapeEq

@[simp] theorem scalarSort_real {model : ModelSchema} :
    (OrderingDomain.real : OrderingDomain model).scalarSort = .real := rfl

@[simp] theorem scalarSort_int {model : ModelSchema} :
    (OrderingDomain.int : OrderingDomain model).scalarSort = .int := rfl

@[simp] theorem scalarSort_ne_bool {model : ModelSchema} (domain : OrderingDomain model) :
    domain.scalarSort ≠ .bool := by
  cases domain <;> simp [scalarSort]

@[simp] theorem scalarSort_ne_ref {model : ModelSchema} (domain : OrderingDomain model)
    (target : TableTarget model.catalog) : domain.scalarSort ≠ .ref target := by
  cases domain <;> simp [scalarSort]

end OrderingDomain

/-- Whether an ordering is emitted by current surface syntax or only accepted
from raw IR/checking. -/
inductive OrderingAvailability where
  | surfaceProduced
  | rawCheckable

/-- Typed claim ordering, retaining both sort domain and producer boundary. -/
inductive ClaimOrdering (model : ModelSchema) (current : TableTarget model.catalog)
    (inputs : InputSignature model current) :
    OrderingDomain model → OrderingAvailability → Type where
  | raceTime : ClaimOrdering model current inputs .real .surfaceProduced
  | key (domain : OrderingDomain model)
      (expr : Term model current inputs domain.scalarSort) :
      ClaimOrdering model current inputs domain .rawCheckable

namespace ClaimOrdering

variable {model : ModelSchema} {current : TableTarget model.catalog}
    {inputs : InputSignature model current}

def erase {domain : OrderingDomain model} {availability : OrderingAvailability} :
    ClaimOrdering model current inputs domain availability → IR.ClaimOrdering
  | .raceTime => .raceTime
  | .key _ expr => .key expr.erase

@[simp] theorem erase_raceTime :
    erase (ClaimOrdering.raceTime :
      ClaimOrdering model current inputs .real .surfaceProduced) = .raceTime := rfl

@[simp] theorem erase_key (domain : OrderingDomain model)
    (expr : Term model current inputs domain.scalarSort) :
    erase (.key domain expr) = .key expr.erase := rfl

theorem surface_domain_real {domain : OrderingDomain model}
    (ordering : ClaimOrdering model current inputs domain .surfaceProduced) :
    domain = .real := by
  cases ordering
  rfl

theorem raw_checkable_is_key {domain : OrderingDomain model}
    (ordering : ClaimOrdering model current inputs domain .rawCheckable) :
    ∃ expr : Term model current inputs domain.scalarSort, ordering = .key domain expr := by
  cases ordering with
  | key _ expr => exact ⟨expr, rfl⟩

end ClaimOrdering

/-- A resource expression retains its resolved target and the ordering domain
retained for later compatibility checking. -/
structure ResourceClaim (model : ModelSchema) (current : TableTarget model.catalog)
    (inputs : InputSignature model current) where
  resourceTarget : TableTarget model.catalog
  resource : Term model current inputs (.ref resourceTarget)
  orderingDomain : OrderingDomain model
  orderingAvailability : OrderingAvailability
  ordering : ClaimOrdering model current inputs orderingDomain orderingAvailability

namespace ResourceClaim

variable {model : ModelSchema} {current : TableTarget model.catalog}
    {inputs : InputSignature model current}

def erase (claim : ResourceClaim model current inputs) : IR.ResourceClaim :=
  { resource := claim.resource.erase, ordering := claim.ordering.erase }

@[simp] theorem erase_resource (claim : ResourceClaim model current inputs) :
    claim.erase.resource = claim.resource.erase := rfl

@[simp] theorem erase_ordering (claim : ResourceClaim model current inputs) :
    claim.erase.ordering = claim.ordering.erase := rfl

end ResourceClaim

/-- Typed term-level transition payload. Raw name/table assembly belongs to the
later model checker. -/
structure TransitionTerms (model : ModelSchema) (current : TableTarget model.catalog)
    (inputs : InputSignature model current) where
  guard : Term model current inputs .bool
  hazard : Term model current inputs .real
  effects : List (Effect model current inputs)
  claims : List (ResourceClaim model current inputs)

namespace TransitionTerms

variable {model : ModelSchema} {current : TableTarget model.catalog}
    {inputs : InputSignature model current}

def eraseGuard (terms : TransitionTerms model current inputs) : IR.Expr := terms.guard.erase
def eraseHazard (terms : TransitionTerms model current inputs) : IR.Expr := terms.hazard.erase
def eraseEffects (terms : TransitionTerms model current inputs) : List IR.Effect :=
  terms.effects.map Effect.erase
def eraseClaims (terms : TransitionTerms model current inputs) : List IR.ResourceClaim :=
  terms.claims.map ResourceClaim.erase

@[simp] theorem eraseGuard_exact (terms : TransitionTerms model current inputs) :
    terms.eraseGuard = terms.guard.erase := rfl

@[simp] theorem eraseHazard_exact (terms : TransitionTerms model current inputs) :
    terms.eraseHazard = terms.hazard.erase := rfl

@[simp] theorem eraseEffects_eq_map (terms : TransitionTerms model current inputs) :
    terms.eraseEffects = terms.effects.map Effect.erase := rfl

@[simp] theorem eraseClaims_eq_map (terms : TransitionTerms model current inputs) :
    terms.eraseClaims = terms.claims.map ResourceClaim.erase := rfl

end TransitionTerms

/-- Existential packaging records the one intrinsic result sort of an expression. -/
structure PackedExpr (model : ModelSchema) (current : TableTarget model.catalog)
    (inputs : InputSignature model current) (scope : RowScope model current inputs) where
  sort : ScalarSort model.catalog
  expr : Expr model current inputs scope sort

namespace PackedExpr

variable {model : ModelSchema} {current : TableTarget model.catalog}
    {inputs : InputSignature model current} {scope : RowScope model current inputs}

def HasResult (expr : PackedExpr model current inputs scope)
    (sort : ScalarSort model.catalog) : Prop := expr.sort = sort

@[simp] theorem has_indexed_result (expr : PackedExpr model current inputs scope) :
    expr.HasResult expr.sort := rfl

theorem resultSort_unique (expr : PackedExpr model current inputs scope)
    {left right : ScalarSort model.catalog} (hl : expr.HasResult left)
    (hr : expr.HasResult right) : left = right := hl.symm.trans hr

end PackedExpr

namespace Expr

variable {model : ModelSchema} {current : TableTarget model.catalog}
    {inputs : InputSignature model current} {scope : RowScope model current inputs}
    {sort : ScalarSort model.catalog}

/-- The result sort is the expression's intrinsic index. -/
def resultSort (_ : Expr model current inputs scope sort) : ScalarSort model.catalog := sort

@[simp] theorem resultSort_eq (expr : Expr model current inputs scope sort) :
    expr.resultSort = sort := rfl

@[simp] theorem erase_intToReal (expr : Expr model current inputs scope .int) :
    (Expr.intToReal expr).erase = expr.erase := rfl

@[simp] theorem erase_real (value : ScientificLiteral) :
    (Expr.real (model := model) (current := current) (inputs := inputs) (scope := scope) value).erase =
      .real value.source := rfl

@[simp] theorem erase_int (value : Int) :
    (Expr.int (model := model) (current := current) (inputs := inputs) (scope := scope) value).erase =
      .int value := rfl

@[simp] theorem erase_bool (value : Bool) :
    (Expr.bool (model := model) (current := current) (inputs := inputs) (scope := scope) value).erase =
      .bool value := rfl

@[simp] theorem erase_add (numeric : NumericSort)
    (lhs rhs : Expr model current inputs scope (numeric.scalarSort model.catalog)) :
    (Expr.add numeric lhs rhs).erase = .add lhs.erase rhs.erase := rfl

@[simp] theorem erase_sub (numeric : NumericSort)
    (lhs rhs : Expr model current inputs scope (numeric.scalarSort model.catalog)) :
    (Expr.sub numeric lhs rhs).erase = .sub lhs.erase rhs.erase := rfl

@[simp] theorem erase_mul (numeric : NumericSort)
    (lhs rhs : Expr model current inputs scope (numeric.scalarSort model.catalog)) :
    (Expr.mul numeric lhs rhs).erase = .mul lhs.erase rhs.erase := rfl

@[simp] theorem erase_div (lhs rhs : Expr model current inputs scope .real) :
    (Expr.div lhs rhs).erase = .div lhs.erase rhs.erase := rfl

@[simp] theorem erase_eq {termSort : ScalarSort model.catalog}
    (lhs rhs : Expr model current inputs scope termSort) :
    (Expr.eq lhs rhs).erase = .eq lhs.erase rhs.erase := rfl

@[simp] theorem erase_ne {termSort : ScalarSort model.catalog}
    (lhs rhs : Expr model current inputs scope termSort) :
    (Expr.ne lhs rhs).erase = .ne lhs.erase rhs.erase := rfl

@[simp] theorem erase_lt (numeric : NumericSort)
    (lhs rhs : Expr model current inputs scope (numeric.scalarSort model.catalog)) :
    (Expr.lt numeric lhs rhs).erase = .lt lhs.erase rhs.erase := rfl

@[simp] theorem erase_le (numeric : NumericSort)
    (lhs rhs : Expr model current inputs scope (numeric.scalarSort model.catalog)) :
    (Expr.le numeric lhs rhs).erase = .le lhs.erase rhs.erase := rfl

@[simp] theorem erase_gt (numeric : NumericSort)
    (lhs rhs : Expr model current inputs scope (numeric.scalarSort model.catalog)) :
    (Expr.gt numeric lhs rhs).erase = .gt lhs.erase rhs.erase := rfl

@[simp] theorem erase_ge (numeric : NumericSort)
    (lhs rhs : Expr model current inputs scope (numeric.scalarSort model.catalog)) :
    (Expr.ge numeric lhs rhs).erase = .ge lhs.erase rhs.erase := rfl

@[simp] theorem erase_and (lhs rhs : Expr model current inputs scope .bool) :
    (Expr.and lhs rhs).erase = .and lhs.erase rhs.erase := rfl

@[simp] theorem erase_or (lhs rhs : Expr model current inputs scope .bool) :
    (Expr.or lhs rhs).erase = .or lhs.erase rhs.erase := rfl

@[simp] theorem erase_not (value : Expr model current inputs scope .bool) :
    (Expr.not value).erase = .not value.erase := rfl

@[simp] theorem erase_enum (attr : AttributeId scope.schema) (enumSchema : EnumSchema)
    (variant : VariantId scope.schema attr enumSchema) :
    (Expr.enum (model := model) (current := current) (inputs := inputs)
      (scope := scope) attr enumSchema variant).erase =
      .enum (enumSchema.variantName scope.schema attr variant) := rfl

@[simp] theorem erase_param (id : ParameterId model.params) :
    (Expr.param (model := model) (current := current) (inputs := inputs)
      (scope := scope) id).erase = .param (model.params.name id) := rfl

@[simp] theorem erase_selfAttr (id : AttributeId scope.schema) :
    (Expr.selfAttr (model := model) (current := current) (inputs := inputs)
      (scope := scope) id).erase = .selfAttr (scope.schema.attributeName id) := rfl

@[simp] theorem erase_enumIs (attr : AttributeId scope.schema) (enumSchema : EnumSchema)
    (variant : VariantId scope.schema attr enumSchema) :
    (Expr.enumIs (model := model) (current := current) (inputs := inputs)
      (scope := scope) attr enumSchema variant).erase =
      .enumIs (scope.schema.attributeName attr)
        (enumSchema.variantName scope.schema attr variant) := rfl

@[simp] theorem erase_input {result : ScalarSort model.catalog} (port : InputId inputs)
    (aggregate : Aggregate model current inputs (.input port) result) :
    (Expr.input (scope := scope) port aggregate).erase =
      .input (inputs.name port) aggregate.erase := rfl

@[simp] theorem erase_agg {result : ScalarSort model.catalog}
    (table : TableId model.catalog scope.owner.box)
    (joinTarget : TableId model.catalog scope.owner.box)
    (fk : ReferenceAttributeId (model.schemaFor (scope.relatedTarget table)) joinTarget)
    (selfFk : ReferenceAttributeId scope.schema joinTarget)
    (op : AggOp model current inputs (.table (scope.relatedTarget table)) result)
    (filter : Expr model current inputs (.table (scope.relatedTarget table)) .bool) :
    (Expr.agg table joinTarget fk selfFk op filter).erase =
      .agg op.erase (model.catalog.tableName (scope.relatedTarget table))
        ((model.schemaFor (scope.relatedTarget table)).attributeName fk.id)
        (scope.schema.attributeName selfFk.id) filter.erase := rfl

/-- Both relational join attributes have one intrinsic reference target even
when their row schemas and table owners differ. -/
theorem relationalJoin_compatible
    (table : TableId model.catalog scope.owner.box)
    (joinTarget : TableId model.catalog scope.owner.box)
    (fk : ReferenceAttributeId (model.schemaFor (scope.relatedTarget table)) joinTarget)
    (selfFk : ReferenceAttributeId scope.schema joinTarget) :
    (model.schemaFor (scope.relatedTarget table)).attributeSort fk.id =
      scope.schema.attributeSort selfFk.id := by
  rw [fk.attributeSort_eq_ref, selfFk.attributeSort_eq_ref]
  simp [RowScope.relatedTarget]

end Expr

namespace AggOp

variable {model : ModelSchema} {current : TableTarget model.catalog}
    {inputs : InputSignature model current} {scope : RowScope model current inputs}

@[simp] theorem erase_count :
    (AggOp.count : AggOp model current inputs scope .int).erase = .count := rfl

@[simp] theorem erase_sum (numeric : NumericSort)
    (value : Expr model current inputs scope (numeric.scalarSort model.catalog)) :
    (AggOp.sum numeric value).erase = .sum value.erase := rfl

end AggOp

namespace Aggregate

variable {model : ModelSchema} {current : TableTarget model.catalog}
    {inputs : InputSignature model current} {scope : RowScope model current inputs}

@[simp] theorem erase_unfiltered {sort : ScalarSort model.catalog}
    (op : AggOp model current inputs scope sort) :
    (Aggregate.unfiltered op).erase = .mk op.erase none := rfl

@[simp] theorem erase_filtered {sort : ScalarSort model.catalog}
    (op : AggOp model current inputs scope sort)
    (filter : Expr model current inputs scope .bool) :
    (Aggregate.filtered op filter).erase = .mk op.erase (some filter.erase) := rfl

end Aggregate

/- Proof-only dependent packages expose every semantic owner carried by the
accepted PRD-0003 identifiers. Equality of packages therefore yields equality
(or heterogeneous equality for dependent components) of the corresponding
owners; no parallel identifier or runtime context is introduced. -/
theorem packedOwner_ne_of_owner_ne {Owner : Type} {Identity : Owner → Type}
    {leftOwner rightOwner : Owner} {left : Identity leftOwner}
    {right : Identity rightOwner} (different : leftOwner ≠ rightOwner) :
    (⟨leftOwner, left⟩ : Σ owner, Identity owner) ≠ ⟨rightOwner, right⟩ :=
  fun same => different (congrArg Sigma.fst same)

abbrev PackedParameterId := Σ owner : ParamContext, ParameterId owner

namespace PackedParameterId

theorem owner_eq_of_eq {leftOwner rightOwner : ParamContext}
    {left : ParameterId leftOwner} {right : ParameterId rightOwner}
    (same : (⟨leftOwner, left⟩ : PackedParameterId) = ⟨rightOwner, right⟩) :
    leftOwner = rightOwner := congrArg Sigma.fst same

end PackedParameterId

abbrev PackedBoxId := Σ catalog : SchemaUniverse, BoxId catalog

namespace PackedBoxId

theorem catalog_eq_of_eq {leftCatalog rightCatalog : SchemaUniverse}
    {left : BoxId leftCatalog} {right : BoxId rightCatalog}
    (same : (⟨leftCatalog, left⟩ : PackedBoxId) = ⟨rightCatalog, right⟩) :
    leftCatalog = rightCatalog := congrArg Sigma.fst same

end PackedBoxId

abbrev PackedTableId :=
  Σ catalog : SchemaUniverse, Σ box : BoxId catalog, TableId catalog box

namespace PackedTableId

theorem catalog_eq_of_eq {leftCatalog rightCatalog : SchemaUniverse}
    {leftBox : BoxId leftCatalog} {rightBox : BoxId rightCatalog}
    {left : TableId leftCatalog leftBox} {right : TableId rightCatalog rightBox}
    (same : (⟨leftCatalog, ⟨leftBox, left⟩⟩ : PackedTableId) =
      ⟨rightCatalog, ⟨rightBox, right⟩⟩) : leftCatalog = rightCatalog :=
  congrArg Sigma.fst same

theorem box_heq_of_eq {leftCatalog rightCatalog : SchemaUniverse}
    {leftBox : BoxId leftCatalog} {rightBox : BoxId rightCatalog}
    {left : TableId leftCatalog leftBox} {right : TableId rightCatalog rightBox}
    (same : (⟨leftCatalog, ⟨leftBox, left⟩⟩ : PackedTableId) =
      ⟨rightCatalog, ⟨rightBox, right⟩⟩) : HEq leftBox rightBox := by
  cases same
  rfl

end PackedTableId

abbrev PackedAttributeId :=
  Σ catalog : SchemaUniverse,
    Σ owner : TableTarget catalog,
      Σ schema : TableSchema catalog owner, AttributeId schema

namespace PackedAttributeId

theorem catalog_eq_of_eq {leftCatalog rightCatalog : SchemaUniverse}
    {leftOwner : TableTarget leftCatalog} {rightOwner : TableTarget rightCatalog}
    {leftSchema : TableSchema leftCatalog leftOwner}
    {rightSchema : TableSchema rightCatalog rightOwner}
    {left : AttributeId leftSchema} {right : AttributeId rightSchema}
    (same : (⟨leftCatalog, ⟨leftOwner, ⟨leftSchema, left⟩⟩⟩ : PackedAttributeId) =
      ⟨rightCatalog, ⟨rightOwner, ⟨rightSchema, right⟩⟩⟩) :
    leftCatalog = rightCatalog := congrArg Sigma.fst same

theorem owner_heq_of_eq {leftCatalog rightCatalog : SchemaUniverse}
    {leftOwner : TableTarget leftCatalog} {rightOwner : TableTarget rightCatalog}
    {leftSchema : TableSchema leftCatalog leftOwner}
    {rightSchema : TableSchema rightCatalog rightOwner}
    {left : AttributeId leftSchema} {right : AttributeId rightSchema}
    (same : (⟨leftCatalog, ⟨leftOwner, ⟨leftSchema, left⟩⟩⟩ : PackedAttributeId) =
      ⟨rightCatalog, ⟨rightOwner, ⟨rightSchema, right⟩⟩⟩) : HEq leftOwner rightOwner := by
  cases same
  rfl

theorem schema_heq_of_eq {leftCatalog rightCatalog : SchemaUniverse}
    {leftOwner : TableTarget leftCatalog} {rightOwner : TableTarget rightCatalog}
    {leftSchema : TableSchema leftCatalog leftOwner}
    {rightSchema : TableSchema rightCatalog rightOwner}
    {left : AttributeId leftSchema} {right : AttributeId rightSchema}
    (same : (⟨leftCatalog, ⟨leftOwner, ⟨leftSchema, left⟩⟩⟩ : PackedAttributeId) =
      ⟨rightCatalog, ⟨rightOwner, ⟨rightSchema, right⟩⟩⟩) : HEq leftSchema rightSchema := by
  cases same
  rfl

end PackedAttributeId

abbrev PackedVariantId :=
  Σ catalog : SchemaUniverse,
    Σ owner : TableTarget catalog,
      Σ schema : TableSchema catalog owner,
        Σ attr : AttributeId schema,
          Σ enumSchema : EnumSchema, VariantId schema attr enumSchema

namespace PackedVariantId

theorem owner_heq_of_eq {leftCatalog rightCatalog : SchemaUniverse}
    {leftOwner : TableTarget leftCatalog} {rightOwner : TableTarget rightCatalog}
    {leftSchema : TableSchema leftCatalog leftOwner}
    {rightSchema : TableSchema rightCatalog rightOwner}
    {leftAttr : AttributeId leftSchema} {rightAttr : AttributeId rightSchema}
    {leftEnum rightEnum : EnumSchema}
    {left : VariantId leftSchema leftAttr leftEnum}
    {right : VariantId rightSchema rightAttr rightEnum}
    (same :
      (⟨leftCatalog, ⟨leftOwner, ⟨leftSchema, ⟨leftAttr, ⟨leftEnum, left⟩⟩⟩⟩⟩ :
        PackedVariantId) =
      ⟨rightCatalog, ⟨rightOwner, ⟨rightSchema, ⟨rightAttr, ⟨rightEnum, right⟩⟩⟩⟩⟩) :
    HEq leftOwner rightOwner := by
  cases same
  rfl

theorem schema_attr_enum_heq_of_eq {leftCatalog rightCatalog : SchemaUniverse}
    {leftOwner : TableTarget leftCatalog} {rightOwner : TableTarget rightCatalog}
    {leftSchema : TableSchema leftCatalog leftOwner}
    {rightSchema : TableSchema rightCatalog rightOwner}
    {leftAttr : AttributeId leftSchema} {rightAttr : AttributeId rightSchema}
    {leftEnum rightEnum : EnumSchema}
    {left : VariantId leftSchema leftAttr leftEnum}
    {right : VariantId rightSchema rightAttr rightEnum}
    (same :
      (⟨leftCatalog, ⟨leftOwner, ⟨leftSchema, ⟨leftAttr, ⟨leftEnum, left⟩⟩⟩⟩⟩ :
        PackedVariantId) =
      ⟨rightCatalog, ⟨rightOwner, ⟨rightSchema, ⟨rightAttr, ⟨rightEnum, right⟩⟩⟩⟩⟩) :
    HEq leftSchema rightSchema ∧ HEq leftAttr rightAttr ∧ HEq leftEnum rightEnum := by
  cases same
  exact ⟨HEq.rfl, HEq.rfl, HEq.rfl⟩

end PackedVariantId

abbrev PackedReferenceAttributeId :=
  Σ catalog : SchemaUniverse,
    Σ owner : TableTarget catalog,
      Σ schema : TableSchema catalog owner,
        Σ target : TableId catalog owner.box, ReferenceAttributeId schema target

namespace PackedReferenceAttributeId

theorem owner_schema_target_heq_of_eq {leftCatalog rightCatalog : SchemaUniverse}
    {leftOwner : TableTarget leftCatalog} {rightOwner : TableTarget rightCatalog}
    {leftSchema : TableSchema leftCatalog leftOwner}
    {rightSchema : TableSchema rightCatalog rightOwner}
    {leftTarget : TableId leftCatalog leftOwner.box}
    {rightTarget : TableId rightCatalog rightOwner.box}
    {left : ReferenceAttributeId leftSchema leftTarget}
    {right : ReferenceAttributeId rightSchema rightTarget}
    (same :
      (⟨leftCatalog, ⟨leftOwner, ⟨leftSchema, ⟨leftTarget, left⟩⟩⟩⟩ :
        PackedReferenceAttributeId) =
      ⟨rightCatalog, ⟨rightOwner, ⟨rightSchema, ⟨rightTarget, right⟩⟩⟩⟩) :
    HEq leftOwner rightOwner ∧ HEq leftSchema rightSchema ∧ HEq leftTarget rightTarget := by
  cases same
  exact ⟨HEq.rfl, HEq.rfl, HEq.rfl⟩

end PackedReferenceAttributeId

end Sembla.Semantics
