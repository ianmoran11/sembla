import Sembla.Semantics.CheckDeclarations

/-!
Canonical model-local term elaboration for the V1 IR.

The checker consumes the accepted declaration context.  Its input signature is
built directly from the source-ordered PRD 0005 port schemas; no secondary name
map or replacement schema is introduced.
-/
namespace Sembla.Semantics

open Sembla

set_option maxHeartbeats 1000000

/-! ## Canonical term context -/

/-- The declaration, box, and current-table identities which determine a term
scope.  The input signature below is a canonical projection, not retained
parallel state. -/
structure TermContext where
  declarations : DeclarationContext
  box : BoxId declarations.modelSchema.catalog
  currentTable : TableId declarations.modelSchema.catalog box

namespace TermContext

abbrev model (Γ : TermContext) : ModelSchema := Γ.declarations.modelSchema
abbrev current (Γ : TermContext) : TableTarget Γ.model.catalog := ⟨Γ.box, Γ.currentTable⟩

/-- Instantiate exactly the accepted source-ordered input schemas at the current
table. -/
def inputs (Γ : TermContext) : InputSignature Γ.model Γ.current :=
  { ports :=
      { scope := Γ.model.catalog.boxName Γ.box ++ ".inputs"
        entries := (Γ.declarations.inputPortSchemas Γ.box).map fun schema =>
          { name := schema.name, rowSchema := schema.instantiate Γ.currentTable }
        uniqueNames := by
          simpa only [List.map_map, Function.comp_apply] using
            Γ.declarations.inputPortSchemas_uniqueNames Γ.box } }

@[simp] theorem inputNames_exact (Γ : TermContext) :
    Γ.inputs.ports.entries.map InputDecl.name =
      (Γ.declarations.inputs Γ.box).map IR.PortDecl.name := by
  simpa only [inputs, List.map_map, Function.comp_apply] using
    Γ.declarations.inputPortSchemas_names Γ.box

@[simp] theorem inputSchemas_erase_exact (Γ : TermContext) :
    Γ.inputs.ports.entries.map (fun input => input.rowSchema.eraseAttributes) =
      (Γ.declarations.inputs Γ.box).map IR.PortDecl.schema := by
  rw [← Γ.declarations.inputPortSchemas_sources Γ.box]
  simp only [inputs, List.map_map, Function.comp_apply]
  apply List.map_congr_left
  intro schema _
  exact schema.instantiate_erases_exact Γ.currentTable

end TermContext

/-- Resolve a port in the sole ordered input signature. -/
def InputSignature.lookup {model : ModelSchema} {current : TableTarget model.catalog}
    (inputs : InputSignature model current) (name : String) : Option (InputId inputs) :=
  (inputs.ports.lookupOrdinal name).map InputId.mk

@[simp] theorem InputSignature.lookup_name {model : ModelSchema}
    {current : TableTarget model.catalog} (inputs : InputSignature model current)
    {wanted : String} {port : InputId inputs}
    (found : inputs.lookup wanted = some port) : inputs.name port = wanted := by
  unfold InputSignature.lookup at found
  cases result : inputs.ports.lookupOrdinal wanted with
  | none => simp [result] at found
  | some ordinal =>
      simp [result] at found
      subst port
      exact inputs.ports.lookupOrdinal_name result

/-! ## Stable term diagnostics -/

inductive TermCheckErrorCategory where
  | unknownParameter
  | unknownAttribute
  | unknownEnumVariant
  | unknownInput
  | unknownTable
  | unknownJoinAttribute
  | nestedInputAggregate
  | cannotInferEnumOwner
  | expectedBool
  | expectedReal
  | expectedNumeric
  | expectedReference
  | expectedOrderable
  | sortMismatch
  | incompatibleEquality
  | incompatibleJoinTargets
  | duplicateResourceClaim
  | unclaimedRefWrite
  deriving Repr, BEq, DecidableEq

inductive ModelCheckPathSegment where
  | model
  | box (index : Nat)
  | transition (index : Nat)
  | output (index : Nat)
  | view (index : Nat)
  | groupedView (index : Nat)
  | summary (index : Nat)
  | guard | hazard
  | effects | effect (index : Nat) | destination | value
  | contests | claim (index : Nat) | resource | orderingKey
  | lhs | rhs | operand
  | aggregate | aggregateFilter | aggregateValue
  | inputPort | tableTarget | joinForeignAttribute | joinSelfAttribute
  | outputBuilder | outputSchema | outputFields | outputField (index : Nat)
  | fieldName | fieldOperation | fieldFilter | fieldValue
  | viewTable | viewFilter | viewValue | viewReducer
  | groupedKeys | groupedKey (index : Nat) | groupedAttribute | groupedBand
  | summaryBox | summaryView | summaryReducer
  deriving Repr, BEq, DecidableEq

structure TermCheckError where
  category : TermCheckErrorCategory
  path : List ModelCheckPathSegment
  deriving Repr, BEq, DecidableEq

def termError (category : TermCheckErrorCategory)
    (path : List ModelCheckPathSegment) : Except TermCheckError α :=
  .error ⟨category, path⟩

/-! ## Intrinsic synthesis witnesses -/

/-- The checker records how a result sort arose in the current row scope.  Enum
origins use an attribute identity in that exact scope; references use their
resolved table identity.  This makes sort comparison constructive. -/
inductive SortOrigin (Γ : TermContext)
    (scope : RowScope Γ.model Γ.current Γ.inputs) :
    ScalarSort Γ.model.catalog → Type where
  | real : SortOrigin Γ scope .real
  | int : SortOrigin Γ scope .int
  | bool : SortOrigin Γ scope .bool
  | enum (attr : AttributeId scope.schema) (enumSchema : EnumSchema)
      (shapeEq : (scope.schema.attr attr).shape = .enum enumSchema) :
      SortOrigin Γ scope (.enum scope.schema attr enumSchema shapeEq)
  | ref (target : TableTarget Γ.model.catalog) : SortOrigin Γ scope (.ref target)

structure CheckedExprResult (Γ : TermContext)
    (scope : RowScope Γ.model Γ.current Γ.inputs) where
  sort : ScalarSort Γ.model.catalog
  expr : Expr Γ.model Γ.current Γ.inputs scope sort
  origin : SortOrigin Γ scope sort

namespace SortOrigin

/-- The origin of a resolved attribute sort. -/
def ofAttribute (Γ : TermContext) {scope : RowScope Γ.model Γ.current Γ.inputs}
    (attr : AttributeId scope.schema) :
    SortOrigin Γ scope (scope.schema.attributeSort attr) := by
  rw [TableSchema.attributeSort]
  split <;> rename_i shapeEq
  · exact .real
  · exact .int
  · exact .enum attr _ shapeEq
  · exact .ref _

end SortOrigin

@[simp] theorem attributeSort_ref_of_shape {catalog : SchemaUniverse}
    {owner : TableTarget catalog} {schema : TableSchema catalog owner}
    (attr : AttributeId schema) (target : TableId catalog owner.box)
    (shapeEq : (schema.attr attr).shape = .ref target) :
    schema.attributeSort attr = .ref ⟨owner.box, target⟩ := by
  simp only [TableSchema.attributeSort]
  split <;> rename_i actual
  · rw [shapeEq] at actual
    contradiction
  · rw [shapeEq] at actual
    contradiction
  · rw [shapeEq] at actual
    contradiction
  · rw [shapeEq] at actual
    cases actual
    rfl

/-- Box identities are exactly their catalog ordinals. -/
theorem boxId_eq_of_val_eq {catalog : SchemaUniverse} {left right : BoxId catalog}
    (same : left.ordinal.val = right.ordinal.val) : left = right := by
  cases left with
  | mk leftOrdinal =>
      cases right with
      | mk rightOrdinal =>
          have : leftOrdinal = rightOrdinal := Fin.ext same
          cases this
          rfl

/-- Table identities within one box are exactly their catalog ordinals. -/
theorem tableId_eq_of_val_eq {catalog : SchemaUniverse} {box : BoxId catalog}
    {left right : TableId catalog box}
    (same : left.ordinal.val = right.ordinal.val) : left = right := by
  cases left with
  | mk leftOrdinal =>
      cases right with
      | mk rightOrdinal =>
          have : leftOrdinal = rightOrdinal := Fin.ext same
          cases this
          rfl

/-- Equal box/table ordinals identify one dependent table target. -/
theorem tableTarget_eq_of_val_eq {catalog : SchemaUniverse}
    {left right : TableTarget catalog}
    (boxSame : left.box.ordinal.val = right.box.ordinal.val)
    (tableSame : left.table.ordinal.val = right.table.ordinal.val) : left = right := by
  cases left with
  | mk leftBox leftTable =>
      cases right with
      | mk rightBox rightTable =>
          have boxEq : leftBox = rightBox := boxId_eq_of_val_eq boxSame
          subst rightBox
          have tableEq : leftTable = rightTable := tableId_eq_of_val_eq tableSame
          subst rightTable
          rfl

/-- Constructive equality for resolved table targets. -/
def tableTargetEq? {catalog : SchemaUniverse} (left right : TableTarget catalog) :
    Option (PLift (left = right)) :=
  if boxSame : left.box.ordinal.val = right.box.ordinal.val then
    if tableSame : left.table.ordinal.val = right.table.ordinal.val then
      some ⟨tableTarget_eq_of_val_eq boxSame tableSame⟩
    else none
  else none

/-- Constructive equality for attributes in one exact schema. -/
def attributeEq? {catalog : SchemaUniverse} {owner : TableTarget catalog}
    {schema : TableSchema catalog owner} (left right : AttributeId schema) :
    Option (PLift (left = right)) :=
  if same : left.ordinal.val = right.ordinal.val then
    some ⟨by
      cases left with
      | mk leftOrdinal =>
          cases right with
          | mk rightOrdinal =>
              have : leftOrdinal = rightOrdinal := Fin.ext same
              cases this
              rfl⟩
  else none

/-- Constructive equality of two result sorts known to arise in the same row
scope. -/
def sameOriginSort? {Γ : TermContext}
    {scope : RowScope Γ.model Γ.current Γ.inputs}
    {left right : ScalarSort Γ.model.catalog}
    (leftOrigin : SortOrigin Γ scope left) (rightOrigin : SortOrigin Γ scope right) :
    Option (PLift (left = right)) := by
  cases leftOrigin with
  | real => cases rightOrigin <;> first | exact some ⟨rfl⟩ | exact none
  | int => cases rightOrigin <;> first | exact some ⟨rfl⟩ | exact none
  | bool => cases rightOrigin <;> first | exact some ⟨rfl⟩ | exact none
  | ref leftTarget =>
      cases rightOrigin with
      | ref rightTarget =>
          match tableTargetEq? leftTarget rightTarget with
          | some ⟨same⟩ => exact some ⟨congrArg ScalarSort.ref same⟩
          | none => exact none
      | _ => exact none
  | enum leftAttr leftEnum leftShape =>
      cases rightOrigin with
      | enum rightAttr rightEnum rightShape =>
          match attributeEq? leftAttr rightAttr with
          | none => exact none
          | some ⟨attrEq⟩ =>
              cases attrEq
              have enumEq : leftEnum = rightEnum := by
                rw [leftShape] at rightShape
                exact AttrShape.enum.inj rightShape
              cases enumEq
              exact some ⟨by rfl⟩
      | _ => exact none

/-- Numeric synthesis package used by canonical promotion. -/
inductive CheckedNumeric (Γ : TermContext)
    (scope : RowScope Γ.model Γ.current Γ.inputs) where
  | real (expr : Expr Γ.model Γ.current Γ.inputs scope .real)
  | int (expr : Expr Γ.model Γ.current Γ.inputs scope .int)

structure CheckedEnumExpr (Γ : TermContext)
    (scope : RowScope Γ.model Γ.current Γ.inputs) where
  attr : AttributeId scope.schema
  enumSchema : EnumSchema
  shapeEq : (scope.schema.attr attr).shape = .enum enumSchema
  expr : Expr Γ.model Γ.current Γ.inputs scope
    (.enum scope.schema attr enumSchema shapeEq)

structure CheckedRefExpr (Γ : TermContext)
    (scope : RowScope Γ.model Γ.current Γ.inputs) where
  target : TableTarget Γ.model.catalog
  expr : Expr Γ.model Γ.current Γ.inputs scope (.ref target)

private def enumOfResult {Γ : TermContext}
    {scope : RowScope Γ.model Γ.current Γ.inputs}
    (result : CheckedExprResult Γ scope) : Option (CheckedEnumExpr Γ scope) := by
  cases result with
  | mk sort expr origin =>
      cases origin with
      | enum attr enumSchema shapeEq => exact some ⟨attr, enumSchema, shapeEq, expr⟩
      | _ => exact none

private def refOfResult {Γ : TermContext}
    {scope : RowScope Γ.model Γ.current Γ.inputs}
    (result : CheckedExprResult Γ scope) : Option (CheckedRefExpr Γ scope) := by
  cases result with
  | mk sort expr origin =>
      cases origin with
      | ref target => exact some ⟨target, expr⟩
      | _ => exact none

/-! ## Aggregate containment at the raw V1 input-row boundary -/

mutual
  def rawExprContainsAggregate : IR.Expr → Bool
    | .real _ | .int _ | .bool _ | .enum _ | .param _ | .selfAttr _ | .enumIs _ _ => false
    | .add lhs rhs | .sub lhs rhs | .mul lhs rhs | .div lhs rhs
    | .eq lhs rhs | .ne lhs rhs | .lt lhs rhs | .le lhs rhs
    | .gt lhs rhs | .ge lhs rhs | .and lhs rhs | .or lhs rhs =>
        rawExprContainsAggregate lhs || rawExprContainsAggregate rhs
    | .not value => rawExprContainsAggregate value
    | .input _ _ | .agg _ _ _ _ _ => true

  def rawAggOpContainsAggregate : IR.AggOp → Bool
    | .count => false
    | .sum value => rawExprContainsAggregate value

  def rawAggregateContainsAggregate : IR.Aggregate → Bool
    | .mk op filter => rawAggOpContainsAggregate op || filter.any rawExprContainsAggregate
end

/-! ## Independent syntax-directed judgments

These relations inspect raw and checked constructors together.  They contain no
checker result and no generic "erases to" constructor.  Numeric promotion is
available only in the mixed-operation constructors below, and a bare enum has
only the expected-checking constructor.
-/
mutual
  inductive ExprSynthesizes (Γ : TermContext) :
      (scope : RowScope Γ.model Γ.current Γ.inputs) → IR.Expr → (sort : ScalarSort Γ.model.catalog) →
        Expr Γ.model Γ.current Γ.inputs scope sort → Prop where
    | real {scope : RowScope Γ.model Γ.current Γ.inputs} (value : ScientificLiteral) :
        ExprSynthesizes Γ scope (.real value.source) .real (.real value)
    | int {scope : RowScope Γ.model Γ.current Γ.inputs} (value : Int) :
        ExprSynthesizes Γ scope (.int value) .int (.int value)
    | bool {scope : RowScope Γ.model Γ.current Γ.inputs} (value : Bool) :
        ExprSynthesizes Γ scope (.bool value) .bool (.bool value)
    | param {scope : RowScope Γ.model Γ.current Γ.inputs} (id : ParameterId Γ.model.params) :
        ExprSynthesizes Γ scope (.param (Γ.model.params.name id))
          ((Γ.model.params.get id).sort.scalarSort Γ.model.catalog) (.param id)
    | selfAttr {scope : RowScope Γ.model Γ.current Γ.inputs} (attr : AttributeId scope.schema) :
        ExprSynthesizes Γ scope (.selfAttr (scope.schema.attributeName attr))
          (scope.schema.attributeSort attr) (.selfAttr attr)
    | addInt {scope : RowScope Γ.model Γ.current Γ.inputs} {leftRaw rightRaw} {left right : Expr Γ.model Γ.current Γ.inputs scope .int}
        (hl : ExprSynthesizes Γ scope leftRaw .int left)
        (hr : ExprSynthesizes Γ scope rightRaw .int right) :
        ExprSynthesizes Γ scope (.add leftRaw rightRaw) .int (.add .int left right)
    | addReal {scope : RowScope Γ.model Γ.current Γ.inputs} {leftRaw rightRaw} {left right : Expr Γ.model Γ.current Γ.inputs scope .real}
        (hl : ExprSynthesizes Γ scope leftRaw .real left)
        (hr : ExprSynthesizes Γ scope rightRaw .real right) :
        ExprSynthesizes Γ scope (.add leftRaw rightRaw) .real (.add .real left right)
    | addIntReal {scope : RowScope Γ.model Γ.current Γ.inputs} {leftRaw rightRaw} {left : Expr Γ.model Γ.current Γ.inputs scope .int}
        {right : Expr Γ.model Γ.current Γ.inputs scope .real}
        (hl : ExprSynthesizes Γ scope leftRaw .int left)
        (hr : ExprSynthesizes Γ scope rightRaw .real right) :
        ExprSynthesizes Γ scope (.add leftRaw rightRaw) .real
          (.add .real (.intToReal left) right)
    | addRealInt {scope : RowScope Γ.model Γ.current Γ.inputs} {leftRaw rightRaw} {left : Expr Γ.model Γ.current Γ.inputs scope .real}
        {right : Expr Γ.model Γ.current Γ.inputs scope .int}
        (hl : ExprSynthesizes Γ scope leftRaw .real left)
        (hr : ExprSynthesizes Γ scope rightRaw .int right) :
        ExprSynthesizes Γ scope (.add leftRaw rightRaw) .real
          (.add .real left (.intToReal right))
    | subInt {scope : RowScope Γ.model Γ.current Γ.inputs} {leftRaw rightRaw} {left right : Expr Γ.model Γ.current Γ.inputs scope .int}
        (hl : ExprSynthesizes Γ scope leftRaw .int left)
        (hr : ExprSynthesizes Γ scope rightRaw .int right) :
        ExprSynthesizes Γ scope (.sub leftRaw rightRaw) .int (.sub .int left right)
    | subReal {scope : RowScope Γ.model Γ.current Γ.inputs} {leftRaw rightRaw} {left right : Expr Γ.model Γ.current Γ.inputs scope .real}
        (hl : ExprSynthesizes Γ scope leftRaw .real left)
        (hr : ExprSynthesizes Γ scope rightRaw .real right) :
        ExprSynthesizes Γ scope (.sub leftRaw rightRaw) .real (.sub .real left right)
    | subIntReal {scope : RowScope Γ.model Γ.current Γ.inputs} {leftRaw rightRaw} {left : Expr Γ.model Γ.current Γ.inputs scope .int}
        {right : Expr Γ.model Γ.current Γ.inputs scope .real}
        (hl : ExprSynthesizes Γ scope leftRaw .int left)
        (hr : ExprSynthesizes Γ scope rightRaw .real right) :
        ExprSynthesizes Γ scope (.sub leftRaw rightRaw) .real
          (.sub .real (.intToReal left) right)
    | subRealInt {scope : RowScope Γ.model Γ.current Γ.inputs} {leftRaw rightRaw} {left : Expr Γ.model Γ.current Γ.inputs scope .real}
        {right : Expr Γ.model Γ.current Γ.inputs scope .int}
        (hl : ExprSynthesizes Γ scope leftRaw .real left)
        (hr : ExprSynthesizes Γ scope rightRaw .int right) :
        ExprSynthesizes Γ scope (.sub leftRaw rightRaw) .real
          (.sub .real left (.intToReal right))
    | mulInt {scope : RowScope Γ.model Γ.current Γ.inputs} {leftRaw rightRaw} {left right : Expr Γ.model Γ.current Γ.inputs scope .int}
        (hl : ExprSynthesizes Γ scope leftRaw .int left)
        (hr : ExprSynthesizes Γ scope rightRaw .int right) :
        ExprSynthesizes Γ scope (.mul leftRaw rightRaw) .int (.mul .int left right)
    | mulReal {scope : RowScope Γ.model Γ.current Γ.inputs} {leftRaw rightRaw} {left right : Expr Γ.model Γ.current Γ.inputs scope .real}
        (hl : ExprSynthesizes Γ scope leftRaw .real left)
        (hr : ExprSynthesizes Γ scope rightRaw .real right) :
        ExprSynthesizes Γ scope (.mul leftRaw rightRaw) .real (.mul .real left right)
    | mulIntReal {scope : RowScope Γ.model Γ.current Γ.inputs} {leftRaw rightRaw} {left : Expr Γ.model Γ.current Γ.inputs scope .int}
        {right : Expr Γ.model Γ.current Γ.inputs scope .real}
        (hl : ExprSynthesizes Γ scope leftRaw .int left)
        (hr : ExprSynthesizes Γ scope rightRaw .real right) :
        ExprSynthesizes Γ scope (.mul leftRaw rightRaw) .real
          (.mul .real (.intToReal left) right)
    | mulRealInt {scope : RowScope Γ.model Γ.current Γ.inputs} {leftRaw rightRaw} {left : Expr Γ.model Γ.current Γ.inputs scope .real}
        {right : Expr Γ.model Γ.current Γ.inputs scope .int}
        (hl : ExprSynthesizes Γ scope leftRaw .real left)
        (hr : ExprSynthesizes Γ scope rightRaw .int right) :
        ExprSynthesizes Γ scope (.mul leftRaw rightRaw) .real
          (.mul .real left (.intToReal right))
    | divReal {scope : RowScope Γ.model Γ.current Γ.inputs} {leftRaw rightRaw} {left right : Expr Γ.model Γ.current Γ.inputs scope .real}
        (hl : ExprSynthesizes Γ scope leftRaw .real left)
        (hr : ExprSynthesizes Γ scope rightRaw .real right) :
        ExprSynthesizes Γ scope (.div leftRaw rightRaw) .real (.div left right)
    | divIntReal {scope : RowScope Γ.model Γ.current Γ.inputs} {leftRaw rightRaw} {left : Expr Γ.model Γ.current Γ.inputs scope .int}
        {right : Expr Γ.model Γ.current Γ.inputs scope .real}
        (hl : ExprSynthesizes Γ scope leftRaw .int left)
        (hr : ExprSynthesizes Γ scope rightRaw .real right) :
        ExprSynthesizes Γ scope (.div leftRaw rightRaw) .real (.div (.intToReal left) right)
    | divRealInt {scope : RowScope Γ.model Γ.current Γ.inputs} {leftRaw rightRaw} {left : Expr Γ.model Γ.current Γ.inputs scope .real}
        {right : Expr Γ.model Γ.current Γ.inputs scope .int}
        (hl : ExprSynthesizes Γ scope leftRaw .real left)
        (hr : ExprSynthesizes Γ scope rightRaw .int right) :
        ExprSynthesizes Γ scope (.div leftRaw rightRaw) .real (.div left (.intToReal right))
    | divInt {scope : RowScope Γ.model Γ.current Γ.inputs} {leftRaw rightRaw} {left right : Expr Γ.model Γ.current Γ.inputs scope .int}
        (hl : ExprSynthesizes Γ scope leftRaw .int left)
        (hr : ExprSynthesizes Γ scope rightRaw .int right) :
        ExprSynthesizes Γ scope (.div leftRaw rightRaw) .real
          (.div (.intToReal left) (.intToReal right))
    | eqSame {scope : RowScope Γ.model Γ.current Γ.inputs} {sort leftRaw rightRaw}
        {left right : Expr Γ.model Γ.current Γ.inputs scope sort}
        (hl : ExprSynthesizes Γ scope leftRaw sort left)
        (hr : ExprSynthesizes Γ scope rightRaw sort right) :
        ExprSynthesizes Γ scope (.eq leftRaw rightRaw) .bool (.eq left right)
    | eqIntReal {scope : RowScope Γ.model Γ.current Γ.inputs} {leftRaw rightRaw} {left : Expr Γ.model Γ.current Γ.inputs scope .int}
        {right : Expr Γ.model Γ.current Γ.inputs scope .real}
        (hl : ExprSynthesizes Γ scope leftRaw .int left)
        (hr : ExprSynthesizes Γ scope rightRaw .real right) :
        ExprSynthesizes Γ scope (.eq leftRaw rightRaw) .bool (.eq (.intToReal left) right)
    | eqRealInt {scope : RowScope Γ.model Γ.current Γ.inputs} {leftRaw rightRaw} {left : Expr Γ.model Γ.current Γ.inputs scope .real}
        {right : Expr Γ.model Γ.current Γ.inputs scope .int}
        (hl : ExprSynthesizes Γ scope leftRaw .real left)
        (hr : ExprSynthesizes Γ scope rightRaw .int right) :
        ExprSynthesizes Γ scope (.eq leftRaw rightRaw) .bool (.eq left (.intToReal right))
    | eqEnumLeft {scope : RowScope Γ.model Γ.current Γ.inputs} {rightRaw} (attr : AttributeId scope.schema) (enumSchema : EnumSchema)
        (shapeEq : (scope.schema.attr attr).shape = .enum enumSchema)
        (leftVariant : VariantId scope.schema attr enumSchema)
        {right : Expr Γ.model Γ.current Γ.inputs scope
          (.enum scope.schema attr enumSchema shapeEq)}
        (hr : ExprSynthesizes Γ scope rightRaw
          (.enum scope.schema attr enumSchema shapeEq) right) :
        ExprSynthesizes Γ scope
          (.eq (.enum (enumSchema.variantName scope.schema attr leftVariant)) rightRaw)
          .bool (.eq (.enum attr enumSchema leftVariant) right)
    | eqEnumRight {scope : RowScope Γ.model Γ.current Γ.inputs} {leftRaw} (attr : AttributeId scope.schema) (enumSchema : EnumSchema)
        (shapeEq : (scope.schema.attr attr).shape = .enum enumSchema)
        {left : Expr Γ.model Γ.current Γ.inputs scope
          (.enum scope.schema attr enumSchema shapeEq)}
        (hl : ExprSynthesizes Γ scope leftRaw
          (.enum scope.schema attr enumSchema shapeEq) left)
        (rightVariant : VariantId scope.schema attr enumSchema) :
        ExprSynthesizes Γ scope
          (.eq leftRaw (.enum (enumSchema.variantName scope.schema attr rightVariant)))
          .bool (.eq left (.enum attr enumSchema rightVariant))
    | neSame {scope : RowScope Γ.model Γ.current Γ.inputs} {sort leftRaw rightRaw}
        {left right : Expr Γ.model Γ.current Γ.inputs scope sort}
        (hl : ExprSynthesizes Γ scope leftRaw sort left)
        (hr : ExprSynthesizes Γ scope rightRaw sort right) :
        ExprSynthesizes Γ scope (.ne leftRaw rightRaw) .bool (.ne left right)
    | neIntReal {scope : RowScope Γ.model Γ.current Γ.inputs} {leftRaw rightRaw} {left : Expr Γ.model Γ.current Γ.inputs scope .int}
        {right : Expr Γ.model Γ.current Γ.inputs scope .real}
        (hl : ExprSynthesizes Γ scope leftRaw .int left)
        (hr : ExprSynthesizes Γ scope rightRaw .real right) :
        ExprSynthesizes Γ scope (.ne leftRaw rightRaw) .bool (.ne (.intToReal left) right)
    | neRealInt {scope : RowScope Γ.model Γ.current Γ.inputs} {leftRaw rightRaw} {left : Expr Γ.model Γ.current Γ.inputs scope .real}
        {right : Expr Γ.model Γ.current Γ.inputs scope .int}
        (hl : ExprSynthesizes Γ scope leftRaw .real left)
        (hr : ExprSynthesizes Γ scope rightRaw .int right) :
        ExprSynthesizes Γ scope (.ne leftRaw rightRaw) .bool (.ne left (.intToReal right))
    | neEnumLeft {scope : RowScope Γ.model Γ.current Γ.inputs} {rightRaw} (attr : AttributeId scope.schema) (enumSchema : EnumSchema)
        (shapeEq : (scope.schema.attr attr).shape = .enum enumSchema)
        (leftVariant : VariantId scope.schema attr enumSchema)
        {right : Expr Γ.model Γ.current Γ.inputs scope
          (.enum scope.schema attr enumSchema shapeEq)}
        (hr : ExprSynthesizes Γ scope rightRaw
          (.enum scope.schema attr enumSchema shapeEq) right) :
        ExprSynthesizes Γ scope
          (.ne (.enum (enumSchema.variantName scope.schema attr leftVariant)) rightRaw)
          .bool (.ne (.enum attr enumSchema leftVariant) right)
    | neEnumRight {scope : RowScope Γ.model Γ.current Γ.inputs} {leftRaw} (attr : AttributeId scope.schema) (enumSchema : EnumSchema)
        (shapeEq : (scope.schema.attr attr).shape = .enum enumSchema)
        {left : Expr Γ.model Γ.current Γ.inputs scope
          (.enum scope.schema attr enumSchema shapeEq)}
        (hl : ExprSynthesizes Γ scope leftRaw
          (.enum scope.schema attr enumSchema shapeEq) left)
        (rightVariant : VariantId scope.schema attr enumSchema) :
        ExprSynthesizes Γ scope
          (.ne leftRaw (.enum (enumSchema.variantName scope.schema attr rightVariant)))
          .bool (.ne left (.enum attr enumSchema rightVariant))
    | ltInt {scope : RowScope Γ.model Γ.current Γ.inputs} {leftRaw rightRaw} {left right : Expr Γ.model Γ.current Γ.inputs scope .int}
        (hl : ExprSynthesizes Γ scope leftRaw .int left)
        (hr : ExprSynthesizes Γ scope rightRaw .int right) :
        ExprSynthesizes Γ scope (.lt leftRaw rightRaw) .bool (.lt .int left right)
    | ltReal {scope : RowScope Γ.model Γ.current Γ.inputs} {leftRaw rightRaw} {left right : Expr Γ.model Γ.current Γ.inputs scope .real}
        (hl : ExprSynthesizes Γ scope leftRaw .real left)
        (hr : ExprSynthesizes Γ scope rightRaw .real right) :
        ExprSynthesizes Γ scope (.lt leftRaw rightRaw) .bool (.lt .real left right)
    | ltIntReal {scope : RowScope Γ.model Γ.current Γ.inputs} {leftRaw rightRaw} {left : Expr Γ.model Γ.current Γ.inputs scope .int}
        {right : Expr Γ.model Γ.current Γ.inputs scope .real}
        (hl : ExprSynthesizes Γ scope leftRaw .int left)
        (hr : ExprSynthesizes Γ scope rightRaw .real right) :
        ExprSynthesizes Γ scope (.lt leftRaw rightRaw) .bool
          (.lt .real (.intToReal left) right)
    | ltRealInt {scope : RowScope Γ.model Γ.current Γ.inputs} {leftRaw rightRaw} {left : Expr Γ.model Γ.current Γ.inputs scope .real}
        {right : Expr Γ.model Γ.current Γ.inputs scope .int}
        (hl : ExprSynthesizes Γ scope leftRaw .real left)
        (hr : ExprSynthesizes Γ scope rightRaw .int right) :
        ExprSynthesizes Γ scope (.lt leftRaw rightRaw) .bool
          (.lt .real left (.intToReal right))
    | leInt {scope : RowScope Γ.model Γ.current Γ.inputs} {leftRaw rightRaw} {left right : Expr Γ.model Γ.current Γ.inputs scope .int}
        (hl : ExprSynthesizes Γ scope leftRaw .int left)
        (hr : ExprSynthesizes Γ scope rightRaw .int right) :
        ExprSynthesizes Γ scope (.le leftRaw rightRaw) .bool (.le .int left right)
    | leReal {scope : RowScope Γ.model Γ.current Γ.inputs} {leftRaw rightRaw} {left right : Expr Γ.model Γ.current Γ.inputs scope .real}
        (hl : ExprSynthesizes Γ scope leftRaw .real left)
        (hr : ExprSynthesizes Γ scope rightRaw .real right) :
        ExprSynthesizes Γ scope (.le leftRaw rightRaw) .bool (.le .real left right)
    | leIntReal {scope : RowScope Γ.model Γ.current Γ.inputs} {leftRaw rightRaw} {left : Expr Γ.model Γ.current Γ.inputs scope .int}
        {right : Expr Γ.model Γ.current Γ.inputs scope .real}
        (hl : ExprSynthesizes Γ scope leftRaw .int left)
        (hr : ExprSynthesizes Γ scope rightRaw .real right) :
        ExprSynthesizes Γ scope (.le leftRaw rightRaw) .bool
          (.le .real (.intToReal left) right)
    | leRealInt {scope : RowScope Γ.model Γ.current Γ.inputs} {leftRaw rightRaw} {left : Expr Γ.model Γ.current Γ.inputs scope .real}
        {right : Expr Γ.model Γ.current Γ.inputs scope .int}
        (hl : ExprSynthesizes Γ scope leftRaw .real left)
        (hr : ExprSynthesizes Γ scope rightRaw .int right) :
        ExprSynthesizes Γ scope (.le leftRaw rightRaw) .bool
          (.le .real left (.intToReal right))
    | gtInt {scope : RowScope Γ.model Γ.current Γ.inputs} {leftRaw rightRaw} {left right : Expr Γ.model Γ.current Γ.inputs scope .int}
        (hl : ExprSynthesizes Γ scope leftRaw .int left)
        (hr : ExprSynthesizes Γ scope rightRaw .int right) :
        ExprSynthesizes Γ scope (.gt leftRaw rightRaw) .bool (.gt .int left right)
    | gtReal {scope : RowScope Γ.model Γ.current Γ.inputs} {leftRaw rightRaw} {left right : Expr Γ.model Γ.current Γ.inputs scope .real}
        (hl : ExprSynthesizes Γ scope leftRaw .real left)
        (hr : ExprSynthesizes Γ scope rightRaw .real right) :
        ExprSynthesizes Γ scope (.gt leftRaw rightRaw) .bool (.gt .real left right)
    | gtIntReal {scope : RowScope Γ.model Γ.current Γ.inputs} {leftRaw rightRaw} {left : Expr Γ.model Γ.current Γ.inputs scope .int}
        {right : Expr Γ.model Γ.current Γ.inputs scope .real}
        (hl : ExprSynthesizes Γ scope leftRaw .int left)
        (hr : ExprSynthesizes Γ scope rightRaw .real right) :
        ExprSynthesizes Γ scope (.gt leftRaw rightRaw) .bool
          (.gt .real (.intToReal left) right)
    | gtRealInt {scope : RowScope Γ.model Γ.current Γ.inputs} {leftRaw rightRaw} {left : Expr Γ.model Γ.current Γ.inputs scope .real}
        {right : Expr Γ.model Γ.current Γ.inputs scope .int}
        (hl : ExprSynthesizes Γ scope leftRaw .real left)
        (hr : ExprSynthesizes Γ scope rightRaw .int right) :
        ExprSynthesizes Γ scope (.gt leftRaw rightRaw) .bool
          (.gt .real left (.intToReal right))
    | geInt {scope : RowScope Γ.model Γ.current Γ.inputs} {leftRaw rightRaw} {left right : Expr Γ.model Γ.current Γ.inputs scope .int}
        (hl : ExprSynthesizes Γ scope leftRaw .int left)
        (hr : ExprSynthesizes Γ scope rightRaw .int right) :
        ExprSynthesizes Γ scope (.ge leftRaw rightRaw) .bool (.ge .int left right)
    | geReal {scope : RowScope Γ.model Γ.current Γ.inputs} {leftRaw rightRaw} {left right : Expr Γ.model Γ.current Γ.inputs scope .real}
        (hl : ExprSynthesizes Γ scope leftRaw .real left)
        (hr : ExprSynthesizes Γ scope rightRaw .real right) :
        ExprSynthesizes Γ scope (.ge leftRaw rightRaw) .bool (.ge .real left right)
    | geIntReal {scope : RowScope Γ.model Γ.current Γ.inputs} {leftRaw rightRaw} {left : Expr Γ.model Γ.current Γ.inputs scope .int}
        {right : Expr Γ.model Γ.current Γ.inputs scope .real}
        (hl : ExprSynthesizes Γ scope leftRaw .int left)
        (hr : ExprSynthesizes Γ scope rightRaw .real right) :
        ExprSynthesizes Γ scope (.ge leftRaw rightRaw) .bool
          (.ge .real (.intToReal left) right)
    | geRealInt {scope : RowScope Γ.model Γ.current Γ.inputs} {leftRaw rightRaw} {left : Expr Γ.model Γ.current Γ.inputs scope .real}
        {right : Expr Γ.model Γ.current Γ.inputs scope .int}
        (hl : ExprSynthesizes Γ scope leftRaw .real left)
        (hr : ExprSynthesizes Γ scope rightRaw .int right) :
        ExprSynthesizes Γ scope (.ge leftRaw rightRaw) .bool
          (.ge .real left (.intToReal right))
    | and {scope : RowScope Γ.model Γ.current Γ.inputs} {leftRaw rightRaw} {left right : Expr Γ.model Γ.current Γ.inputs scope .bool}
        (hl : ExprChecks Γ scope .bool leftRaw left)
        (hr : ExprChecks Γ scope .bool rightRaw right) :
        ExprSynthesizes Γ scope (.and leftRaw rightRaw) .bool (.and left right)
    | or {scope : RowScope Γ.model Γ.current Γ.inputs} {leftRaw rightRaw} {left right : Expr Γ.model Γ.current Γ.inputs scope .bool}
        (hl : ExprChecks Γ scope .bool leftRaw left)
        (hr : ExprChecks Γ scope .bool rightRaw right) :
        ExprSynthesizes Γ scope (.or leftRaw rightRaw) .bool (.or left right)
    | not {scope : RowScope Γ.model Γ.current Γ.inputs} {raw} {value : Expr Γ.model Γ.current Γ.inputs scope .bool}
        (h : ExprChecks Γ scope .bool raw value) :
        ExprSynthesizes Γ scope (.not raw) .bool (.not value)
    | enumIs {scope : RowScope Γ.model Γ.current Γ.inputs} (attr : AttributeId scope.schema) (enumSchema : EnumSchema)
        (variant : VariantId scope.schema attr enumSchema) :
        ExprSynthesizes Γ scope
          (.enumIs (scope.schema.attributeName attr)
            (enumSchema.variantName scope.schema attr variant))
          .bool (.enumIs attr enumSchema variant)
    | input {scope : RowScope Γ.model Γ.current Γ.inputs} {raw sort} (port : InputId Γ.inputs)
        {aggregate : Aggregate Γ.model Γ.current Γ.inputs (.input port) sort}
        (h : AggregateSynthesizes Γ (.input port) raw sort aggregate) :
        ExprSynthesizes Γ scope (.input (Γ.inputs.name port) raw) sort
          (.input port aggregate)
    | agg {scope : RowScope Γ.model Γ.current Γ.inputs} {rawOp rawFilter sort}
        (table : TableId Γ.model.catalog scope.owner.box)
        (joinTarget : TableId Γ.model.catalog scope.owner.box)
        (fk : ReferenceAttributeId (Γ.model.schemaFor (scope.relatedTarget table)) joinTarget)
        (selfFk : ReferenceAttributeId scope.schema joinTarget)
        {op : AggOp Γ.model Γ.current Γ.inputs (.table (scope.relatedTarget table)) sort}
        {filter : Expr Γ.model Γ.current Γ.inputs (.table (scope.relatedTarget table)) .bool}
        (hop : AggOpSynthesizes Γ (.table (scope.relatedTarget table)) rawOp sort op)
        (hfilter : ExprChecks Γ (.table (scope.relatedTarget table)) .bool rawFilter filter) :
        ExprSynthesizes Γ scope
          (.agg rawOp (Γ.model.catalog.tableName (scope.relatedTarget table))
            ((Γ.model.schemaFor (scope.relatedTarget table)).attributeName fk.id)
            (scope.schema.attributeName selfFk.id) rawFilter)
          sort (.agg table joinTarget fk selfFk op filter)

  inductive ExprChecks (Γ : TermContext) :
      (scope : RowScope Γ.model Γ.current Γ.inputs) →
      (expected : ScalarSort Γ.model.catalog) →
      IR.Expr → Expr Γ.model Γ.current Γ.inputs scope expected → Prop where
    | synth {scope : RowScope Γ.model Γ.current Γ.inputs} {raw term} (h : ExprSynthesizes Γ scope raw expected term) :
        ExprChecks Γ scope expected raw term
    | enum {scope : RowScope Γ.model Γ.current Γ.inputs} (attr : AttributeId scope.schema) (enumSchema : EnumSchema)
        (shapeEq : (scope.schema.attr attr).shape = .enum enumSchema)
        (variant : VariantId scope.schema attr enumSchema) :
        ExprChecks Γ scope (.enum scope.schema attr enumSchema shapeEq)
          (.enum (enumSchema.variantName scope.schema attr variant))
          (.enum attr enumSchema variant)

  inductive AggOpSynthesizes (Γ : TermContext) :
      (scope : RowScope Γ.model Γ.current Γ.inputs) → IR.AggOp → (sort : ScalarSort Γ.model.catalog) →
        AggOp Γ.model Γ.current Γ.inputs scope sort → Prop where
    | count {scope : RowScope Γ.model Γ.current Γ.inputs} : AggOpSynthesizes Γ scope .count .int .count
    | sumInt {scope : RowScope Γ.model Γ.current Γ.inputs} {raw} {value : Expr Γ.model Γ.current Γ.inputs scope .int}
        (h : ExprSynthesizes Γ scope raw .int value) :
        AggOpSynthesizes Γ scope (.sum raw) .int (.sum .int value)
    | sumReal {scope : RowScope Γ.model Γ.current Γ.inputs} {raw} {value : Expr Γ.model Γ.current Γ.inputs scope .real}
        (h : ExprSynthesizes Γ scope raw .real value) :
        AggOpSynthesizes Γ scope (.sum raw) .real (.sum .real value)

  inductive AggregateSynthesizes (Γ : TermContext) :
      (scope : RowScope Γ.model Γ.current Γ.inputs) → IR.Aggregate → (sort : ScalarSort Γ.model.catalog) →
        Aggregate Γ.model Γ.current Γ.inputs scope sort → Prop where
    | unfiltered {scope : RowScope Γ.model Γ.current Γ.inputs} {raw sort}
        {op : AggOp Γ.model Γ.current Γ.inputs scope sort}
        (aggregateFree : rawAggOpContainsAggregate raw = false)
        (h : AggOpSynthesizes Γ scope raw sort op) :
        AggregateSynthesizes Γ scope (.mk raw none) sort (.unfiltered op)
    | filtered {scope : RowScope Γ.model Γ.current Γ.inputs} {rawOp rawFilter sort}
        {op : AggOp Γ.model Γ.current Γ.inputs scope sort}
        {filter : Expr Γ.model Γ.current Γ.inputs scope .bool}
        (operationFree : rawAggOpContainsAggregate rawOp = false)
        (filterFree : rawExprContainsAggregate rawFilter = false)
        (hop : AggOpSynthesizes Γ scope rawOp sort op)
        (hfilter : ExprChecks Γ scope .bool rawFilter filter) :
        AggregateSynthesizes Γ scope (.mk rawOp (some rawFilter)) sort
          (.filtered op filter)
end

/-- Independent effect typing; the destination index fixes exact assignment
sort, so no top-level numeric coercion constructor exists. -/
inductive EffectWellTyped (Γ : TermContext) :
    IR.Effect → Effect Γ.model Γ.current Γ.inputs → Prop where
  | setAttr {raw} (destination : AttributeId (Γ.model.schemaFor Γ.current))
      {value : Term Γ.model Γ.current Γ.inputs
        ((Γ.model.schemaFor Γ.current).attributeSort destination)}
      (h : ExprChecks Γ (.table Γ.current)
        ((Γ.model.schemaFor Γ.current).attributeSort destination) raw value) :
      EffectWellTyped Γ
        (.setAttr ((Γ.model.schemaFor Γ.current).attributeName destination) raw)
        (.setAttr destination value)

/-- Independent claim typing with the exact retained ordering domain. -/
inductive ClaimWellTyped (Γ : TermContext) :
    IR.ResourceClaim → ResourceClaim Γ.model Γ.current Γ.inputs → Prop where
  | raceTime {resourceRaw target}
      {resource : Term Γ.model Γ.current Γ.inputs (.ref target)}
      (hresource : ExprSynthesizes Γ (.table Γ.current) resourceRaw (.ref target) resource) :
      ClaimWellTyped Γ { resource := resourceRaw, ordering := .raceTime }
        { resourceTarget := target, resource := resource
          orderingDomain := .real, orderingAvailability := .surfaceProduced
          ordering := .raceTime }
  | key {resourceRaw keyRaw target}
      {resource : Term Γ.model Γ.current Γ.inputs (.ref target)}
      (hresource : ExprSynthesizes Γ (.table Γ.current) resourceRaw (.ref target) resource)
      (domain : OrderingDomain Γ.model)
      {key : Term Γ.model Γ.current Γ.inputs domain.scalarSort}
      (hkey : ExprSynthesizes Γ (.table Γ.current) keyRaw domain.scalarSort key) :
      ClaimWellTyped Γ { resource := resourceRaw, ordering := .key keyRaw }
        { resourceTarget := target, resource := resource
          orderingDomain := domain, orderingAvailability := .rawCheckable
          ordering := .key domain key }

/-- Structural duplicate freedom, independent of executable diagnostics. -/
def ClaimsUnique (claims : List IR.ResourceClaim) : Prop :=
  (claims.map IR.ResourceClaim.resource).Nodup

/-- Syntactic Ref-write coverage, independent of claim evaluation. -/
def RefWritesCovered (Γ : TermContext) (effects : List IR.Effect)
    (claims : List IR.ResourceClaim) : Prop :=
  ∀ effect ∈ effects, match effect with
    | .setAttr destination rhs =>
        ∀ attr, (Γ.model.schemaFor Γ.current).lookupAttribute destination = some attr →
          match ((Γ.model.schemaFor Γ.current).attr attr).shape with
          | .ref _ => ∃ claim ∈ claims, claim.resource = rhs
          | _ => True

/-- Independent transition judgment. Header spelling remains raw while every
term payload is related pointwise in source order. -/
inductive TransitionWellTyped (Γ : TermContext) :
    IR.Transition → TransitionTerms Γ.model Γ.current Γ.inputs → Prop where
  | mk {raw checked}
      (guard : ExprChecks Γ (.table Γ.current) .bool raw.guard checked.guard)
      (hazard : ExprChecks Γ (.table Γ.current) .real raw.hazard checked.hazard)
      (effects : List.Forall₂ (EffectWellTyped Γ) raw.effects checked.effects)
      (claims : List.Forall₂ (ClaimWellTyped Γ) raw.contests checked.claims)
      (unique : ClaimsUnique raw.contests)
      (covered : RefWritesCovered Γ raw.effects raw.contests) :
      TransitionWellTyped Γ raw checked


/- A common structural measure for the mutually recursive raw syntax. -/
mutual
  def rawExprDepth : IR.Expr → Nat
    | .real _ | .int _ | .bool _ | .enum _ | .param _ | .selfAttr _ | .enumIs _ _ => 1
    | .add lhs rhs | .sub lhs rhs | .mul lhs rhs | .div lhs rhs
    | .eq lhs rhs | .ne lhs rhs | .lt lhs rhs | .le lhs rhs
    | .gt lhs rhs | .ge lhs rhs | .and lhs rhs | .or lhs rhs =>
        1 + rawExprDepth lhs + rawExprDepth rhs
    | .not value => 1 + rawExprDepth value
    | .input _ aggregate => 1 + rawAggregateDepth aggregate
    | .agg op _ _ _ filter => 1 + rawAggOpDepth op + rawExprDepth filter

  def rawAggOpDepth : IR.AggOp → Nat
    | .count => 1
    | .sum value => 1 + rawExprDepth value

  def rawAggregateDepth : IR.Aggregate → Nat
    | .mk op none => 1 + rawAggOpDepth op
    | .mk op (some filter) => 1 + rawAggOpDepth op + rawExprDepth filter
end

@[simp] theorem rawExprDepth_positive (raw : IR.Expr) : 0 < rawExprDepth raw := by
  cases raw <;> simp [rawExprDepth] <;> omega

@[simp] theorem rawAggOpDepth_positive (raw : IR.AggOp) : 0 < rawAggOpDepth raw := by
  cases raw <;> simp [rawAggOpDepth] <;> omega

@[simp] theorem rawAggregateDepth_positive (raw : IR.Aggregate) :
    0 < rawAggregateDepth raw := by
  cases raw with
  | mk op filter => cases filter <;> simp [rawAggregateDepth] <;> omega

/-! ## Canonical executable elaboration -/

private def pathError (category : TermCheckErrorCategory)
    (path : List ModelCheckPathSegment) : Except TermCheckError α :=
  .error ⟨category, path⟩

private def numericOfResult {Γ : TermContext}
    {scope : RowScope Γ.model Γ.current Γ.inputs}
    (result : CheckedExprResult Γ scope) : Option (CheckedNumeric Γ scope) :=
  match result with
  | ⟨.real, expr, .real⟩ => some (.real expr)
  | ⟨.int, expr, .int⟩ => some (.int expr)
  | _ => none

private def promotePair {Γ : TermContext}
    {scope : RowScope Γ.model Γ.current Γ.inputs}
    (left right : CheckedNumeric Γ scope) :
    (Σ numeric : NumericSort,
      Expr Γ.model Γ.current Γ.inputs scope (numeric.scalarSort Γ.model.catalog) ×
      Expr Γ.model Γ.current Γ.inputs scope (numeric.scalarSort Γ.model.catalog)) :=
  match left, right with
  | .real lhs, .real rhs => ⟨.real, lhs, rhs⟩
  | .real lhs, .int rhs => ⟨.real, lhs, .intToReal rhs⟩
  | .int lhs, .real rhs => ⟨.real, .intToReal lhs, rhs⟩
  | .int lhs, .int rhs => ⟨.int, lhs, rhs⟩

private def toReal {Γ : TermContext}
    {scope : RowScope Γ.model Γ.current Γ.inputs} :
    CheckedNumeric Γ scope → Expr Γ.model Γ.current Γ.inputs scope .real
  | .real expr => expr
  | .int expr => .intToReal expr

/-- Ordering operators share one elaboration path. -/
inductive OrderingKind where | lt | le | gt | ge

mutual
  def synthExprFuel (fuel : Nat) (Γ : TermContext)
      (scope : RowScope Γ.model Γ.current Γ.inputs) (raw : IR.Expr)
      (path : List ModelCheckPathSegment := []) :
      Except TermCheckError (CheckedExprResult Γ scope) :=
    if positive : 0 < fuel then do
    match raw with
    | .real value =>
        pure ⟨.real, .real ⟨value⟩, .real⟩
    | .int value =>
        pure ⟨.int, .int value, .int⟩
    | .bool value =>
        pure ⟨.bool, .bool value, .bool⟩
    | .enum _ => pathError .cannotInferEnumOwner path
    | .param name =>
        match Γ.model.params.lookup name with
        | none => pathError .unknownParameter path
        | some id =>
            let term : Expr Γ.model Γ.current Γ.inputs scope
                ((Γ.model.params.get id).sort.scalarSort Γ.model.catalog) := .param id
            match sortEq : (Γ.model.params.get id).sort with
            | .real =>
                have sortProof :
                    (Γ.model.params.get id).sort.scalarSort Γ.model.catalog = .real := by
                  simp [sortEq, ParamSort.scalarSort]
                pure ⟨.real, cast (by rw [sortProof]) term, .real⟩
            | .int =>
                have sortProof :
                    (Γ.model.params.get id).sort.scalarSort Γ.model.catalog = .int := by
                  simp [sortEq, ParamSort.scalarSort]
                pure ⟨.int, cast (by rw [sortProof]) term, .int⟩
    | .selfAttr name =>
        match scope.schema.lookupAttribute name with
        | none => pathError .unknownAttribute path
        | some attr =>
            pure ⟨scope.schema.attributeSort attr, .selfAttr attr,
              SortOrigin.ofAttribute Γ attr⟩
    | .add lhs rhs =>
        let left ← synthExprFuel (fuel - 1) Γ scope lhs (path ++ [.lhs])
        let right ← synthExprFuel (fuel - 1) Γ scope rhs (path ++ [.rhs])
        match numericOfResult left, numericOfResult right with
        | some leftNumeric, some rightNumeric =>
            match promotePair leftNumeric rightNumeric with
            | ⟨numeric, leftExpr, rightExpr⟩ =>
                match numeric with
                | .real => pure ⟨.real, .add .real leftExpr rightExpr, .real⟩
                | .int => pure ⟨.int, .add .int leftExpr rightExpr, .int⟩
        | _, _ => pathError .expectedNumeric path
    | .sub lhs rhs =>
        let left ← synthExprFuel (fuel - 1) Γ scope lhs (path ++ [.lhs])
        let right ← synthExprFuel (fuel - 1) Γ scope rhs (path ++ [.rhs])
        match numericOfResult left, numericOfResult right with
        | some leftNumeric, some rightNumeric =>
            match promotePair leftNumeric rightNumeric with
            | ⟨numeric, leftExpr, rightExpr⟩ =>
                match numeric with
                | .real => pure ⟨.real, .sub .real leftExpr rightExpr, .real⟩
                | .int => pure ⟨.int, .sub .int leftExpr rightExpr, .int⟩
        | _, _ => pathError .expectedNumeric path
    | .mul lhs rhs =>
        let left ← synthExprFuel (fuel - 1) Γ scope lhs (path ++ [.lhs])
        let right ← synthExprFuel (fuel - 1) Γ scope rhs (path ++ [.rhs])
        match numericOfResult left, numericOfResult right with
        | some leftNumeric, some rightNumeric =>
            match promotePair leftNumeric rightNumeric with
            | ⟨numeric, leftExpr, rightExpr⟩ =>
                match numeric with
                | .real => pure ⟨.real, .mul .real leftExpr rightExpr, .real⟩
                | .int => pure ⟨.int, .mul .int leftExpr rightExpr, .int⟩
        | _, _ => pathError .expectedNumeric path
    | .div lhs rhs =>
        let left ← synthExprFuel (fuel - 1) Γ scope lhs (path ++ [.lhs])
        let right ← synthExprFuel (fuel - 1) Γ scope rhs (path ++ [.rhs])
        match numericOfResult left, numericOfResult right with
        | some leftNumeric, some rightNumeric =>
            pure ⟨.real, .div (toReal leftNumeric) (toReal rightNumeric), .real⟩
        | _, _ => pathError .expectedNumeric path
    | .eq lhs rhs => checkEqualityFuel (fuel - 1) Γ scope true lhs rhs path
    | .ne lhs rhs => checkEqualityFuel (fuel - 1) Γ scope false lhs rhs path
    | .lt lhs rhs => checkOrderingFuel (fuel - 1) Γ scope .lt lhs rhs path
    | .le lhs rhs => checkOrderingFuel (fuel - 1) Γ scope .le lhs rhs path
    | .gt lhs rhs => checkOrderingFuel (fuel - 1) Γ scope .gt lhs rhs path
    | .ge lhs rhs => checkOrderingFuel (fuel - 1) Γ scope .ge lhs rhs path
    | .and lhs rhs =>
        let left ← checkExprFuel (fuel - 1) Γ scope lhs .bool .bool (path ++ [.lhs])
        let right ← checkExprFuel (fuel - 1) Γ scope rhs .bool .bool (path ++ [.rhs])
        pure ⟨.bool, .and left right, .bool⟩
    | .or lhs rhs =>
        let left ← checkExprFuel (fuel - 1) Γ scope lhs .bool .bool (path ++ [.lhs])
        let right ← checkExprFuel (fuel - 1) Γ scope rhs .bool .bool (path ++ [.rhs])
        pure ⟨.bool, .or left right, .bool⟩
    | .not value =>
        let checked ← checkExprFuel (fuel - 1) Γ scope value .bool .bool (path ++ [.operand])
        pure ⟨.bool, .not checked, .bool⟩
    | .enumIs attrName variantName =>
        match scope.schema.lookupAttribute attrName with
        | none => pathError .unknownAttribute path
        | some attr =>
            match shapeEq : (scope.schema.attr attr).shape with
            | .enum enumSchema =>
                match enumSchema.lookup scope.schema attr shapeEq variantName with
                | none => pathError .unknownEnumVariant path
                | some variant => pure ⟨.bool, .enumIs attr enumSchema variant, .bool⟩
            | _ => pathError .sortMismatch path
    | .input portName aggregate =>
        match Γ.inputs.lookup portName with
        | none => pathError .unknownInput (path ++ [.inputPort])
        | some port =>
            let checked ← synthAggregateFuel (fuel - 1) Γ port aggregate (path ++ [.aggregate])
            match checked with
            | ⟨.real, aggregate, .real⟩ =>
                pure ⟨.real, .input port aggregate, .real⟩
            | ⟨.int, aggregate, .int⟩ =>
                pure ⟨.int, .input port aggregate, .int⟩
            | _ => pathError .expectedNumeric path
    | .agg op tableName fkName selfFkName filter =>
        checkRelationalAggregateFuel (fuel - 1) Γ scope op tableName fkName selfFkName filter path
    else pathError .sortMismatch path
  termination_by fuel
  decreasing_by all_goals omega

  def checkExprFuel (fuel : Nat) (Γ : TermContext)
      (scope : RowScope Γ.model Γ.current Γ.inputs) (raw : IR.Expr)
      (expected : ScalarSort Γ.model.catalog) (expectedOrigin : SortOrigin Γ scope expected)
      (path : List ModelCheckPathSegment := []) :
      Except TermCheckError (Expr Γ.model Γ.current Γ.inputs scope expected) :=
    if positive : 0 < fuel then do
    match raw with
    | .enum variantName =>
        match expectedOrigin with
        | .enum attr enumSchema shapeEq =>
            match enumSchema.lookup scope.schema attr shapeEq variantName with
            | none => pathError .unknownEnumVariant path
            | some variant => pure (.enum attr enumSchema variant)
        | _ => pathError .sortMismatch path
    | _ =>
      let actual ← synthExprFuel (fuel - 1) Γ scope raw path
      match actual with
      | ⟨actualSort, actualExpr, actualOrigin⟩ =>
        match sameOriginSort? actualOrigin expectedOrigin with
        | some ⟨same⟩ =>
            pure (Eq.mp (congrArg
              (fun sort => Expr Γ.model Γ.current Γ.inputs scope sort) same) actualExpr)
        | none =>
            match expectedOrigin with
            | .bool => pathError .expectedBool path
            | .real => pathError .expectedReal path
            | _ => pathError .sortMismatch path
    else pathError .sortMismatch path
  termination_by fuel
  decreasing_by all_goals omega

  def synthAggOpFuel (fuel : Nat) (Γ : TermContext)
      (scope : RowScope Γ.model Γ.current Γ.inputs) (raw : IR.AggOp)
      (inputBoundary : Bool) (path : List ModelCheckPathSegment := []) :
      Except TermCheckError (Σ sort : ScalarSort Γ.model.catalog,
        AggOp Γ.model Γ.current Γ.inputs scope sort × SortOrigin Γ scope sort) :=
    if positive : 0 < fuel then do
    match raw with
    | .count => pure ⟨.int, .count, .int⟩
    | .sum value =>
        if inputBoundary && rawExprContainsAggregate value then
          pathError .nestedInputAggregate (path ++ [.aggregateValue])
        else
          let checked ← synthExprFuel (fuel - 1) Γ scope value (path ++ [.aggregateValue])
          match numericOfResult checked with
          | some (.real expr) => pure ⟨.real, .sum .real expr, .real⟩
          | some (.int expr) => pure ⟨.int, .sum .int expr, .int⟩
          | none => pathError .expectedNumeric (path ++ [.aggregateValue])
    else pathError .sortMismatch path
  termination_by fuel
  decreasing_by all_goals omega

  def synthAggregateFuel (fuel : Nat) (Γ : TermContext) (port : InputId Γ.inputs)
      (raw : IR.Aggregate) (path : List ModelCheckPathSegment := []) :
      Except TermCheckError (Σ sort : ScalarSort Γ.model.catalog,
        Aggregate Γ.model Γ.current Γ.inputs (.input port) sort ×
          SortOrigin Γ (.input port) sort) :=
    if positive : 0 < fuel then do
    match raw with
    | .mk op filter =>
        let ⟨sort, checkedOp, origin⟩ ←
          synthAggOpFuel (fuel - 1) Γ (.input port) op true path
        match filter with
        | none => pure ⟨sort, .unfiltered checkedOp, origin⟩
        | some rawFilter =>
            if rawExprContainsAggregate rawFilter then
              pathError .nestedInputAggregate (path ++ [.aggregateFilter])
            else
              let checkedFilter ← checkExprFuel (fuel - 1) Γ (.input port) rawFilter .bool .bool
                (path ++ [.aggregateFilter])
              pure ⟨sort, .filtered checkedOp checkedFilter, origin⟩
    else pathError .sortMismatch path
  termination_by fuel
  decreasing_by all_goals omega

  def checkEqualityFuel (fuel : Nat) (Γ : TermContext)
      (scope : RowScope Γ.model Γ.current Γ.inputs) (isEq : Bool)
      (lhs rhs : IR.Expr) (path : List ModelCheckPathSegment) :
      Except TermCheckError (CheckedExprResult Γ scope) :=
    if positive : 0 < fuel then do
    match lhs, rhs with
    | .enum _, .enum _ => pathError .cannotInferEnumOwner path
    | .enum variant, other =>
        let right ← synthExprFuel (fuel - 1) Γ scope other (path ++ [.rhs])
        match enumOfResult right with
        | none => pathError .incompatibleEquality path
        | some anchored =>
            let left ← checkExprFuel (fuel - 1) Γ scope (.enum variant)
              (.enum scope.schema anchored.attr anchored.enumSchema anchored.shapeEq)
              (.enum anchored.attr anchored.enumSchema anchored.shapeEq) (path ++ [.lhs])
            if isEq then pure ⟨.bool, .eq left anchored.expr, .bool⟩
            else pure ⟨.bool, .ne left anchored.expr, .bool⟩
    | other, .enum variant =>
        let left ← synthExprFuel (fuel - 1) Γ scope other (path ++ [.lhs])
        match enumOfResult left with
        | none => pathError .incompatibleEquality path
        | some anchored =>
            let right ← checkExprFuel (fuel - 1) Γ scope (.enum variant)
              (.enum scope.schema anchored.attr anchored.enumSchema anchored.shapeEq)
              (.enum anchored.attr anchored.enumSchema anchored.shapeEq) (path ++ [.rhs])
            if isEq then pure ⟨.bool, .eq anchored.expr right, .bool⟩
            else pure ⟨.bool, .ne anchored.expr right, .bool⟩
    | _, _ =>
        let left ← synthExprFuel (fuel - 1) Γ scope lhs (path ++ [.lhs])
        let right ← synthExprFuel (fuel - 1) Γ scope rhs (path ++ [.rhs])
        match numericOfResult left, numericOfResult right with
        | some leftNumeric, some rightNumeric =>
            match promotePair leftNumeric rightNumeric with
            | ⟨_, leftExpr, rightExpr⟩ =>
                if isEq then pure ⟨.bool, .eq leftExpr rightExpr, .bool⟩
                else pure ⟨.bool, .ne leftExpr rightExpr, .bool⟩
        | _, _ =>
            match left, right with
            | ⟨leftSort, leftExpr, leftOrigin⟩,
                ⟨rightSort, rightExpr, rightOrigin⟩ =>
              match sameOriginSort? leftOrigin rightOrigin with
              | some ⟨same⟩ =>
                  let castRight : Expr Γ.model Γ.current Γ.inputs scope leftSort :=
                    Eq.mpr (congrArg
                      (fun sort => Expr Γ.model Γ.current Γ.inputs scope sort) same)
                      rightExpr
                  if isEq then pure ⟨.bool, .eq leftExpr castRight, .bool⟩
                  else pure ⟨.bool, .ne leftExpr castRight, .bool⟩
              | none => pathError .incompatibleEquality path
    else pathError .sortMismatch path
  termination_by fuel
  decreasing_by all_goals omega

  def checkOrderingFuel (fuel : Nat) (Γ : TermContext)
      (scope : RowScope Γ.model Γ.current Γ.inputs)
      (kind : OrderingKind) (lhs rhs : IR.Expr)
      (path : List ModelCheckPathSegment) :
      Except TermCheckError (CheckedExprResult Γ scope) :=
    if positive : 0 < fuel then do
    let left ← synthExprFuel (fuel - 1) Γ scope lhs (path ++ [.lhs])
    let right ← synthExprFuel (fuel - 1) Γ scope rhs (path ++ [.rhs])
    match numericOfResult left, numericOfResult right with
    | some leftNumeric, some rightNumeric =>
        match promotePair leftNumeric rightNumeric with
        | ⟨numeric, leftExpr, rightExpr⟩ =>
            let expr : Expr Γ.model Γ.current Γ.inputs scope .bool :=
              match kind with
              | .lt => .lt numeric leftExpr rightExpr
              | .le => .le numeric leftExpr rightExpr
              | .gt => .gt numeric leftExpr rightExpr
              | .ge => .ge numeric leftExpr rightExpr
            pure ⟨.bool, expr, .bool⟩
    | _, _ => pathError .expectedNumeric path
    else pathError .sortMismatch path
  termination_by fuel
  decreasing_by all_goals omega

  def checkRelationalAggregateFuel (fuel : Nat) (Γ : TermContext)
      (scope : RowScope Γ.model Γ.current Γ.inputs) (rawOp : IR.AggOp)
      (tableName fkName selfFkName : String) (rawFilter : IR.Expr)
      (path : List ModelCheckPathSegment) :
      Except TermCheckError (CheckedExprResult Γ scope) :=
    if positive : 0 < fuel then do
      let box := scope.owner.box
      let table ← match Γ.model.catalog.lookupTable box tableName with
        | none => pathError .unknownTable (path ++ [.tableTarget])
        | some table => pure table
      let related : TableTarget Γ.model.catalog := scope.relatedTarget table
      let relatedScope : RowScope Γ.model Γ.current Γ.inputs := .table related
      let fkId ← match (Γ.model.schemaFor related).lookupAttribute fkName with
        | none => pathError .unknownJoinAttribute (path ++ [.joinForeignAttribute])
        | some attr => pure attr
      match fkShape : ((Γ.model.schemaFor related).attr fkId).shape with
      | .ref joinTarget =>
        let selfFkId ← match scope.schema.lookupAttribute selfFkName with
          | none => pathError .unknownJoinAttribute (path ++ [.joinSelfAttribute])
          | some attr => pure attr
        match selfShape : (scope.schema.attr selfFkId).shape with
        | .ref selfTarget =>
          if targetSame : joinTarget.ordinal.val = selfTarget.ordinal.val then
            have sameTarget : joinTarget = selfTarget := tableId_eq_of_val_eq targetSame
            let fk : ReferenceAttributeId (Γ.model.schemaFor related) joinTarget :=
              { id := fkId
                shapeEq := fkShape
                sortEq := attributeSort_ref_of_shape fkId joinTarget fkShape }
            let selfFkOriginal : ReferenceAttributeId scope.schema selfTarget :=
              { id := selfFkId
                shapeEq := selfShape
                sortEq := attributeSort_ref_of_shape selfFkId selfTarget selfShape }
            let selfFk : ReferenceAttributeId scope.schema joinTarget :=
              Eq.mp (congrArg (fun target => ReferenceAttributeId scope.schema target)
                sameTarget.symm) selfFkOriginal
            let ⟨sort, checkedOp, opOrigin⟩ ←
              synthAggOpFuel (fuel - 1) Γ relatedScope rawOp false
                (path ++ [.aggregateValue])
            let checkedFilter ← checkExprFuel (fuel - 1) Γ relatedScope rawFilter
              .bool .bool (path ++ [.aggregateFilter])
            match sort, checkedOp, opOrigin with
            | .real, checkedOp, .real =>
                pure ⟨.real, .agg table joinTarget fk selfFk checkedOp checkedFilter, .real⟩
            | .int, checkedOp, .int =>
                pure ⟨.int, .agg table joinTarget fk selfFk checkedOp checkedFilter, .int⟩
            | _, _, _ => pathError .expectedNumeric path
          else pathError .incompatibleJoinTargets path
        | _ => pathError .expectedReference (path ++ [.joinSelfAttribute])
      | _ => pathError .expectedReference (path ++ [.joinForeignAttribute])
    else pathError .sortMismatch path
  termination_by fuel
  decreasing_by all_goals omega

end

/-- Public canonical synthesis uses fuel derived solely from raw structure. -/
def synthExpr (Γ : TermContext)
    (scope : RowScope Γ.model Γ.current Γ.inputs) (raw : IR.Expr)
    (path : List ModelCheckPathSegment := []) :
    Except TermCheckError (CheckedExprResult Γ scope) :=
  synthExprFuel (rawExprDepth raw * 4 + 8) Γ scope raw path

/-- Public expected-sort checking never inserts a top-level numeric coercion. -/
def checkExpr (Γ : TermContext)
    (scope : RowScope Γ.model Γ.current Γ.inputs) (raw : IR.Expr)
    (expected : ScalarSort Γ.model.catalog) (origin : SortOrigin Γ scope expected)
    (path : List ModelCheckPathSegment := []) :
    Except TermCheckError (Expr Γ.model Γ.current Γ.inputs scope expected) :=
  checkExprFuel (rawExprDepth raw * 4 + 9) Γ scope raw expected origin path

/-! ## Effects, claims, and transition payloads -/

/-- Check one exact-sort assignment. Expected checking anchors bare enum literals
but never promotes an Int result at the assignment boundary. -/
def checkEffect (Γ : TermContext) (raw : IR.Effect)
    (path : List ModelCheckPathSegment := []) :
    Except TermCheckError (Effect Γ.model Γ.current Γ.inputs) := do
  match raw with
  | .setAttr name value =>
      let destination ← match (Γ.model.schemaFor Γ.current).lookupAttribute name with
        | none => pathError .unknownAttribute (path ++ [.destination])
        | some destination => pure destination
      let checked ← checkExpr Γ (.table Γ.current) value
        ((Γ.model.schemaFor Γ.current).attributeSort destination)
        (SortOrigin.ofAttribute Γ (scope := .table Γ.current) destination)
        (path ++ [.value])
      pure (.setAttr destination checked)

private def checkEffectsAux (Γ : TermContext) (root : List ModelCheckPathSegment) :
    Nat → List IR.Effect → Except TermCheckError (List (Effect Γ.model Γ.current Γ.inputs))
  | _, [] => pure []
  | index, raw :: raws => do
      let checked ← checkEffect Γ raw (root ++ [.effects, .effect index])
      let rest ← checkEffectsAux Γ root (index + 1) raws
      pure (checked :: rest)

/-- Source-ordered effect checking. -/
def checkEffects (Γ : TermContext) (raws : List IR.Effect)
    (path : List ModelCheckPathSegment := []) :
    Except TermCheckError (List (Effect Γ.model Γ.current Γ.inputs)) :=
  checkEffectsAux Γ path 0 raws

/-- Check one resource claim independently. Ordering domains remain
heterogeneous until PRD 0015. -/
def checkClaim (Γ : TermContext) (raw : IR.ResourceClaim)
    (path : List ModelCheckPathSegment := []) :
    Except TermCheckError (ResourceClaim Γ.model Γ.current Γ.inputs) := do
  let resourceResult ← synthExpr Γ (.table Γ.current) raw.resource (path ++ [.resource])
  let resource ← match refOfResult resourceResult with
    | none => pathError .expectedReference (path ++ [.resource])
    | some resource => pure resource
  match raw.ordering with
  | .raceTime =>
      pure
        { resourceTarget := resource.target
          resource := resource.expr
          orderingDomain := .real
          orderingAvailability := .surfaceProduced
          ordering := .raceTime }
  | .key rawKey =>
      let key ← synthExpr Γ (.table Γ.current) rawKey (path ++ [.orderingKey])
      match numericOfResult key with
      | some (.real expr) =>
          pure
            { resourceTarget := resource.target
              resource := resource.expr
              orderingDomain := .real
              orderingAvailability := .rawCheckable
              ordering := .key .real expr }
      | some (.int expr) =>
          pure
            { resourceTarget := resource.target
              resource := resource.expr
              orderingDomain := .int
              orderingAvailability := .rawCheckable
              ordering := .key .int expr }
      | none =>
          match enumOfResult key with
          | some anchored =>
              let domain : OrderingDomain Γ.model :=
                OrderingDomain.enum (owner := Γ.current) (Γ.model.schemaFor Γ.current)
                  anchored.attr anchored.enumSchema anchored.shapeEq
              let keyExpr : Term Γ.model Γ.current Γ.inputs domain.scalarSort :=
                anchored.expr
              pure
                { resourceTarget := resource.target
                  resource := resource.expr
                  orderingDomain := domain
                  orderingAvailability := .rawCheckable
                  ordering := .key domain keyExpr }
          | none => pathError .expectedOrderable (path ++ [.orderingKey])

/-! ## Structural raw-expression equality -/

/- A local structural equality classifier avoids relying on the raw IR's derived
`BEq`, which intentionally has no exported `LawfulBEq` instance. -/
private def rawExprConstructorTag : IR.Expr → Nat
  | .real _ => 0
  | .int _ => 1
  | .bool _ => 2
  | .enum _ => 3
  | .param _ => 4
  | .selfAttr _ => 5
  | .add _ _ => 6
  | .sub _ _ => 7
  | .mul _ _ => 8
  | .div _ _ => 9
  | .eq _ _ => 10
  | .ne _ _ => 11
  | .lt _ _ => 12
  | .le _ _ => 13
  | .gt _ _ => 14
  | .ge _ _ => 15
  | .and _ _ => 16
  | .or _ _ => 17
  | .not _ => 18
  | .enumIs _ _ => 19
  | .input _ _ => 20
  | .agg _ _ _ _ _ => 21

mutual
  private def rawExprStructEq (left right : IR.Expr) : Bool :=
    if rawExprConstructorTag left != rawExprConstructorTag right then false else
      match left, right with
      | .real ⟨ac, ae⟩, .real ⟨bc, be⟩ => ac == bc && ae == be
      | .int a, .int b => a == b
      | .bool a, .bool b => a == b
      | .enum a, .enum b => a == b
      | .param a, .param b => a == b
      | .selfAttr a, .selfAttr b => a == b
      | .add a b, .add c d | .sub a b, .sub c d | .mul a b, .mul c d
      | .div a b, .div c d | .eq a b, .eq c d | .ne a b, .ne c d
      | .lt a b, .lt c d | .le a b, .le c d | .gt a b, .gt c d
      | .ge a b, .ge c d | .and a b, .and c d | .or a b, .or c d =>
          rawExprStructEq a c && rawExprStructEq b d
      | .not a, .not b => rawExprStructEq a b
      | .enumIs a b, .enumIs c d => a == c && b == d
      | .input a b, .input c d => a == c && rawAggregateStructEq b d
      | .agg a b c d e, .agg f g h i j =>
          rawAggOpStructEq a f && b == g && c == h && d == i && rawExprStructEq e j
      | _, _ => false

  private def rawAggOpStructEq : IR.AggOp → IR.AggOp → Bool
    | .count, .count => true
    | .sum a, .sum b => rawExprStructEq a b
    | _, _ => false

  private def rawAggregateStructEq : IR.Aggregate → IR.Aggregate → Bool
    | .mk a b, .mk c d => rawAggOpStructEq a c && match b, d with
      | none, none => true
      | some x, some y => rawExprStructEq x y
      | _, _ => false
end

set_option maxHeartbeats 2000000 in
mutual
  private theorem rawExprStructEq_sound (left : IR.Expr) :
      ∀ right, rawExprStructEq left right = true → left = right := by
    intro right h
    cases left <;> cases right
    all_goals first | (change false = true at h; contradiction) | skip
    case real.real =>
      rename_i left right
      cases left with | mk lc le =>
        cases right with | mk rc re =>
          change (lc == rc && le == re) = true at h
          simp only [Bool.and_eq_true, beq_iff_eq] at h
          cases h.1
          cases h.2
          rfl
    case int.int | bool.bool | enum.enum | param.param | selfAttr.selfAttr =>
      change (_ == _) = true at h
      have same := beq_iff_eq.mp h
      cases same
      rfl
    case add.add | sub.sub | mul.mul | div.div | eq.eq | ne.ne
        | lt.lt | le.le | gt.gt | ge.ge | and.and | or.or =>
      rename_i leftA leftB rightA rightB
      change (rawExprStructEq leftA rightA && rawExprStructEq leftB rightB) = true at h
      have parts : rawExprStructEq leftA rightA = true ∧
          rawExprStructEq leftB rightB = true := by
        simpa only [Bool.and_eq_true] using h
      obtain ⟨first, second⟩ := parts
      have firstEq := rawExprStructEq_sound leftA rightA first
      have secondEq := rawExprStructEq_sound leftB rightB second
      cases firstEq
      cases secondEq
      rfl
    case not.not =>
      rename_i left right
      change rawExprStructEq left right = true at h
      cases rawExprStructEq_sound left right h
      rfl
    case enumIs.enumIs =>
      rename_i leftAttr leftVariant rightAttr rightVariant
      change (leftAttr == rightAttr && leftVariant == rightVariant) = true at h
      simp only [Bool.and_eq_true, beq_iff_eq] at h
      cases h.1
      cases h.2
      rfl
    case input.input =>
      rename_i leftPort leftAggregate rightPort rightAggregate
      change (leftPort == rightPort &&
        rawAggregateStructEq leftAggregate rightAggregate) = true at h
      have parts : (leftPort == rightPort) = true ∧
          rawAggregateStructEq leftAggregate rightAggregate = true := by
        simpa only [Bool.and_eq_true] using h
      obtain ⟨portEq, aggregateEq⟩ := parts
      have portSame := beq_iff_eq.mp portEq
      have aggregateSame := rawAggregateStructEq_sound _ _ aggregateEq
      cases portSame
      cases aggregateSame
      rfl
    case agg.agg =>
      rename_i leftOp leftTable leftForeign leftSelf leftFilter
        rightOp rightTable rightForeign rightSelf rightFilter
      change (rawAggOpStructEq leftOp rightOp && leftTable == rightTable &&
        leftForeign == rightForeign && leftSelf == rightSelf &&
        rawExprStructEq leftFilter rightFilter) = true at h
      simp only [Bool.and_eq_true, beq_iff_eq] at h
      have opSame := rawAggOpStructEq_sound _ _ h.1.1.1.1
      have filterSame := rawExprStructEq_sound _ _ h.2
      cases opSame
      cases h.1.1.1.2
      cases h.1.1.2
      cases h.1.2
      cases filterSame
      rfl
  termination_by rawExprDepth left
  decreasing_by all_goals simp_all [rawExprDepth, rawAggOpDepth, rawAggregateDepth] <;> omega

  private theorem rawAggOpStructEq_sound (left : IR.AggOp) :
      ∀ right, rawAggOpStructEq left right = true → left = right := by
    intro right h
    cases left <;> cases right
    · rfl
    · change false = true at h
      contradiction
    · change false = true at h
      contradiction
    · rename_i left right
      change rawExprStructEq left right = true at h
      cases rawExprStructEq_sound left right h
      rfl
  termination_by rawAggOpDepth left
  decreasing_by all_goals simp_all [rawExprDepth, rawAggOpDepth, rawAggregateDepth] <;> omega

  private theorem rawAggregateStructEq_sound (left : IR.Aggregate) :
      ∀ right, rawAggregateStructEq left right = true → left = right := by
    intro right h
    cases left with
    | mk leftOp leftFilter =>
      cases right with
      | mk rightOp rightFilter =>
        cases leftFilter <;> cases rightFilter
        · change (rawAggOpStructEq leftOp rightOp && true) = true at h
          have opEq : rawAggOpStructEq leftOp rightOp = true := by simpa using h
          cases rawAggOpStructEq_sound _ _ opEq
          rfl
        · change (rawAggOpStructEq leftOp rightOp && false) = true at h
          simp at h
        · change (rawAggOpStructEq leftOp rightOp && false) = true at h
          simp at h
        · rename_i left right
          change (rawAggOpStructEq leftOp rightOp &&
            rawExprStructEq left right) = true at h
          have parts : rawAggOpStructEq leftOp rightOp = true ∧
              rawExprStructEq left right = true := by
            simpa only [Bool.and_eq_true] using h
          cases rawAggOpStructEq_sound _ _ parts.1
          cases rawExprStructEq_sound left right parts.2
          rfl
  termination_by rawAggregateDepth left
  decreasing_by all_goals simp_all [rawExprDepth, rawAggOpDepth, rawAggregateDepth] <;> omega
end

mutual
  private theorem rawExprStructEq_refl (left : IR.Expr) :
      rawExprStructEq left left = true := by
    cases left
    case real value =>
      cases value
      change ((_ == _) && (_ == _)) = true
      simp
    case int | bool | enum | param | selfAttr =>
      change (_ == _) = true
      simp
    case add | sub | mul | div | eq | ne | lt | le | gt | ge | and | or =>
      rename_i left right
      change (rawExprStructEq left left && rawExprStructEq right right) = true
      rw [rawExprStructEq_refl left, rawExprStructEq_refl right]
      rfl
    case not =>
      rename_i value
      change rawExprStructEq value value = true
      exact rawExprStructEq_refl value
    case enumIs =>
      change ((_ == _) && (_ == _)) = true
      simp
    case input =>
      rename_i port aggregate
      change ((port == port) && rawAggregateStructEq aggregate aggregate) = true
      rw [rawAggregateStructEq_refl aggregate]
      simp
    case agg =>
      rename_i op table foreign self filter
      change (rawAggOpStructEq op op && table == table && foreign == foreign &&
        self == self && rawExprStructEq filter filter) = true
      rw [rawAggOpStructEq_refl op, rawExprStructEq_refl filter]
      simp
  termination_by rawExprDepth left
  decreasing_by all_goals simp_all [rawExprDepth, rawAggOpDepth, rawAggregateDepth] <;> omega

  private theorem rawAggOpStructEq_refl (left : IR.AggOp) :
      rawAggOpStructEq left left = true := by
    cases left
    · rfl
    · rename_i value
      change rawExprStructEq value value = true
      exact rawExprStructEq_refl value
  termination_by rawAggOpDepth left
  decreasing_by all_goals simp_all [rawExprDepth, rawAggOpDepth, rawAggregateDepth] <;> omega

  private theorem rawAggregateStructEq_refl (left : IR.Aggregate) :
      rawAggregateStructEq left left = true := by
    cases left with
    | mk op filter =>
      cases filter
      · change (rawAggOpStructEq op op && true) = true
        rw [rawAggOpStructEq_refl op]
        rfl
      · rename_i value
        change (rawAggOpStructEq op op && rawExprStructEq value value) = true
        rw [rawAggOpStructEq_refl op, rawExprStructEq_refl value]
        rfl
  termination_by rawAggregateDepth left
  decreasing_by all_goals simp_all [rawExprDepth, rawAggOpDepth, rawAggregateDepth] <;> omega
end

private theorem rawExprStructEq_eq_true_iff (left right : IR.Expr) :
    rawExprStructEq left right = true ↔ left = right := by
  constructor
  · exact rawExprStructEq_sound left right
  · intro same
    cases same
    exact rawExprStructEq_refl left

private def firstDuplicateResourceAux (seen : List IR.Expr) :
    Nat → List IR.ResourceClaim → Option Nat
  | _, [] => none
  | index, claim :: claims =>
      if seen.any (fun prior => rawExprStructEq prior claim.resource) then some index
      else firstDuplicateResourceAux (claim.resource :: seen) (index + 1) claims

/-- Index of the second occurrence of the first structurally duplicate raw
resource expression. -/
def firstDuplicateResource? (claims : List IR.ResourceClaim) : Option Nat :=
  firstDuplicateResourceAux [] 0 claims

private theorem seenAnyResource_eq_true_iff (seen : List IR.Expr) (resource : IR.Expr) :
    seen.any (fun prior => rawExprStructEq prior resource) = true ↔ resource ∈ seen := by
  simp only [List.any_eq_true]
  constructor
  · rintro ⟨prior, member, equal⟩
    have same := rawExprStructEq_sound prior resource equal
    simpa [same] using member
  · intro member
    exact ⟨resource, member, rawExprStructEq_refl resource⟩

private theorem firstDuplicateResourceAux_none_iff (seen : List IR.Expr)
    (index : Nat) (claims : List IR.ResourceClaim) :
    firstDuplicateResourceAux seen index claims = none ↔
      (claims.map IR.ResourceClaim.resource).Nodup ∧
      ∀ resource ∈ claims.map IR.ResourceClaim.resource, resource ∉ seen := by
  induction claims generalizing seen index with
  | nil => simp [firstDuplicateResourceAux]
  | cons claim claims ih =>
      simp only [firstDuplicateResourceAux]
      split
      · rename_i duplicate
        have member := (seenAnyResource_eq_true_iff seen claim.resource).mp duplicate
        simp [member]
      · rename_i fresh
        have notMember : claim.resource ∉ seen := by
          intro member
          have present := (seenAnyResource_eq_true_iff seen claim.resource).mpr member
          exact fresh present
        rw [ih (seen := claim.resource :: seen) (index := index + 1)]
        constructor
        · rintro ⟨tailUnique, tailFresh⟩
          refine ⟨List.nodup_cons.mpr ⟨?_, tailUnique⟩, ?_⟩
          · intro inTail
            exact (tailFresh claim.resource inTail) (by simp)
          · intro resource member
            rcases List.mem_cons.mp member with same | inTail
            · simpa [same] using notMember
            · intro inSeen
              exact tailFresh resource inTail (List.mem_cons.mpr (Or.inr inSeen))
        · rintro ⟨allUnique, allFresh⟩
          have uniqueParts := List.nodup_cons.mp allUnique
          refine ⟨uniqueParts.2, ?_⟩
          intro resource inTail member
          rcases List.mem_cons.mp member with same | inSeen
          · exact uniqueParts.1 (same ▸ inTail)
          · exact allFresh resource (List.mem_cons.mpr (Or.inr inTail)) inSeen

private theorem firstDuplicateResource_none_iff (claims : List IR.ResourceClaim) :
    firstDuplicateResource? claims = none ↔ ClaimsUnique claims := by
  simpa [firstDuplicateResource?, ClaimsUnique] using
    firstDuplicateResourceAux_none_iff [] 0 claims

private def checkClaimsAux (Γ : TermContext) (root : List ModelCheckPathSegment) :
    Nat → List IR.ResourceClaim →
      Except TermCheckError (List (ResourceClaim Γ.model Γ.current Γ.inputs))
  | _, [] => pure []
  | index, raw :: raws => do
      let checked ← checkClaim Γ raw (root ++ [.contests, .claim index])
      let rest ← checkClaimsAux Γ root (index + 1) raws
      pure (checked :: rest)

/-- Source-ordered claim checking with structural duplicate rejection. -/
def checkClaims (Γ : TermContext) (raws : List IR.ResourceClaim)
    (path : List ModelCheckPathSegment := []) :
    Except TermCheckError (List (ResourceClaim Γ.model Γ.current Γ.inputs)) := do
  match firstDuplicateResource? raws with
  | some index =>
      pathError .duplicateResourceClaim (path ++ [.contests, .claim index, .resource])
  | none => checkClaimsAux Γ path 0 raws

private def rawClaimCovers (claims : List IR.ResourceClaim) (rhs : IR.Expr) : Bool :=
  claims.any fun claim => rawExprStructEq claim.resource rhs

private def firstUnclaimedRefWriteAux (Γ : TermContext)
    (claims : List IR.ResourceClaim) : Nat → List IR.Effect → Option Nat
  | _, [] => none
  | index, effect :: effects =>
      match effect with
      | .setAttr destination rhs =>
          match (Γ.model.schemaFor Γ.current).lookupAttribute destination with
          | some attr =>
              match ((Γ.model.schemaFor Γ.current).attr attr).shape with
              | .ref _ =>
                  if rawClaimCovers claims rhs then
                    firstUnclaimedRefWriteAux Γ claims (index + 1) effects
                  else some index
              | _ => firstUnclaimedRefWriteAux Γ claims (index + 1) effects
          | none => firstUnclaimedRefWriteAux Γ claims (index + 1) effects

/-- Syntactic Ref-write coverage is checked against raw RHS/resource equality. -/
def firstUnclaimedRefWrite? (Γ : TermContext) (effects : List IR.Effect)
    (claims : List IR.ResourceClaim) : Option Nat :=
  firstUnclaimedRefWriteAux Γ claims 0 effects

private theorem rawClaimCovers_eq_true_iff (claims : List IR.ResourceClaim)
    (rhs : IR.Expr) : rawClaimCovers claims rhs = true ↔
      ∃ claim ∈ claims, claim.resource = rhs := by
  simp only [rawClaimCovers, List.any_eq_true]
  constructor
  · rintro ⟨claim, member, equal⟩
    exact ⟨claim, member, rawExprStructEq_sound _ _ equal⟩
  · rintro ⟨claim, member, same⟩
    exact ⟨claim, member, (rawExprStructEq_eq_true_iff _ _).mpr same⟩

private theorem firstUnclaimedRefWriteAux_none_iff (Γ : TermContext)
    (claims : List IR.ResourceClaim) (index : Nat) (effects : List IR.Effect) :
    firstUnclaimedRefWriteAux Γ claims index effects = none ↔
      RefWritesCovered Γ effects claims := by
  induction effects generalizing index with
  | nil => simp [firstUnclaimedRefWriteAux, RefWritesCovered]
  | cons effect effects ih =>
      cases effect with
      | setAttr destination rhs =>
        simp only [firstUnclaimedRefWriteAux]
        split
        · rename_i attr found
          split
          · rename_i target shape
            split
            · rename_i covered
              rw [ih]
              have witness := (rawClaimCovers_eq_true_iff claims rhs).mp covered
              simp [RefWritesCovered, found, shape, witness]
            · rename_i uncovered
              have noWitness : ¬ ∃ claim ∈ claims, claim.resource = rhs := by
                intro witness
                exact uncovered ((rawClaimCovers_eq_true_iff claims rhs).mpr witness)
              simp [RefWritesCovered, found, shape, noWitness]
          · rename_i shape
            rw [ih]
            simp [RefWritesCovered, found, shape]
        · rename_i missing
          rw [ih]
          simp [RefWritesCovered, missing]

private theorem firstUnclaimedRefWrite_none_iff (Γ : TermContext)
    (effects : List IR.Effect) (claims : List IR.ResourceClaim) :
    firstUnclaimedRefWrite? Γ effects claims = none ↔
      RefWritesCovered Γ effects claims :=
  firstUnclaimedRefWriteAux_none_iff Γ claims 0 effects

/-- Canonical transition-payload checker. -/
def checkTransitionTerms (Γ : TermContext) (raw : IR.Transition)
    (path : List ModelCheckPathSegment := []) :
    Except TermCheckError (TransitionTerms Γ.model Γ.current Γ.inputs) := do
  let guard ← checkExpr Γ (.table Γ.current) raw.guard .bool .bool (path ++ [.guard])
  let hazard ← checkExpr Γ (.table Γ.current) raw.hazard .real .real (path ++ [.hazard])
  let effects ← checkEffects Γ raw.effects path
  let claims ← checkClaims Γ raw.contests path
  match firstUnclaimedRefWrite? Γ raw.effects raw.contests with
  | some index =>
      pathError .unclaimedRefWrite (path ++ [.effects, .effect index, .value])
  | none => pure { guard, hazard, effects, claims }

/-! ## Independent syntax-directed judgments -/

/- Checker-owned structural shapes retain resolved ordinals and exact literals
without using raw erasure. `intToReal` is deliberately transparent. -/
mutual
  inductive CheckedExprShape where
    | real (value : IR.Scientific)
    | int (value : Int)
    | bool (value : Bool)
    | enum (attr variant : Nat)
    | param (parameter : Nat)
    | selfAttr (attr : Nat)
    | add (lhs rhs : CheckedExprShape)
    | sub (lhs rhs : CheckedExprShape)
    | mul (lhs rhs : CheckedExprShape)
    | div (lhs rhs : CheckedExprShape)
    | eq (lhs rhs : CheckedExprShape)
    | ne (lhs rhs : CheckedExprShape)
    | lt (lhs rhs : CheckedExprShape)
    | le (lhs rhs : CheckedExprShape)
    | gt (lhs rhs : CheckedExprShape)
    | ge (lhs rhs : CheckedExprShape)
    | and (lhs rhs : CheckedExprShape)
    | or (lhs rhs : CheckedExprShape)
    | not (value : CheckedExprShape)
    | enumIs (attr variant : Nat)
    | input (port : Nat) (aggregate : CheckedAggregateShape)
    | agg (table joinTarget foreignKey selfKey : Nat)
        (op : CheckedAggOpShape) (filter : CheckedExprShape)

  inductive CheckedAggOpShape where
    | count
    | sum (value : CheckedExprShape)

  inductive CheckedAggregateShape where
    | unfiltered (op : CheckedAggOpShape)
    | filtered (op : CheckedAggOpShape) (filter : CheckedExprShape)
end

mutual
  /- Pointwise structural projection of checked syntax. -/
  def checkedExprShape {Γ : TermContext}
      {scope : RowScope Γ.model Γ.current Γ.inputs}
      {sort : ScalarSort Γ.model.catalog}
      (expr : Expr Γ.model Γ.current Γ.inputs scope sort) : CheckedExprShape :=
    match expr with
    | .real value => .real value.source
    | .int value => .int value
    | .bool value => .bool value
    | .enum attr _ variant => .enum attr.ordinal.val variant.ordinal.val
    | .param id => .param id.ordinal.val
    | .selfAttr attr => .selfAttr attr.ordinal.val
    | .intToReal value => checkedExprShape value
    | .add _ lhs rhs => .add (checkedExprShape lhs) (checkedExprShape rhs)
    | .sub _ lhs rhs => .sub (checkedExprShape lhs) (checkedExprShape rhs)
    | .mul _ lhs rhs => .mul (checkedExprShape lhs) (checkedExprShape rhs)
    | .div lhs rhs => .div (checkedExprShape lhs) (checkedExprShape rhs)
    | .eq lhs rhs => .eq (checkedExprShape lhs) (checkedExprShape rhs)
    | .ne lhs rhs => .ne (checkedExprShape lhs) (checkedExprShape rhs)
    | .lt _ lhs rhs => .lt (checkedExprShape lhs) (checkedExprShape rhs)
    | .le _ lhs rhs => .le (checkedExprShape lhs) (checkedExprShape rhs)
    | .gt _ lhs rhs => .gt (checkedExprShape lhs) (checkedExprShape rhs)
    | .ge _ lhs rhs => .ge (checkedExprShape lhs) (checkedExprShape rhs)
    | .and lhs rhs => .and (checkedExprShape lhs) (checkedExprShape rhs)
    | .or lhs rhs => .or (checkedExprShape lhs) (checkedExprShape rhs)
    | .not value => .not (checkedExprShape value)
    | .enumIs attr _ variant => .enumIs attr.ordinal.val variant.ordinal.val
    | .input port aggregate => .input port.ordinal.val (checkedAggregateShape aggregate)
    | .agg table joinTarget fk selfFk op filter =>
        .agg table.ordinal.val joinTarget.ordinal.val fk.id.ordinal.val
          selfFk.id.ordinal.val (checkedAggOpShape op) (checkedExprShape filter)

  def checkedAggOpShape {Γ : TermContext}
      {scope : RowScope Γ.model Γ.current Γ.inputs}
      {sort : ScalarSort Γ.model.catalog}
      (op : AggOp Γ.model Γ.current Γ.inputs scope sort) : CheckedAggOpShape :=
    match op with
    | .count => .count
    | .sum _ value => .sum (checkedExprShape value)

  def checkedAggregateShape {Γ : TermContext}
      {scope : RowScope Γ.model Γ.current Γ.inputs}
      {sort : ScalarSort Γ.model.catalog}
      (aggregate : Aggregate Γ.model Γ.current Γ.inputs scope sort) :
      CheckedAggregateShape :=
    match aggregate with
    | .unfiltered op => .unfiltered (checkedAggOpShape op)
    | .filtered op filter =>
        .filtered (checkedAggOpShape op) (checkedExprShape filter)
end

/-- Pointwise dependent structural equivalence. It preserves checked owner
ordinals and constructor/list shape while ignoring proof witnesses and only the
transparent `intToReal` node. It is intentionally not equality of erasures. -/
def CheckedExprEquivalent {Γ : TermContext}
    {scope : RowScope Γ.model Γ.current Γ.inputs}
    {leftSort rightSort : ScalarSort Γ.model.catalog}
    (left : Expr Γ.model Γ.current Γ.inputs scope leftSort)
    (right : Expr Γ.model Γ.current Γ.inputs scope rightSort) : Prop :=
  checkedExprShape left = checkedExprShape right

@[refl] theorem CheckedExprEquivalent.refl {Γ : TermContext}
    {scope : RowScope Γ.model Γ.current Γ.inputs}
    {sort : ScalarSort Γ.model.catalog}
    (expr : Expr Γ.model Γ.current Γ.inputs scope sort) :
    CheckedExprEquivalent expr expr := rfl

@[symm] theorem CheckedExprEquivalent.symm {Γ : TermContext}
    {scope : RowScope Γ.model Γ.current Γ.inputs}
    {leftSort rightSort : ScalarSort Γ.model.catalog}
    {left : Expr Γ.model Γ.current Γ.inputs scope leftSort}
    {right : Expr Γ.model Γ.current Γ.inputs scope rightSort}
    (same : CheckedExprEquivalent left right) : CheckedExprEquivalent right left :=
  Eq.symm same

@[trans] theorem CheckedExprEquivalent.trans {Γ : TermContext}
    {scope : RowScope Γ.model Γ.current Γ.inputs}
    {firstSort secondSort thirdSort : ScalarSort Γ.model.catalog}
    {first : Expr Γ.model Γ.current Γ.inputs scope firstSort}
    {second : Expr Γ.model Γ.current Γ.inputs scope secondSort}
    {third : Expr Γ.model Γ.current Γ.inputs scope thirdSort}
    (left : CheckedExprEquivalent first second)
    (right : CheckedExprEquivalent second third) : CheckedExprEquivalent first third :=
  Eq.trans left right

/-! ## Immediate erasure consequences of the independent judgments -/

@[simp] theorem ExprSynthesizes.erase_exact {Γ : TermContext}
      {scope : RowScope Γ.model Γ.current Γ.inputs} {raw sort term}
      (h : ExprSynthesizes Γ scope raw sort term) : term.erase = raw := by
    apply ExprSynthesizes.rec
      (motive_1 := fun _ raw _ term _ => term.erase = raw)
      (motive_2 := fun _ _ raw term _ => term.erase = raw)
      (motive_3 := fun _ raw _ op _ => op.erase = raw)
      (motive_4 := fun _ raw _ aggregate _ => aggregate.erase = raw)
      <;> intros <;> simp_all

@[simp] theorem ExprChecks.erase_exact {Γ : TermContext}
      {scope : RowScope Γ.model Γ.current Γ.inputs} {expected raw term}
      (h : ExprChecks Γ scope expected raw term) : term.erase = raw := by
  cases h with
  | synth hs => exact ExprSynthesizes.erase_exact hs
  | enum => rfl

@[simp] theorem AggOpSynthesizes.erase_exact {Γ : TermContext}
      {scope : RowScope Γ.model Γ.current Γ.inputs} {raw sort op}
      (h : AggOpSynthesizes Γ scope raw sort op) : op.erase = raw := by
  cases h with
  | count => rfl
  | sumInt hs => simpa using ExprSynthesizes.erase_exact hs
  | sumReal hs => simpa using ExprSynthesizes.erase_exact hs

@[simp] theorem AggregateSynthesizes.erase_exact {Γ : TermContext}
      {scope : RowScope Γ.model Γ.current Γ.inputs} {raw sort aggregate}
      (h : AggregateSynthesizes Γ scope raw sort aggregate) : aggregate.erase = raw := by
  cases h with
  | unfiltered _ hop => simpa using AggOpSynthesizes.erase_exact hop
  | filtered _ _ hop hfilter =>
      simp [AggOpSynthesizes.erase_exact hop, ExprChecks.erase_exact hfilter]

@[simp] theorem EffectWellTyped.erase_exact {Γ : TermContext} {raw checked}
    (h : EffectWellTyped Γ raw checked) : checked.erase = raw := by
  cases h with
  | setAttr _ hvalue => simp [ExprChecks.erase_exact hvalue]

@[simp] theorem ClaimWellTyped.erase_exact {Γ : TermContext} {raw checked}
    (h : ClaimWellTyped Γ raw checked) : checked.erase = raw := by
  cases h with
  | raceTime hresource => simp [ResourceClaim.erase, ExprSynthesizes.erase_exact hresource]
  | key hresource _ hkey =>
      simp [ResourceClaim.erase, ExprSynthesizes.erase_exact hresource,
        ExprSynthesizes.erase_exact hkey]

private theorem effectsForall₂_erases {Γ : TermContext}
    {raw : List IR.Effect} {checked : List (Effect Γ.model Γ.current Γ.inputs)}
    (h : List.Forall₂ (EffectWellTyped Γ) raw checked) :
    checked.map Effect.erase = raw := by
  induction h with
  | nil => rfl
  | cons head _ tail => simp [EffectWellTyped.erase_exact head, tail]

private theorem claimsForall₂_erases {Γ : TermContext}
    {raw : List IR.ResourceClaim}
    {checked : List (ResourceClaim Γ.model Γ.current Γ.inputs)}
    (h : List.Forall₂ (ClaimWellTyped Γ) raw checked) :
    checked.map ResourceClaim.erase = raw := by
  induction h with
  | nil => rfl
  | cons head _ tail => simp [ClaimWellTyped.erase_exact head, tail]

/-- The independent transition judgment reconstructs every checked payload in
source order. Header fields remain declaration-owned and are intentionally not
part of `TransitionTerms`. -/
@[simp] theorem TransitionWellTyped.eraseGuard_exact {Γ : TermContext}
    {raw : IR.Transition} {checked : TransitionTerms Γ.model Γ.current Γ.inputs}
    (h : TransitionWellTyped Γ raw checked) : checked.eraseGuard = raw.guard := by
  cases h with
  | mk guard => exact ExprChecks.erase_exact guard

@[simp] theorem TransitionWellTyped.eraseHazard_exact {Γ : TermContext}
    {raw : IR.Transition} {checked : TransitionTerms Γ.model Γ.current Γ.inputs}
    (h : TransitionWellTyped Γ raw checked) : checked.eraseHazard = raw.hazard := by
  cases h with
  | mk _ hazard => exact ExprChecks.erase_exact hazard

@[simp] theorem TransitionWellTyped.eraseEffects_exact {Γ : TermContext}
    {raw : IR.Transition} {checked : TransitionTerms Γ.model Γ.current Γ.inputs}
    (h : TransitionWellTyped Γ raw checked) : checked.eraseEffects = raw.effects := by
  cases h with
  | mk _ _ effects => exact effectsForall₂_erases effects

@[simp] theorem TransitionWellTyped.eraseClaims_exact {Γ : TermContext}
    {raw : IR.Transition} {checked : TransitionTerms Γ.model Γ.current Γ.inputs}
    (h : TransitionWellTyped Γ raw checked) : checked.eraseClaims = raw.contests := by
  cases h with
  | mk _ _ _ claims => exact claimsForall₂_erases claims

/-! ## Checker correspondence -/

private def equalityRaw (isEq : Bool) (lhs rhs : IR.Expr) : IR.Expr :=
  if isEq then .eq lhs rhs else .ne lhs rhs

private def orderingRaw (kind : OrderingKind) (lhs rhs : IR.Expr) : IR.Expr :=
  match kind with
  | .lt => .lt lhs rhs
  | .le => .le lhs rhs
  | .gt => .gt lhs rhs
  | .ge => .ge lhs rhs

private inductive NumericSynthesizes {Γ : TermContext}
    {scope : RowScope Γ.model Γ.current Γ.inputs} (raw : IR.Expr) :
    CheckedNumeric Γ scope → Prop where
  | real {expr : Expr Γ.model Γ.current Γ.inputs scope .real}
      (h : ExprSynthesizes Γ scope raw .real expr) :
      NumericSynthesizes raw (.real expr)
  | int {expr : Expr Γ.model Γ.current Γ.inputs scope .int}
      (h : ExprSynthesizes Γ scope raw .int expr) :
      NumericSynthesizes raw (.int expr)

private theorem numericOfResult_sound {Γ : TermContext}
    {scope : RowScope Γ.model Γ.current Γ.inputs} {raw}
    {result : CheckedExprResult Γ scope} {numeric : CheckedNumeric Γ scope}
    (h : ExprSynthesizes Γ scope raw result.sort result.expr)
    (found : numericOfResult result = some numeric) :
    NumericSynthesizes raw numeric := by
  cases result with
  | mk sort expr origin =>
      cases origin <;> simp [numericOfResult] at found
      · cases found
        exact .real h
      · cases found
        exact .int h

private theorem enumOfResult_sound {Γ : TermContext}
    {scope : RowScope Γ.model Γ.current Γ.inputs} {raw}
    {result : CheckedExprResult Γ scope} {anchored : CheckedEnumExpr Γ scope}
    (h : ExprSynthesizes Γ scope raw result.sort result.expr)
    (found : enumOfResult result = some anchored) :
    ExprSynthesizes Γ scope raw
      (.enum scope.schema anchored.attr anchored.enumSchema anchored.shapeEq)
      anchored.expr := by
  cases result with
  | mk sort expr origin =>
      cases origin <;> simp [enumOfResult] at found
      cases found
      exact h

/-- Simultaneous soundness statement for the seven fuel-bounded term checkers. -/
private structure FuelSound (fuel : Nat) : Prop where
  synthExpr : ∀ {Γ : TermContext}
      {scope : RowScope Γ.model Γ.current Γ.inputs} {raw path result},
      synthExprFuel fuel Γ scope raw path = .ok result →
        ExprSynthesizes Γ scope raw result.sort result.expr
  checkExpr : ∀ {Γ : TermContext}
      {scope : RowScope Γ.model Γ.current Γ.inputs}
      {raw expected origin path result},
      checkExprFuel fuel Γ scope raw expected origin path = .ok result →
        ExprChecks Γ scope expected raw result
  synthAggOp : ∀ {Γ : TermContext}
      {scope : RowScope Γ.model Γ.current Γ.inputs}
      {raw inputBoundary path result},
      synthAggOpFuel fuel Γ scope raw inputBoundary path = .ok result →
        AggOpSynthesizes Γ scope raw result.1 result.2.1
  synthAggregate : ∀ {Γ : TermContext} {port : InputId Γ.inputs}
      {raw path result},
      synthAggregateFuel fuel Γ port raw path = .ok result →
        AggregateSynthesizes Γ (.input port) raw result.1 result.2.1
  equality : ∀ {Γ : TermContext}
      {scope : RowScope Γ.model Γ.current Γ.inputs}
      {isEq lhs rhs path result},
      checkEqualityFuel fuel Γ scope isEq lhs rhs path = .ok result →
        ExprSynthesizes Γ scope (equalityRaw isEq lhs rhs) result.sort result.expr
  ordering : ∀ {Γ : TermContext}
      {scope : RowScope Γ.model Γ.current Γ.inputs}
      {kind lhs rhs path result},
      checkOrderingFuel fuel Γ scope kind lhs rhs path = .ok result →
        ExprSynthesizes Γ scope (orderingRaw kind lhs rhs) result.sort result.expr
  relational : ∀ {Γ : TermContext}
      {scope : RowScope Γ.model Γ.current Γ.inputs}
      {rawOp tableName fkName selfFkName rawFilter path result},
      checkRelationalAggregateFuel fuel Γ scope rawOp tableName fkName selfFkName
          rawFilter path = .ok result →
        ExprSynthesizes Γ scope
          (.agg rawOp tableName fkName selfFkName rawFilter) result.sort result.expr

private theorem synthAggOpFuel_input_free {fuel : Nat} {Γ : TermContext}
    {scope : RowScope Γ.model Γ.current Γ.inputs} {raw path result}
    (success : synthAggOpFuel fuel Γ scope raw true path = .ok result) :
    rawAggOpContainsAggregate raw = false := by
  cases raw with
  | count => rfl
  | sum value =>
      cases aggregateFree : rawExprContainsAggregate value with
      | false => simpa [rawAggOpContainsAggregate] using aggregateFree
      | true =>
          cases fuel with
          | zero =>
              rw [synthAggOpFuel.eq_def (path := path)] at success
              simp [pathError] at success
          | succ fuel =>
              rw [synthAggOpFuel.eq_def (path := path)] at success
              rw [dif_pos (Nat.succ_pos fuel)] at success
              simp [aggregateFree, pathError] at success

private theorem fuelSound (fuel : Nat) : FuelSound fuel := by
  induction fuel with
  | zero =>
      constructor
      · intro Γ scope raw path result success
        rw [synthExprFuel.eq_def (path := path)] at success
        simp [pathError] at success
      · intro Γ scope raw expected origin path result success
        rw [checkExprFuel.eq_def (path := path)] at success
        simp [pathError] at success
      · intro Γ scope raw inputBoundary path result success
        rw [synthAggOpFuel.eq_def (path := path)] at success
        simp [pathError] at success
      · intro Γ port raw path result success
        rw [synthAggregateFuel.eq_def (path := path)] at success
        simp [pathError] at success
      · intro Γ scope isEq lhs rhs path result success
        rw [checkEqualityFuel.eq_def] at success
        simp [pathError] at success
      · intro Γ scope kind lhs rhs path result success
        rw [checkOrderingFuel.eq_def] at success
        simp [pathError] at success
      · intro Γ scope rawOp tableName fkName selfFkName rawFilter path result success
        rw [checkRelationalAggregateFuel.eq_def] at success
        simp [pathError] at success
  | succ fuel ih =>
      constructor
      · intro Γ scope raw path result success
        rw [synthExprFuel.eq_def (path := path)] at success
        rw [dif_pos (Nat.succ_pos fuel)] at success
        simp only [Nat.succ_sub_one] at success
        cases raw with
        | real value =>
            cases success
            exact ExprSynthesizes.real _
        | int value =>
            cases success
            exact ExprSynthesizes.int value
        | bool value =>
            cases success
            exact ExprSynthesizes.bool value
        | enum variant => simp [pathError] at success
        | param name =>
            cases found : Γ.model.params.lookup name with
            | none => simp [found, pathError] at success
            | some id =>
                simp only [found] at success
                split at success <;> rename_i sortEq
                · cases success
                  convert ExprSynthesizes.param (Γ := Γ) (scope := scope) id using 1 <;>
                    simp [sortEq, ParamSort.scalarSort,
                      Γ.model.params.parameterLookup_name found]
                · cases success
                  convert ExprSynthesizes.param (Γ := Γ) (scope := scope) id using 1 <;>
                    simp [sortEq, ParamSort.scalarSort,
                      Γ.model.params.parameterLookup_name found]
        | selfAttr name =>
            cases found : scope.schema.lookupAttribute name with
            | none => simp [found, pathError] at success
            | some attr =>
                simp only [found] at success
                cases success
                simpa [scope.schema.attributeLookup_name found] using
                  ExprSynthesizes.selfAttr (Γ := Γ) attr
        | add lhs rhs =>
            simp only at success
            cases leftSuccess : synthExprFuel fuel Γ scope lhs (path ++ [.lhs]) with
            | error error =>
                rw [leftSuccess] at success
                simp only [Bind.bind, Monad.toBind, Except.instMonad, Except.bind] at success
                cases success
            | ok left =>
                rw [leftSuccess] at success
                have hl := ih.synthExpr leftSuccess
                cases rightSuccess : synthExprFuel fuel Γ scope rhs (path ++ [.rhs]) with
                | error error =>
                    rw [rightSuccess] at success
                    simp only [Bind.bind, Monad.toBind, Except.instMonad, Except.bind] at success
                    cases success
                | ok right =>
                    rw [rightSuccess] at success
                    have hr := ih.synthExpr rightSuccess
                    simp only [Bind.bind, Monad.toBind, Except.instMonad, Except.bind] at success
                    cases leftFound : numericOfResult left with
                    | none =>
                        rw [leftFound] at success
                        simp [pathError] at success
                    | some leftNumeric =>
                        rw [leftFound] at success
                        cases rightFound : numericOfResult right with
                        | none =>
                            rw [rightFound] at success
                            simp [pathError] at success
                        | some rightNumeric =>
                            rw [rightFound] at success
                            have hnl := numericOfResult_sound hl leftFound
                            have hnr := numericOfResult_sound hr rightFound
                            cases hnl with
                            | real hl =>
                                cases hnr with
                                | real hr =>
                                    simp [leftFound, rightFound, promotePair] at success
                                    cases success
                                    exact ExprSynthesizes.addReal hl hr
                                | int hr =>
                                    simp [leftFound, rightFound, promotePair] at success
                                    cases success
                                    exact ExprSynthesizes.addRealInt hl hr
                            | int hl =>
                                cases hnr with
                                | real hr =>
                                    simp [leftFound, rightFound, promotePair] at success
                                    cases success
                                    exact ExprSynthesizes.addIntReal hl hr
                                | int hr =>
                                    simp [leftFound, rightFound, promotePair] at success
                                    cases success
                                    exact ExprSynthesizes.addInt hl hr
        | sub lhs rhs =>
            simp only at success
            cases leftSuccess : synthExprFuel fuel Γ scope lhs (path ++ [.lhs]) with
            | error error =>
                rw [leftSuccess] at success
                simp only [Bind.bind, Monad.toBind, Except.instMonad, Except.bind] at success
                cases success
            | ok left =>
                rw [leftSuccess] at success
                have hl := ih.synthExpr leftSuccess
                cases rightSuccess : synthExprFuel fuel Γ scope rhs (path ++ [.rhs]) with
                | error error =>
                    rw [rightSuccess] at success
                    simp only [Bind.bind, Monad.toBind, Except.instMonad, Except.bind] at success
                    cases success
                | ok right =>
                    rw [rightSuccess] at success
                    have hr := ih.synthExpr rightSuccess
                    simp only [Bind.bind, Monad.toBind, Except.instMonad, Except.bind] at success
                    cases leftFound : numericOfResult left with
                    | none =>
                        rw [leftFound] at success
                        simp [pathError] at success
                    | some leftNumeric =>
                        rw [leftFound] at success
                        cases rightFound : numericOfResult right with
                        | none =>
                            rw [rightFound] at success
                            simp [pathError] at success
                        | some rightNumeric =>
                            rw [rightFound] at success
                            have hnl := numericOfResult_sound hl leftFound
                            have hnr := numericOfResult_sound hr rightFound
                            cases hnl with
                            | real hl =>
                                cases hnr with
                                | real hr =>
                                    simp [leftFound, rightFound, promotePair] at success
                                    cases success
                                    exact ExprSynthesizes.subReal hl hr
                                | int hr =>
                                    simp [leftFound, rightFound, promotePair] at success
                                    cases success
                                    exact ExprSynthesizes.subRealInt hl hr
                            | int hl =>
                                cases hnr with
                                | real hr =>
                                    simp [leftFound, rightFound, promotePair] at success
                                    cases success
                                    exact ExprSynthesizes.subIntReal hl hr
                                | int hr =>
                                    simp [leftFound, rightFound, promotePair] at success
                                    cases success
                                    exact ExprSynthesizes.subInt hl hr
        | mul lhs rhs =>
            simp only at success
            cases leftSuccess : synthExprFuel fuel Γ scope lhs (path ++ [.lhs]) with
            | error error =>
                rw [leftSuccess] at success
                simp only [Bind.bind, Monad.toBind, Except.instMonad, Except.bind] at success
                cases success
            | ok left =>
                rw [leftSuccess] at success
                have hl := ih.synthExpr leftSuccess
                cases rightSuccess : synthExprFuel fuel Γ scope rhs (path ++ [.rhs]) with
                | error error =>
                    rw [rightSuccess] at success
                    simp only [Bind.bind, Monad.toBind, Except.instMonad, Except.bind] at success
                    cases success
                | ok right =>
                    rw [rightSuccess] at success
                    have hr := ih.synthExpr rightSuccess
                    simp only [Bind.bind, Monad.toBind, Except.instMonad, Except.bind] at success
                    cases leftFound : numericOfResult left with
                    | none =>
                        rw [leftFound] at success
                        simp [pathError] at success
                    | some leftNumeric =>
                        rw [leftFound] at success
                        cases rightFound : numericOfResult right with
                        | none =>
                            rw [rightFound] at success
                            simp [pathError] at success
                        | some rightNumeric =>
                            rw [rightFound] at success
                            have hnl := numericOfResult_sound hl leftFound
                            have hnr := numericOfResult_sound hr rightFound
                            cases hnl with
                            | real hl =>
                                cases hnr with
                                | real hr =>
                                    simp [leftFound, rightFound, promotePair] at success
                                    cases success
                                    exact ExprSynthesizes.mulReal hl hr
                                | int hr =>
                                    simp [leftFound, rightFound, promotePair] at success
                                    cases success
                                    exact ExprSynthesizes.mulRealInt hl hr
                            | int hl =>
                                cases hnr with
                                | real hr =>
                                    simp [leftFound, rightFound, promotePair] at success
                                    cases success
                                    exact ExprSynthesizes.mulIntReal hl hr
                                | int hr =>
                                    simp [leftFound, rightFound, promotePair] at success
                                    cases success
                                    exact ExprSynthesizes.mulInt hl hr
        | div lhs rhs =>
            simp only at success
            cases leftSuccess : synthExprFuel fuel Γ scope lhs (path ++ [.lhs]) with
            | error error =>
                rw [leftSuccess] at success
                simp only [Bind.bind, Monad.toBind, Except.instMonad, Except.bind] at success
                cases success
            | ok left =>
                rw [leftSuccess] at success
                have hl := ih.synthExpr leftSuccess
                cases rightSuccess : synthExprFuel fuel Γ scope rhs (path ++ [.rhs]) with
                | error error =>
                    rw [rightSuccess] at success
                    simp only [Bind.bind, Monad.toBind, Except.instMonad, Except.bind] at success
                    cases success
                | ok right =>
                    rw [rightSuccess] at success
                    have hr := ih.synthExpr rightSuccess
                    simp only [Bind.bind, Monad.toBind, Except.instMonad, Except.bind] at success
                    cases leftFound : numericOfResult left with
                    | none =>
                        rw [leftFound] at success
                        simp [pathError] at success
                    | some leftNumeric =>
                        rw [leftFound] at success
                        cases rightFound : numericOfResult right with
                        | none =>
                            rw [rightFound] at success
                            simp [pathError] at success
                        | some rightNumeric =>
                            rw [rightFound] at success
                            have hnl := numericOfResult_sound hl leftFound
                            have hnr := numericOfResult_sound hr rightFound
                            cases hnl with
                            | real hl =>
                                cases hnr with
                                | real hr =>
                                    simp [leftFound, rightFound, toReal] at success
                                    cases success
                                    exact ExprSynthesizes.divReal hl hr
                                | int hr =>
                                    simp [leftFound, rightFound, toReal] at success
                                    cases success
                                    exact ExprSynthesizes.divRealInt hl hr
                            | int hl =>
                                cases hnr with
                                | real hr =>
                                    simp [leftFound, rightFound, toReal] at success
                                    cases success
                                    exact ExprSynthesizes.divIntReal hl hr
                                | int hr =>
                                    simp [leftFound, rightFound, toReal] at success
                                    cases success
                                    exact ExprSynthesizes.divInt hl hr
        | eq lhs rhs => simpa [equalityRaw] using ih.equality success
        | ne lhs rhs => simpa [equalityRaw] using ih.equality success
        | lt lhs rhs => simpa [orderingRaw] using ih.ordering success
        | le lhs rhs => simpa [orderingRaw] using ih.ordering success
        | gt lhs rhs => simpa [orderingRaw] using ih.ordering success
        | ge lhs rhs => simpa [orderingRaw] using ih.ordering success
        | and lhs rhs =>
            simp only at success
            cases leftSuccess : checkExprFuel fuel Γ scope lhs .bool .bool
                (path ++ [.lhs]) with
            | error error =>
                rw [leftSuccess] at success
                simp only [Bind.bind, Monad.toBind, Except.instMonad, Except.bind] at success
                cases success
            | ok left =>
                rw [leftSuccess] at success
                have hl := ih.checkExpr leftSuccess
                cases rightSuccess : checkExprFuel fuel Γ scope rhs .bool .bool
                    (path ++ [.rhs]) with
                | error error =>
                    rw [rightSuccess] at success
                    simp only [Bind.bind, Monad.toBind, Except.instMonad, Except.bind] at success
                    cases success
                | ok right =>
                    rw [rightSuccess] at success
                    have hr := ih.checkExpr rightSuccess
                    simp only [Bind.bind, Monad.toBind, Except.instMonad, Except.bind] at success
                    cases success
                    exact ExprSynthesizes.and hl hr
        | or lhs rhs =>
            simp only at success
            cases leftSuccess : checkExprFuel fuel Γ scope lhs .bool .bool
                (path ++ [.lhs]) with
            | error error =>
                rw [leftSuccess] at success
                simp only [Bind.bind, Monad.toBind, Except.instMonad, Except.bind] at success
                cases success
            | ok left =>
                rw [leftSuccess] at success
                have hl := ih.checkExpr leftSuccess
                cases rightSuccess : checkExprFuel fuel Γ scope rhs .bool .bool
                    (path ++ [.rhs]) with
                | error error =>
                    rw [rightSuccess] at success
                    simp only [Bind.bind, Monad.toBind, Except.instMonad, Except.bind] at success
                    cases success
                | ok right =>
                    rw [rightSuccess] at success
                    have hr := ih.checkExpr rightSuccess
                    simp only [Bind.bind, Monad.toBind, Except.instMonad, Except.bind] at success
                    cases success
                    exact ExprSynthesizes.or hl hr
        | not value =>
            simp only at success
            cases checkedSuccess : checkExprFuel fuel Γ scope value .bool .bool
                (path ++ [.operand]) with
            | error error =>
                rw [checkedSuccess] at success
                simp only [Bind.bind, Monad.toBind, Except.instMonad, Except.bind] at success
                cases success
            | ok checked =>
                rw [checkedSuccess] at success
                have h := ih.checkExpr checkedSuccess
                simp only [Bind.bind, Monad.toBind, Except.instMonad, Except.bind] at success
                cases success
                exact ExprSynthesizes.not h
        | enumIs attrName variantName =>
            simp only at success
            cases attrFound : scope.schema.lookupAttribute attrName with
            | none => simp [attrFound, pathError] at success
            | some attr =>
                rw [attrFound] at success
                simp only at success
                split at success <;> rename_i shapeEq
                · rename_i enumSchema
                  cases variantFound : enumSchema.lookup scope.schema attr shapeEq variantName with
                  | none =>
                      rw [variantFound] at success
                      simp [pathError] at success
                  | some variant =>
                      rw [variantFound] at success
                      cases success
                      simpa [scope.schema.attributeLookup_name attrFound,
                        enumSchema.enumLookup_name scope.schema attr shapeEq variantFound] using
                        ExprSynthesizes.enumIs (Γ := Γ) attr enumSchema variant
                · simp [pathError] at success
        | input portName aggregate =>
            simp only at success
            cases portFound : Γ.inputs.lookup portName with
            | none => simp [portFound, pathError] at success
            | some port =>
                rw [portFound] at success
                simp only at success
                cases aggregateSuccess : synthAggregateFuel fuel Γ port aggregate
                    (path ++ [.aggregate]) with
                | error error =>
                    rw [aggregateSuccess] at success
                    simp only [Bind.bind, Monad.toBind, Except.instMonad, Except.bind] at success
                    cases success
                | ok checked =>
                    rw [aggregateSuccess] at success
                    have haggregate := ih.synthAggregate aggregateSuccess
                    simp only [Bind.bind, Monad.toBind, Except.instMonad, Except.bind] at success
                    cases checked with
                    | mk sort payload =>
                        rcases payload with ⟨checkedAggregate, checkedOrigin⟩
                        cases checkedOrigin <;> simp [pathError] at success
                        · cases success
                          simpa [Γ.inputs.lookup_name portFound] using
                            ExprSynthesizes.input (Γ := Γ) (scope := scope) port haggregate
                        · cases success
                          simpa [Γ.inputs.lookup_name portFound] using
                            ExprSynthesizes.input (Γ := Γ) (scope := scope) port haggregate
        | agg rawOp tableName fkName selfFkName rawFilter =>
            exact ih.relational success
      · intro Γ scope raw expected origin path result success
        rw [checkExprFuel.eq_def (path := path)] at success
        rw [dif_pos (Nat.succ_pos fuel)] at success
        simp only [Nat.succ_sub_one] at success
        split at success
        · rename_i rawIgnored variantName
          cases origin with
          | enum attr enumSchema shapeEq =>
              simp only at success
              cases variantFound : enumSchema.lookup scope.schema attr shapeEq variantName with
              | none =>
                  rw [variantFound] at success
                  simp [pathError] at success
              | some variant =>
                  rw [variantFound] at success
                  cases success
                  simpa [enumSchema.enumLookup_name scope.schema attr shapeEq variantFound] using
                    ExprChecks.enum (Γ := Γ) attr enumSchema shapeEq variant
          | real => simp [pathError] at success
          | int => simp [pathError] at success
          | bool => simp [pathError] at success
          | ref target => simp [pathError] at success
        · cases actualSuccess : synthExprFuel fuel Γ scope raw path with
          | error error =>
              rw [actualSuccess] at success
              simp only [Bind.bind, Monad.toBind, Except.instMonad, Except.bind] at success
              cases success
          | ok actual =>
              rw [actualSuccess] at success
              have hactual := ih.synthExpr actualSuccess
              simp only [Bind.bind, Monad.toBind, Except.instMonad, Except.bind] at success
              cases actual with
              | mk actualSort actualExpr actualOrigin =>
                  cases sameFound : sameOriginSort? actualOrigin origin with
                  | none =>
                      rw [sameFound] at success
                      cases origin <;> simp [pathError] at success
                  | some lifted =>
                      rw [sameFound] at success
                      cases lifted with
                      | up same =>
                          cases same
                          cases success
                          exact ExprChecks.synth hactual
      · intro Γ scope raw inputBoundary path result success
        rw [synthAggOpFuel.eq_def (path := path)] at success
        rw [dif_pos (Nat.succ_pos fuel)] at success
        simp only [Nat.succ_sub_one] at success
        cases raw with
        | count =>
            cases success
            exact AggOpSynthesizes.count
        | sum value =>
            simp only at success
            split at success
            · simp [pathError] at success
            · cases checkedSuccess : synthExprFuel fuel Γ scope value
                  (path ++ [.aggregateValue]) with
              | error error =>
                  rw [checkedSuccess] at success
                  simp only [Bind.bind, Monad.toBind, Except.instMonad, Except.bind] at success
                  cases success
              | ok checked =>
                  rw [checkedSuccess] at success
                  have hchecked := ih.synthExpr checkedSuccess
                  simp only [Bind.bind, Monad.toBind, Except.instMonad, Except.bind] at success
                  cases numericFound : numericOfResult checked with
                  | none =>
                      rw [numericFound] at success
                      simp [pathError] at success
                  | some numeric =>
                      rw [numericFound] at success
                      have hn := numericOfResult_sound hchecked numericFound
                      cases hn with
                      | real h =>
                          cases success
                          exact AggOpSynthesizes.sumReal h
                      | int h =>
                          cases success
                          exact AggOpSynthesizes.sumInt h
      · intro Γ port raw path result success
        rw [synthAggregateFuel.eq_def (path := path)] at success
        rw [dif_pos (Nat.succ_pos fuel)] at success
        simp only [Nat.succ_sub_one] at success
        cases raw with
        | mk op filter =>
            simp only at success
            cases opSuccess : synthAggOpFuel fuel Γ (.input port) op true path with
            | error error =>
                rw [opSuccess] at success
                simp only [Bind.bind, Monad.toBind, Except.instMonad, Except.bind] at success
                cases success
            | ok checkedOp =>
                rw [opSuccess] at success
                have hop := ih.synthAggOp opSuccess
                have operationFree := synthAggOpFuel_input_free opSuccess
                simp only [Bind.bind, Monad.toBind, Except.instMonad, Except.bind] at success
                cases checkedOp with
                | mk sort payload =>
                    rcases payload with ⟨opTerm, opOrigin⟩
                    cases filter with
                    | none =>
                        cases success
                        exact AggregateSynthesizes.unfiltered operationFree hop
                    | some rawFilter =>
                        simp only at success
                        split at success <;> rename_i filterCondition
                        · simp [pathError] at success
                        · have filterFree : rawExprContainsAggregate rawFilter = false := by
                            cases filterEq : rawExprContainsAggregate rawFilter
                            · rfl
                            · exact False.elim (filterCondition filterEq)
                          cases filterSuccess : checkExprFuel fuel Γ (.input port) rawFilter
                              .bool .bool (path ++ [.aggregateFilter]) with
                          | error error =>
                              rw [filterSuccess] at success
                              simp only [Bind.bind, Monad.toBind, Except.instMonad,
                                Except.bind] at success
                              cases success
                          | ok checkedFilter =>
                              rw [filterSuccess] at success
                              have hfilter := ih.checkExpr filterSuccess
                              simp only [Bind.bind, Monad.toBind, Except.instMonad,
                                Except.bind] at success
                              cases success
                              exact AggregateSynthesizes.filtered operationFree filterFree
                                hop hfilter
      · intro Γ scope isEq lhs rhs path result success
        rw [checkEqualityFuel.eq_def] at success
        rw [dif_pos (Nat.succ_pos fuel)] at success
        simp only [Nat.succ_sub_one] at success
        split at success
        · simp [pathError] at success
        · rename_i lhsOriginal rhsOriginal variantName notEnum
          cases rightSuccess : synthExprFuel fuel Γ scope rhs (path ++ [.rhs]) with
          | error error =>
              rw [rightSuccess] at success
              simp only [Bind.bind, Monad.toBind, Except.instMonad, Except.bind] at success
              cases success
          | ok right =>
              rw [rightSuccess] at success
              have hrightRaw := ih.synthExpr rightSuccess
              simp only [Bind.bind, Monad.toBind, Except.instMonad, Except.bind] at success
              cases enumFound : enumOfResult right with
              | none =>
                  rw [enumFound] at success
                  simp [pathError] at success
              | some anchored =>
                  rw [enumFound] at success
                  simp only at success
                  have hright := enumOfResult_sound hrightRaw enumFound
                  cases leftSuccess : checkExprFuel fuel Γ scope (.enum variantName)
                      (.enum scope.schema anchored.attr anchored.enumSchema anchored.shapeEq)
                      (.enum anchored.attr anchored.enumSchema anchored.shapeEq)
                      (path ++ [.lhs]) with
                  | error error =>
                      rw [leftSuccess] at success
                      simp only [Bind.bind, Monad.toBind, Except.instMonad,
                        Except.bind] at success
                      cases success
                  | ok left =>
                      rw [leftSuccess] at success
                      have hleft := ih.checkExpr leftSuccess
                      simp only [Bind.bind, Monad.toBind, Except.instMonad,
                        Except.bind] at success
                      cases isEq with
                      | false =>
                          simp only [Bool.false_eq_true, if_false] at success
                          cases success
                          cases hleft with
                          | synth impossible => cases impossible
                          | enum =>
                              rename_i enumShape leftVariant
                              simpa [equalityRaw] using
                                ExprSynthesizes.neEnumLeft anchored.attr anchored.enumSchema
                                  anchored.shapeEq leftVariant hright
                      | true =>
                          simp only [if_pos rfl] at success
                          cases success
                          cases hleft with
                          | synth impossible => cases impossible
                          | enum =>
                              rename_i enumShape leftVariant
                              simpa [equalityRaw] using
                                ExprSynthesizes.eqEnumLeft anchored.attr anchored.enumSchema
                                  anchored.shapeEq leftVariant hright
        · rename_i lhsOriginal rhsOriginal variantName lhsNotEnum bothNotEnum
          cases leftSuccess : synthExprFuel fuel Γ scope lhs (path ++ [.lhs]) with
          | error error =>
              rw [leftSuccess] at success
              simp only [Bind.bind, Monad.toBind, Except.instMonad, Except.bind] at success
              cases success
          | ok left =>
              rw [leftSuccess] at success
              have hleftRaw := ih.synthExpr leftSuccess
              simp only [Bind.bind, Monad.toBind, Except.instMonad, Except.bind] at success
              cases enumFound : enumOfResult left with
              | none =>
                  rw [enumFound] at success
                  simp [pathError] at success
              | some anchored =>
                  rw [enumFound] at success
                  simp only at success
                  have hleft := enumOfResult_sound hleftRaw enumFound
                  cases rightSuccess : checkExprFuel fuel Γ scope (.enum variantName)
                      (.enum scope.schema anchored.attr anchored.enumSchema anchored.shapeEq)
                      (.enum anchored.attr anchored.enumSchema anchored.shapeEq)
                      (path ++ [.rhs]) with
                  | error error =>
                      rw [rightSuccess] at success
                      simp only [Bind.bind, Monad.toBind, Except.instMonad,
                        Except.bind] at success
                      cases success
                  | ok right =>
                      rw [rightSuccess] at success
                      have hright := ih.checkExpr rightSuccess
                      simp only [Bind.bind, Monad.toBind, Except.instMonad,
                        Except.bind] at success
                      cases isEq with
                      | false =>
                          simp only [Bool.false_eq_true, if_false] at success
                          cases success
                          cases hright with
                          | synth impossible => cases impossible
                          | enum =>
                              rename_i enumShape rightVariant
                              simpa [equalityRaw] using
                                ExprSynthesizes.neEnumRight anchored.attr anchored.enumSchema
                                  anchored.shapeEq hleft rightVariant
                      | true =>
                          simp only [if_pos rfl] at success
                          cases success
                          cases hright with
                          | synth impossible => cases impossible
                          | enum =>
                              rename_i enumShape rightVariant
                              simpa [equalityRaw] using
                                ExprSynthesizes.eqEnumRight anchored.attr anchored.enumSchema
                                  anchored.shapeEq hleft rightVariant
        · cases leftSuccess : synthExprFuel fuel Γ scope lhs (path ++ [.lhs]) with
          | error error =>
              rw [leftSuccess] at success
              simp only [Bind.bind, Monad.toBind, Except.instMonad, Except.bind] at success
              cases success
          | ok left =>
              rw [leftSuccess] at success
              have hleft := ih.synthExpr leftSuccess
              cases rightSuccess : synthExprFuel fuel Γ scope rhs (path ++ [.rhs]) with
              | error error =>
                  rw [rightSuccess] at success
                  simp only [Bind.bind, Monad.toBind, Except.instMonad,
                    Except.bind] at success
                  cases success
              | ok right =>
                  rw [rightSuccess] at success
                  have hright := ih.synthExpr rightSuccess
                  simp only [Bind.bind, Monad.toBind, Except.instMonad,
                    Except.bind] at success
                  cases leftFound : numericOfResult left with
                  | none =>
                      rw [leftFound] at success
                      cases left with
                      | mk leftSort leftExpr leftOrigin =>
                          cases right with
                          | mk rightSort rightExpr rightOrigin =>
                              simp only at success
                              cases sameFound : sameOriginSort? leftOrigin rightOrigin with
                              | none =>
                                  rw [sameFound] at success
                                  simp [pathError] at success
                              | some lifted =>
                                  rw [sameFound] at success
                                  cases lifted with
                                  | up same =>
                                      cases same
                                      cases isEq with
                                      | false =>
                                          simp only [Bool.false_eq_true, if_false] at success
                                          cases success
                                          simpa [equalityRaw] using
                                            ExprSynthesizes.neSame hleft hright
                                      | true =>
                                          simp only [if_pos rfl] at success
                                          cases success
                                          simpa [equalityRaw] using
                                            ExprSynthesizes.eqSame hleft hright
                  | some leftNumeric =>
                      rw [leftFound] at success
                      cases rightFound : numericOfResult right with
                      | none =>
                          rw [rightFound] at success
                          cases left with
                          | mk leftSort leftExpr leftOrigin =>
                              cases right with
                              | mk rightSort rightExpr rightOrigin =>
                                  simp only at success
                                  cases sameFound : sameOriginSort? leftOrigin rightOrigin with
                                  | none =>
                                      rw [sameFound] at success
                                      simp [pathError] at success
                                  | some lifted =>
                                      rw [sameFound] at success
                                      cases lifted with
                                      | up same =>
                                          cases same
                                          cases isEq with
                                          | false =>
                                              simp only [Bool.false_eq_true, if_false] at success
                                              cases success
                                              simpa [equalityRaw] using
                                                ExprSynthesizes.neSame hleft hright
                                          | true =>
                                              simp only [if_pos rfl] at success
                                              cases success
                                              simpa [equalityRaw] using
                                                ExprSynthesizes.eqSame hleft hright
                      | some rightNumeric =>
                          rw [rightFound] at success
                          have hnl := numericOfResult_sound hleft leftFound
                          have hnr := numericOfResult_sound hright rightFound
                          cases hnl with
                          | real hl =>
                              cases hnr with
                              | real hr =>
                                  cases isEq with
                                  | false =>
                                      simp [promotePair] at success
                                      cases success
                                      simpa [equalityRaw] using
                                        ExprSynthesizes.neSame hl hr
                                  | true =>
                                      simp [promotePair] at success
                                      cases success
                                      simpa [equalityRaw] using
                                        ExprSynthesizes.eqSame hl hr
                              | int hr =>
                                  cases isEq with
                                  | false =>
                                      simp [promotePair] at success
                                      cases success
                                      simpa [equalityRaw] using
                                        ExprSynthesizes.neRealInt hl hr
                                  | true =>
                                      simp [promotePair] at success
                                      cases success
                                      simpa [equalityRaw] using
                                        ExprSynthesizes.eqRealInt hl hr
                          | int hl =>
                              cases hnr with
                              | real hr =>
                                  cases isEq with
                                  | false =>
                                      simp [promotePair] at success
                                      cases success
                                      simpa [equalityRaw] using
                                        ExprSynthesizes.neIntReal hl hr
                                  | true =>
                                      simp [promotePair] at success
                                      cases success
                                      simpa [equalityRaw] using
                                        ExprSynthesizes.eqIntReal hl hr
                              | int hr =>
                                  cases isEq with
                                  | false =>
                                      simp [promotePair] at success
                                      cases success
                                      simpa [equalityRaw] using
                                        ExprSynthesizes.neSame hl hr
                                  | true =>
                                      simp [promotePair] at success
                                      cases success
                                      simpa [equalityRaw] using
                                        ExprSynthesizes.eqSame hl hr
      · intro Γ scope kind lhs rhs path result success
        rw [checkOrderingFuel.eq_def] at success
        rw [dif_pos (Nat.succ_pos fuel)] at success
        simp only [Nat.succ_sub_one] at success
        cases leftSuccess : synthExprFuel fuel Γ scope lhs (path ++ [.lhs]) with
        | error error =>
            rw [leftSuccess] at success
            simp only [Bind.bind, Monad.toBind, Except.instMonad, Except.bind] at success
            cases success
        | ok left =>
            rw [leftSuccess] at success
            have hleft := ih.synthExpr leftSuccess
            cases rightSuccess : synthExprFuel fuel Γ scope rhs (path ++ [.rhs]) with
            | error error =>
                rw [rightSuccess] at success
                simp only [Bind.bind, Monad.toBind, Except.instMonad, Except.bind] at success
                cases success
            | ok right =>
                rw [rightSuccess] at success
                have hright := ih.synthExpr rightSuccess
                simp only [Bind.bind, Monad.toBind, Except.instMonad, Except.bind] at success
                cases leftFound : numericOfResult left with
                | none =>
                    rw [leftFound] at success
                    simp [pathError] at success
                | some leftNumeric =>
                    rw [leftFound] at success
                    cases rightFound : numericOfResult right with
                    | none =>
                        rw [rightFound] at success
                        simp [pathError] at success
                    | some rightNumeric =>
                        rw [rightFound] at success
                        have hnl := numericOfResult_sound hleft leftFound
                        have hnr := numericOfResult_sound hright rightFound
                        cases hnl with
                        | real hl =>
                            cases hnr with
                            | real hr =>
                                cases kind with
                                | lt =>
                                    simp [promotePair] at success
                                    cases success
                                    exact ExprSynthesizes.ltReal hl hr
                                | le =>
                                    simp [promotePair] at success
                                    cases success
                                    exact ExprSynthesizes.leReal hl hr
                                | gt =>
                                    simp [promotePair] at success
                                    cases success
                                    exact ExprSynthesizes.gtReal hl hr
                                | ge =>
                                    simp [promotePair] at success
                                    cases success
                                    exact ExprSynthesizes.geReal hl hr
                            | int hr =>
                                cases kind with
                                | lt =>
                                    simp [promotePair] at success
                                    cases success
                                    exact ExprSynthesizes.ltRealInt hl hr
                                | le =>
                                    simp [promotePair] at success
                                    cases success
                                    exact ExprSynthesizes.leRealInt hl hr
                                | gt =>
                                    simp [promotePair] at success
                                    cases success
                                    exact ExprSynthesizes.gtRealInt hl hr
                                | ge =>
                                    simp [promotePair] at success
                                    cases success
                                    exact ExprSynthesizes.geRealInt hl hr
                        | int hl =>
                            cases hnr with
                            | real hr =>
                                cases kind with
                                | lt =>
                                    simp [promotePair] at success
                                    cases success
                                    exact ExprSynthesizes.ltIntReal hl hr
                                | le =>
                                    simp [promotePair] at success
                                    cases success
                                    exact ExprSynthesizes.leIntReal hl hr
                                | gt =>
                                    simp [promotePair] at success
                                    cases success
                                    exact ExprSynthesizes.gtIntReal hl hr
                                | ge =>
                                    simp [promotePair] at success
                                    cases success
                                    exact ExprSynthesizes.geIntReal hl hr
                            | int hr =>
                                cases kind with
                                | lt =>
                                    simp [promotePair] at success
                                    cases success
                                    exact ExprSynthesizes.ltInt hl hr
                                | le =>
                                    simp [promotePair] at success
                                    cases success
                                    exact ExprSynthesizes.leInt hl hr
                                | gt =>
                                    simp [promotePair] at success
                                    cases success
                                    exact ExprSynthesizes.gtInt hl hr
                                | ge =>
                                    simp [promotePair] at success
                                    cases success
                                    exact ExprSynthesizes.geInt hl hr
      · intro Γ scope rawOp tableName fkName selfFkName rawFilter path result success
        rw [checkRelationalAggregateFuel.eq_def] at success
        rw [dif_pos (Nat.succ_pos fuel)] at success
        simp only [Nat.succ_sub_one] at success
        let box := scope.owner.box
        cases tableFound : Γ.model.catalog.lookupTable box tableName with
        | none =>
            rw [tableFound] at success
            simp only [pathError, Pure.pure, Bind.bind, Monad.toBind,
              Except.instMonad, Except.pure, Except.bind] at success
            cases success
        | some table =>
            rw [tableFound] at success
            simp only [Pure.pure, Bind.bind, Monad.toBind, Except.instMonad,
              Except.pure, Except.bind] at success
            let related : TableTarget Γ.model.catalog := scope.relatedTarget table
            let relatedScope : RowScope Γ.model Γ.current Γ.inputs := .table related
            cases fkFound : (Γ.model.schemaFor related).lookupAttribute fkName with
            | none =>
                rw [fkFound] at success
                simp only [pathError, Pure.pure, Bind.bind, Monad.toBind,
                  Except.instMonad, Except.pure, Except.bind] at success
                cases success
            | some fkId =>
                rw [fkFound] at success
                simp only [Pure.pure, Bind.bind, Monad.toBind, Except.instMonad,
                  Except.pure, Except.bind] at success
                split at success <;> rename_i fkShape
                · rename_i joinTarget
                  cases selfFound : scope.schema.lookupAttribute selfFkName with
                  | none =>
                      rw [selfFound] at success
                      simp only [pathError, Pure.pure, Bind.bind, Monad.toBind,
                        Except.instMonad, Except.pure, Except.bind] at success
                      cases success
                  | some selfFkId =>
                      rw [selfFound] at success
                      simp only [Pure.pure, Bind.bind, Monad.toBind, Except.instMonad,
                        Except.pure, Except.bind] at success
                      split at success <;> rename_i selfShape
                      · rename_i selfTarget
                        split at success <;> rename_i targetSame
                        · have sameTarget : joinTarget = selfTarget :=
                            tableId_eq_of_val_eq targetSame
                          let fk : ReferenceAttributeId (Γ.model.schemaFor related)
                              joinTarget :=
                            { id := fkId
                              shapeEq := fkShape
                              sortEq := attributeSort_ref_of_shape fkId joinTarget fkShape }
                          let selfFkOriginal : ReferenceAttributeId scope.schema selfTarget :=
                            { id := selfFkId
                              shapeEq := selfShape
                              sortEq := attributeSort_ref_of_shape selfFkId selfTarget
                                selfShape }
                          let selfFk : ReferenceAttributeId scope.schema joinTarget :=
                            Eq.mp (congrArg
                              (fun target => ReferenceAttributeId scope.schema target)
                              sameTarget.symm) selfFkOriginal
                          have selfFkIdExact : selfFk.id = selfFkId := by
                            dsimp [selfFk]
                            cases sameTarget
                            rfl
                          cases opSuccess : synthAggOpFuel fuel Γ relatedScope rawOp false
                              (path ++ [.aggregateValue]) with
                          | error error =>
                              rw [opSuccess] at success
                              simp only [Bind.bind, Monad.toBind, Except.instMonad,
                                Except.bind] at success
                              cases success
                          | ok checkedOp =>
                              rw [opSuccess] at success
                              have hop := ih.synthAggOp opSuccess
                              cases filterSuccess : checkExprFuel fuel Γ relatedScope rawFilter
                                  .bool .bool (path ++ [.aggregateFilter]) with
                              | error error =>
                                  rw [filterSuccess] at success
                                  simp only [Bind.bind, Monad.toBind, Except.instMonad,
                                    Except.bind] at success
                                  cases success
                              | ok checkedFilter =>
                                  rw [filterSuccess] at success
                                  have hfilter := ih.checkExpr filterSuccess
                                  simp only [Bind.bind, Monad.toBind, Except.instMonad,
                                    Except.bind] at success
                                  cases checkedOp with
                                  | mk sort payload =>
                                      rcases payload with ⟨opTerm, opOrigin⟩
                                      cases opOrigin <;> simp [pathError] at success
                                      · cases success
                                        rw [← Γ.model.catalog.tableLookup_name box tableFound,
                                          ← (Γ.model.schemaFor related).attributeLookup_name fkFound,
                                          ← scope.schema.attributeLookup_name selfFound]
                                        simpa only [related, box, RowScope.relatedTarget, fk,
                                          selfFkIdExact] using
                                          ExprSynthesizes.agg (Γ := Γ) table joinTarget fk
                                            selfFk hop hfilter
                                      · cases success
                                        rw [← Γ.model.catalog.tableLookup_name box tableFound,
                                          ← (Γ.model.schemaFor related).attributeLookup_name fkFound,
                                          ← scope.schema.attributeLookup_name selfFound]
                                        simpa only [related, box, RowScope.relatedTarget, fk,
                                          selfFkIdExact] using
                                          ExprSynthesizes.agg (Γ := Γ) table joinTarget fk
                                            selfFk hop hfilter
                        · simp [pathError] at success
                      · simp [pathError] at success
                · simp [pathError] at success

/-! ## Declarative completeness -/

@[simp] private theorem OrderedContext.lookupOrdinal_nameAt_self
    {α : Type} {nameOf : α → String} (ctx : OrderedContext α nameOf)
    (index : Fin ctx.entries.length) :
    ctx.lookupOrdinal (ctx.nameAt index) = some index := by
  obtain ⟨found, hfound⟩ := ctx.lookupOrdinal_complete (wanted := ctx.nameAt index)
    (List.mem_map.mpr ⟨ctx.entries.get index,
      List.get_mem ctx.entries index.val index.isLt, rfl⟩)
  have same : found = index := ctx.nameAt_injective (ctx.lookupOrdinal_name hfound)
  simpa [same] using hfound

@[simp] private theorem ParamContext.lookup_name_self (ctx : ParamContext)
    (id : ParameterId ctx) : ctx.lookup (ctx.name id) = some id := by
  cases id with
  | mk ordinal =>
      simp only [ParamContext.lookup, ParamContext.name]
      rw [OrderedContext.lookupOrdinal_nameAt_self]
      rfl

@[simp] theorem TableSchema.lookupAttribute_name_self
    {catalog : SchemaUniverse} {owner : TableTarget catalog}
    (schema : TableSchema catalog owner) (attr : AttributeId schema) :
    schema.lookupAttribute (schema.attributeName attr) = some attr := by
  cases attr with
  | mk ordinal =>
      simp only [TableSchema.lookupAttribute, TableSchema.attributeName,
        TableSchema.attr]
      change Option.map AttributeId.mk
        (schema.attributes.lookupOrdinal (schema.attributes.nameAt ordinal)) = _
      rw [OrderedContext.lookupOrdinal_nameAt_self]
      rfl

@[simp] theorem SchemaUniverse.lookupTable_name_self
    (catalog : SchemaUniverse) (box : BoxId catalog)
    (table : TableId catalog box) :
    catalog.lookupTable box (catalog.tableName ⟨box, table⟩) = some table := by
  cases table with
  | mk ordinal =>
      simp only [SchemaUniverse.lookupTable, SchemaUniverse.tableName,
        SchemaUniverse.tableHeader]
      change Option.map TableId.mk ((catalog.boxHeader box).tables.lookupOrdinal
        ((catalog.boxHeader box).tables.nameAt ordinal)) = _
      rw [OrderedContext.lookupOrdinal_nameAt_self]
      rfl

@[simp] theorem SchemaUniverse.lookupBox_name_self
    (catalog : SchemaUniverse) (box : BoxId catalog) :
    catalog.lookupBox (catalog.boxName box) = some box := by
  cases box with
  | mk ordinal =>
      simp only [SchemaUniverse.lookupBox, SchemaUniverse.boxName,
        SchemaUniverse.boxHeader]
      change Option.map BoxId.mk (catalog.boxes.lookupOrdinal
        (catalog.boxes.nameAt ordinal)) = _
      rw [OrderedContext.lookupOrdinal_nameAt_self]
      rfl

@[simp] private theorem InputSignature.lookup_name_self {model : ModelSchema}
    {current : TableTarget model.catalog} (inputs : InputSignature model current)
    (port : InputId inputs) : inputs.lookup (inputs.name port) = some port := by
  cases port with
  | mk ordinal =>
      simp only [InputSignature.lookup, InputSignature.name, InputSignature.get]
      change Option.map InputId.mk (inputs.ports.lookupOrdinal
        (inputs.ports.nameAt ordinal)) = _
      rw [OrderedContext.lookupOrdinal_nameAt_self]
      rfl

@[simp] private theorem EnumSchema.lookup_variantName_self
    {catalog : SchemaUniverse} {owner : TableTarget catalog}
    (schema : TableSchema catalog owner) (attr : AttributeId schema)
    (enumSchema : EnumSchema) (shapeEq : (schema.attr attr).shape = .enum enumSchema)
    (variant : VariantId schema attr enumSchema) :
    enumSchema.lookup schema attr shapeEq
      (enumSchema.variantName schema attr variant) = some variant := by
  obtain ⟨found, hfound⟩ := OrderedContext.findNameIndex_complete
    (names := enumSchema.variants)
    (wanted := enumSchema.variantName schema attr variant) (by
      exact List.get_mem enumSchema.variants variant.ordinal.val variant.ordinal.isLt)
  have foundName := OrderedContext.findNameIndex_sound hfound
  have ordinalSame : found = variant.ordinal :=
    enumSchema.uniqueVariants.get_inj_iff.mp (by
      simpa [EnumSchema.variantName] using foundName)
  cases variant with
  | mk variantShape ordinal =>
      simp only at ordinalSame
      subst found
      unfold EnumSchema.lookup
      simpa [EnumSchema.variantName] using hfound

@[simp] private theorem numericOfResult_real {Γ : TermContext}
    {scope : RowScope Γ.model Γ.current Γ.inputs}
    (expr : Expr Γ.model Γ.current Γ.inputs scope .real)
    (origin : SortOrigin Γ scope .real) :
    numericOfResult ⟨.real, expr, origin⟩ = some (.real expr) := by cases origin; rfl

@[simp] private theorem numericOfResult_int {Γ : TermContext}
    {scope : RowScope Γ.model Γ.current Γ.inputs}
    (expr : Expr Γ.model Γ.current Γ.inputs scope .int)
    (origin : SortOrigin Γ scope .int) :
    numericOfResult ⟨.int, expr, origin⟩ = some (.int expr) := by cases origin; rfl

@[simp] private theorem numericOfResult_bool {Γ : TermContext}
    {scope : RowScope Γ.model Γ.current Γ.inputs}
    (expr : Expr Γ.model Γ.current Γ.inputs scope .bool)
    (origin : SortOrigin Γ scope .bool) :
    numericOfResult ⟨.bool, expr, origin⟩ = none := by cases origin; rfl

@[simp] private theorem numericOfResult_enum {Γ : TermContext}
    {scope : RowScope Γ.model Γ.current Γ.inputs}
    (attr : AttributeId scope.schema) (enumSchema : EnumSchema)
    (shapeEq : (scope.schema.attr attr).shape = .enum enumSchema)
    (expr : Expr Γ.model Γ.current Γ.inputs scope
      (.enum scope.schema attr enumSchema shapeEq))
    (origin : SortOrigin Γ scope (.enum scope.schema attr enumSchema shapeEq)) :
    numericOfResult ⟨_, expr, origin⟩ = none := by cases origin; rfl

@[simp] private theorem numericOfResult_ref {Γ : TermContext}
    {scope : RowScope Γ.model Γ.current Γ.inputs}
    (target : TableTarget Γ.model.catalog)
    (expr : Expr Γ.model Γ.current Γ.inputs scope (.ref target))
    (origin : SortOrigin Γ scope (.ref target)) :
    numericOfResult ⟨_, expr, origin⟩ = none := by cases origin; rfl

@[simp] private theorem enumOfResult_enum {Γ : TermContext}
    {scope : RowScope Γ.model Γ.current Γ.inputs}
    (attr : AttributeId scope.schema) (enumSchema : EnumSchema)
    (shapeEq : (scope.schema.attr attr).shape = .enum enumSchema)
    (expr : Expr Γ.model Γ.current Γ.inputs scope
      (.enum scope.schema attr enumSchema shapeEq)) :
    enumOfResult ⟨_, expr, .enum attr enumSchema shapeEq⟩ =
      some ⟨attr, enumSchema, shapeEq, expr⟩ := rfl

@[simp] private theorem promotePair_real_real {Γ : TermContext}
    {scope : RowScope Γ.model Γ.current Γ.inputs}
    (left right : Expr Γ.model Γ.current Γ.inputs scope .real) :
    promotePair (.real left) (.real right) = ⟨.real, left, right⟩ := rfl

@[simp] private theorem promotePair_real_int {Γ : TermContext}
    {scope : RowScope Γ.model Γ.current Γ.inputs}
    (left : Expr Γ.model Γ.current Γ.inputs scope .real)
    (right : Expr Γ.model Γ.current Γ.inputs scope .int) :
    promotePair (.real left) (.int right) = ⟨.real, left, .intToReal right⟩ := rfl

@[simp] private theorem promotePair_int_real {Γ : TermContext}
    {scope : RowScope Γ.model Γ.current Γ.inputs}
    (left : Expr Γ.model Γ.current Γ.inputs scope .int)
    (right : Expr Γ.model Γ.current Γ.inputs scope .real) :
    promotePair (.int left) (.real right) = ⟨.real, .intToReal left, right⟩ := rfl

@[simp] private theorem promotePair_int_int {Γ : TermContext}
    {scope : RowScope Γ.model Γ.current Γ.inputs}
    (left right : Expr Γ.model Γ.current Γ.inputs scope .int) :
    promotePair (.int left) (.int right) = ⟨.int, left, right⟩ := rfl

@[simp] private theorem toReal_real {Γ : TermContext}
    {scope : RowScope Γ.model Γ.current Γ.inputs}
    (expr : Expr Γ.model Γ.current Γ.inputs scope .real) :
    toReal (.real expr) = expr := rfl

@[simp] private theorem toReal_int {Γ : TermContext}
    {scope : RowScope Γ.model Γ.current Γ.inputs}
    (expr : Expr Γ.model Γ.current Γ.inputs scope .int) :
    toReal (.int expr) = .intToReal expr := rfl

@[simp] private theorem sameOriginSort_bool {Γ : TermContext}
    {scope : RowScope Γ.model Γ.current Γ.inputs} :
    sameOriginSort?
      (SortOrigin.bool : SortOrigin Γ scope .bool) .bool = some ⟨rfl⟩ := rfl

@[simp] private theorem sameOriginSort_enum {Γ : TermContext}
    {scope : RowScope Γ.model Γ.current Γ.inputs}
    (attr : AttributeId scope.schema) (enumSchema : EnumSchema)
    (shapeEq : (scope.schema.attr attr).shape = .enum enumSchema) :
    sameOriginSort? (SortOrigin.enum attr enumSchema shapeEq)
      (SortOrigin.enum attr enumSchema shapeEq) = some ⟨rfl⟩ := by
  simp [sameOriginSort?, attributeEq?]

@[simp] private theorem sameOriginSort_ref {Γ : TermContext}
    {scope : RowScope Γ.model Γ.current Γ.inputs}
    (target : TableTarget Γ.model.catalog) :
    sameOriginSort? (SortOrigin.ref target : SortOrigin Γ scope (.ref target))
      (SortOrigin.ref target) = some ⟨rfl⟩ := by
  simp [sameOriginSort?, tableTargetEq?]

theorem sameOriginSort_complete {Γ : TermContext}
    {scope : RowScope Γ.model Γ.current Γ.inputs}
    {sort : ScalarSort Γ.model.catalog}
    (left right : SortOrigin Γ scope sort) :
    ∃ same : PLift (sort = sort), sameOriginSort? left right = some same := by
  cases left with
  | real =>
      cases right
      exact ⟨⟨rfl⟩, rfl⟩
  | int =>
      cases right
      exact ⟨⟨rfl⟩, rfl⟩
  | bool =>
      cases right
      exact ⟨⟨rfl⟩, rfl⟩
  | enum leftAttr leftEnum leftShape =>
      cases right
      exact ⟨⟨rfl⟩, by simp [sameOriginSort?, attributeEq?]⟩
  | ref leftTarget =>
      cases right
      exact ⟨⟨rfl⟩, by simp [sameOriginSort?, tableTargetEq?]⟩

theorem AggOpSynthesizes.sort_numeric {Γ : TermContext}
    {scope : RowScope Γ.model Γ.current Γ.inputs} {raw sort op}
    (h : AggOpSynthesizes Γ scope raw sort op) : sort = .int ∨ sort = .real := by
  cases h with
  | count | sumInt => exact Or.inl rfl
  | sumReal => exact Or.inr rfl

private theorem AggregateSynthesizes.sort_numeric {Γ : TermContext}
    {scope : RowScope Γ.model Γ.current Γ.inputs} {raw sort aggregate}
    (h : AggregateSynthesizes Γ scope raw sort aggregate) :
    sort = .int ∨ sort = .real := by
  cases h with
  | unfiltered aggregateFree hop | filtered _ _ hop _ =>
      cases hop with
      | count | sumInt => exact Or.inl rfl
      | sumReal => exact Or.inr rfl

private theorem ExprSynthesizes.raw_ne_enum {Γ : TermContext}
    {scope : RowScope Γ.model Γ.current Γ.inputs} {raw sort term}
    (h : ExprSynthesizes Γ scope raw sort term) (name : String) : raw ≠ .enum name := by
  intro same
  subst raw
  cases h

set_option maxHeartbeats 500000 in
private theorem declarativeFuel_complete {Γ : TermContext}
    {scope : RowScope Γ.model Γ.current Γ.inputs} {raw sort term}
    (h : ExprSynthesizes Γ scope raw sort term) :
    ∀ fuel path, rawExprDepth raw * 4 + 8 ≤ fuel →
      ∃ origin : SortOrigin Γ scope sort,
        synthExprFuel fuel Γ scope raw path = .ok ⟨sort, term, origin⟩ := by
  apply ExprSynthesizes.rec
    (motive_1 := fun scope raw sort term _ =>
      ∀ fuel path, rawExprDepth raw * 4 + 8 ≤ fuel →
        ∃ origin : SortOrigin Γ scope sort,
          synthExprFuel fuel Γ scope raw path = .ok ⟨sort, term, origin⟩)
    (motive_2 := fun scope expected raw term _ =>
      ∀ fuel path (origin : SortOrigin Γ scope expected),
        rawExprDepth raw * 4 + 9 ≤ fuel →
          checkExprFuel fuel Γ scope raw expected origin path = .ok term)
    (motive_3 := fun scope raw sort op _ =>
      ∀ fuel path inputBoundary,
        rawAggOpDepth raw * 4 + 8 ≤ fuel →
        (inputBoundary = true → rawAggOpContainsAggregate raw = false) →
          ∃ origin : SortOrigin Γ scope sort,
            synthAggOpFuel fuel Γ scope raw inputBoundary path =
              .ok ⟨sort, op, origin⟩)
    (motive_4 := fun scope raw sort aggregate _ =>
      ∀ (port : InputId Γ.inputs) (scopeEq : scope = .input port) fuel path,
        rawAggregateDepth raw * 4 + 8 ≤ fuel →
          ∃ origin : SortOrigin Γ (.input port) sort,
            synthAggregateFuel fuel Γ port raw path =
              .ok ⟨sort, Eq.mp (congrArg (fun scope =>
                Aggregate Γ.model Γ.current Γ.inputs scope sort) scopeEq) aggregate,
                origin⟩)
  case real =>
    intros scope value fuel path bound
    rw [synthExprFuel.eq_def (path := path), dif_pos (by omega)]
    exact ⟨.real, rfl⟩
  case int =>
    intros scope value fuel path bound
    rw [synthExprFuel.eq_def (path := path), dif_pos (by omega)]
    exact ⟨.int, rfl⟩
  case bool =>
    intros scope value fuel path bound
    rw [synthExprFuel.eq_def (path := path), dif_pos (by omega)]
    exact ⟨.bool, rfl⟩
  case param =>
    intros scope id fuel path bound
    rw [synthExprFuel.eq_def (path := path), dif_pos (by omega)]
    simp only [ParamContext.lookup_name_self]
    split <;> rename_i sortEq
    · let scalarEq : (Γ.model.params.get id).sort.scalarSort Γ.model.catalog = .real := by
        simp [sortEq, ParamSort.scalarSort]
      refine ⟨Eq.mpr (congrArg (SortOrigin Γ scope) scalarEq) .real, ?_⟩
      apply congrArg Except.ok
      rw [CheckedExprResult.mk.injEq]
      refine ⟨scalarEq.symm, cast_heq _ _, ?_⟩
      exact (cast_heq _ _).symm
    · let scalarEq : (Γ.model.params.get id).sort.scalarSort Γ.model.catalog = .int := by
        simp [sortEq, ParamSort.scalarSort]
      refine ⟨Eq.mpr (congrArg (SortOrigin Γ scope) scalarEq) .int, ?_⟩
      apply congrArg Except.ok
      rw [CheckedExprResult.mk.injEq]
      refine ⟨scalarEq.symm, cast_heq _ _, ?_⟩
      exact (cast_heq _ _).symm
  case selfAttr =>
    intros scope attr fuel path bound
    rw [synthExprFuel.eq_def (path := path), dif_pos (by omega)]
    simp only [TableSchema.lookupAttribute_name_self]
    exact ⟨SortOrigin.ofAttribute Γ attr, rfl⟩
  case addInt | subInt | mulInt =>
      intros scope leftRaw rightRaw left right hl hr ihl ihr fuel path bound
      rw [synthExprFuel.eq_def (path := path), dif_pos (by omega)]
      simp only [Nat.succ_sub_one]
      simp only [rawExprDepth] at bound
      obtain ⟨leftOrigin, leftOk⟩ := ihl (fuel - 1) (path ++ [.lhs]) (by omega)
      obtain ⟨rightOrigin, rightOk⟩ := ihr (fuel - 1) (path ++ [.rhs]) (by omega)
      refine ⟨.int, ?_⟩
      rw [leftOk, rightOk]
      cases leftOrigin
      cases rightOrigin
      rfl
  case addReal | addIntReal | addRealInt | subReal | subIntReal | subRealInt
    | mulReal | mulIntReal | mulRealInt =>
      intros scope leftRaw rightRaw left right hl hr ihl ihr fuel path bound
      rw [synthExprFuel.eq_def (path := path), dif_pos (by omega)]
      simp only [Nat.succ_sub_one]
      simp only [rawExprDepth] at bound
      obtain ⟨leftOrigin, leftOk⟩ := ihl (fuel - 1) (path ++ [.lhs]) (by omega)
      obtain ⟨rightOrigin, rightOk⟩ := ihr (fuel - 1) (path ++ [.rhs]) (by omega)
      refine ⟨.real, ?_⟩
      rw [leftOk, rightOk]
      cases leftOrigin
      cases rightOrigin
      rfl
  case divReal =>
    intros scope leftRaw rightRaw left right hl hr ihl ihr fuel path bound
    rw [synthExprFuel.eq_def (path := path), dif_pos (by omega)]
    simp only [Nat.succ_sub_one]
    simp only [rawExprDepth] at bound
    obtain ⟨leftOrigin, leftOk⟩ := ihl (fuel - 1) (path ++ [.lhs]) (by omega)
    obtain ⟨rightOrigin, rightOk⟩ := ihr (fuel - 1) (path ++ [.rhs]) (by omega)
    refine ⟨.real, ?_⟩
    rw [leftOk, rightOk]
    cases leftOrigin
    cases rightOrigin
    rfl
  case divIntReal =>
    intros scope leftRaw rightRaw left right hl hr ihl ihr fuel path bound
    rw [synthExprFuel.eq_def (path := path), dif_pos (by omega)]
    simp only [Nat.succ_sub_one]
    simp only [rawExprDepth] at bound
    obtain ⟨leftOrigin, leftOk⟩ := ihl (fuel - 1) (path ++ [.lhs]) (by omega)
    obtain ⟨rightOrigin, rightOk⟩ := ihr (fuel - 1) (path ++ [.rhs]) (by omega)
    refine ⟨.real, ?_⟩
    rw [leftOk, rightOk]
    cases leftOrigin
    cases rightOrigin
    rfl
  case divRealInt =>
    intros scope leftRaw rightRaw left right hl hr ihl ihr fuel path bound
    rw [synthExprFuel.eq_def (path := path), dif_pos (by omega)]
    simp only [Nat.succ_sub_one]
    simp only [rawExprDepth] at bound
    obtain ⟨leftOrigin, leftOk⟩ := ihl (fuel - 1) (path ++ [.lhs]) (by omega)
    obtain ⟨rightOrigin, rightOk⟩ := ihr (fuel - 1) (path ++ [.rhs]) (by omega)
    refine ⟨.real, ?_⟩
    rw [leftOk, rightOk]
    cases leftOrigin
    cases rightOrigin
    rfl
  case divInt =>
    intros scope leftRaw rightRaw left right hl hr ihl ihr fuel path bound
    rw [synthExprFuel.eq_def (path := path), dif_pos (by omega)]
    simp only [Nat.succ_sub_one]
    simp only [rawExprDepth] at bound
    obtain ⟨leftOrigin, leftOk⟩ := ihl (fuel - 1) (path ++ [.lhs]) (by omega)
    obtain ⟨rightOrigin, rightOk⟩ := ihr (fuel - 1) (path ++ [.rhs]) (by omega)
    refine ⟨.real, ?_⟩
    rw [leftOk, rightOk]
    cases leftOrigin
    cases rightOrigin
    rfl
  case eqSame | neSame =>
    intros scope sort leftRaw rightRaw left right hl hr ihl ihr fuel path bound
    rw [synthExprFuel.eq_def (path := path), dif_pos (by omega)]
    simp only [Nat.succ_sub_one]
    rw [checkEqualityFuel.eq_def, dif_pos (by omega)]
    have leftNotEnum := hl.raw_ne_enum
    have rightNotEnum := hr.raw_ne_enum
    split
    case h_1 => simp_all
    case h_2 => simp_all
    case h_3 => simp_all
    case h_4 =>
      simp only [rawExprDepth] at bound
      obtain ⟨leftOrigin, leftOk⟩ := ihl (fuel - 1 - 1) (path ++ [.lhs]) (by omega)
      obtain ⟨rightOrigin, rightOk⟩ := ihr (fuel - 1 - 1) (path ++ [.rhs]) (by omega)
      rw [leftOk, rightOk]
      refine ⟨.bool, ?_⟩
      cases leftOrigin with
      | real =>
          cases rightOrigin
          simp only [numericOfResult_real, promotePair_real_real]
          rfl
      | int =>
          cases rightOrigin
          simp only [numericOfResult_int, promotePair_int_int]
          rfl
      | bool =>
          cases rightOrigin
          simp only [Bind.bind, Except.instMonad, Except.bind,
            numericOfResult_bool, sameOriginSort_bool]
          rfl
      | enum attr enumSchema shapeEq =>
          cases rightOrigin with
          | enum _ _ rightShape =>
              have shapeSame : rightShape = shapeEq := Subsingleton.elim _ _
              rw [shapeSame]
              simp only [Bind.bind, Except.instMonad, Except.bind,
                numericOfResult_enum, sameOriginSort_enum]
              rfl
      | ref target =>
          cases rightOrigin
          simp only [Bind.bind, Except.instMonad, Except.bind,
            numericOfResult_ref, sameOriginSort_ref]
          rfl
  case eqIntReal | eqRealInt | neIntReal | neRealInt =>
    intros scope leftRaw rightRaw left right hl hr ihl ihr fuel path bound
    rw [synthExprFuel.eq_def (path := path), dif_pos (by omega)]
    simp only [Nat.succ_sub_one]
    rw [checkEqualityFuel.eq_def, dif_pos (by omega)]
    have leftNotEnum := hl.raw_ne_enum
    have rightNotEnum := hr.raw_ne_enum
    split
    case h_1 => simp_all
    case h_2 => simp_all
    case h_3 => simp_all
    case h_4 =>
      simp only [rawExprDepth] at bound
      obtain ⟨leftOrigin, leftOk⟩ := ihl (fuel - 1 - 1) (path ++ [.lhs]) (by omega)
      obtain ⟨rightOrigin, rightOk⟩ := ihr (fuel - 1 - 1) (path ++ [.rhs]) (by omega)
      rw [leftOk, rightOk]
      refine ⟨.bool, ?_⟩
      cases leftOrigin
      cases rightOrigin
      rfl
  case eqEnumLeft | neEnumLeft =>
    intros scope rightRaw attr enumSchema shapeEq leftVariant right hr ihr fuel path bound
    rw [synthExprFuel.eq_def (path := path), dif_pos (by omega)]
    simp only [Nat.succ_sub_one]
    rw [checkEqualityFuel.eq_def, dif_pos (by omega)]
    have rightNotEnum := hr.raw_ne_enum
    split
    case h_1 => cases hr
    case h_2 =>
      simp_all only [IR.Expr.enum.injEq]
      subst_vars
      simp only [rawExprDepth] at bound
      obtain ⟨rightOrigin, rightOk⟩ := ihr (fuel - 1 - 1) (path ++ [.rhs]) (by omega)
      rw [rightOk]
      cases rightOrigin with
      | enum _ _ rightShape =>
          have shapeSame : rightShape = shapeEq := Subsingleton.elim _ _
          rw [shapeSame]
          simp only [Bind.bind, Except.instMonad, Except.bind, enumOfResult_enum]
          refine ⟨.bool, ?_⟩
          simp [checkExprFuel.eq_def, EnumSchema.lookup_variantName_self,
            Bind.bind, Pure.pure, Except.instMonad, Except.bind, Except.pure,
            show 0 < fuel - 1 - 1 by omega]
    case h_3 => cases hr
    case h_4 =>
      exfalso
      aesop
  case eqEnumRight | neEnumRight =>
    intros scope leftRaw attr enumSchema shapeEq left hl rightVariant ihl fuel path bound
    rw [synthExprFuel.eq_def (path := path), dif_pos (by omega)]
    simp only [Nat.succ_sub_one]
    rw [checkEqualityFuel.eq_def, dif_pos (by omega)]
    have leftNotEnum := hl.raw_ne_enum
    split
    case h_1 => cases hl
    case h_2 => cases hl
    case h_3 =>
      simp_all only [IR.Expr.enum.injEq]
      subst_vars
      simp only [rawExprDepth] at bound
      obtain ⟨leftOrigin, leftOk⟩ := ihl (fuel - 1 - 1) (path ++ [.lhs]) (by omega)
      rw [leftOk]
      cases leftOrigin with
      | enum _ _ leftShape =>
          have shapeSame : leftShape = shapeEq := Subsingleton.elim _ _
          rw [shapeSame]
          simp only [Bind.bind, Except.instMonad, Except.bind, enumOfResult_enum]
          refine ⟨.bool, ?_⟩
          simp [checkExprFuel.eq_def, EnumSchema.lookup_variantName_self,
            Bind.bind, Pure.pure, Except.instMonad, Except.bind, Except.pure,
            show 0 < fuel - 1 - 1 by omega]
    case h_4 =>
      exfalso
      aesop
  case ltInt | ltReal | ltIntReal | ltRealInt | leInt | leReal | leIntReal
    | leRealInt | gtInt | gtReal | gtIntReal | gtRealInt | geInt | geReal
    | geIntReal | geRealInt =>
      intros scope leftRaw rightRaw left right hl hr ihl ihr fuel path bound
      rw [synthExprFuel.eq_def (path := path), dif_pos (by omega)]
      simp only [Nat.succ_sub_one]
      rw [checkOrderingFuel.eq_def, dif_pos (by omega)]
      simp only [rawExprDepth] at bound
      obtain ⟨leftOrigin, leftOk⟩ := ihl (fuel - 1 - 1) (path ++ [.lhs]) (by omega)
      obtain ⟨rightOrigin, rightOk⟩ := ihr (fuel - 1 - 1) (path ++ [.rhs]) (by omega)
      refine ⟨.bool, ?_⟩
      rw [leftOk, rightOk]
      cases leftOrigin
      cases rightOrigin
      rfl
  case agg =>
    intros scope rawOp rawFilter sort table joinTarget fk selfFk op filter hop hfilter
      ihOp ihFilter fuel path bound
    rw [synthExprFuel.eq_def (path := path), dif_pos (by omega)]
    simp only [Nat.succ_sub_one]
    rw [checkRelationalAggregateFuel.eq_def, dif_pos (by omega)]
    simp only [RowScope.relatedTarget]
    rw [SchemaUniverse.lookupTable_name_self]
    simp only [Bind.bind, Pure.pure, Except.instMonad, Except.bind, Except.pure]
    rw [TableSchema.lookupAttribute_name_self]
    simp only [Bind.bind, Pure.pure, Except.instMonad, Except.bind, Except.pure]
    have fkShapeExact :
        ((Γ.model.schemaFor { box := scope.owner.box, table := table }).attr fk.id).shape =
          .ref joinTarget := by
      simpa [RowScope.relatedTarget] using fk.shapeEq
    split
    case h_1 =>
      rename_i foundTarget foundShape
      have targetSame : foundTarget = joinTarget :=
        AttrShape.ref.inj (foundShape.symm.trans fkShapeExact)
      subst foundTarget
      have foundShapeSame : foundShape = fkShapeExact := Subsingleton.elim _ _
      rw [foundShapeSame]
      rw [TableSchema.lookupAttribute_name_self]
      simp only [Bind.bind, Pure.pure, Except.instMonad, Except.bind, Except.pure]
      have selfShapeExact : (scope.schema.attr selfFk.id).shape = .ref joinTarget :=
        selfFk.shapeEq
      split
      case h_1 =>
        rename_i foundSelfTarget foundSelfShape
        have selfTargetSame : foundSelfTarget = joinTarget :=
          AttrShape.ref.inj (foundSelfShape.symm.trans selfShapeExact)
        subst foundSelfTarget
        have foundSelfShapeSame : foundSelfShape = selfShapeExact := Subsingleton.elim _ _
        rw [foundSelfShapeSame]
        rw [dif_pos rfl]
        simp only [rawExprDepth] at bound
        obtain ⟨opOrigin, opOk⟩ := ihOp (fuel - 1 - 1)
          (path ++ [.aggregateValue]) false (by omega) (by simp)
        have filterOk := ihFilter (fuel - 1 - 1) (path ++ [.aggregateFilter])
          .bool (by omega)
        have opOkExact := opOk
        have filterOkExact := filterOk
        simp only [RowScope.relatedTarget] at opOkExact filterOkExact
        rw [opOkExact, filterOkExact]
        rcases hop.sort_numeric with sortInt | sortReal
        · subst sort
          cases opOrigin
          refine ⟨.int, ?_⟩
          rfl
        · subst sort
          cases opOrigin
          refine ⟨.real, ?_⟩
          rfl
      case h_2 =>
        exfalso
        apply_assumption
        exact selfShapeExact
    case h_2 =>
      exfalso
      apply_assumption
      exact fkShapeExact
  case and | or =>
    intros scope leftRaw rightRaw left right hl hr ihl ihr fuel path bound
    rw [synthExprFuel.eq_def (path := path), dif_pos (by omega)]
    simp only [rawExprDepth, Nat.succ_sub_one] at bound ⊢
    have leftOk := ihl (fuel - 1)
      (path ++ [ModelCheckPathSegment.lhs]) .bool (by omega)
    have rightOk := ihr (fuel - 1)
      (path ++ [ModelCheckPathSegment.rhs]) .bool (by omega)
    rw [leftOk, rightOk]
    exact ⟨.bool, rfl⟩
  case not =>
    intros scope raw value h ih fuel path bound
    rw [synthExprFuel.eq_def (path := path), dif_pos (by omega)]
    simp only [rawExprDepth, Nat.succ_sub_one] at bound ⊢
    have checkedOk := ih (fuel - 1)
      (path ++ [ModelCheckPathSegment.operand]) .bool (by omega)
    rw [checkedOk]
    exact ⟨.bool, rfl⟩
  case enumIs =>
    intros scope attr enumSchema variant fuel path bound
    rw [synthExprFuel.eq_def (path := path), dif_pos (by omega)]
    simp only [TableSchema.lookupAttribute_name_self]
    split
    case h_1 =>
      rename_i foundSchema shapeEq
      have schemaSame : foundSchema = enumSchema :=
        AttrShape.enum.inj (shapeEq.symm.trans variant.shapeEq)
      subst foundSchema
      have shapeSame : shapeEq = variant.shapeEq := Subsingleton.elim _ _
      rw [shapeSame, EnumSchema.lookup_variantName_self]
      exact ⟨.bool, rfl⟩
    case h_2 =>
      exfalso
      apply_assumption
      exact variant.shapeEq
  case input =>
    intros scope raw sort port aggregate hAggregate ih fuel path bound
    rcases hAggregate.sort_numeric with sortInt | sortReal
    · subst sort
      rw [synthExprFuel.eq_def (path := path), dif_pos (by omega)]
      simp only [InputSignature.lookup_name_self, rawExprDepth, Nat.succ_sub_one] at bound ⊢
      obtain ⟨origin, aggregateOk⟩ := ih port rfl (fuel - 1)
        (path ++ [.aggregate]) (by omega)
      rw [aggregateOk]
      cases origin
      refine ⟨.int, ?_⟩
      simp only [Bind.bind, Except.instMonad, Except.bind]
      rfl
    · subst sort
      rw [synthExprFuel.eq_def (path := path), dif_pos (by omega)]
      simp only [InputSignature.lookup_name_self, rawExprDepth, Nat.succ_sub_one] at bound ⊢
      obtain ⟨origin, aggregateOk⟩ := ih port rfl (fuel - 1)
        (path ++ [.aggregate]) (by omega)
      rw [aggregateOk]
      cases origin
      refine ⟨.real, ?_⟩
      simp only [Bind.bind, Except.instMonad, Except.bind]
      rfl
  case synth =>
    intros scope expected raw term hSynth ih fuel path expectedOrigin bound
    rw [checkExprFuel.eq_def (path := path), dif_pos (by omega)]
    split
    · cases hSynth
    · obtain ⟨actualOrigin, actualOk⟩ := ih (fuel - 1) path (by omega)
      rw [actualOk]
      simp only [Bind.bind, Except.instMonad, Except.bind]
      obtain ⟨same, sameOk⟩ := sameOriginSort_complete actualOrigin expectedOrigin
      rw [sameOk]
      cases same with
      | up equality =>
          have equalityRfl : equality = rfl := Subsingleton.elim _ _
          rw [equalityRfl]
          rfl
  case enum =>
    intros scope attr enumSchema shapeEq variant fuel path origin bound
    rw [checkExprFuel.eq_def (path := path), dif_pos (by omega)]
    cases origin with
    | enum _ _ originShape =>
        have shapeSame : originShape = shapeEq := Subsingleton.elim _ _
        rw [shapeSame]
        change (match enumSchema.lookup scope.schema attr shapeEq
            (enumSchema.variantName scope.schema attr variant) with
          | none => pathError .unknownEnumVariant path
          | some found => Except.ok (Expr.enum attr enumSchema found)) =
            Except.ok (Expr.enum attr enumSchema variant)
        rw [EnumSchema.lookup_variantName_self]
  case count =>
    intros scope fuel path inputBoundary bound boundaryFree
    rw [synthAggOpFuel.eq_def (path := path), dif_pos (by omega)]
    exact ⟨.int, rfl⟩
  case sumInt =>
    intros scope raw value hValue ih fuel path inputBoundary bound boundaryFree
    rw [synthAggOpFuel.eq_def (path := path), dif_pos (by omega)]
    simp only [rawAggOpDepth, Nat.succ_sub_one] at bound ⊢
    cases inputBoundary
    · obtain ⟨origin, valueOk⟩ := ih (fuel - 1) (path ++ [.aggregateValue]) (by omega)
      rw [valueOk]
      cases origin
      exact ⟨.int, rfl⟩
    · have valueFree : rawExprContainsAggregate raw = false := by
        simpa [rawAggOpContainsAggregate] using boundaryFree rfl
      simp only [Bool.true_eq, true_and, valueFree, Bool.false_eq_true, ↓reduceIte]
      obtain ⟨origin, valueOk⟩ := ih (fuel - 1) (path ++ [.aggregateValue]) (by omega)
      rw [valueOk]
      cases origin
      exact ⟨.int, rfl⟩
  case sumReal =>
    intros scope raw value hValue ih fuel path inputBoundary bound boundaryFree
    rw [synthAggOpFuel.eq_def (path := path), dif_pos (by omega)]
    simp only [rawAggOpDepth, Nat.succ_sub_one] at bound ⊢
    cases inputBoundary
    · obtain ⟨origin, valueOk⟩ := ih (fuel - 1) (path ++ [.aggregateValue]) (by omega)
      rw [valueOk]
      cases origin
      exact ⟨.real, rfl⟩
    · have valueFree : rawExprContainsAggregate raw = false := by
        simpa [rawAggOpContainsAggregate] using boundaryFree rfl
      simp only [Bool.true_eq, true_and, valueFree, Bool.false_eq_true, ↓reduceIte]
      obtain ⟨origin, valueOk⟩ := ih (fuel - 1) (path ++ [.aggregateValue]) (by omega)
      rw [valueOk]
      cases origin
      exact ⟨.real, rfl⟩
  case unfiltered =>
    intros scope raw sort op aggregateFree hop ih port scopeEq fuel path bound
    cases scopeEq
    rw [synthAggregateFuel.eq_def (path := path), dif_pos (by omega)]
    simp only [rawAggregateDepth, Nat.succ_sub_one] at bound ⊢
    obtain ⟨origin, opOk⟩ := ih (fuel - 1) path true (by omega)
      (fun _ => aggregateFree)
    rw [opOk]
    exact ⟨origin, rfl⟩
  case filtered =>
    intros scope rawOp rawFilter sort op filter operationFree filterFree hop hfilter
      ihOp ihFilter port scopeEq fuel path bound
    cases scopeEq
    rw [synthAggregateFuel.eq_def (path := path), dif_pos (by omega)]
    simp only [rawAggregateDepth, Nat.succ_sub_one] at bound ⊢
    obtain ⟨origin, opOk⟩ := ihOp (fuel - 1) path true (by omega)
      (fun _ => operationFree)
    rw [opOk]
    simp only [Bind.bind, Except.instMonad, Except.bind]
    rw [if_neg (by simpa [filterFree])]
    have filterOk := ihFilter (fuel - 1) (path ++ [.aggregateFilter]) .bool (by omega)
    rw [filterOk]
    exact ⟨origin, rfl⟩
  all_goals aesop (config := { maxRuleApplications := 100 })

/-- Every independent canonical synthesis derivation is reproduced exactly by
canonical synthesis; proof witnesses are the only existential component. -/
theorem synthExpr_complete {Γ : TermContext}
    {scope : RowScope Γ.model Γ.current Γ.inputs} {raw sort term}
    (h : ExprSynthesizes Γ scope raw sort term)
    (path : List ModelCheckPathSegment := []) :
    ∃ origin : SortOrigin Γ scope sort,
      synthExpr Γ scope raw path = .ok ⟨sort, term, origin⟩ := by
  simpa [synthExpr] using
    declarativeFuel_complete h (rawExprDepth raw * 4 + 8) path (by omega)

/-- Every independent expected-sort derivation is reproduced exactly by the
canonical expected checker for its explicitly supplied owner origin. -/
theorem checkExpr_complete {Γ : TermContext}
    {scope : RowScope Γ.model Γ.current Γ.inputs} {expected raw term}
    (h : ExprChecks Γ scope expected raw term)
    (origin : SortOrigin Γ scope expected)
    (path : List ModelCheckPathSegment := []) :
    checkExpr Γ scope raw expected origin path = .ok term := by
  cases h with
  | synth synthesized =>
      rw [checkExpr, checkExprFuel.eq_def (path := path), dif_pos (by
        have := rawExprDepth_positive raw
        omega)]
      split
      · cases synthesized
      · obtain ⟨actualOrigin, actualOk⟩ := declarativeFuel_complete synthesized
          (rawExprDepth raw * 4 + 9 - 1) path (by
            have := rawExprDepth_positive raw
            omega)
        rw [actualOk]
        simp only [Bind.bind, Except.instMonad, Except.bind]
        obtain ⟨same, sameOk⟩ := sameOriginSort_complete actualOrigin origin
        rw [sameOk]
        cases same with
        | up equality =>
            have equalityRfl : equality = rfl := Subsingleton.elim _ _
            rw [equalityRfl]
            rfl
  | enum attr enumSchema shapeEq variant =>
      rw [checkExpr, checkExprFuel.eq_def (path := path), dif_pos (by omega)]
      cases origin with
      | enum _ _ originShape =>
          have shapeSame : originShape = shapeEq := Subsingleton.elim _ _
          rw [shapeSame]
          simp [EnumSchema.lookup_variantName_self, Bind.bind, Pure.pure,
            Except.instMonad, Except.bind, Except.pure]

/-- Successful aggregate-operation synthesis yields its independent judgment. -/
theorem synthAggOpFuel_sound {fuel : Nat} {Γ : TermContext}
    {scope : RowScope Γ.model Γ.current Γ.inputs}
    {raw inputBoundary path result}
    (success : synthAggOpFuel fuel Γ scope raw inputBoundary path = .ok result) :
    AggOpSynthesizes Γ scope raw result.1 result.2.1 :=
  (fuelSound fuel).synthAggOp success

/-- Every declarative aggregate operation is reproduced by any sufficiently
large fuel budget, including the input-aggregate freedom boundary. -/
theorem synthAggOpFuel_complete {Γ : TermContext}
    {scope : RowScope Γ.model Γ.current Γ.inputs} {raw sort op}
    (typed : AggOpSynthesizes Γ scope raw sort op)
    (fuel : Nat) (inputBoundary : Bool)
    (path : List ModelCheckPathSegment := [])
    (bound : rawAggOpDepth raw * 4 + 8 ≤ fuel)
    (boundaryFree : inputBoundary = true → rawAggOpContainsAggregate raw = false) :
    ∃ origin : SortOrigin Γ scope sort,
      synthAggOpFuel fuel Γ scope raw inputBoundary path = .ok ⟨sort, op, origin⟩ := by
  cases typed
  case count =>
    refine ⟨.int, ?_⟩
    rw [synthAggOpFuel.eq_def (path := path), dif_pos (by omega)]
    rfl
  case sumInt raw value valueTyped =>
    rw [synthAggOpFuel.eq_def (path := path), dif_pos (by omega)]
    have valueFree : (inputBoundary && rawExprContainsAggregate raw) = false := by
      cases inputBoundary
      · rfl
      · simpa [rawAggOpContainsAggregate] using boundaryFree rfl
    simp only [valueFree, Bool.false_eq_true, ↓reduceIte]
    obtain ⟨origin, valueOk⟩ := declarativeFuel_complete valueTyped
      (fuel - 1) (path ++ [.aggregateValue]) (by
        simp only [rawAggOpDepth] at bound
        omega)
    rw [valueOk]
    cases origin
    exact ⟨.int, rfl⟩
  case sumReal raw value valueTyped =>
    rw [synthAggOpFuel.eq_def (path := path), dif_pos (by omega)]
    have valueFree : (inputBoundary && rawExprContainsAggregate raw) = false := by
      cases inputBoundary
      · rfl
      · simpa [rawAggOpContainsAggregate] using boundaryFree rfl
    simp only [valueFree, Bool.false_eq_true, ↓reduceIte]
    obtain ⟨origin, valueOk⟩ := declarativeFuel_complete valueTyped
      (fuel - 1) (path ++ [.aggregateValue]) (by
        simp only [rawAggOpDepth] at bound
        omega)
    rw [valueOk]
    cases origin
    exact ⟨.real, rfl⟩

/-- Successful canonical synthesis certifies the independent syntax-directed
synthesis judgment. -/
theorem synthExpr_sound {Γ : TermContext}
    {scope : RowScope Γ.model Γ.current Γ.inputs} {raw path result}
    (success : synthExpr Γ scope raw path = .ok result) :
    ExprSynthesizes Γ scope raw result.sort result.expr := by
  exact (fuelSound (rawExprDepth raw * 4 + 8)).synthExpr success

/-- Successful expected-sort checking certifies the independent checking
judgment without introducing a top-level numeric coercion. -/
theorem checkExpr_sound {Γ : TermContext}
    {scope : RowScope Γ.model Γ.current Γ.inputs}
    {raw expected origin path result}
    (success : checkExpr Γ scope raw expected origin path = .ok result) :
    ExprChecks Γ scope expected raw result := by
  exact (fuelSound (rawExprDepth raw * 4 + 9)).checkExpr success

@[simp] theorem synthExpr_success_erase_exact {Γ : TermContext}
    {scope : RowScope Γ.model Γ.current Γ.inputs} {raw path result}
    (success : synthExpr Γ scope raw path = .ok result) :
    result.expr.erase = raw :=
  (synthExpr_sound success).erase_exact

@[simp] theorem checkExpr_success_erase_exact {Γ : TermContext}
    {scope : RowScope Γ.model Γ.current Γ.inputs}
    {raw expected origin path result}
    (success : checkExpr Γ scope raw expected origin path = .ok result) :
    result.erase = raw :=
  (checkExpr_sound success).erase_exact

/-- Executable synthesis is deterministic, hence two successful canonical
checker results for one raw expression carry the same intrinsic result sort. -/
theorem synthExpr_resultSort_unique {Γ : TermContext}
    {scope : RowScope Γ.model Γ.current Γ.inputs} {raw path}
    {left right : CheckedExprResult Γ scope}
    (hl : synthExpr Γ scope raw path = .ok left)
    (hr : synthExpr Γ scope raw path = .ok right) : left.sort = right.sort := by
  rw [hl] at hr
  cases hr
  rfl

/-- Canonical declarative synthesis has one result sort. This follows from
completeness and executable determinism rather than being encoded in the
judgment. -/
theorem ExprSynthesizes.sort_unique {Γ : TermContext}
    {scope : RowScope Γ.model Γ.current Γ.inputs} {raw}
    {leftSort rightSort : ScalarSort Γ.model.catalog}
    {left : Expr Γ.model Γ.current Γ.inputs scope leftSort}
    {right : Expr Γ.model Γ.current Γ.inputs scope rightSort}
    (hl : ExprSynthesizes Γ scope raw leftSort left)
    (hr : ExprSynthesizes Γ scope raw rightSort right) : leftSort = rightSort := by
  obtain ⟨leftOrigin, leftOk⟩ := synthExpr_complete hl []
  obtain ⟨rightOrigin, rightOk⟩ := synthExpr_complete hr []
  exact synthExpr_resultSort_unique leftOk rightOk

/-- Expected checking is likewise deterministic for one explicitly supplied
owner and sort. -/
theorem checkExpr_result_unique {Γ : TermContext}
    {scope : RowScope Γ.model Γ.current Γ.inputs} {raw expected origin path}
    {left right : Expr Γ.model Γ.current Γ.inputs scope expected}
    (hl : checkExpr Γ scope raw expected origin path = .ok left)
    (hr : checkExpr Γ scope raw expected origin path = .ok right) : left = right := by
  rw [hl] at hr
  exact Except.ok.inj hr

/-! ## Transition-checker correspondence -/

private theorem checkEffect_sound {Γ : TermContext} {raw checked path}
    (success : checkEffect Γ raw path = .ok checked) :
    EffectWellTyped Γ raw checked := by
  cases raw with
  | setAttr name value =>
      simp only [checkEffect] at success
      split at success
      · contradiction
      · rename_i destination found
        simp only [Pure.pure, Except.pure, Bind.bind, Except.bind] at success
        split at success
        · contradiction
        · rename_i term termOk
          cases success
          have typed := checkExpr_sound termOk
          have nameEq := (Γ.model.schemaFor Γ.current).attributeLookup_name found
          rw [← nameEq]
          exact .setAttr destination typed

private theorem checkEffect_complete {Γ : TermContext} {raw checked}
    (typed : EffectWellTyped Γ raw checked) (path : List ModelCheckPathSegment := []) :
    checkEffect Γ raw path = .ok checked := by
  cases typed with
  | setAttr destination valueTyped =>
      simp only [checkEffect, TableSchema.lookupAttribute_name_self,
        Pure.pure, Except.pure, Bind.bind, Except.bind]
      rw [checkExpr_complete (path := path ++ [ModelCheckPathSegment.value]) valueTyped
        (SortOrigin.ofAttribute Γ (scope := .table Γ.current) destination)]

private theorem checkEffectsAux_sound {Γ : TermContext} {root index raws checked}
    (success : checkEffectsAux Γ root index raws = .ok checked) :
    List.Forall₂ (EffectWellTyped Γ) raws checked := by
  induction raws generalizing index checked with
  | nil =>
      simp only [checkEffectsAux, Pure.pure, Except.pure] at success
      cases success
      exact .nil
  | cons raw raws ih =>
      simp only [checkEffectsAux, Pure.pure, Except.pure, Bind.bind, Except.bind] at success
      split at success
      · contradiction
      · rename_i first firstOk
        split at success
        · contradiction
        · rename_i rest restOk
          cases success
          exact .cons (checkEffect_sound firstOk) (ih restOk)

private theorem checkEffectsAux_complete {Γ : TermContext} {raws checked}
    (typed : List.Forall₂ (EffectWellTyped Γ) raws checked) :
    ∀ root index, checkEffectsAux Γ root index raws = .ok checked := by
  induction typed with
  | nil => intro root index; rfl
  | cons head tail ih =>
      intro root index
      simp only [checkEffectsAux, Pure.pure, Except.pure, Bind.bind, Except.bind]
      rw [checkEffect_complete (path := root ++ [ModelCheckPathSegment.effects, ModelCheckPathSegment.effect index]) head]
      rw [ih root (index + 1)]

private theorem checkEffects_sound {Γ : TermContext} {raws checked path}
    (success : checkEffects Γ raws path = .ok checked) :
    List.Forall₂ (EffectWellTyped Γ) raws checked :=
  checkEffectsAux_sound success

private theorem checkEffects_complete {Γ : TermContext} {raws checked}
    (typed : List.Forall₂ (EffectWellTyped Γ) raws checked)
    (path : List ModelCheckPathSegment := []) :
    checkEffects Γ raws path = .ok checked :=
  checkEffectsAux_complete typed path 0

private theorem checkClaim_sound {Γ : TermContext} {raw checked path}
    (success : checkClaim Γ raw path = .ok checked) :
    ClaimWellTyped Γ raw checked := by
  cases raw with
  | mk resourceRaw ordering =>
    cases ordering with
    | raceTime =>
      simp only [checkClaim, Pure.pure, Except.pure, Bind.bind, Except.bind] at success
      split at success
      · contradiction
      · rename_i resourceResult resourceOk
        split at success
        · contradiction
        · rename_i resource foundResource
          cases resourceResult with
          | mk sort resourceExpr resourceOrigin =>
            cases resourceOrigin <;> simp_all [refOfResult]
            cases foundResource
            cases success
            exact .raceTime (synthExpr_sound resourceOk)
    | key keyRaw =>
      simp only [checkClaim, Pure.pure, Except.pure, Bind.bind, Except.bind] at success
      split at success
      · contradiction
      · rename_i resourceResult resourceOk
        split at success
        · contradiction
        · rename_i resource foundResource
          split at success
          · contradiction
          · rename_i keyResult keyResultOk
            cases resourceResult with
            | mk resourceSort resourceExpr resourceOrigin =>
              cases resourceOrigin <;> simp_all [refOfResult]
              cases foundResource
              cases keyResult with
              | mk keySort keyExpr keyOrigin =>
                cases keyOrigin
                case real =>
                  simp only [numericOfResult] at success
                  cases success
                  exact .key (synthExpr_sound resourceOk) .real
                    (synthExpr_sound keyResultOk)
                case int =>
                  simp only [numericOfResult] at success
                  cases success
                  exact .key (synthExpr_sound resourceOk) .int
                    (synthExpr_sound keyResultOk)
                case enum =>
                  simp only [numericOfResult, enumOfResult] at success
                  cases success
                  exact .key (synthExpr_sound resourceOk) _
                    (synthExpr_sound keyResultOk)
                case bool => simp [numericOfResult, enumOfResult, pathError] at success
                case ref => simp [numericOfResult, enumOfResult, pathError] at success

private theorem checkClaim_complete {Γ : TermContext} {raw checked}
    (typed : ClaimWellTyped Γ raw checked)
    (path : List ModelCheckPathSegment := []) :
    checkClaim Γ raw path = .ok checked := by
  cases typed with
  | raceTime resourceTyped =>
      obtain ⟨resourceOrigin, resourceOk⟩ :=
        synthExpr_complete (path := path ++ [ModelCheckPathSegment.resource]) resourceTyped
      cases resourceOrigin
      simp only [checkClaim, Pure.pure, Except.pure, Bind.bind, Except.bind]
      rw [resourceOk]
      rfl
  | key resourceTyped domain keyTyped =>
      obtain ⟨resourceOrigin, resourceOk⟩ :=
        synthExpr_complete (path := path ++ [ModelCheckPathSegment.resource]) resourceTyped
      obtain ⟨keyOrigin, keyOk⟩ :=
        synthExpr_complete (path := path ++ [ModelCheckPathSegment.orderingKey]) keyTyped
      cases resourceOrigin
      cases domain <;> cases keyOrigin
      all_goals simp only [checkClaim, Pure.pure, Except.pure, Bind.bind, Except.bind]
      all_goals rw [resourceOk, keyOk]
      all_goals rfl

private theorem checkClaimsAux_sound {Γ : TermContext} {root index raws checked}
    (success : checkClaimsAux Γ root index raws = .ok checked) :
    List.Forall₂ (ClaimWellTyped Γ) raws checked := by
  induction raws generalizing index checked with
  | nil =>
      simp only [checkClaimsAux, Pure.pure, Except.pure] at success
      cases success
      exact .nil
  | cons raw raws ih =>
      simp only [checkClaimsAux, Pure.pure, Except.pure, Bind.bind, Except.bind] at success
      split at success
      · contradiction
      · rename_i first firstOk
        split at success
        · contradiction
        · rename_i rest restOk
          cases success
          exact .cons (checkClaim_sound firstOk) (ih restOk)

private theorem checkClaimsAux_complete {Γ : TermContext} {raws checked}
    (typed : List.Forall₂ (ClaimWellTyped Γ) raws checked) :
    ∀ root index, checkClaimsAux Γ root index raws = .ok checked := by
  induction typed with
  | nil => intro root index; rfl
  | cons head tail ih =>
      intro root index
      simp only [checkClaimsAux, Pure.pure, Except.pure, Bind.bind, Except.bind]
      rw [checkClaim_complete
        (path := root ++ [ModelCheckPathSegment.contests, ModelCheckPathSegment.claim index]) head]
      rw [ih root (index + 1)]

private theorem checkClaims_sound {Γ : TermContext} {raws checked path}
    (success : checkClaims Γ raws path = .ok checked) :
    List.Forall₂ (ClaimWellTyped Γ) raws checked ∧ ClaimsUnique raws := by
  simp only [checkClaims, Pure.pure, Except.pure, Bind.bind, Except.bind] at success
  split at success
  · contradiction
  · rename_i noDuplicate
    exact ⟨checkClaimsAux_sound success,
      (firstDuplicateResource_none_iff raws).mp noDuplicate⟩

private theorem checkClaims_complete {Γ : TermContext} {raws checked}
    (typed : List.Forall₂ (ClaimWellTyped Γ) raws checked)
    (unique : ClaimsUnique raws) (path : List ModelCheckPathSegment := []) :
    checkClaims Γ raws path = .ok checked := by
  simp only [checkClaims, Pure.pure, Except.pure, Bind.bind, Except.bind]
  rw [(firstDuplicateResource_none_iff raws).mpr unique]
  exact checkClaimsAux_complete typed path 0

/-- Successful transition checking certifies every independent transition rule. -/
theorem checkTransitionTerms_sound {Γ : TermContext} {raw checked path}
    (success : checkTransitionTerms Γ raw path = .ok checked) :
    TransitionWellTyped Γ raw checked := by
  simp only [checkTransitionTerms, Pure.pure, Except.pure,
    Bind.bind, Except.bind] at success
  split at success
  · contradiction
  · rename_i guard guardOk
    split at success
    · contradiction
    · rename_i hazard hazardOk
      split at success
      · contradiction
      · rename_i effects effectsOk
        split at success
        · contradiction
        · rename_i claims claimsOk
          split at success
          · contradiction
          · rename_i covered
            cases success
            have checkedClaims := checkClaims_sound claimsOk
            exact .mk (checkExpr_sound guardOk) (checkExpr_sound hazardOk)
              (checkEffects_sound effectsOk) checkedClaims.1 checkedClaims.2
              ((firstUnclaimedRefWrite_none_iff Γ raw.effects raw.contests).mp covered)

/-- Every declaratively well-typed transition payload is reproduced exactly. -/
theorem checkTransitionTerms_complete {Γ : TermContext} {raw checked}
    (typed : TransitionWellTyped Γ raw checked)
    (path : List ModelCheckPathSegment := []) :
    checkTransitionTerms Γ raw path = .ok checked := by
  cases typed with
  | mk guard hazard effects claims unique covered =>
      simp only [checkTransitionTerms, Pure.pure, Except.pure,
        Bind.bind, Except.bind]
      rw [checkExpr_complete
        (path := path ++ [ModelCheckPathSegment.guard]) guard .bool]
      rw [checkExpr_complete
        (path := path ++ [ModelCheckPathSegment.hazard]) hazard .real]
      rw [checkEffects_complete (path := path) effects]
      rw [checkClaims_complete (path := path) claims unique]
      rw [(firstUnclaimedRefWrite_none_iff Γ raw.effects raw.contests).mpr covered]

end Sembla.Semantics
