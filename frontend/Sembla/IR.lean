namespace Sembla.IR

/-- An exact finite decimal number.  Keeping a coefficient and base-10 exponent
    preserves source values without routing JSON through Lean's intentionally
    short `Float.toString` representation. -/
structure Scientific where
  coefficient : Int
  exponent : Int
deriving Repr, BEq

instance : OfScientific Scientific where
  ofScientific coefficient hasDot decimalPlacesOrExponent :=
    if hasDot then
      ⟨Int.ofNat coefficient, -Int.ofNat decimalPlacesOrExponent⟩
    else
      ⟨Int.ofNat coefficient, Int.ofNat decimalPlacesOrExponent⟩

instance : Neg Scientific where
  neg value := { value with coefficient := -value.coefficient }

inductive ParamType where | real | int deriving Repr, BEq
inductive ParamValue where | real (value : Scientific) | int (value : Int) deriving Repr, BEq
inductive PriorFamily where | normal | logNormal | uniform deriving Repr, BEq
structure Prior where
  family : PriorFamily
  args : List Scientific
deriving Repr, BEq
structure ParamDecl where
  name : String
  ty : ParamType
  default : ParamValue
  prior : Option Prior
deriving Repr, BEq

inductive AttrType where
  | real
  | int
  | enum (variants : List String)
  | ref (table : String)
deriving Repr, BEq
structure Attr where
  name : String
  ty : AttrType
deriving Repr, BEq
structure Table where
  name : String
  sizeHint : Nat
  attrs : List Attr
deriving Repr, BEq

mutual
  inductive Expr where
    | real (value : Scientific)
    | int (value : Int)
    | bool (value : Bool)
    | enum (variant : String)
    | param (name : String)
    | selfAttr (name : String)
    | add (lhs rhs : Expr)
    | sub (lhs rhs : Expr)
    | mul (lhs rhs : Expr)
    | div (lhs rhs : Expr)
    | eq (lhs rhs : Expr)
    | ne (lhs rhs : Expr)
    | lt (lhs rhs : Expr)
    | le (lhs rhs : Expr)
    | gt (lhs rhs : Expr)
    | ge (lhs rhs : Expr)
    | and (lhs rhs : Expr)
    | or (lhs rhs : Expr)
    | not (expr : Expr)
    | enumIs (attr variant : String)
    | input (port : String) (agg : Aggregate)
    | agg (op : AggOp) (table fkAttr selfFkAttr : String) (filter : Expr)
  deriving Repr, BEq

  inductive AggOp where
    | count
    | sum (value : Expr)
  deriving Repr, BEq

  inductive Aggregate where
    | mk (op : AggOp) (filter : Option Expr)
  deriving Repr, BEq
end

inductive Effect where
  | setAttr (attr : String) (value : Expr)
deriving Repr, BEq
inductive ClaimOrdering where
  | raceTime
  | key (expr : Expr)
deriving Repr, BEq
structure ResourceClaim where
  resource : Expr
  ordering : ClaimOrdering
deriving Repr, BEq
structure Transition where
  name : String
  table : String
  guard : Expr
  hazard : Expr
  effects : List Effect
  contests : List ResourceClaim
deriving Repr, BEq

structure PortDecl where
  name : String
  schema : List Attr
deriving Repr, BEq
structure OutputField where
  name : String
  op : AggOp
  filter : Option Expr
deriving Repr, BEq
inductive OutputBuilder where
  | perTable (table : String) (fields : List OutputField)
deriving Repr, BEq
structure OutputDecl where
  name : String
  schema : List Attr
  builder : OutputBuilder
deriving Repr, BEq

inductive ViewReduce where
  | sum | count | min | max
deriving Repr, BEq
structure ViewDecl where
  name : String
  table : String
  filter : Option Expr
  value : Option Expr
  reduce : ViewReduce
deriving Repr, BEq
structure Box where
  name : String
  tables : List Table
  transitions : List Transition
  inputs : List PortDecl
  outputs : List OutputDecl
  views : List ViewDecl
deriving Repr, BEq
structure WireEndpoint where
  box : String
  port : String
deriving Repr, BEq
structure Wire where
  source : WireEndpoint
  target : WireEndpoint
deriving Repr, BEq

inductive SummaryReduce where
  | sum | min | max | last | argmaxTick
deriving Repr, BEq
structure SummaryDecl where
  name : String
  box : String
  view : String
  reduce : SummaryReduce
deriving Repr, BEq
structure Model where
  name : String
  dt : Scientific
  params : List ParamDecl
  boxes : List Box
  wires : List Wire
  summaries : List SummaryDecl
deriving Repr, BEq

end Sembla.IR
