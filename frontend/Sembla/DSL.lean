import Lean.Elab.Term
import Lean.Elab.Command
import Sembla.IR
import Sembla.ParameterTable
import Sembla.WidgetDisplay

namespace Sembla.DSL
open Lean Elab Term Sembla.IR Sembla.Widgets Sembla.WidgetDisplay

inductive SurfaceTy where
  | real | int | bool
  | enum (variants : List String)
  | ref (target : String)
deriving Repr, BEq

structure SurfaceAttr where
  name : String
  ty : SurfaceTy
  nameToken : Syntax
  refTargetToken : Option Syntax := none
  variantTokens : List (String × Syntax) := []
  /-- Present only when an attribute was authored using a named finite domain. -/
  domainName : Option String := none

inductive SurfacePriorFamily where
  | normal
  | logNormal
deriving Repr, BEq

structure SurfacePrior where
  family : SurfacePriorFamily
  args : TSyntax `term × TSyntax `term

structure SurfaceParam where
  sourceName : String
  name : String
  token : Syntax
  ty : SurfaceTy := .real
  default : TSyntax `term
  /-- Prior arguments retain the original collected-surface API shape. -/
  prior : Option (TSyntax `term × TSyntax `term)
  /-- The family is separate so legacy callers default to LogNormal. -/
  priorFamily : SurfacePriorFamily := .logNormal

structure SurfaceDomain where
  name : String
  token : Syntax
  domain : ParameterTable.IndexDomain
  memberTokens : List (String × Syntax) := []
  legacyIndex : Bool := false

abbrev SurfaceIndex := SurfaceDomain

structure SurfaceFunctionArg where
  name : String
  token : Syntax
  domainName : String
  domainToken : Syntax

structure SurfaceParamFamily where
  sourceName : String
  name : String
  token : Syntax
  dimensions : List String
  dimensionTokens : List Syntax
  ty : SurfaceTy
  domains : List String := []
  callNotation : Bool := false

structure SurfaceExprFunctionCell where
  key : List (ParameterTable.IndexMember × Syntax)
  expression : TSyntax `semblaExpr
  token : Syntax

structure SurfaceExprFunction where
  name : String
  token : Syntax
  args : List SurfaceFunctionArg
  ty : SurfaceTy
  cells : List SurfaceExprFunctionCell

structure SurfaceProjectionTarget where
  boxName : String
  systemName : String
  attrName : String
  token : Syntax

structure SurfacePartitionMember where
  label : String
  token : Syntax
  lower : Nat
  upper : Option Nat

structure SurfacePartition where
  name : String
  token : Syntax
  target : SurfaceProjectionTarget
  members : List SurfacePartitionMember

inductive SurfacePatternAtom where
  | assignment (attr : TSyntax `ident) (value : TSyntax `semblaExpr) (token : Syntax)
  | predicate (expression : TSyntax `semblaExpr) (token : Syntax)

structure SurfaceStateAlias where
  name : String
  token : Syntax
  system : TSyntax `ident
  args : List SurfaceFunctionArg
  atoms : List SurfacePatternAtom

structure SurfaceInput where
  name : String
  token : Syntax
  schema : List SurfaceAttr

structure SurfaceSystem where
  logicalName : String
  token : Syntax
  irName : String
  irNameToken : Syntax
  size : TSyntax `term
  attrs : List SurfaceAttr

structure SurfaceContest where
  resource : TSyntax `ident

inductive SurfaceBinderMode where
  | legacyMatched
  | static
  | inferredMatched

deriving Repr, BEq

structure SurfaceTransitionBinder where
  name : String
  token : Syntax
  domainName : Option String := none
  domainToken : Option Syntax := none
  mode : SurfaceBinderMode := .legacyMatched
  matchedAttribute : Option String := none

inductive SurfaceRelationConstraint where
  | equal (left right : TSyntax `ident) (token : Syntax)
  | notEqual (left right : TSyntax `ident) (token : Syntax)

structure SurfaceStateApplication where
  name : String
  token : Syntax
  args : List (TSyntax `ident)

inductive SurfaceRelationItem where
  | source (applications : List SurfaceStateApplication) (token : Syntax)
  | hazard (expression : TSyntax `semblaExpr) (token : Syntax)
  | claim (contest : SurfaceContest) (token : Syntax)
  | set (assignment : TSyntax `semblaSet) (token : Syntax)
  | become (application : SurfaceStateApplication) (token : Syntax)

inductive SurfaceTransitionBody where
  | general
      (system : TSyntax `ident)
      (guard : TSyntax `semblaExpr)
      (hazard : TSyntax `semblaExpr)
      (contests : List SurfaceContest)
      (sets : List (TSyntax `semblaSet))
  | reaction
      (system : Option (TSyntax `ident))
      (stateAttr : Option (TSyntax `ident))
      (source : TSyntax `ident)
      (hazard : TSyntax `semblaExpr)
      (destination : TSyntax `ident)
  | namedReaction
      (system : TSyntax `ident)
      (source : SurfaceStateApplication)
      (axes : List (TSyntax `ident))
      (hazard : TSyntax `semblaExpr)
      (destination : SurfaceStateApplication)
  | relation
      (system : TSyntax `ident)
      (constraints : List SurfaceRelationConstraint)
      (items : List SurfaceRelationItem)

structure SurfaceTransition where
  name : String
  token : Syntax
  body : SurfaceTransitionBody
  binders : List SurfaceTransitionBinder := []

structure SurfaceOutputField where
  name : String
  token : Syntax
  op : String
  value : Option (TSyntax `semblaExpr)
  filter : Option (TSyntax `semblaExpr)

structure SurfaceOutput where
  name : String
  token : Syntax
  schema : List SurfaceAttr
  system : TSyntax `ident
  fields : List SurfaceOutputField

structure SurfaceView where
  name : String
  token : Syntax
  system : TSyntax `ident
  filter : Option (TSyntax `semblaExpr)
  value : Option (TSyntax `semblaExpr)
  reduce : String

structure SurfaceGroupKey where
  attr : TSyntax `ident
  bandWidth : Option (Nat × Syntax)

structure SurfaceGroupedView where
  name : String
  token : Syntax
  system : TSyntax `ident
  filter : Option (TSyntax `semblaExpr)
  keys : List SurfaceGroupKey
  scopedSyntax : Bool := false

structure SurfaceSummary where
  name : String
  token : Syntax
  box : TSyntax `ident
  view : TSyntax `ident
  reduce : String

structure SurfaceBox where
  name : String
  token : Syntax
  systems : List SurfaceSystem
  inputs : List SurfaceInput
  transitions : List SurfaceTransition
  outputs : List SurfaceOutput
  views : List SurfaceView
  groupedViews : List SurfaceGroupedView
  aliases : List SurfaceStateAlias := []

/-- One parsed port declaration in its original position among the other
    input/output declarations of a command-layout box. -/
inductive SurfacePortItem where
  | input (declaration : SurfaceInput)
  | output (declaration : SurfaceOutput)

/-- The shared box result plus cross-direction port author order. -/
structure CollectedSurfaceBox where
  surfaceBox : SurfaceBox
  ports : List SurfacePortItem

structure SurfaceWire where
  fromBox : TSyntax `ident
  fromPort : TSyntax `ident
  toBox : TSyntax `ident
  toPort : TSyntax `ident

/-- The single collected input to the surface semantic kernel.  Frontends retain
    their declaration token separately from optional runtime-name metadata so
    diagnostics and future command syntax never need to reconstruct anchors. -/
structure SurfaceModel where
  declarationName : String
  declarationToken : Syntax
  runtimeName : Option (String × Syntax)
  dt : TSyntax `term
  params : List SurfaceParam
  boxes : List SurfaceBox
  wires : List SurfaceWire
  summaries : List SurfaceSummary
  indexes : List SurfaceIndex := []
  families : List SurfaceParamFamily := []
  domains : List SurfaceDomain := []
  partitions : List SurfacePartition := []
  exprFunctions : List SurfaceExprFunction := []

register_option sembla.maxFamilyExpansion : Nat := {
  defValue := 10000
  descr := "maximum scalar parameters or transitions emitted by one indexed family"
}

/-- Attribute declarations occur exactly once, inside their actual system or
    port declaration.  Transition and output contexts are derived from these
    declarations by the enclosing model elaborator. -/
declare_syntax_cat semblaAttr
syntax ident ident ":" "{" ident,* "}" : semblaAttr
syntax "attr" ident ":" "Real" : semblaAttr
syntax "attr" ident ":" "ℝ" : semblaAttr
syntax "attr" ident ":" "Int" : semblaAttr
syntax ident ":" "Real" : semblaAttr
syntax ident ":" "ℝ" : semblaAttr
syntax ident ":" "Int" : semblaAttr
syntax "ref" ident ":" ident : semblaAttr

declare_syntax_cat semblaRealTerm
syntax scientific : semblaRealTerm
syntax "-" scientific : semblaRealTerm
syntax ident : semblaRealTerm
syntax "(" term ")" : semblaRealTerm

declare_syntax_cat semblaParam
syntax "param" ident ":" "ℝ" ":=" term "~" ident semblaRealTerm semblaRealTerm : semblaParam
syntax "param" ident ":" "ℝ" ":=" term : semblaParam
syntax "param" ident ":" "Real" ":=" term "prior" ident "(" term "," term ")" : semblaParam
syntax "param" ident ":" "Real" ":=" term : semblaParam
syntax "param" ident ":" "Int" ":=" term "~" ident semblaRealTerm semblaRealTerm : semblaParam
syntax "param" ident ":" "Int" ":=" term : semblaParam

declare_syntax_cat semblaExpr
syntax:max ident "[" ident,* "]" : semblaExpr
syntax:max ident "(" semblaExpr,* ")" : semblaExpr
syntax:max ident : semblaExpr
syntax:max "parameter" ident : semblaExpr
syntax:max num : semblaExpr
syntax:max scientific : semblaExpr
syntax:max &"true" : semblaExpr
syntax:max &"false" : semblaExpr
syntax:max "(" semblaExpr ")" : semblaExpr
syntax:75 "¬" semblaExpr:75 : semblaExpr
syntax:max "countBy " ident " (" semblaExpr ")" : semblaExpr
syntax:max "sizeBy " ident : semblaExpr
syntax:max "freq" "(" semblaExpr ")" "over" ident : semblaExpr
-- Recovery forms exist only to replace generic parser failures with teaching diagnostics.
syntax:max "freq" "(" semblaExpr ")" "over" : semblaExpr
syntax:max "freq" "(" semblaExpr ")" : semblaExpr
syntax:max "freq" ident "=" ident "over" ident : semblaExpr
syntax:max "freq" ident "over" ident : semblaExpr
syntax:max "inputSum" ident "field" ident : semblaExpr
syntax:70 semblaExpr:70 " * " semblaExpr:71 : semblaExpr
syntax:70 semblaExpr:70 " · " semblaExpr:71 : semblaExpr
syntax:70 semblaExpr:70 " / " semblaExpr:71 : semblaExpr
syntax:65 semblaExpr:65 " + " semblaExpr:66 : semblaExpr
syntax:65 semblaExpr:65 " - " semblaExpr:66 : semblaExpr
syntax:55 semblaExpr:56 " = " semblaExpr:55 : semblaExpr
syntax:55 semblaExpr:56 " ≠ " semblaExpr:55 : semblaExpr
syntax:55 semblaExpr:56 " < " semblaExpr:55 : semblaExpr
syntax:55 semblaExpr:56 " ≤ " semblaExpr:55 : semblaExpr
syntax:55 semblaExpr:56 " > " semblaExpr:55 : semblaExpr
syntax:55 semblaExpr:56 " ≥ " semblaExpr:55 : semblaExpr
syntax:55 ident " ∈ " ident : semblaExpr
syntax:40 semblaExpr:41 " && " semblaExpr:40 : semblaExpr
syntax:40 semblaExpr:41 " ∧ " semblaExpr:40 : semblaExpr

declare_syntax_cat semblaSet
syntax ident ":=" semblaExpr : semblaSet

declare_syntax_cat semblaSystem
syntax "system" ident "(" "rows" ":=" term ")" "where" "[" semblaAttr,* "]" : semblaSystem
syntax "system" ident "(" ident ":=" str ")" "(" "rows" ":=" term ")"
  "where" "[" semblaAttr,* "]" : semblaSystem
syntax "system" ident "as" str "rows" "(" term ")" "where" "[" semblaAttr,* "]" : semblaSystem

declare_syntax_cat semblaInput
syntax "input" ident "{" semblaAttr,* "}" : semblaInput

declare_syntax_cat semblaContest
syntax "contest" ident "by" ident : semblaContest
-- Recovery-only form for the mandatory-ordering teaching diagnostic.
syntax "contest" ident : semblaContest

declare_syntax_cat semblaArrowTail
syntax "guard" semblaExpr : semblaArrowTail
syntax "set" "[" semblaSet,* "]" : semblaArrowTail
syntax "contest" ident "by" ident : semblaArrowTail
syntax "contest" ident : semblaArrowTail

declare_syntax_cat semblaTransition
syntax "transition" ident "[" ident,* "]" "on" ident "where" "guard" semblaExpr
  "hazard" semblaExpr semblaContest* "set" "[" semblaSet,* "]" : semblaTransition
syntax "transition" ident "on" ident "where" "guard" semblaExpr "hazard" semblaExpr
  semblaContest* "set" "[" semblaSet,* "]" : semblaTransition
syntax ident "[" ident,* "]" "on" ident ":" ident ":" ident "→" "[" semblaExpr "]" ident : semblaTransition
syntax ident "on" ident ":" ident "[" ident,* "]" "→" "[" semblaExpr "]" ident : semblaTransition
syntax ident "on" ident ":" ident "[" ident,* "]" "→" "[" semblaExpr "]" ident "(" ident,* ")" : semblaTransition
syntax ident ":" ident "→" "[" semblaExpr "]" ident : semblaTransition
syntax ident "on" ident ":" ident "→" "[" semblaExpr "]" ident : semblaTransition
syntax ident ":" ident ":" ident "→" "[" semblaExpr "]" ident : semblaTransition
syntax ident "on" ident ":" ident ":" ident "→" "[" semblaExpr "]" ident : semblaTransition
syntax ident ":" ident "→" "[" semblaExpr "]" ident semblaArrowTail : semblaTransition
syntax ident "on" ident ":" ident "→" "[" semblaExpr "]" ident
  semblaArrowTail : semblaTransition
syntax ident ":" ident ":" ident "→" "[" semblaExpr "]" ident
  semblaArrowTail : semblaTransition
syntax ident "on" ident ":" ident ":" ident "→" "[" semblaExpr "]" ident
  semblaArrowTail : semblaTransition

declare_syntax_cat semblaOutputField
syntax "field" ident ":=" "count" "where" semblaExpr : semblaOutputField
syntax "field" ident ":=" "sum" "(" semblaExpr ")" : semblaOutputField

declare_syntax_cat semblaOutput
syntax "output" ident "{" semblaAttr,* "}" "from" ident "fields" "[" semblaOutputField,* "]" : semblaOutput

declare_syntax_cat semblaViewReduce
syntax "sum" : semblaViewReduce
syntax "count" : semblaViewReduce
syntax "min" : semblaViewReduce
syntax "max" : semblaViewReduce

declare_syntax_cat semblaView
syntax "view" ident "from" ident "reduce" semblaViewReduce : semblaView
syntax "view" ident "from" ident "where" semblaExpr "reduce" semblaViewReduce : semblaView
syntax "view" ident "from" ident "using" semblaExpr "reduce" semblaViewReduce : semblaView
syntax "view" ident "from" ident "where" semblaExpr "using" semblaExpr
  "reduce" semblaViewReduce : semblaView

declare_syntax_cat semblaGroupedKey
syntax ident : semblaGroupedKey
syntax ident ident num : semblaGroupedKey
syntax ident "(" ident "," num ")" : semblaGroupedKey

/-- Scoped view blocks use function-style bands and intentionally omit the
    legacy three-token key form so layout can terminate after a plain key. -/
declare_syntax_cat semblaScopedGroupedKey
syntax ident : semblaScopedGroupedKey
syntax ident "(" ident "," num ")" : semblaScopedGroupedKey

declare_syntax_cat semblaGroupedView
syntax "grouped" "view" ident ":=" "count" ident "by" semblaGroupedKey,* : semblaGroupedView
syntax "grouped" "view" ident ":=" "count" ident "by" semblaGroupedKey,*
  "where" semblaExpr : semblaGroupedView

declare_syntax_cat semblaBoxView
syntax semblaView : semblaBoxView
syntax semblaGroupedView : semblaBoxView

declare_syntax_cat semblaSummaryReduce
syntax "sum" : semblaSummaryReduce
syntax "min" : semblaSummaryReduce
syntax "max" : semblaSummaryReduce
syntax "last" : semblaSummaryReduce
syntax "argmax_tick" : semblaSummaryReduce

declare_syntax_cat semblaSummary
syntax "summary" ident "from" ident "view" ident "reduce" semblaSummaryReduce : semblaSummary

declare_syntax_cat semblaSummaryBlock
syntax "summaries" "[" semblaSummary,* "]" : semblaSummaryBlock

declare_syntax_cat semblaBox
syntax "box" ident "where"
  "systems" "[" semblaSystem,* "]"
  "inputs" "[" semblaInput,* "]"
  "transitions" "[" semblaTransition,* "]"
  "outputs" "[" semblaOutput,* "]" : semblaBox
syntax "box" ident "where"
  "systems" "[" semblaSystem,* "]"
  "inputs" "[" semblaInput,* "]"
  "transitions" "[" semblaTransition,* "]"
  "outputs" "[" semblaOutput,* "]"
  "views" "[" semblaBoxView,* "]" : semblaBox

declare_syntax_cat semblaWire
syntax "wire" ident ident "->" ident ident : semblaWire

/-- Command-only layout grammar. These nodes retain the user's tokens and are
    collected directly into the shared surface graph below. -/
declare_syntax_cat semblaCommandAttr
syntax ident ":" "{" ident,* "}" : semblaCommandAttr
syntax ident ":" "ℝ" : semblaCommandAttr
syntax ident ":" "Int" : semblaCommandAttr
syntax ident ":" ident : semblaCommandAttr

declare_syntax_cat semblaCommandRows
syntax num ident : semblaCommandRows
syntax term : semblaCommandRows

declare_syntax_cat semblaCommandSystem
syntax "system" ident "(" "rows" ":=" semblaCommandRows ")" : semblaCommandSystem
syntax "system" ident "(" ident ":=" str ")" "(" "rows" ":=" semblaCommandRows ")" : semblaCommandSystem
syntax "system" ident "(" "rows" ":=" semblaCommandRows ")" "where"
  many1Indent(ppLine semblaCommandAttr) : semblaCommandSystem
syntax "system" ident "(" ident ":=" str ")" "(" "rows" ":=" semblaCommandRows ")" "where"
  many1Indent(ppLine semblaCommandAttr) : semblaCommandSystem

declare_syntax_cat semblaCommandInput
syntax "input" ident "where" many1Indent(ppLine semblaCommandAttr) : semblaCommandInput

declare_syntax_cat semblaTypedBinder
syntax ident ":" ident : semblaTypedBinder

declare_syntax_cat semblaStateAtom
syntax ident ":=" semblaExpr : semblaStateAtom
syntax "match" semblaExpr : semblaStateAtom

declare_syntax_cat semblaStateDecl
syntax ident ident "on" ident "where" many1Indent(ppLine semblaStateAtom) : semblaStateDecl
syntax ident ident "(" semblaTypedBinder,* ")" "on" ident "where"
  many1Indent(ppLine semblaStateAtom) : semblaStateDecl

declare_syntax_cat semblaStateApplication
syntax ident : semblaStateApplication
syntax ident "(" ident,* ")" : semblaStateApplication

declare_syntax_cat semblaRelationConstraint
syntax ident "=" ident : semblaRelationConstraint
syntax ident "≠" ident : semblaRelationConstraint

declare_syntax_cat semblaRelationItem
syntax "from" sepBy1(semblaStateApplication, ",") : semblaRelationItem
syntax "hazard" semblaExpr : semblaRelationItem
syntax ident ident "by" ident : semblaRelationItem
syntax "set" semblaSet : semblaRelationItem
syntax ident semblaStateApplication : semblaRelationItem

declare_syntax_cat semblaRelationDecl
syntax ident ident "(" semblaTypedBinder,* ")" "on" ident "where"
  many1Indent(ppLine semblaRelationItem) : semblaRelationDecl
syntax ident ident "(" semblaTypedBinder,* ")" "on" ident ident ident
  sepBy1(semblaRelationConstraint, ",") "where"
  many1Indent(ppLine semblaRelationItem) : semblaRelationDecl

declare_syntax_cat semblaCommandTransitionItem
syntax "guard" semblaExpr : semblaCommandTransitionItem
syntax "hazard" semblaExpr : semblaCommandTransitionItem
syntax "set" semblaSet : semblaCommandTransitionItem
syntax "contest" ident "by" ident : semblaCommandTransitionItem
syntax "contest" ident : semblaCommandTransitionItem

declare_syntax_cat semblaCommandGeneralTransition
syntax "transition" ident "[" ident,* "]" "on" ident "where"
  many1Indent(ppLine semblaCommandTransitionItem) : semblaCommandGeneralTransition
syntax "transition" ident "[" semblaTypedBinder,* "]" "on" ident "where"
  many1Indent(ppLine semblaCommandTransitionItem) : semblaCommandGeneralTransition
syntax "transition" ident "on" ident "where"
  many1Indent(ppLine semblaCommandTransitionItem) : semblaCommandGeneralTransition

declare_syntax_cat semblaCommandOutputField
syntax ident ":" "Int" ":=" "count" "where" semblaExpr : semblaCommandOutputField
syntax ident ":" "ℝ" ":=" "count" "where" semblaExpr : semblaCommandOutputField
syntax ident ":" "Int" ":=" "sum" "(" semblaExpr ")" : semblaCommandOutputField
syntax ident ":" "ℝ" ":=" "sum" "(" semblaExpr ")" : semblaCommandOutputField
-- Recovery-only count-with-value forms are rejected deliberately by the collector.
syntax ident ":" "Int" ":=" "count" "(" semblaExpr ")" "where" semblaExpr : semblaCommandOutputField
syntax ident ":" "ℝ" ":=" "count" "(" semblaExpr ")" "where" semblaExpr : semblaCommandOutputField

declare_syntax_cat semblaCommandOutput
syntax "output" ident "from" ident "where"
  many1Indent(ppLine semblaCommandOutputField) : semblaCommandOutput

declare_syntax_cat semblaCommandViewReduce
syntax "sum" : semblaCommandViewReduce
syntax "min" : semblaCommandViewReduce
syntax "max" : semblaCommandViewReduce

declare_syntax_cat semblaCommandView
syntax "view" ident ":=" "count" ident : semblaCommandView
syntax "view" ident ":=" "count" ident "where" semblaExpr : semblaCommandView
syntax "view" ident ":=" "count" ident "using" semblaExpr : semblaCommandView
syntax "view" ident ":=" "count" ident "where" semblaExpr "using" semblaExpr : semblaCommandView
syntax "view" ident ":=" semblaCommandViewReduce ident : semblaCommandView
syntax "view" ident ":=" semblaCommandViewReduce ident "where" semblaExpr : semblaCommandView
syntax "view" ident ":=" semblaCommandViewReduce ident "using" semblaExpr : semblaCommandView
syntax "view" ident ":=" semblaCommandViewReduce ident "where" semblaExpr
  "using" semblaExpr : semblaCommandView

/-- A table-scoped observation entry. The presence of `by` selects grouped-view
    lowering; otherwise the entry lowers to an ordinary view. -/
declare_syntax_cat semblaCommandScopedView
syntax ident ":=" "count" : semblaCommandScopedView
syntax ident ":=" "count" "where" semblaExpr : semblaCommandScopedView
syntax ident ":=" semblaCommandViewReduce semblaExpr : semblaCommandScopedView
syntax ident ":=" semblaCommandViewReduce semblaExpr "where" semblaExpr : semblaCommandScopedView
syntax ident ":=" "count" "by" semblaScopedGroupedKey,* : semblaCommandScopedView
syntax ident ":=" "count" "where" semblaExpr "by" semblaScopedGroupedKey,* : semblaCommandScopedView

declare_syntax_cat semblaCommandViews
syntax "views" ident "where"
  many1Indent(ppLine semblaCommandScopedView) : semblaCommandViews

declare_syntax_cat semblaCommandBoxItem
syntax semblaCommandSystem : semblaCommandBoxItem
syntax semblaCommandInput : semblaCommandBoxItem
syntax semblaCommandGeneralTransition : semblaCommandBoxItem
syntax semblaStateDecl : semblaCommandBoxItem
syntax semblaRelationDecl : semblaCommandBoxItem
syntax semblaTransition : semblaCommandBoxItem
syntax semblaCommandOutput : semblaCommandBoxItem
syntax semblaCommandView : semblaCommandBoxItem
syntax semblaGroupedView : semblaCommandBoxItem
syntax semblaCommandViews : semblaCommandBoxItem
syntax "contest" ident : semblaCommandBoxItem

declare_syntax_cat semblaCommandBox
syntax "box" ident "where" manyIndent(ppLine semblaCommandBoxItem) : semblaCommandBox

declare_syntax_cat semblaCommandSummaryReduce
syntax "sum" : semblaCommandSummaryReduce
syntax "min" : semblaCommandSummaryReduce
syntax "max" : semblaCommandSummaryReduce
syntax "last" : semblaCommandSummaryReduce
syntax "argmaxₜ" : semblaCommandSummaryReduce

declare_syntax_cat semblaCommandSummary
syntax "summary" ident ":=" semblaCommandSummaryReduce ident : semblaCommandSummary

declare_syntax_cat semblaCommandScopedSummary
syntax ident ":=" semblaCommandSummaryReduce ident : semblaCommandScopedSummary

declare_syntax_cat semblaCommandSummaries
syntax "summaries" ident "where"
  many1Indent(ppLine semblaCommandScopedSummary) : semblaCommandSummaries

declare_syntax_cat semblaIndexDecl
-- Parse the leading word as an identifier and validate it below so adding these
-- model-local forms does not reserve `index` or `domain` in importing modules.
syntax ident ident ":=" num ".." num : semblaIndexDecl
syntax ident ident ":=" "{" ident,* "}" : semblaIndexDecl

declare_syntax_cat semblaPartitionCell
syntax ident ":=" num "..<" num : semblaPartitionCell
syntax ident ":=" num ".." : semblaPartitionCell

declare_syntax_cat semblaPartitionDecl
syntax ident ident "projects" ident "where" many1Indent(ppLine semblaPartitionCell) : semblaPartitionDecl

declare_syntax_cat semblaFamilyKey
syntax ident : semblaFamilyKey
syntax num : semblaFamilyKey

declare_syntax_cat semblaFamilyCell
syntax "[" semblaFamilyKey,* "]" ":=" term "~" ident semblaRealTerm semblaRealTerm : semblaFamilyCell
syntax "[" semblaFamilyKey,* "]" ":=" term : semblaFamilyCell

declare_syntax_cat semblaExprFunctionCell
syntax "[" semblaFamilyKey,* "]" ":=" ident : semblaExprFunctionCell
syntax "[" semblaFamilyKey,* "]" ":=" scientific : semblaExprFunctionCell
syntax "[" semblaFamilyKey,* "]" ":=" num : semblaExprFunctionCell
syntax "[" semblaFamilyKey,* "]" ":=" "(" semblaExpr ")" : semblaExprFunctionCell

declare_syntax_cat semblaParamFamily
syntax "param" ident "[" ident,* "]" ":" "ℝ" "where"
  many1Indent(ppLine semblaFamilyCell) : semblaParamFamily
syntax "param" ident "[" ident,* "]" ":" "Int" "where"
  many1Indent(ppLine semblaFamilyCell) : semblaParamFamily
syntax "param" ident "[" ident,* "]" ":" "ℝ" "from" ident str ident str : semblaParamFamily
syntax "param" ident "[" ident,* "]" ":" "Int" "from" ident str ident str : semblaParamFamily
syntax "param" ident "[" ident,* "]" ":" "ℝ" "from" ident str ident str ident str : semblaParamFamily
syntax "param" ident "[" ident,* "]" ":" "Int" "from" ident str ident str ident str : semblaParamFamily
syntax "param" ident "(" semblaTypedBinder,* ")" ":" "ℝ" "where"
  many1Indent(ppLine semblaFamilyCell) : semblaParamFamily
syntax "param" ident "(" semblaTypedBinder,* ")" ":" "Int" "where"
  many1Indent(ppLine semblaFamilyCell) : semblaParamFamily
syntax "param" ident "(" semblaTypedBinder,* ")" ":" "ℝ" "from" ident str ident str : semblaParamFamily
syntax "param" ident "(" semblaTypedBinder,* ")" ":" "Int" "from" ident str ident str : semblaParamFamily
syntax "param" ident "(" semblaTypedBinder,* ")" ":" "ℝ" "from" ident str ident str ident str : semblaParamFamily
syntax "param" ident "(" semblaTypedBinder,* ")" ":" "Int" "from" ident str ident str ident str : semblaParamFamily

declare_syntax_cat semblaExprFunctionDecl
syntax ident ident "(" semblaTypedBinder,* ")" ":" "ℝ" "where"
  many1Indent(ppLine semblaExprFunctionCell) : semblaExprFunctionDecl
syntax ident ident "(" semblaTypedBinder,* ")" ":" "Int" "where"
  many1Indent(ppLine semblaExprFunctionCell) : semblaExprFunctionDecl

declare_syntax_cat semblaCommandModelItem
syntax semblaIndexDecl : semblaCommandModelItem
syntax semblaPartitionDecl : semblaCommandModelItem
syntax semblaExprFunctionDecl : semblaCommandModelItem
syntax semblaParamFamily : semblaCommandModelItem
syntax semblaParam : semblaCommandModelItem
syntax semblaCommandBox : semblaCommandModelItem
syntax semblaWire : semblaCommandModelItem
syntax semblaCommandSummary : semblaCommandModelItem
syntax semblaCommandSummaries : semblaCommandModelItem
syntax "contest" ident : semblaCommandModelItem

syntax (name := semblaModelCommand) "sembla_model" ident
  "(" "dt" ":=" term ")" "where"
  manyIndent(ppLine semblaCommandModelItem) : command
syntax (name := semblaNamedModelCommand) "sembla_model" ident
  "(" ident ":=" str ")" "(" "dt" ":=" term ")" "where"
  manyIndent(ppLine semblaCommandModelItem) : command
-- Recovery headers used only for a deliberate mandatory-dt diagnostic.
syntax (name := semblaModelMissingDtCommand) "sembla_model" ident "where"
  manyIndent(ppLine semblaCommandModelItem) : command
syntax (name := semblaNamedModelMissingDtCommand) "sembla_model" ident
  "(" ident ":=" str ")" "where"
  manyIndent(ppLine semblaCommandModelItem) : command
-- Recovery for a common dedented box declaration.
syntax (name := semblaMisplacedSystemCommand) "system" ident "(" "rows" ":=" term ")" : command

private def identText (stx : TSyntax `ident) : String := stx.getId.getString!

private def isAsciiLower (c : Char) : Bool := 'a'.toNat ≤ c.toNat && c.toNat ≤ 'z'.toNat
private def isAsciiUpper (c : Char) : Bool := 'A'.toNat ≤ c.toNat && c.toNat ≤ 'Z'.toNat
private def isAsciiLetter (c : Char) : Bool := isAsciiLower c || isAsciiUpper c
private def isAsciiDigit (c : Char) : Bool := '0'.toNat ≤ c.toNat && c.toNat ≤ '9'.toNat

private def greekRuntimeComponent? : Char → Option String
  | 'β' => some "beta"
  | 'γ' => some "gamma"
  | 'λ' => some "lambda"
  | 'μ' => some "mu"
  | 'σ' => some "sigma"
  | 'τ' => some "tau"
  | 'θ' => some "theta"
  | _ => none

private def snakeCaseAscii : Option Char → List Char → List Char
  | _, [] => []
  | previous, current :: rest =>
      let previousStartsBoundary := match previous with
        | some value => isAsciiLower value || isAsciiDigit value
        | none => false
      let acronymBoundary := match previous, rest with
        | some value, next :: _ => isAsciiUpper value && isAsciiLower next
        | _, _ => false
      let boundary := isAsciiUpper current && (previousStartsBoundary || acronymBoundary)
      (if boundary then ['_'] else []) ++ [current.toLower] ++
        snakeCaseAscii (some current) rest

/-- Derive the frozen runtime name from one accepted surface identifier. -/
private def deriveRuntimeName (source : String) : Except String String := do
  let chars := source.toList
  let first ← match chars.head? with
    | some value => pure value
    | none => throw "identifier must not be empty"
  if first == '_' then
    throw s!"identifier '{source}' has an unsupported separator pattern"
  unless isAsciiLetter first || (greekRuntimeComponent? first).isSome do
    if isAsciiDigit first then
      throw s!"identifier '{source}' must begin with an ASCII letter or documented Greek letter"
    throw s!"identifier '{source}' contains unsupported character '{first}'"
  let mut previousWasUnderscore := false
  let mut expanded : List Char := []
  for current in chars do
    if current == '_' then
      if previousWasUnderscore then
        throw s!"identifier '{source}' has an unsupported separator pattern"
      previousWasUnderscore := true
      expanded := expanded ++ ['_']
    else
      previousWasUnderscore := false
      if isAsciiLetter current || isAsciiDigit current then
        expanded := expanded ++ [current]
      else
        match greekRuntimeComponent? current with
        | some replacement => expanded := expanded ++ replacement.toList
        | none => throw s!"identifier '{source}' contains unsupported character '{current}'"
  if previousWasUnderscore then
    throw s!"identifier '{source}' has an unsupported separator pattern"
  pure (String.mk (snakeCaseAscii none expanded))

/-- Derive an identifier used only as a suffix component. A numeric-leading
    escaped domain member is safe here because the already-validated family or
    relation base precedes it. All separator and character rules remain the
    same as for standalone runtime names. -/
private def deriveRuntimeComponent (source : String) : Except String String := do
  let chars := source.toList
  let first ← match chars.head? with
    | some value => pure value
    | none => throw "identifier must not be empty"
  if !isAsciiDigit first then return ← deriveRuntimeName source
  let mut previousWasUnderscore := false
  let mut expanded : List Char := []
  for current in chars do
    if current == '_' then
      if previousWasUnderscore then
        throw s!"identifier '{source}' has an unsupported separator pattern"
      previousWasUnderscore := true
      expanded := expanded ++ ['_']
    else
      previousWasUnderscore := false
      if isAsciiLetter current || isAsciiDigit current then
        expanded := expanded ++ [current]
      else
        match greekRuntimeComponent? current with
        | some replacement => expanded := expanded ++ replacement.toList
        | none => throw s!"identifier '{source}' contains unsupported character '{current}'"
  if previousWasUnderscore then
    throw s!"identifier '{source}' has an unsupported separator pattern"
  pure (String.mk (snakeCaseAscii none expanded))

/-- Derive the frozen runtime name at the original identifier token. Surface
    frontends reuse this entry point so name rules and positioned diagnostics
    remain identical. -/
def deriveRuntimeNameAt (token : TSyntax `ident) : TermElabM String := do
  match deriveRuntimeName (identText token) with
  | .ok name => pure name
  | .error message => throwErrorAt token message

structure SurfaceFamilyCell where
  key : List (ParameterTable.IndexMember × Syntax)
  default : TSyntax `term
  «prior» : Option SurfacePrior
  token : Syntax

inductive SurfaceFamilySource where
  | inline (cells : List SurfaceFamilyCell)
  | external (format : ParameterTable.Format) (path : String) (pathToken : Syntax)
      (digest : String) (shaToken : Syntax)

structure SurfaceFamilyDecl where
  sourceName : String
  name : String
  token : Syntax
  dimensions : List (String × Syntax)
  ty : SurfaceTy
  source : SurfaceFamilySource
  domains : List (String × Syntax) := []
  callNotation : Bool := false

inductive SurfaceParamItem where
  | scalar (declaration : SurfaceParam)
  | family (declaration : SurfaceFamilyDecl)

private def realSurfaceTerm (stx : TSyntax `semblaRealTerm) : TermElabM (TSyntax `term) := do
  match stx with
  | `(semblaRealTerm| $value:scientific) => `(term| $value)
  | `(semblaRealTerm| -$value:scientific) => `(term| -$value)
  | `(semblaRealTerm| $name:ident) => `(term| $name)
  | `(semblaRealTerm| ($value:term)) => pure value
  | _ => throwUnsupportedSyntax

private def scientificText (stx : TSyntax `scientific) : Option String :=
  match stx.raw with
  | .node _ _ #[.atom _ value] => some value
  | _ => none

/-- A conservative decimal order check keeps every emitted real inside Rust
    `f64`'s finite, non-underflowing range.  The supported fixtures are far
    inside these bounds; rejecting fringe subnormals is preferable to emitting
    JSON that Rust rounds to zero or infinity. -/
private def scientificOrder (text : String) : Option (Bool × _root_.Int) := do
  let exponentParts := if text.contains 'e' then text.splitOn "e" else text.splitOn "E"
  let (mantissa, explicitExponent) ← match exponentParts with
    | [mantissa] => some (mantissa, 0)
    | [mantissa, exponent] => some (mantissa, ← exponent.toInt?)
    | _ => none
  let decimalParts := mantissa.splitOn "."
  let fractionalDigits ← match decimalParts with
    | [_] => some 0
    | [_, fraction] => some fraction.length
    | _ => none
  let digits := (mantissa.replace "." "").dropWhile (· == '0')
  if digits.isEmpty then
    pure (true, 0)
  else
    pure (false, explicitExponent - Int.ofNat fractionalDigits + Int.ofNat digits.length - 1)

private def validateScientific (stx : TSyntax `scientific) (positive : Bool) : TermElabM Unit := do
  let some text := scientificText stx
    | throwErrorAt stx "invalid decimal literal"
  let some (isZero, order) := scientificOrder text
    | throwErrorAt stx "invalid decimal literal"
  if positive && isZero then
    throwErrorAt stx "tick width must be greater than zero"
  if !isZero && (order > 307 || order < -323) then
    throwErrorAt stx "decimal literal is outside the supported finite f64 range"

private def validateRealTerm (stx : TSyntax `term) : TermElabM Unit := do
  match stx with
  | `(term| $value:scientific) => validateScientific value false
  | `(term| -$value:scientific) => validateScientific value false
  | _ => throwErrorAt stx "real declarations require a decimal or scientific literal"

private def validateIntTerm (stx : TSyntax `term) : TermElabM Unit := do
  match stx with
  | `(term| $value:num) =>
      if value.raw.isNatLit?.getD 0 > 9223372036854775807 then
        throwErrorAt stx "integer literal is outside the supported i64 range"
  | `(term| -$value:num) =>
      if value.raw.isNatLit?.getD 0 > 9223372036854775808 then
        throwErrorAt stx "integer literal is outside the supported i64 range"
  | _ => throwErrorAt stx "Int parameter defaults require an integer literal"

private def validateStep (stx : TSyntax `term) : TermElabM Unit := do
  match stx with
  | `(term| $value:scientific) => validateScientific value true
  | _ => throwErrorAt stx "tick width must be a positive decimal or scientific literal"

private def validateSize (stx : TSyntax `term) : TermElabM Unit := do
  match stx.raw.isNatLit? with
  | some value =>
      if value > 18446744073709551615 then
        throwErrorAt stx "row count exceeds the IR u64 range"
  | none => throwErrorAt stx "row count must be a natural-number literal"

private def parseAttr (stx : TSyntax `semblaAttr) : TermElabM SurfaceAttr := do
  match stx with
  | `(semblaAttr| $keyword:ident $name:ident : { $variants:ident,* }) =>
      unless identText keyword == "state" do throwUnsupportedSyntax
      let variantTokens := variants.getElems.toList.map fun variant =>
        (identText variant, variant.raw)
      pure {
        name := identText name
        ty := .enum (variantTokens.map (·.1))
        nameToken := name.raw
        variantTokens := variantTokens }
  | `(semblaAttr| attr $name:ident : Real) | `(semblaAttr| $name:ident : Real)
  | `(semblaAttr| attr $name:ident : ℝ) | `(semblaAttr| $name:ident : ℝ) =>
      pure { name := identText name, ty := .real, nameToken := name.raw }
  | `(semblaAttr| attr $name:ident : Int) | `(semblaAttr| $name:ident : Int) =>
      pure { name := identText name, ty := .int, nameToken := name.raw }
  | `(semblaAttr| ref $name:ident : $target:ident) =>
      pure {
        name := identText name
        ty := .ref (identText target)
        nameToken := name.raw
        refTargetToken := some target.raw }
  | _ => throwUnsupportedSyntax

/-- Parse one parameter declaration through the shared surface kernel. -/
def parseSurfaceParam (stx : TSyntax `semblaParam) : TermElabM SurfaceParam := do
  match stx with
  | `(semblaParam| param $name:ident : ℝ := $default:term ~ $family:ident
        $a:semblaRealTerm $b:semblaRealTerm) =>
      let priorFamily ← match identText family with
        | "LogNormal" => pure SurfacePriorFamily.logNormal
        | "Normal" => pure SurfacePriorFamily.normal
        | other => throwErrorAt family s!"unknown prior family '{other}'"
      pure ⟨identText name, ← deriveRuntimeNameAt name, name.raw, .real, default,
        some (← realSurfaceTerm a, ← realSurfaceTerm b), priorFamily⟩
  | `(semblaParam| param $name:ident : ℝ := $default:term) =>
      pure ⟨identText name, ← deriveRuntimeNameAt name, name.raw, .real, default, none, .logNormal⟩
  | `(semblaParam| param $name:ident : Real := $default:term prior $family:ident($a:term, $b:term)) =>
      unless identText family == "LogNormal" do
        throwErrorAt family "legacy Real prior family must be 'LogNormal'"
      pure ⟨identText name, identText name, name.raw, .real, default,
        some (a, b), .logNormal⟩
  | `(semblaParam| param $name:ident : Real := $default:term) =>
      pure ⟨identText name, identText name, name.raw, .real, default, none, .logNormal⟩
  | `(semblaParam| param $name:ident : Int := $_default:term ~ $_family:ident
        $_a:semblaRealTerm $_b:semblaRealTerm) =>
      throwErrorAt name "priors are not supported on Int parameters"
  | `(semblaParam| param $name:ident : Int := $default:term) =>
      pure ⟨identText name, ← deriveRuntimeNameAt name, name.raw, .int, default, none, .logNormal⟩
  | _ => throwUnsupportedSyntax

private def parseIndexDecl (stx : TSyntax `semblaIndexDecl) : TermElabM SurfaceIndex := do
  match stx with
  | `(semblaIndexDecl| $keyword:ident $name:ident := $lower:num .. $upper:num) =>
      unless identText keyword == "index" || identText keyword == "domain" do throwUnsupportedSyntax
      let lowerValue := lower.raw.isNatLit?.getD 0
      let upperValue := upper.raw.isNatLit?.getD 0
      if lowerValue > upperValue then
        throwErrorAt upper "index range lower bound must not exceed its upper bound"
      pure (SurfaceDomain.mk (identText name) name.raw (.range lowerValue upperValue)
        [] (identText keyword == "index"))
  | `(semblaIndexDecl| $keyword:ident $name:ident := { $members:ident,* }) =>
      unless identText keyword == "index" || identText keyword == "domain" do throwUnsupportedSyntax
      let values := members.getElems.toList.map identText
      if values.isEmpty then throwErrorAt name "enum index must declare at least one member"
      let mut seen : List String := []
      for member in members.getElems do
        let value := identText member
        if seen.contains value then throwErrorAt member "duplicate index member '{value}'"
        seen := value :: seen
      pure (SurfaceDomain.mk (identText name) name.raw (.enumeration values)
        (members.getElems.toList.map fun member => (identText member, member.raw))
        (identText keyword == "index"))
  | _ => throwUnsupportedSyntax

private def parsePartitionCell (stx : TSyntax `semblaPartitionCell) :
    TermElabM SurfacePartitionMember := do
  match stx with
  | `(semblaPartitionCell| $label:ident := $lower:num ..< $upper:num) =>
      let lowerValue := lower.raw.isNatLit?.getD 0
      let upperValue := upper.raw.isNatLit?.getD 0
      if lowerValue >= upperValue then
        throwErrorAt upper "partition cell upper bound must be greater than its lower bound"
      pure ⟨identText label, label.raw, lowerValue, some upperValue⟩
  | `(semblaPartitionCell| $label:ident := $lower:num ..) =>
      pure ⟨identText label, label.raw, lower.raw.isNatLit?.getD 0, none⟩
  | _ => throwUnsupportedSyntax

private def parsePartitionDecl (stx : TSyntax `semblaPartitionDecl) :
    TermElabM SurfacePartition := do
  match stx with
  | `(semblaPartitionDecl| $keyword:ident $name:ident projects $target:ident where
        $cells:semblaPartitionCell*) =>
      unless identText keyword == "partition" do throwUnsupportedSyntax
      let components := target.getId.components
      let (boxName, systemName, attrName) ← match components with
        | [boxName, systemName, attrName] =>
            pure (boxName.getString!, systemName.getString!, attrName.getString!)
        | _ => throwErrorAt target
            "partition projection target must have the form 'box.System.attribute'"
      let members ← cells.toList.mapM parsePartitionCell
      if members.isEmpty then throwErrorAt name "partition must declare at least one cell"
      let mut seen : List String := []
      let mut expectedLower : Option Nat := none
      let mut sawOpen := false
      for member in members do
        if seen.contains member.label then
          throwErrorAt member.token "duplicate partition member '{member.label}'"
        seen := member.label :: seen
        if sawOpen then throwErrorAt member.token "open partition cell must be final"
        if let some expected := expectedLower then
          unless member.lower == expected do
            throwErrorAt member.token
              "partition cells must be contiguous; expected lower bound {expected}"
        match member.upper with
        | some upper => expectedLower := some upper
        | none => sawOpen := true
      unless sawOpen do throwErrorAt name "partition must end with one open interval"
      pure ⟨identText name, name.raw, ⟨boxName, systemName, attrName, target.raw⟩, members⟩
  | _ => throwUnsupportedSyntax

private def parseExprFunctionCell (stx : TSyntax `semblaExprFunctionCell) :
    TermElabM SurfaceExprFunctionCell := do
  let finish (keys : Array (TSyntax `semblaFamilyKey))
      (expression : TSyntax `semblaExpr) := do
      let parsedKeys ← keys.toList.mapM fun (key : TSyntax `semblaFamilyKey) =>
        match key with
        | `(semblaFamilyKey| $value:num) =>
            pure (.int (value.raw.isNatLit?.getD 0), value.raw)
        | `(semblaFamilyKey| $value:ident) => pure (.enum (identText value), value.raw)
        | _ => throwUnsupportedSyntax
      pure ⟨parsedKeys, expression, stx.raw⟩
  match stx with
  | `(semblaExprFunctionCell| [$keys:semblaFamilyKey,*] := $value:ident) =>
      finish keys (← `(semblaExpr| $value:ident))
  | `(semblaExprFunctionCell| [$keys:semblaFamilyKey,*] := $value:scientific) =>
      finish keys (← `(semblaExpr| $value:scientific))
  | `(semblaExprFunctionCell| [$keys:semblaFamilyKey,*] := $value:num) =>
      finish keys (← `(semblaExpr| $value:num))
  | `(semblaExprFunctionCell| [$keys:semblaFamilyKey,*] := ($value:semblaExpr)) =>
      finish keys value
  | _ => throwUnsupportedSyntax

private def parseExprFunctionDecl (stx : TSyntax `semblaExprFunctionDecl) :
    TermElabM SurfaceExprFunction := do
  let finish (keyword name : TSyntax `ident)
      (args : Array (TSyntax `semblaTypedBinder)) (ty : SurfaceTy)
      (cells : Array (TSyntax `semblaExprFunctionCell)) := do
    unless identText keyword == "function" do throwUnsupportedSyntax
    let parsedArgs ← args.toList.mapM fun (arg : TSyntax `semblaTypedBinder) =>
      match arg with
      | `(semblaTypedBinder| $argName:ident : $domain:ident) =>
          pure ⟨identText argName, argName.raw, identText domain, domain.raw⟩
      | _ => throwUnsupportedSyntax
    if parsedArgs.isEmpty then throwErrorAt name "expression function requires at least one argument"
    pure ⟨identText name, name.raw, parsedArgs, ty,
      ← cells.toList.mapM parseExprFunctionCell⟩
  match stx with
  | `(semblaExprFunctionDecl| $keyword:ident $name:ident ($args:semblaTypedBinder,*) : ℝ where
        $cells:semblaExprFunctionCell*) => finish keyword name args .real cells
  | `(semblaExprFunctionDecl| $keyword:ident $name:ident ($args:semblaTypedBinder,*) : Int where
        $cells:semblaExprFunctionCell*) => finish keyword name args .int cells
  | _ => throwUnsupportedSyntax

private def parseFamilyKey (stx : TSyntax `semblaFamilyKey) :
    TermElabM (ParameterTable.IndexMember × Syntax) := do
  match stx with
  | `(semblaFamilyKey| $value:num) => pure (.int (value.raw.isNatLit?.getD 0), value.raw)
  | `(semblaFamilyKey| $value:ident) => pure (.enum (identText value), value.raw)
  | _ => throwUnsupportedSyntax

private def parseFamilyCell (stx : TSyntax `semblaFamilyCell) : TermElabM SurfaceFamilyCell := do
  match stx with
  | `(semblaFamilyCell| [$keys:semblaFamilyKey,*] := $default:term ~ $family:ident
        $a:semblaRealTerm $b:semblaRealTerm) =>
      let priorFamily ← match identText family with
        | "LogNormal" => pure SurfacePriorFamily.logNormal
        | "Normal" => pure SurfacePriorFamily.normal
        | other => throwErrorAt family s!"unknown prior family '{other}'"
      pure ⟨← keys.getElems.toList.mapM parseFamilyKey, default,
        some { family := priorFamily, args := (← realSurfaceTerm a, ← realSurfaceTerm b) }, stx.raw⟩
  | `(semblaFamilyCell| [$keys:semblaFamilyKey,*] := $default:term) =>
      pure ⟨← keys.getElems.toList.mapM parseFamilyKey, default, none, stx.raw⟩
  | _ => throwUnsupportedSyntax

private def parseTypedBinder (stx : TSyntax `semblaTypedBinder) :
    TermElabM SurfaceFunctionArg := do
  match stx with
  | `(semblaTypedBinder| $name:ident : $domain:ident) =>
      pure ⟨identText name, name.raw, identText domain, domain.raw⟩
  | _ => throwUnsupportedSyntax

private def parseParamFamily (stx : TSyntax `semblaParamFamily) : TermElabM SurfaceFamilyDecl := do
  let inline (name : TSyntax `ident) (dimensions : Array (TSyntax `ident))
      (ty : SurfaceTy) (cells : Array (TSyntax `semblaFamilyCell)) := do
    let runtimeName ← deriveRuntimeNameAt name
    let parsedCells ← cells.toList.mapM parseFamilyCell
    pure (SurfaceFamilyDecl.mk (identText name) runtimeName name.raw
      (dimensions.toList.map fun token => (identText token, token.raw)) ty
      (.inline parsedCells) [] false)
  let inlineFunction (name : TSyntax `ident) (args : Array (TSyntax `semblaTypedBinder))
      (ty : SurfaceTy) (cells : Array (TSyntax `semblaFamilyCell)) := do
    let parsed ← args.toList.mapM parseTypedBinder
    let runtimeName ← deriveRuntimeNameAt name
    let parsedCells ← cells.toList.mapM parseFamilyCell
    pure (SurfaceFamilyDecl.mk (identText name) runtimeName name.raw
      (parsed.map fun arg => (arg.name, arg.token)) ty (.inline parsedCells)
      (parsed.map fun arg => (arg.domainName, arg.domainToken)) true)
  let external (name : TSyntax `ident) (dimensions : Array (TSyntax `ident))
      (ty : SurfaceTy) (format : ParameterTable.Format) (path digest : TSyntax `str) := do
    let runtimeName ← deriveRuntimeNameAt name
    pure (SurfaceFamilyDecl.mk (identText name) runtimeName name.raw
      (dimensions.toList.map fun token => (identText token, token.raw)) ty
      (.external format path.getString path.raw digest.getString digest.raw) [] false)
  let externalFunction (name : TSyntax `ident) (args : Array (TSyntax `semblaTypedBinder))
      (ty : SurfaceTy) (format : ParameterTable.Format) (path digest : TSyntax `str) := do
    let parsed ← args.toList.mapM parseTypedBinder
    let runtimeName ← deriveRuntimeNameAt name
    pure (SurfaceFamilyDecl.mk (identText name) runtimeName name.raw
      (parsed.map fun arg => (arg.name, arg.token)) ty
      (.external format path.getString path.raw digest.getString digest.raw)
      (parsed.map fun arg => (arg.domainName, arg.domainToken)) true)
  let csvV2Format (formatKeyword schemaKeyword : TSyntax `ident)
      (schema : TSyntax `str) : TermElabM ParameterTable.Format := do
    unless identText formatKeyword == "csv" do
      throwErrorAt formatKeyword "explicit parameter-table schema is supported only for CSV"
    unless identText schemaKeyword == "schema" do
      throwErrorAt schemaKeyword "external CSV parameter table requires 'schema'"
    unless schema.getString == "sembla.parameter-family/v2" do
      throwErrorAt schema "CSV schema must be 'sembla.parameter-family/v2'"
    pure .csvV2
  match stx with
  | `(semblaParamFamily| param $name:ident [$dimensions:ident,*] : ℝ where
        $cells:semblaFamilyCell*) => inline name dimensions .real cells
  | `(semblaParamFamily| param $name:ident [$dimensions:ident,*] : Int where
        $cells:semblaFamilyCell*) => inline name dimensions .int cells
  | `(semblaParamFamily| param $name:ident [$dimensions:ident,*] : ℝ from
        $formatKeyword:ident $path:str $schemaKeyword:ident $schema:str
        $hashKeyword:ident $digest:str) =>
      unless identText hashKeyword == "sha256" do
        throwErrorAt hashKeyword "external parameter table requires 'sha256'"
      external name dimensions .real (← csvV2Format formatKeyword schemaKeyword schema) path digest
  | `(semblaParamFamily| param $name:ident [$dimensions:ident,*] : Int from
        $formatKeyword:ident $path:str $schemaKeyword:ident $schema:str
        $hashKeyword:ident $digest:str) =>
      unless identText hashKeyword == "sha256" do
        throwErrorAt hashKeyword "external parameter table requires 'sha256'"
      external name dimensions .int (← csvV2Format formatKeyword schemaKeyword schema) path digest
  | `(semblaParamFamily| param $name:ident [$dimensions:ident,*] : ℝ from
        $formatKeyword:ident $path:str $hashKeyword:ident $digest:str) =>
      let format ← match identText formatKeyword with
        | "csv" => pure .csv
        | "json" => pure .json
        | other => throwErrorAt formatKeyword "unknown parameter-table format '{other}'"
      unless identText hashKeyword == "sha256" do
        throwErrorAt hashKeyword "external parameter table requires 'sha256'"
      external name dimensions .real format path digest
  | `(semblaParamFamily| param $name:ident [$dimensions:ident,*] : Int from
        $formatKeyword:ident $path:str $hashKeyword:ident $digest:str) =>
      let format ← match identText formatKeyword with
        | "csv" => pure .csv
        | "json" => pure .json
        | other => throwErrorAt formatKeyword "unknown parameter-table format '{other}'"
      unless identText hashKeyword == "sha256" do
        throwErrorAt hashKeyword "external parameter table requires 'sha256'"
      external name dimensions .int format path digest
  | `(semblaParamFamily| param $name:ident ($args:semblaTypedBinder,*) : ℝ where
        $cells:semblaFamilyCell*) => inlineFunction name args .real cells
  | `(semblaParamFamily| param $name:ident ($args:semblaTypedBinder,*) : Int where
        $cells:semblaFamilyCell*) => inlineFunction name args .int cells
  | `(semblaParamFamily| param $name:ident ($args:semblaTypedBinder,*) : ℝ from
        $formatKeyword:ident $path:str $schemaKeyword:ident $schema:str
        $hashKeyword:ident $digest:str) =>
      unless identText hashKeyword == "sha256" do
        throwErrorAt hashKeyword "external parameter table requires 'sha256'"
      externalFunction name args .real (← csvV2Format formatKeyword schemaKeyword schema) path digest
  | `(semblaParamFamily| param $name:ident ($args:semblaTypedBinder,*) : Int from
        $formatKeyword:ident $path:str $schemaKeyword:ident $schema:str
        $hashKeyword:ident $digest:str) =>
      unless identText hashKeyword == "sha256" do
        throwErrorAt hashKeyword "external parameter table requires 'sha256'"
      externalFunction name args .int (← csvV2Format formatKeyword schemaKeyword schema) path digest
  | `(semblaParamFamily| param $name:ident ($args:semblaTypedBinder,*) : ℝ from
        $formatKeyword:ident $path:str $hashKeyword:ident $digest:str) =>
      let format ← match identText formatKeyword with
        | "csv" => pure .csv | "json" => pure .json
        | other => throwErrorAt formatKeyword "unknown parameter-table format '{other}'"
      unless identText hashKeyword == "sha256" do
        throwErrorAt hashKeyword "external parameter table requires 'sha256'"
      externalFunction name args .real format path digest
  | `(semblaParamFamily| param $name:ident ($args:semblaTypedBinder,*) : Int from
        $formatKeyword:ident $path:str $hashKeyword:ident $digest:str) =>
      let format ← match identText formatKeyword with
        | "csv" => pure .csv | "json" => pure .json
        | other => throwErrorAt formatKeyword "unknown parameter-table format '{other}'"
      unless identText hashKeyword == "sha256" do
        throwErrorAt hashKeyword "external parameter table requires 'sha256'"
      externalFunction name args .int format path digest
  | _ => throwUnsupportedSyntax

private def cartesianMembers : List (List ParameterTable.IndexMember) →
    List (List ParameterTable.IndexMember)
  | [] => [[]]
  | members :: rest =>
      members.bind fun member => (cartesianMembers rest).map fun suffix => member :: suffix

private def domainCardinality : ParameterTable.IndexDomain → Nat
  | .range lower upper => upper - lower + 1
  | .enumeration members => members.length

/-- Check the cap before materialising a range or Cartesian product. -/
private def checkExpansionCardinality (kind : String) (token : Syntax)
    (domains : List ParameterTable.IndexDomain) : TermElabM Nat := do
  let limit := (← getOptions).getNat `sembla.maxFamilyExpansion 10000
  let mut expansionCount := 1
  for domain in domains do
    let cardinality := domainCardinality domain
    if cardinality > limit ||
        (cardinality != 0 && expansionCount > limit / cardinality) then
      throwErrorAt token
        "{kind} expansion exceeds sembla.maxFamilyExpansion={limit}"
    expansionCount := expansionCount * cardinality
  pure expansionCount

private def canonicalIndexComponent (member : ParameterTable.IndexMember) : Except String String :=
  match member with
  | .int value => pure (toString value)
  | .enum value => deriveRuntimeComponent value

private def familyRuntimeName (base : String) (key : List ParameterTable.IndexMember) :
    Except String String := do
  let components ← key.mapM canonicalIndexComponent
  pure (base ++ "_" ++ String.intercalate "_" components)

private def parseExternalTerm (text fileName : String) : TermElabM (TSyntax `term) := do
  match Lean.Parser.runParserCategory (← getEnv) `term text fileName with
  | .ok parsedSyntax => pure ⟨parsedSyntax⟩
  | .error message => throwError "invalid parameter-table literal '{text}': {message}"

private def externalFamilyCells (declaration : SurfaceFamilyDecl)
    (format : ParameterTable.Format) (path : String) (pathToken : Syntax)
    (expectedDigest : String) (shaToken : Syntax) : TermElabM (List SurfaceFamilyCell) := do
  match ParameterTable.validateSha256 expectedDigest with
  | .error message => throwErrorAt shaToken message
  | .ok () => pure ()
  let tablePath := System.FilePath.mk path
  if tablePath.isAbsolute then
    throwErrorAt pathToken "parameter table path must be relative to the declaring Lean source"
  let context ← readThe Lean.Core.Context
  let sourcePath := System.FilePath.mk context.fileName
  let some directory := sourcePath.parent
    | throwErrorAt pathToken "cannot resolve parameter table relative to this source file"
  let resolvedPath := directory / tablePath
  let result ← ParameterTable.loadPinned format resolvedPath
    (declaration.dimensions.map (·.1)) expectedDigest
  let table ← match result with
    | .ok table => pure table
    | .error message =>
        if message.startsWith "SHA-256" then throwErrorAt shaToken message
        else throwErrorAt pathToken message
  table.cells.mapM fun cell => do
    let default ← parseExternalTerm cell.defaultText resolvedPath.toString
    let parsedPrior ← cell.prior.mapM fun rawPrior => do
      let family := match rawPrior.family with
        | .normal => SurfacePriorFamily.normal
        | .logNormal => SurfacePriorFamily.logNormal
      pure (SurfacePrior.mk family
        (← parseExternalTerm rawPrior.args.1 resolvedPath.toString,
          ← parseExternalTerm rawPrior.args.2 resolvedPath.toString))
    pure ⟨cell.key.map fun member => (member, declaration.token), default, parsedPrior,
      declaration.token⟩

private def expandParamFamily (indexes : List SurfaceIndex) (declaration : SurfaceFamilyDecl) :
    TermElabM (SurfaceParamFamily × List SurfaceParam) := do
  if declaration.dimensions.isEmpty then
    throwErrorAt declaration.token "parameter family requires at least one dimension"
  let mut seenDimensions : List String := []
  let mut domains : List ParameterTable.IndexDomain := []
  for (dimension, token) in declaration.dimensions do
    if seenDimensions.contains dimension then throwErrorAt token "duplicate family dimension '{dimension}'"
    seenDimensions := dimension :: seenDimensions
    let lookupName := if declaration.callNotation then
      (declaration.domains.get? domains.length).map (·.1) |>.getD dimension
      else dimension
    let lookupToken := if declaration.callNotation then
      (declaration.domains.get? domains.length).map (·.2) |>.getD token
      else token
    let domain ← match indexes.find? (·.name == lookupName) with
      | some found => pure found.domain
      | none =>
          let kind := if declaration.callNotation then "domain" else "index"
          throwErrorAt lookupToken "unknown {kind} '{lookupName}'"
    domains := domains ++ [domain]
  let _ ← checkExpansionCardinality "parameter family" declaration.token domains
  let combinations := cartesianMembers (domains.map (·.members))
  let cells ← match declaration.source with
    | .inline cells => pure cells
    | .external format path pathToken sha shaToken =>
        externalFamilyCells declaration format path pathToken sha shaToken
  if declaration.ty == .int then
    for cell in cells do
      if cell.prior.isSome then
        throwErrorAt cell.token "priors are not supported on Int parameter families"
  let mut checked : List (List ParameterTable.IndexMember × SurfaceFamilyCell) := []
  for cell in cells do
    if cell.key.length != domains.length then
      throwErrorAt cell.token "parameter-family cell has {cell.key.length} keys; expected {domains.length}"
    for ((member, token), domain) in cell.key.zip domains do
      unless domain.members.contains member do
        throwErrorAt token "parameter-family key member '{member.component}' is outside its index domain"
    let key := cell.key.map (·.1)
    if checked.any (·.1 == key) then throwErrorAt cell.token "duplicate parameter-family cell"
    checked := checked ++ [(key, cell)]
  let mut params : List SurfaceParam := []
  for key in combinations do
    let cell ← match checked.find? (·.1 == key) with
      | some found => pure found.2
      | none => throwErrorAt declaration.token
          "parameter family is missing cell [{String.intercalate ", " (key.map (·.component))}]"
    let runtimeName ← match familyRuntimeName declaration.name key with
      | .ok value => pure value
      | .error message => throwErrorAt declaration.token message
    params := params ++ [{
      sourceName := runtimeName
      name := runtimeName
      token := cell.token
      ty := declaration.ty
      default := cell.default
      «prior» := cell.«prior».map (·.args)
      priorFamily := cell.«prior».map (·.family) |>.getD .logNormal }]
  let family : SurfaceParamFamily := {
    sourceName := declaration.sourceName
    name := declaration.name
    token := declaration.token
    dimensions := declaration.dimensions.map (·.1)
    dimensionTokens := declaration.dimensions.map (·.2)
    domains := if declaration.callNotation then declaration.domains.map (·.1)
      else declaration.dimensions.map (·.1)
    callNotation := declaration.callNotation
    ty := declaration.ty }
  pure (family, params)

private def parseSystem (stx : TSyntax `semblaSystem) : TermElabM SurfaceSystem := do
  match stx with
  | `(semblaSystem| system $logical:ident (rows := $size:term) where [$attrs:semblaAttr,*]) =>
      pure ⟨identText logical, logical.raw, ← deriveRuntimeNameAt logical, logical.raw, size,
        ← attrs.getElems.toList.mapM parseAttr⟩
  | `(semblaSystem| system $logical:ident ($overrideKeyword:ident := $irName:str)
        (rows := $size:term) where [$attrs:semblaAttr,*]) =>
      unless identText overrideKeyword == "name" do
        throwErrorAt overrideKeyword "expected 'name' table override"
      pure ⟨identText logical, logical.raw, irName.getString, irName.raw, size,
        ← attrs.getElems.toList.mapM parseAttr⟩
  | `(semblaSystem| system $logical:ident as $irName:str rows($size:term) where [$attrs:semblaAttr,*]) =>
      pure ⟨identText logical, logical.raw, irName.getString, irName.raw, size,
        ← attrs.getElems.toList.mapM parseAttr⟩
  | _ => throwUnsupportedSyntax

private def parseInput (stx : TSyntax `semblaInput) : TermElabM SurfaceInput := do
  match stx with
  | `(semblaInput| input $name:ident { $attrs:semblaAttr,* }) =>
      pure ⟨identText name, name.raw, ← attrs.getElems.toList.mapM parseAttr⟩
  | _ => throwUnsupportedSyntax

private def finishContest (resourceName : TSyntax `ident)
    (ordering : Option (TSyntax `ident)) : TermElabM SurfaceContest := do
  let some ordering := ordering
    | throwErrorAt resourceName "contest declaration requires 'by race_time'"
  unless identText ordering == "race_time" do
    throwErrorAt ordering
      "keyed contest orderings are not yet supported (DECISIONS §K7); expected 'race_time'"
  pure ⟨resourceName⟩

private def parseContest (stx : TSyntax `semblaContest) : TermElabM SurfaceContest := do
  match stx with
  | `(semblaContest| contest $resourceName:ident by $ordering:ident) =>
      finishContest resourceName (some ordering)
  | `(semblaContest| contest $resourceName:ident) => finishContest resourceName none
  | _ => throwUnsupportedSyntax

private def rejectReactionContest (tail : TSyntax `semblaArrowTail) :
    TermElabM SurfaceTransition := do
  match tail with
  | `(semblaArrowTail| contest $_resourceName:ident by $_ordering:ident)
  | `(semblaArrowTail| contest $_resourceName:ident) =>
      throwErrorAt tail "reaction arrows cannot declare contests; use the general transition form"
  | _ => throwErrorAt tail
      "reaction arrows cannot declare additional guards or effects; use 'transition ... where'"

private def legacyTransitionBinder (token : TSyntax `ident) : SurfaceTransitionBinder :=
  { name := identText token, token := token.raw }

private def parseTransition (stx : TSyntax `semblaTransition) : TermElabM SurfaceTransition := do
  match stx with
  | `(semblaTransition| transition $name:ident [$binders:ident,*] on $onSystem:ident where
        guard $guardExpr:semblaExpr hazard $hazardExpr:semblaExpr
        $contests:semblaContest* set [$assignments:semblaSet,*]) =>
      if binders.getElems.isEmpty then
        throwErrorAt name "indexed transition family requires at least one index"
      pure ⟨identText name, name.raw,
        .general onSystem guardExpr hazardExpr
          (← contests.toList.mapM parseContest) assignments.getElems.toList,
        binders.getElems.toList.map legacyTransitionBinder⟩
  | `(semblaTransition| transition $name:ident on $onSystem:ident where
        guard $guardExpr:semblaExpr hazard $hazardExpr:semblaExpr
        $contests:semblaContest* set [$assignments:semblaSet,*]) =>
      pure ⟨identText name, name.raw,
        .general onSystem guardExpr hazardExpr
          (← contests.toList.mapM parseContest) assignments.getElems.toList, []⟩
  | `(semblaTransition| $name:ident [$binders:ident,*] on $onSystem:ident :
        $stateAttr:ident : $source:ident → [$hazardExpr:semblaExpr] $destination:ident) =>
      if binders.getElems.isEmpty then
        throwErrorAt name "indexed transition family requires at least one index"
      pure ⟨identText name, name.raw,
        .reaction (some onSystem) (some stateAttr) source hazardExpr destination,
        binders.getElems.toList.map legacyTransitionBinder⟩
  | `(semblaTransition| $name:ident on $onSystem:ident : $sourceAlias:ident
        [$axes:ident,*] → [$hazardExpr:semblaExpr] $destinationAlias:ident) =>
      if axes.getElems.isEmpty then
        throwErrorAt sourceAlias "indexed named-state arrow requires at least one axis"
      pure ⟨identText name, name.raw,
        .namedReaction onSystem ⟨identText sourceAlias, sourceAlias.raw, []⟩
          axes.getElems.toList hazardExpr
          ⟨identText destinationAlias, destinationAlias.raw, []⟩, []⟩
  | `(semblaTransition| $name:ident on $onSystem:ident : $sourceAlias:ident
        [$axes:ident,*] → [$hazardExpr:semblaExpr] $destinationAlias:ident
        ($destinationArgs:ident,*)) =>
      if axes.getElems.isEmpty then
        throwErrorAt sourceAlias "indexed named-state arrow requires at least one axis"
      pure ⟨identText name, name.raw,
        .namedReaction onSystem ⟨identText sourceAlias, sourceAlias.raw, []⟩
          axes.getElems.toList hazardExpr
          ⟨identText destinationAlias, destinationAlias.raw, destinationArgs.getElems.toList⟩, []⟩
  | `(semblaTransition| $name:ident : $source:ident → [$hazardExpr:semblaExpr]
        $destination:ident) =>
      pure ⟨identText name, name.raw,
        .reaction none none source hazardExpr destination, []⟩
  | `(semblaTransition| $name:ident on $onSystem:ident : $source:ident →
        [$hazardExpr:semblaExpr] $destination:ident) =>
      pure ⟨identText name, name.raw,
        .reaction (some onSystem) none source hazardExpr destination, []⟩
  | `(semblaTransition| $name:ident : $stateAttr:ident : $source:ident →
        [$hazardExpr:semblaExpr] $destination:ident) =>
      pure ⟨identText name, name.raw,
        .reaction none (some stateAttr) source hazardExpr destination, []⟩
  | `(semblaTransition| $name:ident on $onSystem:ident : $stateAttr:ident :
        $source:ident → [$hazardExpr:semblaExpr] $destination:ident) =>
      pure ⟨identText name, name.raw,
        .reaction (some onSystem) (some stateAttr) source hazardExpr destination, []⟩
  | `(semblaTransition| $_name:ident : $_source:ident → [$_hazardExpr:semblaExpr]
        $_destination:ident $tail:semblaArrowTail) =>
      rejectReactionContest tail
  | `(semblaTransition| $_name:ident on $_onSystem:ident : $_source:ident →
        [$_hazardExpr:semblaExpr] $_destination:ident $tail:semblaArrowTail) =>
      rejectReactionContest tail
  | `(semblaTransition| $_name:ident : $_stateAttr:ident : $_source:ident →
        [$_hazardExpr:semblaExpr] $_destination:ident $tail:semblaArrowTail) =>
      rejectReactionContest tail
  | `(semblaTransition| $_name:ident on $_onSystem:ident : $_stateAttr:ident :
        $_source:ident → [$_hazardExpr:semblaExpr] $_destination:ident
        $tail:semblaArrowTail) =>
      rejectReactionContest tail
  | _ => throwUnsupportedSyntax

private def parseOutputField (stx : TSyntax `semblaOutputField) : TermElabM SurfaceOutputField := do
  match stx with
  | `(semblaOutputField| field $name:ident := count where $filter:semblaExpr) =>
      pure ⟨identText name, name.raw, "count", none, some filter⟩
  | `(semblaOutputField| field $name:ident := sum ($value:semblaExpr)) =>
      pure ⟨identText name, name.raw, "sum", some value, none⟩
  | _ => throwUnsupportedSyntax

private def parseOutput (stx : TSyntax `semblaOutput) : TermElabM SurfaceOutput := do
  match stx with
  | `(semblaOutput| output $name:ident { $schema:semblaAttr,* } from $fromSystem:ident
        fields [$fieldDecls:semblaOutputField,*]) =>
      pure ⟨identText name, name.raw, ← schema.getElems.toList.mapM parseAttr, fromSystem,
        ← fieldDecls.getElems.toList.mapM parseOutputField⟩
  | _ => throwUnsupportedSyntax

private def parseViewReduce (stx : TSyntax `semblaViewReduce) : TermElabM String := do
  match stx with
  | `(semblaViewReduce| sum) => pure "sum"
  | `(semblaViewReduce| count) => pure "count"
  | `(semblaViewReduce| min) => pure "min"
  | `(semblaViewReduce| max) => pure "max"
  | _ => throwUnsupportedSyntax

private def parseView (stx : TSyntax `semblaView) : TermElabM SurfaceView := do
  match stx with
  | `(semblaView| view $name:ident from $source:ident reduce $reducer:semblaViewReduce) =>
      pure ⟨identText name, name.raw, source, none, none, ← parseViewReduce reducer⟩
  | `(semblaView| view $name:ident from $source:ident where $filter:semblaExpr
        reduce $reducer:semblaViewReduce) =>
      pure ⟨identText name, name.raw, source, some filter, none, ← parseViewReduce reducer⟩
  | `(semblaView| view $name:ident from $source:ident using $value:semblaExpr
        reduce $reducer:semblaViewReduce) =>
      pure ⟨identText name, name.raw, source, none, some value, ← parseViewReduce reducer⟩
  | `(semblaView| view $name:ident from $source:ident where $filter:semblaExpr
        using $value:semblaExpr reduce $reducer:semblaViewReduce) =>
      pure ⟨identText name, name.raw, source, some filter, some value, ← parseViewReduce reducer⟩
  | _ => throwUnsupportedSyntax

private def parseGroupedKey (stx : TSyntax `semblaGroupedKey) : TermElabM SurfaceGroupKey := do
  let finishBand (keyword attrName : TSyntax `ident) (width : TSyntax `num) := do
    unless identText keyword == "band" do throwUnsupportedSyntax
    let some value := width.raw.isNatLit?
      | throwErrorAt width "grouped band width must be a positive integer literal"
    pure ⟨attrName, some (value, width.raw)⟩
  match stx with
  | `(semblaGroupedKey| $attrName:ident) => pure ⟨attrName, none⟩
  | `(semblaGroupedKey| $keyword:ident $attrName:ident $width:num) =>
      finishBand keyword attrName width
  | `(semblaGroupedKey| $keyword:ident ($attrName:ident, $width:num)) =>
      finishBand keyword attrName width
  | _ => throwUnsupportedSyntax

private def parseScopedGroupedKey (stx : TSyntax `semblaScopedGroupedKey) :
    TermElabM SurfaceGroupKey := do
  match stx with
  | `(semblaScopedGroupedKey| $attrName:ident) => pure ⟨attrName, none⟩
  | `(semblaScopedGroupedKey| $keyword:ident ($attrName:ident, $width:num)) =>
      unless identText keyword == "band" do throwUnsupportedSyntax
      let some value := width.raw.isNatLit?
        | throwErrorAt width "grouped band width must be a positive integer literal"
      pure ⟨attrName, some (value, width.raw)⟩
  | _ => throwUnsupportedSyntax

private def parseGroupedView (stx : TSyntax `semblaGroupedView) :
    TermElabM SurfaceGroupedView := do
  match stx with
  | `(semblaGroupedView| grouped view $name:ident := count $source:ident by
        $keys:semblaGroupedKey,*) =>
      pure ⟨identText name, name.raw, source, none,
        ← keys.getElems.toList.mapM parseGroupedKey, false⟩
  | `(semblaGroupedView| grouped view $name:ident := count $source:ident by
        $keys:semblaGroupedKey,* where $filter:semblaExpr) =>
      pure ⟨identText name, name.raw, source, some filter,
        ← keys.getElems.toList.mapM parseGroupedKey, false⟩
  | _ => throwUnsupportedSyntax

private def parseSummaryReduce (stx : TSyntax `semblaSummaryReduce) : TermElabM String := do
  match stx with
  | `(semblaSummaryReduce| sum) => pure "sum"
  | `(semblaSummaryReduce| min) => pure "min"
  | `(semblaSummaryReduce| max) => pure "max"
  | `(semblaSummaryReduce| last) => pure "last"
  | `(semblaSummaryReduce| argmax_tick) => pure "argmax_tick"
  | _ => throwUnsupportedSyntax

private def parseSummary (stx : TSyntax `semblaSummary) : TermElabM SurfaceSummary := do
  match stx with
  | `(semblaSummary| summary $name:ident from $boxName:ident view $viewName:ident
        reduce $reducer:semblaSummaryReduce) =>
      pure ⟨identText name, name.raw, boxName, viewName, ← parseSummaryReduce reducer⟩
  | _ => throwUnsupportedSyntax

private def parseSummaryBlock (stx : TSyntax `semblaSummaryBlock) :
    TermElabM (List SurfaceSummary) := do
  match stx with
  | `(semblaSummaryBlock| summaries [$declarations:semblaSummary,*]) =>
      declarations.getElems.toList.mapM parseSummary
  | _ => throwUnsupportedSyntax

private def parseBox (stx : TSyntax `semblaBox) : TermElabM SurfaceBox := do
  match stx with
  | `(semblaBox| box $name:ident where
        systems [$systemDecls:semblaSystem,*]
        inputs [$inputDecls:semblaInput,*]
        transitions [$transitionDecls:semblaTransition,*]
        outputs [$outputDecls:semblaOutput,*]) =>
      pure ⟨identText name, name.raw,
        ← systemDecls.getElems.toList.mapM parseSystem,
        ← inputDecls.getElems.toList.mapM parseInput,
        ← transitionDecls.getElems.toList.mapM parseTransition,
        ← outputDecls.getElems.toList.mapM parseOutput, [], [], []⟩
  | `(semblaBox| box $name:ident where
        systems [$systemDecls:semblaSystem,*]
        inputs [$inputDecls:semblaInput,*]
        transitions [$transitionDecls:semblaTransition,*]
        outputs [$outputDecls:semblaOutput,*]
        views [$viewDecls:semblaBoxView,*]) =>
      let mut scalarViews : List SurfaceView := []
      let mut groupedViews : List SurfaceGroupedView := []
      for declaration in viewDecls.getElems do
        match declaration with
        | `(semblaBoxView| $scalarDecl:semblaView) =>
            scalarViews := scalarViews ++ [← parseView scalarDecl]
        | `(semblaBoxView| $groupedDecl:semblaGroupedView) =>
            groupedViews := groupedViews ++ [← parseGroupedView groupedDecl]
        | _ => throwUnsupportedSyntax
      pure ⟨identText name, name.raw,
        ← systemDecls.getElems.toList.mapM parseSystem,
        ← inputDecls.getElems.toList.mapM parseInput,
        ← transitionDecls.getElems.toList.mapM parseTransition,
        ← outputDecls.getElems.toList.mapM parseOutput,
        scalarViews, groupedViews, []⟩
  | _ => throwUnsupportedSyntax

private def parseWire (stx : TSyntax `semblaWire) : TermElabM SurfaceWire := do
  match stx with
  | `(semblaWire| wire $fromBox:ident $fromPort:ident -> $toBox:ident $toPort:ident) =>
      pure ⟨fromBox, fromPort, toBox, toPort⟩
  | _ => throwUnsupportedSyntax

private def parseCommandAttr (stx : TSyntax `semblaCommandAttr) : TermElabM SurfaceAttr := do
  match stx with
  | `(semblaCommandAttr| $name:ident : { $variants:ident,* }) =>
      let variantTokens := variants.getElems.toList.map fun variant =>
        (identText variant, variant.raw)
      pure {
        name := identText name
        ty := .enum (variantTokens.map (·.1))
        nameToken := name.raw
        variantTokens := variantTokens }
  | `(semblaCommandAttr| $name:ident : ℝ) =>
      pure { name := identText name, ty := .real, nameToken := name.raw }
  | `(semblaCommandAttr| $name:ident : Int) =>
      pure { name := identText name, ty := .int, nameToken := name.raw }
  | `(semblaCommandAttr| $name:ident : $target:ident) =>
      pure {
        name := identText name
        ty := .ref (identText target)
        nameToken := name.raw
        refTargetToken := some target.raw }
  | _ => throwUnsupportedSyntax

private def parseCommandRows (stx : TSyntax `semblaCommandRows) :
    TermElabM (TSyntax `term) := do
  match stx.raw.getArgs with
  | #[head, suffix] =>
      let some prefixDigits := head.isNatLit?
        | throwErrorAt head "row count must be a natural-number literal"
      let suffixToken : TSyntax `ident := ⟨suffix⟩
      let digits := toString prefixDigits ++ (identText suffixToken).replace "_" ""
      pure ⟨Syntax.mkNumLit digits (SourceInfo.fromRef stx.raw true)⟩
  | _ =>
      match stx with
      | `(semblaCommandRows| $value:term) => pure value
      | _ => throwUnsupportedSyntax

private def parseCommandSystem (stx : TSyntax `semblaCommandSystem) :
    TermElabM SurfaceSystem := do
  match stx with
  | `(semblaCommandSystem| system $logical:ident (rows := $size:semblaCommandRows)) =>
      pure ⟨identText logical, logical.raw, ← deriveRuntimeNameAt logical, logical.raw,
        ← parseCommandRows size, []⟩
  | `(semblaCommandSystem| system $logical:ident ($overrideKeyword:ident := $irName:str)
        (rows := $size:semblaCommandRows)) =>
      unless identText overrideKeyword == "name" do
        throwErrorAt overrideKeyword "expected 'name' table override"
      pure ⟨identText logical, logical.raw, irName.getString, irName.raw,
        ← parseCommandRows size, []⟩
  | `(semblaCommandSystem| system $logical:ident
        (rows := $size:semblaCommandRows) where $attrs:semblaCommandAttr*) =>
      pure ⟨identText logical, logical.raw, ← deriveRuntimeNameAt logical, logical.raw,
        ← parseCommandRows size, ← attrs.toList.mapM parseCommandAttr⟩
  | `(semblaCommandSystem| system $logical:ident ($overrideKeyword:ident := $irName:str)
        (rows := $size:semblaCommandRows) where $attrs:semblaCommandAttr*) =>
      unless identText overrideKeyword == "name" do
        throwErrorAt overrideKeyword "expected 'name' table override"
      pure ⟨identText logical, logical.raw, irName.getString, irName.raw,
        ← parseCommandRows size, ← attrs.toList.mapM parseCommandAttr⟩
  | _ => throwUnsupportedSyntax

private def parseCommandInput (stx : TSyntax `semblaCommandInput) :
    TermElabM SurfaceInput := do
  match stx with
  | `(semblaCommandInput| input $name:ident where $attrs:semblaCommandAttr*) =>
      pure ⟨identText name, name.raw, ← attrs.toList.mapM parseCommandAttr⟩
  | _ => throwUnsupportedSyntax

private def parseCommandGeneralTransition
    (stx : TSyntax `semblaCommandGeneralTransition) : TermElabM SurfaceTransition := do
  let finish (name selectedToken : TSyntax `ident) (binders : List SurfaceTransitionBinder)
      (items : Array (TSyntax `semblaCommandTransitionItem)) := do
    let mut guardExpr : Option (TSyntax `semblaExpr) := none
    let mut hazardExpr : Option (TSyntax `semblaExpr) := none
    let mut contests : List SurfaceContest := []
    let mut assignments : List (TSyntax `semblaSet) := []
    for item in items do
      match item with
      | `(semblaCommandTransitionItem| guard $expression:semblaExpr) =>
          if guardExpr.isSome then
            throwErrorAt item "general transition '{identText name}' has duplicate guard"
          guardExpr := some expression
      | `(semblaCommandTransitionItem| hazard $expression:semblaExpr) =>
          if hazardExpr.isSome then
            throwErrorAt item "general transition '{identText name}' has duplicate hazard"
          hazardExpr := some expression
      | `(semblaCommandTransitionItem| set $assignment:semblaSet) =>
          assignments := assignments ++ [assignment]
      | `(semblaCommandTransitionItem| contest $resourceName:ident by $ordering:ident) =>
          contests := contests ++ [← finishContest resourceName (some ordering)]
      | `(semblaCommandTransitionItem| contest $resourceName:ident) =>
          contests := contests ++ [← finishContest resourceName none]
      | _ => throwUnsupportedSyntax
    let resolvedGuard ← guardExpr.getDM
      (throwErrorAt name "general transition '{identText name}' requires exactly one guard")
    let resolvedHazard ← hazardExpr.getDM
      (throwErrorAt name "general transition '{identText name}' requires exactly one hazard")
    if assignments.isEmpty then
      throwErrorAt name "general transition '{identText name}' requires at least one set effect"
    pure ⟨identText name, name.raw,
      .general selectedToken resolvedGuard resolvedHazard contests assignments, binders⟩
  match stx with
  | `(semblaCommandGeneralTransition| transition $name:ident [$typed:semblaTypedBinder,*]
        on $selectedToken:ident where $items:semblaCommandTransitionItem*) =>
      if typed.getElems.isEmpty then
        throwErrorAt name "typed transition family requires at least one binder"
      let args ← typed.getElems.toList.mapM parseTypedBinder
      let binders := args.map fun arg => SurfaceTransitionBinder.mk arg.name arg.token
        (some arg.domainName) (some arg.domainToken) .static none
      finish name selectedToken binders items
  | `(semblaCommandGeneralTransition| transition $name:ident [$binders:ident,*]
        on $selectedToken:ident where $items:semblaCommandTransitionItem*) =>
      if binders.getElems.isEmpty then
        throwErrorAt name "indexed transition family requires at least one index"
      finish name selectedToken (binders.getElems.toList.map legacyTransitionBinder) items
  | `(semblaCommandGeneralTransition| transition $name:ident on $selectedToken:ident where
        $items:semblaCommandTransitionItem*) =>
      finish name selectedToken [] items
  | _ => throwUnsupportedSyntax

private def parseCommandOutputField (stx : TSyntax `semblaCommandOutputField) :
    TermElabM (SurfaceAttr × SurfaceOutputField) := do
  match stx with
  | `(semblaCommandOutputField| $name:ident : Int := count where $filter:semblaExpr) =>
      pure ({ name := identText name, ty := .int, nameToken := name.raw },
        ⟨identText name, name.raw, "count", none, some filter⟩)
  | `(semblaCommandOutputField| $name:ident : ℝ := count where $filter:semblaExpr) =>
      pure ({ name := identText name, ty := .real, nameToken := name.raw },
        ⟨identText name, name.raw, "count", none, some filter⟩)
  | `(semblaCommandOutputField| $name:ident : Int := sum ($value:semblaExpr)) =>
      pure ({ name := identText name, ty := .int, nameToken := name.raw },
        ⟨identText name, name.raw, "sum", some value, none⟩)
  | `(semblaCommandOutputField| $name:ident : ℝ := sum ($value:semblaExpr)) =>
      pure ({ name := identText name, ty := .real, nameToken := name.raw },
        ⟨identText name, name.raw, "sum", some value, none⟩)
  | `(semblaCommandOutputField| $name:ident : Int := count
        ($_value:semblaExpr) where $_filter:semblaExpr)
  | `(semblaCommandOutputField| $name:ident : ℝ := count
        ($_value:semblaExpr) where $_filter:semblaExpr) =>
      throwErrorAt name "count output field '{identText name}' cannot declare a value expression"
  | _ => throwUnsupportedSyntax

private def parseCommandOutput (stx : TSyntax `semblaCommandOutput) :
    TermElabM SurfaceOutput := do
  match stx with
  | `(semblaCommandOutput| output $name:ident from $selectedToken:ident where
        $fieldSyntax:semblaCommandOutputField*) =>
      let parsedFields ← fieldSyntax.toList.mapM parseCommandOutputField
      pure ⟨identText name, name.raw, parsedFields.map (·.1), selectedToken,
        parsedFields.map (·.2)⟩
  | _ => throwUnsupportedSyntax

private def parseCommandViewReduce (stx : TSyntax `semblaCommandViewReduce) :
    TermElabM String := do
  match stx with
  | `(semblaCommandViewReduce| sum) => pure "sum"
  | `(semblaCommandViewReduce| min) => pure "min"
  | `(semblaCommandViewReduce| max) => pure "max"
  | _ => throwUnsupportedSyntax

private def parseCommandView (stx : TSyntax `semblaCommandView) : TermElabM SurfaceView := do
  match stx with
  | `(semblaCommandView| view $name:ident := count $selectedToken:ident) =>
      pure ⟨identText name, name.raw, selectedToken, none, none, "count"⟩
  | `(semblaCommandView| view $name:ident := count $selectedToken:ident where
        $filter:semblaExpr) =>
      pure ⟨identText name, name.raw, selectedToken, some filter, none, "count"⟩
  | `(semblaCommandView| view $name:ident := count $selectedToken:ident using
        $value:semblaExpr) =>
      pure ⟨identText name, name.raw, selectedToken, none, some value, "count"⟩
  | `(semblaCommandView| view $name:ident := count $selectedToken:ident where
        $filter:semblaExpr using $value:semblaExpr) =>
      pure ⟨identText name, name.raw, selectedToken, some filter, some value, "count"⟩
  | `(semblaCommandView| view $name:ident := sum $selectedToken:ident) =>
      pure ⟨identText name, name.raw, selectedToken, none, none, "sum"⟩
  | `(semblaCommandView| view $name:ident := min $selectedToken:ident) =>
      pure ⟨identText name, name.raw, selectedToken, none, none, "min"⟩
  | `(semblaCommandView| view $name:ident := max $selectedToken:ident) =>
      pure ⟨identText name, name.raw, selectedToken, none, none, "max"⟩
  | `(semblaCommandView| view $name:ident := sum $selectedToken:ident where $filter:semblaExpr) =>
      pure ⟨identText name, name.raw, selectedToken, some filter, none, "sum"⟩
  | `(semblaCommandView| view $name:ident := min $selectedToken:ident where $filter:semblaExpr) =>
      pure ⟨identText name, name.raw, selectedToken, some filter, none, "min"⟩
  | `(semblaCommandView| view $name:ident := max $selectedToken:ident where $filter:semblaExpr) =>
      pure ⟨identText name, name.raw, selectedToken, some filter, none, "max"⟩
  | `(semblaCommandView| view $name:ident := sum $selectedToken:ident using $value:semblaExpr) =>
      pure ⟨identText name, name.raw, selectedToken, none, some value, "sum"⟩
  | `(semblaCommandView| view $name:ident := min $selectedToken:ident using $value:semblaExpr) =>
      pure ⟨identText name, name.raw, selectedToken, none, some value, "min"⟩
  | `(semblaCommandView| view $name:ident := max $selectedToken:ident using $value:semblaExpr) =>
      pure ⟨identText name, name.raw, selectedToken, none, some value, "max"⟩
  | `(semblaCommandView| view $name:ident := sum $selectedToken:ident where
        $filter:semblaExpr using $value:semblaExpr) =>
      pure ⟨identText name, name.raw, selectedToken, some filter, some value, "sum"⟩
  | `(semblaCommandView| view $name:ident := min $selectedToken:ident where
        $filter:semblaExpr using $value:semblaExpr) =>
      pure ⟨identText name, name.raw, selectedToken, some filter, some value, "min"⟩
  | `(semblaCommandView| view $name:ident := max $selectedToken:ident where
        $filter:semblaExpr using $value:semblaExpr) =>
      pure ⟨identText name, name.raw, selectedToken, some filter, some value, "max"⟩
  | _ => throwUnsupportedSyntax

private def parseCommandViews (stx : TSyntax `semblaCommandViews) :
    TermElabM (List SurfaceView × List SurfaceGroupedView) := do
  match stx with
  | `(semblaCommandViews| views $selectedToken:ident where
        $items:semblaCommandScopedView*) =>
      let mut viewDecls : List SurfaceView := []
      let mut groupedViewDecls : List SurfaceGroupedView := []
      for item in items do
        match item with
        | `(semblaCommandScopedView| $name:ident := count) =>
            viewDecls := viewDecls ++ [
              ⟨identText name, name.raw, selectedToken, none, none, "count"⟩]
        | `(semblaCommandScopedView| $name:ident := count where $filter:semblaExpr) =>
            viewDecls := viewDecls ++ [
              ⟨identText name, name.raw, selectedToken, some filter, none, "count"⟩]
        | `(semblaCommandScopedView| $name:ident :=
              $reducer:semblaCommandViewReduce $value:semblaExpr) =>
            viewDecls := viewDecls ++ [
              ⟨identText name, name.raw, selectedToken, none, some value,
                ← parseCommandViewReduce reducer⟩]
        | `(semblaCommandScopedView| $name:ident :=
              $reducer:semblaCommandViewReduce $value:semblaExpr where
              $filter:semblaExpr) =>
            viewDecls := viewDecls ++ [
              ⟨identText name, name.raw, selectedToken, some filter, some value,
                ← parseCommandViewReduce reducer⟩]
        | `(semblaCommandScopedView| $name:ident := count by
              $keys:semblaScopedGroupedKey,*) =>
            groupedViewDecls := groupedViewDecls ++ [
              ⟨identText name, name.raw, selectedToken, none,
                ← keys.getElems.toList.mapM parseScopedGroupedKey, true⟩]
        | `(semblaCommandScopedView| $name:ident := count where
              $filter:semblaExpr by $keys:semblaScopedGroupedKey,*) =>
            groupedViewDecls := groupedViewDecls ++ [
              ⟨identText name, name.raw, selectedToken, some filter,
                ← keys.getElems.toList.mapM parseScopedGroupedKey, true⟩]
        | _ => throwUnsupportedSyntax
      pure (viewDecls, groupedViewDecls)
  | _ => throwUnsupportedSyntax

private def parseCommandSummaryReduce (stx : TSyntax `semblaCommandSummaryReduce) :
    TermElabM String := do
  match stx with
  | `(semblaCommandSummaryReduce| sum) => pure "sum"
  | `(semblaCommandSummaryReduce| min) => pure "min"
  | `(semblaCommandSummaryReduce| max) => pure "max"
  | `(semblaCommandSummaryReduce| last) => pure "last"
  | `(semblaCommandSummaryReduce| argmaxₜ) => pure "argmax_tick"
  | _ => throwUnsupportedSyntax

private def parseCommandSummary (stx : TSyntax `semblaCommandSummary) :
    TermElabM SurfaceSummary := do
  let finish (name endpoint : TSyntax `ident) (reducerName : String) := do
    match endpoint.getId.components with
    | [boxName, viewName] =>
        let boxToken := Lean.mkIdentFrom endpoint boxName
        let viewTokenBase := Lean.mkIdentFrom endpoint viewName
        let viewToken : TSyntax `ident := match endpoint.raw.getHeadInfo with
          | .original leading position trailing endPosition =>
              let viewPosition := String.Pos.mk
                (position.byteIdx + boxName.getString!.utf8ByteSize + 1)
              let emptyLeading := Substring.mk leading.str viewPosition viewPosition
              ⟨viewTokenBase.raw.setInfo
                (.original emptyLeading viewPosition trailing endPosition)⟩
          | _ => viewTokenBase
        pure ⟨identText name, name.raw, boxToken, viewToken, reducerName⟩
    | _ => throwErrorAt endpoint "summary source must have the form 'box.view'"
  match stx with
  | `(semblaCommandSummary| summary $name:ident := sum $endpoint:ident) =>
      finish name endpoint "sum"
  | `(semblaCommandSummary| summary $name:ident := min $endpoint:ident) =>
      finish name endpoint "min"
  | `(semblaCommandSummary| summary $name:ident := max $endpoint:ident) =>
      finish name endpoint "max"
  | `(semblaCommandSummary| summary $name:ident := last $endpoint:ident) =>
      finish name endpoint "last"
  | `(semblaCommandSummary| summary $name:ident := argmaxₜ $endpoint:ident) =>
      finish name endpoint "argmax_tick"
  | _ => throwUnsupportedSyntax

private def parseCommandSummaries (stx : TSyntax `semblaCommandSummaries) :
    TermElabM (List SurfaceSummary) := do
  match stx with
  | `(semblaCommandSummaries| summaries $boxToken:ident where
        $items:semblaCommandScopedSummary*) =>
      let mut summaryDecls : List SurfaceSummary := []
      for item in items do
        match item with
        | `(semblaCommandScopedSummary| $name:ident :=
              $reducer:semblaCommandSummaryReduce $viewToken:ident) =>
            summaryDecls := summaryDecls ++ [
              ⟨identText name, name.raw, boxToken, viewToken,
                ← parseCommandSummaryReduce reducer⟩]
        | _ => throwUnsupportedSyntax
      pure summaryDecls
  | _ => throwUnsupportedSyntax

private def parseStateApplication (stx : TSyntax `semblaStateApplication) :
    TermElabM SurfaceStateApplication := do
  match stx with
  | `(semblaStateApplication| $name:ident) =>
      pure ⟨identText name, name.raw, []⟩
  | `(semblaStateApplication| $name:ident ($args:ident,*)) =>
      pure ⟨identText name, name.raw, args.getElems.toList⟩
  | _ => throwUnsupportedSyntax

private def parseStateDecl (stx : TSyntax `semblaStateDecl) :
    TermElabM SurfaceStateAlias := do
  let finish (keyword name selectedToken : TSyntax `ident)
      (args : Array (TSyntax `semblaTypedBinder))
      (atoms : Array (TSyntax `semblaStateAtom)) := do
    unless identText keyword == "state" do throwUnsupportedSyntax
    let parsedArgs ← args.toList.mapM parseTypedBinder
    let mut parsedAtoms : List SurfacePatternAtom := []
    for atom in atoms do
      match atom with
      | `(semblaStateAtom| $columnToken:ident := $value:semblaExpr) =>
          parsedAtoms := parsedAtoms ++ [.assignment columnToken value atom.raw]
      | `(semblaStateAtom| match $expression:semblaExpr) =>
          parsedAtoms := parsedAtoms ++ [.predicate expression atom.raw]
      | _ => throwUnsupportedSyntax
    if parsedAtoms.isEmpty then throwErrorAt name "state alias body must not be empty"
    pure ⟨identText name, name.raw, selectedToken, parsedArgs, parsedAtoms⟩
  match stx with
  | `(semblaStateDecl| $keyword:ident $name:ident on $selected:ident where
        $atoms:semblaStateAtom*) => finish keyword name selected #[] atoms
  | `(semblaStateDecl| $keyword:ident $name:ident ($args:semblaTypedBinder,*)
        on $selected:ident where $atoms:semblaStateAtom*) => finish keyword name selected args atoms
  | _ => throwUnsupportedSyntax

private def parseRelationConstraint (stx : TSyntax `semblaRelationConstraint) :
    TermElabM SurfaceRelationConstraint := do
  match stx with
  | `(semblaRelationConstraint| $left:ident = $right:ident) =>
      pure (.equal left right stx.raw)
  | `(semblaRelationConstraint| $left:ident ≠ $right:ident) =>
      pure (.notEqual left right stx.raw)
  | _ => throwUnsupportedSyntax

private def parseRelationItem (stx : TSyntax `semblaRelationItem) :
    TermElabM SurfaceRelationItem := do
  match stx with
  | `(semblaRelationItem| from $apps:semblaStateApplication,*) =>
      let parsed ← apps.getElems.toList.mapM parseStateApplication
      if parsed.isEmpty then throwErrorAt stx "relation source list must not be empty"
      pure (.source parsed stx.raw)
  | `(semblaRelationItem| hazard $expression:semblaExpr) =>
      pure (.hazard expression stx.raw)
  | `(semblaRelationItem| $keyword:ident $resource:ident by $ordering:ident) =>
      unless identText keyword == "claim" do throwUnsupportedSyntax
      pure (.claim (← finishContest resource (some ordering)) stx.raw)
  | `(semblaRelationItem| set $assignment:semblaSet) =>
      pure (.set assignment stx.raw)
  | `(semblaRelationItem| $keyword:ident $application:semblaStateApplication) =>
      unless identText keyword == "become" do throwUnsupportedSyntax
      pure (.become (← parseStateApplication application) stx.raw)
  | _ => throwUnsupportedSyntax

private def parseRelationDecl (stx : TSyntax `semblaRelationDecl) :
    TermElabM SurfaceTransition := do
  let finish (keyword name selectedToken : TSyntax `ident)
      (args : Array (TSyntax `semblaTypedBinder))
      (constraints : List SurfaceRelationConstraint)
      (items : Array (TSyntax `semblaRelationItem)) := do
    unless identText keyword == "relation" do throwUnsupportedSyntax
    let parsedArgs ← args.toList.mapM parseTypedBinder
    if parsedArgs.isEmpty then throwErrorAt name "relation requires at least one binder"
    let binders : List SurfaceTransitionBinder := parsedArgs.map fun arg =>
      SurfaceTransitionBinder.mk arg.name arg.token (some arg.domainName)
        (some arg.domainToken) .static none
    let parsedItems ← items.toList.mapM parseRelationItem
    let hazardCount := parsedItems.countP fun item => match item with
      | .hazard _ _ => true | _ => false
    unless hazardCount == 1 do
      throwErrorAt name "relation '{identText name}' requires exactly one hazard"
    let sourceCount := parsedItems.countP fun item => match item with
      | .source _ _ => true | _ => false
    unless sourceCount == 1 do
      throwErrorAt name "relation '{identText name}' requires exactly one from list"
    pure ⟨identText name, name.raw, .relation selectedToken constraints parsedItems, binders⟩
  match stx with
  | `(semblaRelationDecl| $keyword:ident $name:ident ($args:semblaTypedBinder,*)
        on $selected:ident where $items:semblaRelationItem*) =>
      finish keyword name selected args [] items
  | `(semblaRelationDecl| $keyword:ident $name:ident ($args:semblaTypedBinder,*)
        on $selected:ident $subject:ident $to:ident $constraints:semblaRelationConstraint,* where
        $items:semblaRelationItem*) =>
      unless identText subject == "subject" && identText to == "to" do throwUnsupportedSyntax
      finish keyword name selected args (← constraints.getElems.toList.mapM parseRelationConstraint) items
  | _ => throwUnsupportedSyntax

/-- Collect one command-layout box through the shared surface kernel while
    retaining the original interleaving of input and output declarations. -/
def parseCommandBoxWithPorts
    (stx : TSyntax `semblaCommandBox) : TermElabM CollectedSurfaceBox := do
  match stx with
  | `(semblaCommandBox| box $name:ident where $items:semblaCommandBoxItem*) =>
      let mut systemDecls : List SurfaceSystem := []
      let mut inputDecls : List SurfaceInput := []
      let mut transitionDecls : List SurfaceTransition := []
      let mut outputDecls : List SurfaceOutput := []
      let mut viewDecls : List SurfaceView := []
      let mut groupedViewDecls : List SurfaceGroupedView := []
      let mut aliasDecls : List SurfaceStateAlias := []
      let mut portDecls : List SurfacePortItem := []
      for item in items do
        match item with
        | `(semblaCommandBoxItem| $decl:semblaCommandSystem) =>
            systemDecls := systemDecls ++ [← parseCommandSystem decl]
        | `(semblaCommandBoxItem| $decl:semblaCommandInput) =>
            let parsed ← parseCommandInput decl
            inputDecls := inputDecls ++ [parsed]
            portDecls := portDecls ++ [.input parsed]
        | `(semblaCommandBoxItem| $decl:semblaCommandGeneralTransition) =>
            transitionDecls := transitionDecls ++ [← parseCommandGeneralTransition decl]
        | `(semblaCommandBoxItem| $decl:semblaStateDecl) =>
            aliasDecls := aliasDecls ++ [← parseStateDecl decl]
        | `(semblaCommandBoxItem| $decl:semblaRelationDecl) =>
            transitionDecls := transitionDecls ++ [← parseRelationDecl decl]
        | `(semblaCommandBoxItem| $decl:semblaTransition) =>
            transitionDecls := transitionDecls ++ [← parseTransition decl]
        | `(semblaCommandBoxItem| $decl:semblaCommandOutput) =>
            let parsed ← parseCommandOutput decl
            outputDecls := outputDecls ++ [parsed]
            portDecls := portDecls ++ [.output parsed]
        | `(semblaCommandBoxItem| $decl:semblaCommandView) =>
            viewDecls := viewDecls ++ [← parseCommandView decl]
        | `(semblaCommandBoxItem| $decl:semblaGroupedView) =>
            groupedViewDecls := groupedViewDecls ++ [← parseGroupedView decl]
        | `(semblaCommandBoxItem| $decl:semblaCommandViews) =>
            let (scopedViews, scopedGroupedViews) ← parseCommandViews decl
            viewDecls := viewDecls ++ scopedViews
            groupedViewDecls := groupedViewDecls ++ scopedGroupedViews
        | `(semblaCommandBoxItem| contest $unsupported:ident) =>
            throwErrorAt unsupported "contest is declared inside a transition body"
        | _ => throwUnsupportedSyntax
      let surfaceBox : SurfaceBox :=
        ⟨identText name, name.raw, systemDecls, inputDecls, transitionDecls,
          outputDecls, viewDecls, groupedViewDecls, aliasDecls⟩
      pure ⟨surfaceBox, portDecls⟩
  | _ => throwUnsupportedSyntax

/-- Existing command-box API; model elaboration intentionally ignores the
    additional port-order metadata. -/
def parseCommandBox (stx : TSyntax `semblaCommandBox) : TermElabM SurfaceBox := do
  pure (← parseCommandBoxWithPorts stx).surfaceBox

private def validateDomainSystemAmbiguity (domains : List SurfaceDomain)
    (allBoxes : List SurfaceBox) : TermElabM Unit := do
  for domain in domains do
    if allBoxes.any fun boxCtx =>
        boxCtx.systems.any (·.logicalName == domain.name) then
      throwErrorAt domain.token
        "name '{domain.name}' is ambiguous between a domain and a system"

private def resolveNamedAttr (domains : List SurfaceDomain) (allBoxes : List SurfaceBox)
    (column : SurfaceAttr) : TermElabM SurfaceAttr := do
  match column.ty with
  | .ref target =>
      let domain? := domains.find? (·.name == target)
      let systemExists := allBoxes.any fun candidateBox =>
        candidateBox.systems.any (·.logicalName == target)
      if domain?.isSome && systemExists then
        throwErrorAt (column.refTargetToken.getD column.nameToken)
          "name '{target}' is ambiguous between a domain and a system"
      match domain? with
      | none => pure column
      | some domainDecl =>
          match domainDecl.domain with
          | .range _ _ => pure (SurfaceAttr.mk column.name .int column.nameToken
              none column.variantTokens (some target))
          | .enumeration variants => pure (SurfaceAttr.mk column.name (.enum variants)
              column.nameToken none domainDecl.memberTokens (some target))
  | _ => pure column

private def resolveNamedDomainsInBox (domains : List SurfaceDomain) (allBoxes : List SurfaceBox)
    (boxCtx : SurfaceBox) : TermElabM SurfaceBox := do
  let resolvedSystems ← boxCtx.systems.mapM fun selected => do
    let resolvedAttrs ← selected.attrs.mapM (resolveNamedAttr domains allBoxes)
    pure { selected with attrs := resolvedAttrs }
  let resolvedInputs ← boxCtx.inputs.mapM fun inputDecl => do
    let resolvedSchema ← inputDecl.schema.mapM (resolveNamedAttr domains allBoxes)
    pure { inputDecl with schema := resolvedSchema }
  let resolvedOutputs ← boxCtx.outputs.mapM fun outputDecl => do
    let resolvedSchema ← outputDecl.schema.mapM (resolveNamedAttr domains allBoxes)
    pure { outputDecl with schema := resolvedSchema }
  pure (SurfaceBox.mk boxCtx.name boxCtx.token resolvedSystems resolvedInputs
    boxCtx.transitions resolvedOutputs boxCtx.views boxCtx.groupedViews boxCtx.aliases)

private def validatePartitionTargets (partitions : List SurfacePartition)
    (boxes : List SurfaceBox) : TermElabM Unit := do
  for partition in partitions do
    let boxCtx ← match boxes.find? (·.name == partition.target.boxName) with
      | some found => pure found
      | none => throwErrorAt partition.target.token
          "unknown partition projection box '{partition.target.boxName}'"
    let selected ← match boxCtx.systems.find? (·.logicalName == partition.target.systemName) with
      | some found => pure found
      | none => throwErrorAt partition.target.token
          "unknown partition projection system '{partition.target.systemName}'"
    let column ← match selected.attrs.find? (·.name == partition.target.attrName) with
      | some found => pure found
      | none => throwErrorAt partition.target.token
          "unknown partition projection attribute '{partition.target.attrName}'"
    unless column.ty == .int do
      throwErrorAt partition.target.token "partition projection attribute must have type Int"

private partial def containsExprFunctionCall (names : List String) (stx : Syntax) : Bool :=
  match stx with
  | `(semblaExpr| $called:ident ($_args:semblaExpr,*)) =>
      names.contains (identText called) || stx.getArgs.any (containsExprFunctionCall names)
  | _ => stx.getArgs.any (containsExprFunctionCall names)

private partial def rejectAggregates (context : String) (stx : Syntax) : TermElabM Unit := do
  match stx with
  | `(semblaExpr| inputSum $_port:ident field $_field:ident)
  | `(semblaExpr| countBy $_countKey:ident ($_filter:semblaExpr))
  | `(semblaExpr| sizeBy $_sizeKey:ident)
  | `(semblaExpr| freq ($_predicate:semblaExpr) over $_freqKey:ident) =>
      throwErrorAt stx "aggregates are not supported in {context}"
  | _ =>
      for child in stx.getArgs do
        rejectAggregates context child

private def validateExprFunctions (domains : List SurfaceDomain)
    (functions : List SurfaceExprFunction) : TermElabM Unit := do
  for functionDecl in functions do
    let mut seenArgs : List String := []
    let mut functionDomains : List ParameterTable.IndexDomain := []
    for arg in functionDecl.args do
      if seenArgs.contains arg.name then
        throwErrorAt arg.token "duplicate expression-function argument '{arg.name}'"
      seenArgs := arg.name :: seenArgs
      let domain ← match domains.find? (·.name == arg.domainName) with
        | some found => pure found.domain
        | none => throwErrorAt arg.domainToken "unknown domain '{arg.domainName}'"
      functionDomains := functionDomains ++ [domain]
    let _ ← checkExpansionCardinality "expression function" functionDecl.token functionDomains
    let combinations := cartesianMembers (functionDomains.map (·.members))
    let mut checked : List (List ParameterTable.IndexMember) := []
    for cell in functionDecl.cells do
      if cell.key.length != functionDomains.length then
        throwErrorAt cell.token
          "expression-function cell has {cell.key.length} keys; expected {functionDomains.length}"
      for ((member, token), domain) in cell.key.zip functionDomains do
        unless domain.members.contains member do
          throwErrorAt token "expression-function key member '{member.component}' is outside its domain"
      let key := cell.key.map (·.1)
      if checked.contains key then throwErrorAt cell.token "duplicate expression-function cell"
      if containsExprFunctionCall (functions.map (·.name)) cell.expression then
        throwErrorAt cell.token "expression-function calls are not supported in function cells"
      rejectAggregates "expression-function cells" cell.expression
      checked := checked ++ [key]
    for key in combinations do
      unless checked.contains key do
        throwErrorAt functionDecl.token
          "expression function is missing cell [{String.intercalate ", " (key.map (·.component))}]"

private def collectCommandSurfaceModel (declaration : TSyntax `ident)
    (runtimeOverride : Option (TSyntax `str)) (stepWidth : TSyntax `term)
    (items : List (TSyntax `semblaCommandModelItem)) : TermElabM SurfaceModel := do
  let mut indexes : List SurfaceIndex := []
  let mut partitions : List SurfacePartition := []
  let mut exprFunctions : List SurfaceExprFunction := []
  let mut paramItems : List SurfaceParamItem := []
  let mut boxDecls : List SurfaceBox := []
  let mut wireDecls : List SurfaceWire := []
  let mut summaryDecls : List SurfaceSummary := []
  for item in items do
    match item with
    | `(semblaCommandModelItem| $decl:semblaIndexDecl) =>
        let parsed ← parseIndexDecl decl
        if indexes.any (·.name == parsed.name) || partitions.any (·.name == parsed.name) then
          throwErrorAt parsed.token (if parsed.legacyIndex then
            "duplicate index '{parsed.name}'" else "duplicate domain '{parsed.name}'")
        indexes := indexes ++ [parsed]
    | `(semblaCommandModelItem| $decl:semblaPartitionDecl) =>
        let parsed ← parsePartitionDecl decl
        if partitions.any (·.name == parsed.name) || indexes.any (·.name == parsed.name) then
          throwErrorAt parsed.token "duplicate domain '{parsed.name}'"
        partitions := partitions ++ [parsed]
    | `(semblaCommandModelItem| $decl:semblaExprFunctionDecl) =>
        let parsed ← parseExprFunctionDecl decl
        if exprFunctions.any (·.name == parsed.name) then
          throwErrorAt parsed.token "duplicate expression function '{parsed.name}'"
        exprFunctions := exprFunctions ++ [parsed]
    | `(semblaCommandModelItem| $decl:semblaParamFamily) =>
        paramItems := paramItems ++ [.family (← parseParamFamily decl)]
    | `(semblaCommandModelItem| $decl:semblaParam) =>
        paramItems := paramItems ++ [.scalar (← parseSurfaceParam decl)]
    | `(semblaCommandModelItem| $decl:semblaCommandBox) =>
        boxDecls := boxDecls ++ [← parseCommandBox decl]
    | `(semblaCommandModelItem| $decl:semblaWire) =>
        wireDecls := wireDecls ++ [← parseWire decl]
    | `(semblaCommandModelItem| $decl:semblaCommandSummary) =>
        summaryDecls := summaryDecls ++ [← parseCommandSummary decl]
    | `(semblaCommandModelItem| $decl:semblaCommandSummaries) =>
        summaryDecls := summaryDecls ++ (← parseCommandSummaries decl)
    | `(semblaCommandModelItem| contest $unsupported:ident) =>
        throwErrorAt unsupported "contest is declared inside a transition body"
    | _ => throwUnsupportedSyntax
  for partition in partitions do
    indexes := indexes ++ [SurfaceDomain.mk partition.name partition.token
      (.enumeration (partition.members.map (·.label)))
      (partition.members.map fun member => (member.label, member.token)) false]
  let namedDomains := indexes.filter fun domain => !domain.legacyIndex
  let collectedBoxes := boxDecls
  validateDomainSystemAmbiguity namedDomains collectedBoxes
  boxDecls ← boxDecls.mapM (resolveNamedDomainsInBox namedDomains collectedBoxes)
  validatePartitionTargets partitions boxDecls
  validateExprFunctions namedDomains exprFunctions
  let mut paramDecls : List SurfaceParam := []
  let mut families : List SurfaceParamFamily := []
  for item in paramItems do
    match item with
    | .scalar parameterDecl => paramDecls := paramDecls ++ [parameterDecl]
    | .family familyDecl =>
        if families.any (·.sourceName == familyDecl.sourceName) then
          throwErrorAt familyDecl.token "duplicate parameter family '{familyDecl.sourceName}'"
        let (family, expanded) ← expandParamFamily indexes familyDecl
        families := families ++ [family]
        paramDecls := paramDecls ++ expanded
  let runtimeName ← match runtimeOverride with
    | some value => pure (value.getString, value.raw)
    | none => pure (← deriveRuntimeNameAt declaration, declaration.raw)
  pure ⟨identText declaration, declaration.raw, some runtimeName, stepWidth,
    paramDecls, boxDecls, wireDecls, summaryDecls, indexes, families,
    indexes.filter (fun domain => !domain.legacyIndex), partitions, exprFunctions⟩

private def collectLegacySurfaceModel (name : TSyntax `str) (stepWidth : TSyntax `term)
    (paramDecls : List (TSyntax `semblaParam)) (boxDecls : List (TSyntax `semblaBox))
    (wireDecls : List (TSyntax `semblaWire))
    (summaryBlock : Option (TSyntax `semblaSummaryBlock)) : TermElabM SurfaceModel := do
  let summaryCtx ← match summaryBlock with
    | some declarations => parseSummaryBlock declarations
    | none => pure []
  pure ⟨name.getString, name.raw, none, stepWidth,
    ← paramDecls.mapM parseSurfaceParam,
    ← boxDecls.mapM parseBox,
    ← wireDecls.mapM parseWire,
    summaryCtx, [], [], [], [], []⟩

private def ensureUnique (kind : String) (entries : List (String × Syntax)) : TermElabM Unit := do
  let mut seen : List String := []
  for (name, token) in entries do
    if seen.contains name then throwErrorAt token "duplicate {kind} '{name}'"
    seen := name :: seen

private def ensureUniqueRuntimeNames (kind : String)
    (entries : List (String × String × Syntax)) : TermElabM Unit := do
  let mut seen : List (String × String) := []
  for (runtimeName, sourceName, token) in entries do
    match seen.find? (·.1 == runtimeName) with
    | some (_, firstSource) => throwErrorAt token
        "duplicate {kind} runtime name '{runtimeName}' for declarations '{firstSource}' and '{sourceName}'"
    | none => seen := (runtimeName, sourceName) :: seen

private def validateAttrs (kind : String) (attrs : List SurfaceAttr) : TermElabM Unit := do
  ensureUnique kind (attrs.map fun column => (column.name, column.nameToken))
  for column in attrs do
    match column.ty with
    | .enum variants =>
        if variants.isEmpty then
          throwErrorAt column.nameToken
            "enum attribute '{column.name}' must declare at least one variant"
        ensureUnique "enum variant" column.variantTokens
    | _ => pure ()

private def lookupSystem (boxCtx : SurfaceBox) (token : TSyntax `ident) : TermElabM SurfaceSystem := do
  let name := identText token
  match boxCtx.systems.find? (·.logicalName == name) with
  | some found => pure found
  | none => throwErrorAt token "unknown system '{name}'"

private def lookupAttr (attrs : List SurfaceAttr) (token : TSyntax `ident) : TermElabM SurfaceAttr := do
  let name := identText token
  match attrs.find? (·.name == name) with
  | some found => pure found
  | none => throwErrorAt token "unknown state or attribute '{name}'"

private def typeName : SurfaceTy → String
  | .real => "Real"
  | .int => "Int"
  | .bool => "Bool"
  | .enum _ => "Enum"
  | .ref _ => "Ref"

private def isNumeric : SurfaceTy → Bool
  | .real | .int => true
  | _ => false

private def sameType (expected actual : SurfaceTy) : Bool :=
  match expected, actual with
  | .real, .real | .int, .int | .bool, .bool => true
  | .enum lhs, .enum rhs => lhs == rhs
  | .ref lhs, .ref rhs => lhs == rhs
  | _, _ => false

private def equalityCompatible (left right : SurfaceTy) : Bool :=
  sameType left right || (isNumeric left && isNumeric right)

private def attrTerm (boxCtx : SurfaceBox) (column : SurfaceAttr) : TermElabM (TSyntax `term) := do
  let name := Lean.quote column.name
  match column.ty with
  | .real => `(Attr.mk $name AttrType.real)
  | .int => `(Attr.mk $name AttrType.int)
  | .enum variants =>
      let values : Array (TSyntax `term) := variants.toArray.map fun value => ⟨Syntax.mkStrLit value⟩
      `(Attr.mk $name (AttrType.enum [$values,*]))
  | .ref target =>
      match boxCtx.systems.find? (·.logicalName == target) with
      | none => throwErrorAt (column.refTargetToken.getD column.nameToken)
          "unknown reference target '{target}'"
      | some found => `(Attr.mk $name (AttrType.ref $(Lean.quote found.irName)))
  | .bool => throwErrorAt column.nameToken "Boolean state columns are not part of IR v0.1"

private def frequencyRowLocalMessage : String :=
  "frequency predicates are row-local; aggregates join on declared Ref keys only"

private partial def validateFrequencyPredicate (stx : Syntax) : TermElabM Unit := do
  match stx with
  | `(semblaExpr| inputSum $_port:ident field $_field:ident)
  | `(semblaExpr| countBy $_countKey:ident ($_filter:semblaExpr))
  | `(semblaExpr| sizeBy $_sizeKey:ident)
  | `(semblaExpr| freq ($_nested:semblaExpr) over $_freqKey:ident) =>
      throwErrorAt stx frequencyRowLocalMessage
  | _ =>
      for child in stx.getArgs do
        validateFrequencyPredicate child

private def frequencyKey (tableCtx : SurfaceSystem) (token : TSyntax `ident) :
    TermElabM SurfaceAttr := do
  let name := identText token
  let column ← match tableCtx.attrs.find? (·.name == name) with
    | some found => pure found
    | none => throwErrorAt token
        "unknown frequency key attribute '{name}' on system '{tableCtx.logicalName}'"
  match column.ty with
  | .ref _ => pure column
  | actual => throwErrorAt token
      "frequency key attribute '{name}' on system '{tableCtx.logicalName}' must have type Ref; found {typeName actual}"

private def keyedCountTerm (tableCtx : SurfaceSystem) (key : SurfaceAttr)
    (filter : TSyntax `term) : TermElabM (TSyntax `term) :=
  `(Expr.agg AggOp.count $(Lean.quote tableCtx.irName) $(Lean.quote key.name)
    $(Lean.quote key.name) $filter)

structure SurfaceIndexBinding where
  name : String
  token : Syntax
  member : ParameterTable.IndexMember
  domainName : String := ""
  mode : SurfaceBinderMode := .legacyMatched
  matchedAttribute : Option String := none

private partial def elaborateExpr (tableCtx : SurfaceSystem) (attrs : List SurfaceAttr)
    (paramCtx : List SurfaceParam) (inputCtx : List SurfaceInput) (stx : Syntax)
    (declaration : Option String := none) (frequencyPredicate : Bool := false)
    (familyCtx : List SurfaceParamFamily := [])
    (bindingCtx : List SurfaceIndexBinding := [])
    (domainCtx : List SurfaceDomain := [])
    (functionCtx : List SurfaceExprFunction := [])
    (partitionCtx : List SurfacePartition := []) :
    TermElabM (TSyntax `term × SurfaceTy) := do
  let recur := fun expression =>
    elaborateExpr tableCtx attrs paramCtx inputCtx expression declaration frequencyPredicate
      familyCtx bindingCtx domainCtx functionCtx partitionCtx
  let lookupExprAttr := fun (token : TSyntax `ident) => do
    let name := identText token
    match attrs.find? (·.name == name) with
    | some found => pure found
    | none =>
        if frequencyPredicate then
          throwErrorAt token
            "unknown row attribute '{name}' in frequency predicate; {frequencyRowLocalMessage}"
        else
          match declaration with
          | some context => throwErrorAt token "{context}: unknown state or attribute '{name}'"
          | none => throwErrorAt token "unknown state or attribute '{name}'"
  match stx with
  | `(semblaExpr| ($inner:semblaExpr)) => recur inner
  | `(semblaExpr| $value:num) => pure (← `(Expr.int $value), .int)
  | `(semblaExpr| $value:scientific) =>
      validateScientific value false
      pure (← `(Expr.real $value), .real)
  | `(semblaExpr| true) => pure (← `(Expr.bool true), .bool)
  | `(semblaExpr| false) => pure (← `(Expr.bool false), .bool)
  | `(semblaExpr| ¬$inner:semblaExpr) =>
      let (term, ty) ← recur inner
      unless ty == .bool do throwErrorAt inner "operand of ¬ must have type Bool"
      pure (← `(Expr.not $term), .bool)
  | `(semblaExpr| $functionName:ident ($arguments:semblaExpr,*)) =>
      let sourceName := identText functionName
      let resolveArguments (expectedDomains : List String) := do
        unless arguments.getElems.size == expectedDomains.length do
          throwErrorAt functionName
            "function '{sourceName}' expects {expectedDomains.length} arguments"
        (arguments.getElems.toList.zip expectedDomains).mapM fun (argument, domainName) => do
          let domainDecl ← match domainCtx.find? (·.name == domainName) with
            | some found => pure found
            | none => throwErrorAt functionName "unknown domain '{domainName}'"
          let member ← match argument with
            | `(semblaExpr| $value:num) => pure (ParameterTable.IndexMember.int
                (value.raw.isNatLit?.getD 0))
            | `(semblaExpr| $value:ident) =>
                let valueName := identText value
                match bindingCtx.find? (·.name == valueName) with
                | some binding =>
                    unless binding.domainName == domainName do
                      throwErrorAt value "argument '{valueName}' has domain '{binding.domainName}'; expected '{domainName}'"
                    pure binding.member
                | none => pure (.enum valueName)
            | _ => throwErrorAt argument "function arguments must be bound values or domain literals"
          unless domainDecl.domain.members.contains member do
            throwErrorAt argument "function argument is outside domain '{domainName}'"
          pure member
      match familyCtx.find? fun family => family.callNotation && family.sourceName == sourceName with
      | some family =>
          let members ← resolveArguments family.domains
          let runtimeName ← match familyRuntimeName family.name members with
            | .ok value => pure value
            | .error message => throwErrorAt functionName message
          unless paramCtx.any (·.name == runtimeName) do
            throwErrorAt functionName "parameter family cell '{runtimeName}' is not declared"
          pure (← `(Expr.param $(Lean.quote runtimeName)), family.ty)
      | none =>
          let functionDecl ← match functionCtx.find? (·.name == sourceName) with
            | some found => pure found
            | none => throwErrorAt functionName "unknown mathematical function '{sourceName}'"
          let members ← resolveArguments (functionDecl.args.map (·.domainName))
          let cell ← match functionDecl.cells.find? (fun cell => cell.key.map (·.1) == members) with
            | some found => pure found
            | none => throwErrorAt functionName "expression-function cell is not declared"
          let formalBindings := (functionDecl.args.zip members).map fun (arg, member) =>
            SurfaceIndexBinding.mk arg.name arg.token member arg.domainName .static none
          let (term, actualTy) ← elaborateExpr tableCtx attrs paramCtx inputCtx cell.expression declaration
            frequencyPredicate familyCtx (formalBindings ++ bindingCtx) domainCtx functionCtx partitionCtx
          unless sameType functionDecl.ty actualTy do
            throwErrorAt cell.token
              "expression function '{functionDecl.name}' cell has type {typeName actualTy}; expected {typeName functionDecl.ty}"
          pure (term, functionDecl.ty)
  | `(semblaExpr| $familyName:ident [$arguments:ident,*]) =>
      let sourceName := identText familyName
      let family ← match familyCtx.find? (·.sourceName == sourceName) with
        | some found => pure found
        | none => throwErrorAt familyName "unknown parameter family '{sourceName}'"
      let argumentNames := arguments.getElems.toList.map identText
      unless argumentNames == family.dimensions do
        throwErrorAt familyName
          "parameter family '{sourceName}' expects indexes [{String.intercalate ", " family.dimensions}] in that order"
      let members ← (arguments.getElems.toList.zip family.domains).mapM fun (argument, expectedDomain) => do
        let name := identText argument
        match bindingCtx.find? (·.name == name) with
        | some binding =>
            unless binding.domainName == expectedDomain do
              throwErrorAt argument
                "index '{name}' has domain '{binding.domainName}'; expected '{expectedDomain}'"
            pure binding.member
        | none => throwErrorAt argument "index '{name}' is not bound by this transition family"
      let runtimeName ← match familyRuntimeName family.name members with
        | .ok value => pure value
        | .error message => throwErrorAt familyName message
      unless paramCtx.any (·.name == runtimeName) do
        throwErrorAt familyName "parameter family cell '{runtimeName}' is not declared"
      pure (← `(Expr.param $(Lean.quote runtimeName)), family.ty)
  | `(semblaExpr| parameter $name:ident) =>
      let value := identText name
      let paramDecl ← match paramCtx.find? (·.sourceName == value) with
        | some found => pure found
        | none =>
            if frequencyPredicate then
              throwErrorAt name
                "unknown model parameter '{value}' in frequency predicate; {frequencyRowLocalMessage}"
            else
              throwErrorAt name "undeclared parameter '{value}'"
      pure (← `(Expr.param $(Lean.quote paramDecl.name)), paramDecl.ty)
  | `(semblaExpr| $name:ident) =>
      let value := identText name
      if value == "true" then pure (← `(Expr.bool true), .bool)
      else if value == "false" then pure (← `(Expr.bool false), .bool)
      else if let some binding := bindingCtx.find? (·.name == value) then
        let domainTy := match domainCtx.find? (·.name == binding.domainName) with
          | some { domain := .enumeration variants, .. } => SurfaceTy.enum variants
          | _ => SurfaceTy.int
        match binding.member with
        | .enum member => pure (← `(Expr.enum $(Lean.quote member)), domainTy)
        | .int member =>
            let memberTerm := Lean.quote member
            pure (← `(Expr.int (Int.ofNat $memberTerm)), domainTy)
      else match attrs.find? (·.name == value), paramCtx.find? (·.sourceName == value) with
      | some _, some _ => throwErrorAt name
          "ambiguous identifier '{value}': both an attribute and parameter are in scope"
      | some column, none => pure (← `(Expr.selfAttr $(Lean.quote column.name)), column.ty)
      | none, some paramDecl => pure (← `(Expr.param $(Lean.quote paramDecl.name)), paramDecl.ty)
      | none, none =>
          if frequencyPredicate then
            throwErrorAt name
              "unknown row attribute or model parameter '{value}' in frequency predicate; {frequencyRowLocalMessage}"
          else
            match declaration with
            | some context => throwErrorAt name "{context}: unknown state or attribute '{value}'"
            | none => throwErrorAt name "unknown state or attribute '{value}'"
  | `(semblaExpr| freq ($predicate:semblaExpr) over $key:ident) =>
      let keyAttr ← frequencyKey tableCtx key
      validateFrequencyPredicate predicate
      let (predicateTerm, predicateTy) ←
        elaborateExpr tableCtx attrs paramCtx inputCtx predicate declaration true familyCtx bindingCtx
          domainCtx functionCtx partitionCtx
      unless predicateTy == .bool do
        throwErrorAt predicate
          "frequency predicate has type {typeName predicateTy}; expected Bool"
      let numerator ← keyedCountTerm tableCtx keyAttr predicateTerm
      let trueTerm ← `(Expr.bool true)
      let denominator ← keyedCountTerm tableCtx keyAttr trueTerm
      pure (← `(Expr.div $numerator $denominator), .real)
  | `(semblaExpr| freq ($_predicate:semblaExpr) over)
  | `(semblaExpr| freq ($_predicate:semblaExpr)) =>
      throwErrorAt stx
        "frequency syntax requires a key: use 'freq (<predicate>) over <ref>'"
  | `(semblaExpr| freq $_lhs:ident = $_rhs:ident over $_key:ident)
  | `(semblaExpr| freq $_value:ident over $_key:ident) =>
      throwErrorAt stx
        "frequency syntax requires parentheses around the predicate: use 'freq (<predicate>) over <ref>'"
  | `(semblaExpr| countBy $fk:ident ($filter:semblaExpr)) =>
      let fkAttr ← lookupExprAttr fk
      match fkAttr.ty with
      | .ref _ => pure ()
      | _ => throwErrorAt fk "countBy key '{identText fk}' must be a Ref attribute"
      let (filterTerm, filterTy) ← recur filter
      if filterTy != .bool then throwErrorAt filter "aggregate filter must have type Bool"
      pure (← keyedCountTerm tableCtx fkAttr filterTerm, .int)
  | `(semblaExpr| sizeBy $fk:ident) =>
      let fkAttr ← lookupExprAttr fk
      match fkAttr.ty with
      | .ref _ => pure ()
      | _ => throwErrorAt fk "sizeBy key '{identText fk}' must be a Ref attribute"
      let trueTerm ← `(Expr.bool true)
      pure (← keyedCountTerm tableCtx fkAttr trueTerm, .int)
  | `(semblaExpr| inputSum $port:ident field $column:ident) =>
      let portName := identText port
      let fieldName := identText column
      match inputCtx.find? (·.name == portName) with
      | none => throwErrorAt port "unknown input port '{portName}'"
      | some inputDecl =>
          match inputDecl.schema.find? (·.name == fieldName) with
          | none => throwErrorAt column "unknown input field '{portName}.{fieldName}'"
          | some inputField =>
              unless isNumeric inputField.ty do
                throwErrorAt column "input sum field '{portName}.{fieldName}' must be numeric"
              pure (← `(Expr.input $(Lean.quote portName)
                (Aggregate.mk (AggOp.sum (Expr.selfAttr $(Lean.quote fieldName))) none)), inputField.ty)
  | `(semblaExpr| $lhs:semblaExpr * $rhs:semblaExpr) => elaborateNumericBinary "mul" lhs rhs recur
  | `(semblaExpr| $lhs:semblaExpr · $rhs:semblaExpr) => elaborateNumericBinary "mul" lhs rhs recur
  | `(semblaExpr| $lhs:semblaExpr / $rhs:semblaExpr) => elaborateNumericBinary "div" lhs rhs recur
  | `(semblaExpr| $lhs:semblaExpr + $rhs:semblaExpr) => elaborateNumericBinary "add" lhs rhs recur
  | `(semblaExpr| $lhs:semblaExpr - $rhs:semblaExpr) => elaborateNumericBinary "sub" lhs rhs recur
  | `(semblaExpr| $lhs:semblaExpr = $rhs:semblaExpr) =>
      elaborateEnumComparison "eq" lhs rhs recur
  | `(semblaExpr| $lhs:semblaExpr ≠ $rhs:semblaExpr) =>
      elaborateEnumComparison "ne" lhs rhs recur
  | `(semblaExpr| $lhs:semblaExpr < $rhs:semblaExpr) => elaborateComparison "lt" lhs rhs recur
  | `(semblaExpr| $lhs:semblaExpr ≤ $rhs:semblaExpr) => elaborateComparison "le" lhs rhs recur
  | `(semblaExpr| $lhs:semblaExpr > $rhs:semblaExpr) => elaborateComparison "gt" lhs rhs recur
  | `(semblaExpr| $lhs:semblaExpr ≥ $rhs:semblaExpr) => elaborateComparison "ge" lhs rhs recur
  | `(semblaExpr| $attrName:ident ∈ $bandName:ident) =>
      let binding ← match bindingCtx.find? (·.name == identText bandName) with
        | some found => pure found
        | none => throwErrorAt bandName "partition member '{identText bandName}' is not bound"
      let partition ← match partitionCtx.find? (·.name == binding.domainName) with
        | some found => pure found
        | none => throwErrorAt bandName "domain '{binding.domainName}' is not a projected partition"
      unless identText attrName == partition.target.attrName do
        throwErrorAt attrName "partition '{partition.name}' projects attribute '{partition.target.attrName}'"
      let label ← match binding.member with
        | .enum value => pure value
        | _ => throwErrorAt bandName "partition binding must be an enum label"
      let member ← match partition.members.find? (·.label == label) with
        | some found => pure found
        | none => throwErrorAt bandName "unknown partition member '{label}'"
      let lowerNat := Lean.quote member.lower
      let lowerTerm ← `(Expr.int (Int.ofNat $lowerNat))
      let geTerm ← `(Expr.ge (Expr.selfAttr $(Lean.quote partition.target.attrName)) $lowerTerm)
      match member.upper with
      | none => pure (geTerm, .bool)
      | some upper =>
          let upperNat := Lean.quote upper
          let upperTerm ← `(Expr.int (Int.ofNat $upperNat))
          pure (← `(Expr.and $geTerm
            (Expr.lt (Expr.selfAttr $(Lean.quote partition.target.attrName)) $upperTerm)), .bool)
  | `(semblaExpr| $lhs:semblaExpr && $rhs:semblaExpr) => elaborateAnd "&&" lhs rhs recur
  | `(semblaExpr| $lhs:semblaExpr ∧ $rhs:semblaExpr) => elaborateAnd "∧" lhs rhs recur
  | _ => throwErrorAt stx "unsupported Sembla expression"
where
  elaborateNumericBinary (kind : String) (lhs rhs : Syntax)
      (recur : Syntax → TermElabM (TSyntax `term × SurfaceTy)) : TermElabM (TSyntax `term × SurfaceTy) := do
    let (left, leftTy) ← recur lhs
    let (right, rightTy) ← recur rhs
    unless (leftTy == .real || leftTy == .int) && (rightTy == .real || rightTy == .int) do
      throwErrorAt stx "numeric operator requires numeric operands"
    let resultTy := if kind == "div" || leftTy == .real || rightTy == .real then .real else .int
    let term ← match kind with
      | "mul" => `(Expr.mul $left $right)
      | "div" => `(Expr.div $left $right)
      | "add" => `(Expr.add $left $right)
      | _ => `(Expr.sub $left $right)
    pure (term, resultTy)
  elaborateEnumComparison (kind : String) (lhs rhs : Syntax)
      (recur : Syntax → TermElabM (TSyntax `term × SurfaceTy)) : TermElabM (TSyntax `term × SurfaceTy) := do
    match lhs, rhs with
    | `(semblaExpr| $attrName:ident), `(semblaExpr| $variant:ident) =>
        let sourceName := identText attrName
        match attrs.find? (·.name == sourceName) with
        | some column =>
            if paramCtx.any (·.sourceName == sourceName) then
              throwErrorAt attrName
                "ambiguous identifier '{sourceName}': both an attribute and parameter are in scope"
            match column.ty with
            | .enum variants =>
                let variantName ← match bindingCtx.find? (·.name == identText variant) with
                  | some { member := .enum value, .. } => pure value
                  | some _ => throwErrorAt variant "comparison operands have incompatible types"
                  | none => pure (identText variant)
                unless variants.contains variantName do
                  throwErrorAt variant "unknown variant '{variantName}' for attribute '{column.name}'"
                if kind == "eq" then
                  pure (← `(Expr.enumIs $(Lean.quote column.name) $(Lean.quote variantName)), .bool)
                else
                  pure (← `(Expr.ne (Expr.selfAttr $(Lean.quote column.name))
                    (Expr.enum $(Lean.quote variantName))), .bool)
            | _ => elaborateComparison kind lhs rhs recur
        | none => elaborateComparison kind lhs rhs recur
    | _, _ => elaborateComparison kind lhs rhs recur
  elaborateAnd (operatorName : String) (lhs rhs : Syntax)
      (recur : Syntax → TermElabM (TSyntax `term × SurfaceTy)) : TermElabM (TSyntax `term × SurfaceTy) := do
    let (left, leftTy) ← recur lhs
    let (right, rightTy) ← recur rhs
    if leftTy != .bool then throwErrorAt lhs "left operand of {operatorName} must have type Bool"
    if rightTy != .bool then throwErrorAt rhs "right operand of {operatorName} must have type Bool"
    pure (← `(Expr.and $left $right), .bool)
  elaborateComparison (kind : String) (lhs rhs : Syntax)
      (recur : Syntax → TermElabM (TSyntax `term × SurfaceTy)) : TermElabM (TSyntax `term × SurfaceTy) := do
    let (left, leftTy) ← recur lhs
    let (right, rightTy) ← recur rhs
    if kind == "eq" || kind == "ne" then
      unless equalityCompatible leftTy rightTy do
        throwErrorAt rhs "comparison operands have incompatible types"
    else
      unless isNumeric leftTy && isNumeric rightTy do
        throwErrorAt rhs "ordered comparison operands must be numeric"
    let term ← match kind with
      | "eq" => `(Expr.eq $left $right)
      | "ne" => `(Expr.ne $left $right)
      | "lt" => `(Sembla.IR.Expr.lt $left $right)
      | "le" => `(Expr.le $left $right)
      | "gt" => `(Expr.gt $left $right)
      | _ => `(Expr.ge $left $right)
    pure (term, .bool)

private def enumAttrs (selected : SurfaceSystem) : List SurfaceAttr :=
  selected.attrs.filter fun column =>
    match column.ty with
    | .enum _ => true
    | _ => false

private def attrHasVariant (column : SurfaceAttr) (variant : String) : Bool :=
  match column.ty with
  | .enum variants => variants.contains variant
  | _ => false

private def commaNames (names : List String) : String :=
  names |> String.intercalate ", "

structure ResolvedReaction where
  selected : SurfaceSystem
  stateAttr : SurfaceAttr
  source : String
  destination : String

private def resolveReaction (boxCtx : SurfaceBox) (transitionName : String)
    (transitionToken : Syntax) (systemToken : Option (TSyntax `ident))
    (attributeToken : Option (TSyntax `ident)) (sourceToken : TSyntax `ident)
    (destinationToken : TSyntax `ident) : TermElabM ResolvedReaction := do
  let sourceName := identText sourceToken
  let destinationName := identText destinationToken
  let selected ← match systemToken with
    | some token => lookupSystem boxCtx token
    | none =>
        let candidates := boxCtx.systems.filter fun candidate =>
          match attributeToken with
          | some token =>
              let attributeName := identText token
              candidate.attrs.any fun column =>
                column.name == attributeName &&
                  attrHasVariant column sourceName && attrHasVariant column destinationName
          | none =>
              let columns := enumAttrs candidate
              columns.any fun column =>
                attrHasVariant column sourceName && attrHasVariant column destinationName
        let candidates := match candidates, attributeToken with
          | [], some token =>
              let attributeName := identText token
              let named := boxCtx.systems.filter fun (candidate : SurfaceSystem) =>
                candidate.attrs.any fun column =>
                  column.name == attributeName &&
                    match column.ty with | .enum _ => true | _ => false
              match named with
              | [only] => [only]
              | _ => []
          | found, _ => found
        match candidates with
        | [only] => pure only
        | [] =>
            let considered := commaNames
              (boxCtx.systems.map fun candidate => candidate.logicalName)
            let considered := if considered.isEmpty then "<none>" else considered
            throwErrorAt transitionToken
              "no compatible system for reaction '{transitionName}' among systems: {considered}; add 'on System'"
        | many =>
            throwErrorAt transitionToken
              "multiple compatible systems for reaction '{transitionName}': {commaNames (many.map fun candidate => candidate.logicalName)}; add 'on System'"
  let stateAttr ← match attributeToken with
    | some token =>
        let column ← lookupAttr selected.attrs token
        match column.ty with
        | .enum _ => pure column
        | _ => throwErrorAt token
            "reaction state attribute '{column.name}' must have type Enum"
    | none =>
        let columns := enumAttrs selected
        match columns with
        | [] => throwErrorAt sourceToken
            "system '{selected.logicalName}' has no enum state attributes; add 'attribute:'"
        | [only] => pure only
        | many =>
            let sourceColumns := many.filter (attrHasVariant · sourceName)
            let destinationColumns := many.filter (attrHasVariant · destinationName)
            let sameColumn := many.any fun column =>
              attrHasVariant column sourceName && attrHasVariant column destinationName
            if !sameColumn && !sourceColumns.isEmpty && !destinationColumns.isEmpty then
              throwErrorAt destinationToken
                "source variant '{sourceName}' occurs in state columns {commaNames (sourceColumns.map (·.name))}, but destination variant '{destinationName}' occurs in {commaNames (destinationColumns.map (·.name))}; reaction endpoints must belong to the same state attribute"
            throwErrorAt sourceToken
              "system '{selected.logicalName}' has multiple enum state attributes: {commaNames (many.map (·.name))}; add 'attribute:'"
  let variants := match stateAttr.ty with
    | .enum values => values
    | _ => []
  unless variants.contains sourceName do
    throwErrorAt sourceToken
      "unknown source variant '{sourceName}' for state attribute '{stateAttr.name}'"
  unless variants.contains destinationName do
    throwErrorAt destinationToken
      "unknown destination variant '{destinationName}' for state attribute '{stateAttr.name}'"
  pure ⟨selected, stateAttr, sourceName, destinationName⟩

private def applicationBindings (domains : List SurfaceDomain)
    (outer : List SurfaceIndexBinding) (args : List SurfaceFunctionArg)
    (application : SurfaceStateApplication) : TermElabM (List SurfaceIndexBinding) := do
  unless application.args.length == args.length do
    throwErrorAt application.token
      "state alias '{application.name}' expects {args.length} arguments"
  (args.zip application.args).mapM fun (formal, actual) => do
    let domain ← match domains.find? (·.name == formal.domainName) with
      | some found => pure found
      | none => throwErrorAt formal.domainToken "unknown domain '{formal.domainName}'"
    let actualName := identText actual
    let member ← match outer.find? (·.name == actualName) with
      | some binding =>
          unless binding.domainName == formal.domainName do
            throwErrorAt actual
              "state argument '{actualName}' has domain '{binding.domainName}'; expected '{formal.domainName}'"
          pure binding.member
      | none =>
          match domain.domain with
          | .enumeration _ => pure (.enum actualName)
          | .range _ _ =>
              match actualName.toNat? with
              | some value => pure (.int value)
              | none => throwErrorAt actual "range-domain state arguments must be natural literals"
    unless domain.domain.members.contains member do
      throwErrorAt actual "state argument is outside domain '{formal.domainName}'"
    pure (SurfaceIndexBinding.mk formal.name actual.raw member formal.domainName .static none)

private def validateExprFunctionBodies (paramCtx : List SurfaceParam)
    (families : List SurfaceParamFamily) (domains : List SurfaceDomain)
    (functions : List SurfaceExprFunction) (partitions : List SurfacePartition) :
    TermElabM Unit := do
  let dummySize ← `(term| 0)
  let dummy : SurfaceSystem :=
    { logicalName := "<expression-function>"
      token := Syntax.missing
      irName := "<expression-function>"
      irNameToken := Syntax.missing
      size := dummySize
      attrs := [] }
  for functionDecl in functions do
    for cell in functionDecl.cells do
      let bindings := (functionDecl.args.zip cell.key).map fun (arg, member) =>
        SurfaceIndexBinding.mk arg.name arg.token member.1 arg.domainName .static none
      let (term, actualTy) ← elaborateExpr dummy [] paramCtx [] cell.expression
        (some s!"expression function '{functionDecl.name}'")
        (familyCtx := families) (bindingCtx := bindings) (domainCtx := domains)
        (functionCtx := functions) (partitionCtx := partitions)
      let _ := term
      unless sameType functionDecl.ty actualTy do
        throwErrorAt cell.token
          "expression function '{functionDecl.name}' cell has type {typeName actualTy}; expected {typeName functionDecl.ty}"

private def partitionGuardTerm (boxCtx : SurfaceBox) (selected : SurfaceSystem)
    (partition : SurfacePartition) (binding : SurfaceIndexBinding) :
    TermElabM (TSyntax `term) := do
  unless partition.target.boxName == boxCtx.name &&
      partition.target.systemName == selected.logicalName do
    throwErrorAt partition.token
      "partition '{partition.name}' is not compatible with system '{selected.logicalName}'"
  let label ← match binding.member with
    | .enum value => pure value
    | _ => throwErrorAt binding.token "partition argument must be a partition label"
  let member ← match partition.members.find? (·.label == label) with
    | some found => pure found
    | none => throwErrorAt binding.token "unknown partition member '{label}'"
  let lowerNat := Lean.quote member.lower
  let lowerTerm ← `(Expr.int (Int.ofNat $lowerNat))
  let geTerm ← `(Expr.ge (Expr.selfAttr $(Lean.quote partition.target.attrName)) $lowerTerm)
  match member.upper with
  | none => pure geTerm
  | some upper =>
      let upperNat := Lean.quote upper
      let upperTerm ← `(Expr.int (Int.ofNat $upperNat))
      `(Expr.and $geTerm
        (Expr.lt (Expr.selfAttr $(Lean.quote partition.target.attrName)) $upperTerm))

private def aliasGuardAtoms (paramCtx : List SurfaceParam) (boxCtx : SurfaceBox)
    (selected : SurfaceSystem) (application : SurfaceStateApplication)
    (families : List SurfaceParamFamily) (bindings : List SurfaceIndexBinding)
    (domains : List SurfaceDomain) (functions : List SurfaceExprFunction)
    (partitions : List SurfacePartition) : TermElabM (List (TSyntax `term)) := do
  if let some partition := partitions.find? (·.name == application.name) then
    unless application.args.length == 1 do
      throwErrorAt application.token "partition application expects exactly one argument"
    let actual := application.args.head!
    let actualName := identText actual
    let binding ← match bindings.find? (·.name == actualName) with
      | some found => pure found
      | none =>
          let domain := domains.find? (·.name == partition.name)
          let member := ParameterTable.IndexMember.enum actualName
          unless domain.any (·.domain.members.contains member) do
            throwErrorAt actual "unknown partition label '{actualName}'"
          pure (SurfaceIndexBinding.mk actualName actual.raw member partition.name .static none)
    unless binding.domainName == partition.name do
      throwErrorAt actual "partition argument has incompatible domain"
    return [← partitionGuardTerm boxCtx selected partition binding]
  let alias ← match boxCtx.aliases.find? (·.name == application.name) with
    | some found => pure found
    | none => throwErrorAt application.token "unknown state alias '{application.name}'"
  unless identText alias.system == selected.logicalName do
    throwErrorAt application.token
      "state alias '{alias.name}' selects system '{identText alias.system}', not '{selected.logicalName}'"
  let localBindings ← applicationBindings domains bindings alias.args application
  let allBindings := localBindings ++ bindings
  alias.atoms.mapM fun atom => match atom with
    | .assignment attrName value token => do
        let destination ← lookupAttr selected.attrs attrName
        match destination.ty with
        | .enum variants =>
            match value with
            | `(semblaExpr| $identifier:ident) =>
                let authored := identText identifier
                let concrete ← match allBindings.find? (·.name == authored) with
                  | some { member := .enum member, .. } => pure member
                  | some _ => throwErrorAt identifier "state assignment has incompatible type"
                  | none => pure authored
                unless variants.contains concrete do
                  throwErrorAt identifier "unknown variant '{concrete}' for attribute '{destination.name}'"
                `(Expr.enumIs $(Lean.quote destination.name) $(Lean.quote concrete))
            | _ => throwErrorAt value "enum state assignments require a value or variant"
        | .ref _ => throwErrorAt token "state aliases cannot match Ref attributes by assignment"
        | _ =>
            let (valueTerm, valueTy) ← elaborateExpr selected selected.attrs paramCtx boxCtx.inputs
              value (familyCtx := families) (bindingCtx := allBindings) (domainCtx := domains)
              (functionCtx := functions) (partitionCtx := partitions)
            unless sameType destination.ty valueTy do
              throwErrorAt token "state assignment has incompatible type"
            `(Expr.eq (Expr.selfAttr $(Lean.quote destination.name)) $valueTerm)
    | .predicate expression token => do
        let (term, ty) ← elaborateExpr selected selected.attrs paramCtx boxCtx.inputs expression
          (familyCtx := families) (bindingCtx := allBindings) (domainCtx := domains)
          (functionCtx := functions) (partitionCtx := partitions)
        unless ty == .bool do throwErrorAt token "state match expression must have type Bool"
        pure term

private def validateStateAliases (paramCtx : List SurfaceParam) (boxCtx : SurfaceBox)
    (families : List SurfaceParamFamily) (domains : List SurfaceDomain)
    (functions : List SurfaceExprFunction) (partitions : List SurfacePartition) :
    TermElabM Unit := do
  ensureUnique "state alias" (boxCtx.aliases.map fun alias => (alias.name, alias.token))
  for alias in boxCtx.aliases do
    if partitions.any (·.name == alias.name) then
      throwErrorAt alias.token
        "name '{alias.name}' is ambiguous between a partition and a state alias"
    ensureUnique "state alias argument" (alias.args.map fun arg => (arg.name, arg.token))
    let selected ← lookupSystem boxCtx alias.system
    let mut bindings : List SurfaceIndexBinding := []
    for arg in alias.args do
      let domain ← match domains.find? (·.name == arg.domainName) with
        | some found => pure found
        | none => throwErrorAt arg.domainToken "unknown domain '{arg.domainName}'"
      let member ← match domain.domain.members.head? with
        | some found => pure found
        | none => throwErrorAt arg.domainToken "domain '{arg.domainName}' must not be empty"
      bindings := bindings ++ [SurfaceIndexBinding.mk arg.name arg.token member
        arg.domainName .static none]
    for atom in alias.atoms do
      let expression := match atom with
        | .assignment _ value _ => value
        | .predicate value _ => value
      rejectAggregates "state aliases" expression
      if let .assignment attrName value token := atom then
        let destination ← lookupAttr selected.attrs attrName
        if let `(semblaExpr| $identifier:ident) := value then
          if let some formal := alias.args.find? (·.name == identText identifier) then
            let sourceDomain := (domains.find? (·.name == formal.domainName)).map (·.domain)
            let compatible := match sourceDomain, destination.ty with
              | some (.range _ _), .int => true
              | some (.enumeration members), .enum variants => members == variants
              | _, _ => false
            unless compatible do
              throwErrorAt token "state assignment has incompatible type"
    let application : SurfaceStateApplication :=
      { name := alias.name, token := alias.token,
        args := alias.args.map fun arg => ⟨arg.token⟩ }
    let _ ← aliasGuardAtoms paramCtx boxCtx selected application families bindings
      domains functions partitions

private def aliasEffectTerms (paramCtx : List SurfaceParam) (boxCtx : SurfaceBox)
    (selected : SurfaceSystem) (application : SurfaceStateApplication)
    (families : List SurfaceParamFamily) (bindings : List SurfaceIndexBinding)
    (domains : List SurfaceDomain) (functions : List SurfaceExprFunction)
    (partitions : List SurfacePartition) : TermElabM (List (TSyntax `term)) := do
  if partitions.any (·.name == application.name) then
    throwErrorAt application.token "projected partitions cannot appear after become"
  let alias ← match boxCtx.aliases.find? (·.name == application.name) with
    | some found => pure found
    | none => throwErrorAt application.token "unknown state alias '{application.name}'"
  unless identText alias.system == selected.logicalName do
    throwErrorAt application.token "state alias selects an incompatible system"
  let localBindings ← applicationBindings domains bindings alias.args application
  let allBindings := localBindings ++ bindings
  let mut effects : List (TSyntax `term) := []
  for atom in alias.atoms do
    match atom with
    | .predicate _ token =>
        throwErrorAt token "state alias containing match cannot be used after become"
    | .assignment attrName value token =>
        let destination ← lookupAttr selected.attrs attrName
        match destination.ty with
        | .ref _ => throwErrorAt attrName
            "writes to Ref attributes require resource claims, which are not supported by this DSL"
        | .enum variants =>
            match value with
            | `(semblaExpr| $identifier:ident) =>
                let authored := identText identifier
                let concrete ← match allBindings.find? (·.name == authored) with
                  | some { member := .enum member, .. } => pure member
                  | some _ => throwErrorAt identifier "effect value has incompatible type"
                  | none => pure authored
                unless variants.contains concrete do
                  throwErrorAt identifier
                    "unknown variant '{concrete}' for attribute '{destination.name}'"
                effects := effects ++ [← `(Effect.setAttr $(Lean.quote destination.name)
                  (Expr.enum $(Lean.quote concrete)))]
            | _ => throwErrorAt value "enum effect values must be variant literals"
        | .real | .int =>
            rejectAggregates "effect expressions" value
            let (valueTerm, valueTy) ← elaborateExpr selected selected.attrs paramCtx
              boxCtx.inputs value (familyCtx := families) (bindingCtx := allBindings)
              (domainCtx := domains) (functionCtx := functions) (partitionCtx := partitions)
            unless sameType destination.ty valueTy do
              throwErrorAt token "effect value has incompatible type"
            effects := effects ++ [← `(Effect.setAttr $(Lean.quote destination.name) $valueTerm)]
        | .bool => throwErrorAt token "effect value has incompatible type"
  pure effects

private partial def rightFoldAnd (atoms : List (TSyntax `term)) (token : Syntax) :
    TermElabM (TSyntax `term) := do
  match atoms with
  | [] => throwErrorAt token "source pattern expands to no guard atoms"
  | [only] => pure only
  | head :: tail => `(Expr.and $head $(← rightFoldAnd tail token))

private def selectedSystemForTransition (boxCtx : SurfaceBox)
    (transitionDecl : SurfaceTransition) : TermElabM SurfaceSystem := do
  match transitionDecl.body with
  | .general onSystem _ _ _ _ => lookupSystem boxCtx onSystem
  | .reaction onSystem stateAttr source _ destination =>
      return (← resolveReaction boxCtx transitionDecl.name transitionDecl.token
        onSystem stateAttr source destination).selected
  | .namedReaction onSystem _ _ _ _ => lookupSystem boxCtx onSystem
  | .relation onSystem _ _ => lookupSystem boxCtx onSystem

structure ResolvedTransitionBody where
  selected : SurfaceSystem
  guardTerm : TSyntax `term
  hazardSyntax : TSyntax `semblaExpr
  effectTerms : Array (TSyntax `term)
  contestTerms : Array (TSyntax `term)

/-- Shared identifier-assignment validation for expanded transitions and
    reaction arrows.  Reactions retain their original destination token while
    using the same enum-membership, Ref-write, and value-type checks. -/
private def identifierEffectTerm (paramCtx : List SurfaceParam) (boxCtx : SurfaceBox)
    (selected : SurfaceSystem) (attrName value : TSyntax `ident)
    (families : List SurfaceParamFamily := []) (bindings : List SurfaceIndexBinding := [])
    (domains : List SurfaceDomain := []) (functions : List SurfaceExprFunction := [])
    (partitions : List SurfacePartition := []) : TermElabM (TSyntax `term) := do
  let destination ← lookupAttr selected.attrs attrName
  match destination.ty with
  | .ref _ => throwErrorAt attrName
      "writes to Ref attributes require resource claims, which are not supported by this DSL"
  | _ => pure ()
  let authoredValueName := identText value
  let valueTerm ← match destination.ty with
    | .enum variants =>
        let valueName ← match bindings.find? (·.name == authoredValueName) with
          | some { member := .enum member, .. } => pure member
          | some _ => throwErrorAt value "effect value has incompatible type"
          | none => pure authoredValueName
        unless variants.contains valueName do
          throwErrorAt value "unknown variant '{valueName}' for attribute '{destination.name}'"
        `(Expr.enum $(Lean.quote valueName))
    | _ =>
        let (term, actualTy) ←
          elaborateExpr selected selected.attrs paramCtx boxCtx.inputs value
            (familyCtx := families) (bindingCtx := bindings) (domainCtx := domains)
            (functionCtx := functions) (partitionCtx := partitions)
        unless sameType destination.ty actualTy do
          throwErrorAt value "effect value has incompatible type"
        pure term
  `(Effect.setAttr $(Lean.quote destination.name) $valueTerm)

/-- Effect values share the scalar expression elaborator used by guards and
    hazards. Aggregates remain a deliberate surface rejection until a runtime
    effect regression pins their snapshot and cache behavior. -/
private partial def rejectEffectAggregates (stx : Syntax) : TermElabM Unit := do
  match stx with
  | `(semblaExpr| inputSum $_port:ident field $_field:ident)
  | `(semblaExpr| countBy $_countKey:ident ($_filter:semblaExpr))
  | `(semblaExpr| sizeBy $_sizeKey:ident)
  | `(semblaExpr| freq ($_predicate:semblaExpr) over $_freqKey:ident) =>
      throwErrorAt stx "aggregates are not supported in effect expressions"
  | _ =>
      for child in stx.getArgs do
        rejectEffectAggregates child

private def effectTerm (paramCtx : List SurfaceParam) (boxCtx : SurfaceBox)
    (selected : SurfaceSystem) (attrName : TSyntax `ident) (value : TSyntax `semblaExpr)
    (families : List SurfaceParamFamily := []) (bindings : List SurfaceIndexBinding := [])
    (domains : List SurfaceDomain := []) (functions : List SurfaceExprFunction := [])
    (partitions : List SurfacePartition := []) : TermElabM (TSyntax `term) := do
  let destination ← lookupAttr selected.attrs attrName
  match destination.ty with
  | .ref _ => throwErrorAt attrName
      "writes to Ref attributes require resource claims, which are not supported by this DSL"
  | .enum variants =>
      match value with
      | `(semblaExpr| $variant:ident) =>
          let variantName ← match bindings.find? (·.name == identText variant) with
            | some { member := .enum member, .. } => pure member
            | some _ => throwErrorAt variant "effect value has incompatible type"
            | none => pure (identText variant)
          unless variants.contains variantName do
            throwErrorAt variant
              "unknown variant '{variantName}' for attribute '{destination.name}'"
          `(Effect.setAttr $(Lean.quote destination.name) (Expr.enum $(Lean.quote variantName)))
      | _ => throwErrorAt value "enum effect values must be variant literals"
  | .real | .int =>
      rejectEffectAggregates value
      let (valueTerm, actualTy) ←
        elaborateExpr selected selected.attrs paramCtx boxCtx.inputs value
          (familyCtx := families) (bindingCtx := bindings) (domainCtx := domains)
          (functionCtx := functions) (partitionCtx := partitions)
      unless sameType destination.ty actualTy do
        throwErrorAt value "effect value has incompatible type"
      `(Effect.setAttr $(Lean.quote destination.name) $valueTerm)
  | .bool => throwErrorAt value "effect value has incompatible type"

private def resolveTransitionBody (paramCtx : List SurfaceParam) (boxCtx : SurfaceBox)
    (transitionDecl : SurfaceTransition) (families : List SurfaceParamFamily := [])
    (bindings : List SurfaceIndexBinding := []) (domains : List SurfaceDomain := [])
    (functions : List SurfaceExprFunction := []) (partitions : List SurfacePartition := []) :
    TermElabM ResolvedTransitionBody := do
  match transitionDecl.body with
  | .reaction onSystem stateAttr source hazardExpr destination =>
      let resolved ← resolveReaction boxCtx transitionDecl.name transitionDecl.token
        onSystem stateAttr source destination
      let guardTerm ← `(Expr.enumIs $(Lean.quote resolved.stateAttr.name)
        $(Lean.quote resolved.source))
      let attrName := stateAttr.getD ⟨resolved.stateAttr.nameToken⟩
      let effectTerm ← identifierEffectTerm paramCtx boxCtx resolved.selected attrName destination
        families bindings domains functions partitions
      pure ⟨resolved.selected, guardTerm, hazardExpr, #[effectTerm], #[]⟩
  | .general onSystem guardExpr hazardExpr contests assignments =>
      let selected ← lookupSystem boxCtx onSystem
      let (guardTerm, guardTy) ←
        elaborateExpr selected selected.attrs paramCtx boxCtx.inputs guardExpr
          (familyCtx := families) (bindingCtx := bindings) (domainCtx := domains)
          (functionCtx := functions) (partitionCtx := partitions)
      if guardTy != .bool then
        throwErrorAt guardExpr "guard has type {typeName guardTy}; expected Bool"
      let mut contestTerms : Array (TSyntax `term) := #[]
      let mut contestedAttrs : List String := []
      for claimDecl in contests do
        let resource ← lookupAttr selected.attrs claimDecl.resource
        match resource.ty with
        | .ref _ => pure ()
        | _ => throwErrorAt claimDecl.resource
            "contest attribute '{resource.name}' must have type Ref"
        if contestedAttrs.contains resource.name then
          throwErrorAt claimDecl.resource "duplicate contest for attribute '{resource.name}'"
        contestedAttrs := contestedAttrs ++ [resource.name]
        contestTerms := contestTerms.push
          (← `(ResourceClaim.mk (Expr.selfAttr $(Lean.quote resource.name)) ClaimOrdering.raceTime))
      let mut effects : Array (TSyntax `term) := #[]
      for assignment in assignments do
        match assignment with
        | `(semblaSet| $attrName:ident := $value:semblaExpr) =>
            effects := effects.push (← effectTerm paramCtx boxCtx selected attrName value
              families bindings domains functions partitions)
        | _ => throwUnsupportedSyntax
      pure ⟨selected, guardTerm, hazardExpr, effects, contestTerms⟩
  | .namedReaction onSystem source _axes hazardExpr destination =>
      let selected ← lookupSystem boxCtx onSystem
      let sourceAtoms ← aliasGuardAtoms paramCtx boxCtx selected source families bindings
        domains functions partitions
      let guardTerm ← rightFoldAnd sourceAtoms source.token
      let effectList ← aliasEffectTerms paramCtx boxCtx selected destination families bindings
        domains functions partitions
      if effectList.isEmpty then
        throwErrorAt destination.token "named-state destination expands to no effects"
      pure ⟨selected, guardTerm, hazardExpr, effectList.toArray, #[]⟩
  | .relation onSystem _constraints items =>
      let selected ← lookupSystem boxCtx onSystem
      let mut sourceAtoms : List (TSyntax `term) := []
      let mut hazardSyntax : Option (TSyntax `semblaExpr) := none
      let mut effects : Array (TSyntax `term) := #[]
      let mut contestTerms : Array (TSyntax `term) := #[]
      let mut contestedAttrs : List String := []
      for item in items do
        match item with
        | .source applications _ =>
            for application in applications do
              sourceAtoms := sourceAtoms ++ (← aliasGuardAtoms paramCtx boxCtx selected
                application families bindings domains functions partitions)
        | .hazard expression _ => hazardSyntax := some expression
        | .claim claimDecl _ =>
            let resource ← lookupAttr selected.attrs claimDecl.resource
            match resource.ty with
            | .ref _ => pure ()
            | _ => throwErrorAt claimDecl.resource
                "contest attribute '{resource.name}' must have type Ref"
            if contestedAttrs.contains resource.name then
              throwErrorAt claimDecl.resource "duplicate contest for attribute '{resource.name}'"
            contestedAttrs := contestedAttrs ++ [resource.name]
            contestTerms := contestTerms.push
              (← `(ResourceClaim.mk (Expr.selfAttr $(Lean.quote resource.name))
                ClaimOrdering.raceTime))
        | .set assignment _ =>
            match assignment with
            | `(semblaSet| $attrName:ident := $value:semblaExpr) =>
                effects := effects.push (← effectTerm paramCtx boxCtx selected attrName value
                  families bindings domains functions partitions)
            | _ => throwUnsupportedSyntax
        | .become application _ =>
            effects := effects ++ (← aliasEffectTerms paramCtx boxCtx selected application
              families bindings domains functions partitions).toArray
      if effects.isEmpty then
        throwErrorAt transitionDecl.token "relation requires at least one effect after expansion"
      let hazardExpr ← hazardSyntax.getDM
        (throwErrorAt transitionDecl.token "relation requires exactly one hazard")
      let guardTerm ← rightFoldAnd sourceAtoms transitionDecl.token
      pure ⟨selected, guardTerm, hazardExpr, effects, contestTerms⟩

private partial def transitionInstances (indexes : List SurfaceIndex) (boxCtx : SurfaceBox)
    (transitionDecl : SurfaceTransition) :
    TermElabM (List (String × List SurfaceIndexBinding)) := do
  if transitionDecl.binders.isEmpty then
    match transitionDecl.body with
    | .namedReaction onSystem _ axes _ _ =>
        let selected ← lookupSystem boxCtx onSystem
        let mut seenAxes : List String := []
        let mut inferred : List SurfaceTransitionBinder := []
        for axis in axes do
          let axisName := identText axis
          if seenAxes.contains axisName then
            throwErrorAt axis "duplicate inferred arrow axis '{axisName}'"
          seenAxes := axisName :: seenAxes
          let column ← lookupAttr selected.attrs axis
          let domainName ← match column.domainName with
            | some found => pure found
            | none => throwErrorAt axis
                "inferred arrow axis '{axisName}' must use a named-domain attribute"
          inferred := inferred ++ [SurfaceTransitionBinder.mk axisName axis.raw
            (some domainName) (some axis.raw) .inferredMatched (some axisName)]
        return ← transitionInstances indexes boxCtx { transitionDecl with binders := inferred }
    | _ => return [(transitionDecl.name, [])]
  match transitionDecl.body with
  | .reaction none _ _ _ _ => throwErrorAt transitionDecl.token
      "indexed reaction families require an explicit 'on System'"
  | _ => pure ()
  let selected ← selectedSystemForTransition boxCtx transitionDecl
  let mut seen : List String := []
  let mut domains : List (SurfaceTransitionBinder × ParameterTable.IndexDomain) := []
  for binder in transitionDecl.binders do
    let name := binder.name
    if seen.contains name then throwErrorAt binder.token "duplicate transition-family index '{name}'"
    seen := name :: seen
    let domainName := binder.domainName.getD name
    let indexDecl ← match indexes.find? (·.name == domainName) with
      | some found => pure found
      | none => throwErrorAt (binder.domainToken.getD binder.token) "unknown index '{domainName}'"
    if binder.mode != .static then
      let attrName := binder.matchedAttribute.getD name
      let column ← match selected.attrs.find? (·.name == attrName) with
        | some found => pure found
        | none => throwErrorAt binder.token "unknown state or attribute '{attrName}'"
      match indexDecl.domain, column.ty with
      | .range _ _, .int => pure ()
      | .enumeration members, .enum variants =>
          unless members == variants do
            throwErrorAt binder.token "index '{domainName}' members do not match attribute '{attrName}'"
      | .range _ _, _ => throwErrorAt binder.token "range index '{domainName}' requires an Int attribute"
      | .enumeration _, _ => throwErrorAt binder.token "enum index '{domainName}' requires a matching enum attribute"
    domains := domains ++ [(binder, indexDecl.domain)]
  let _ ← checkExpansionCardinality "transition family" transitionDecl.token (domains.map (·.2))
  let mut combinations := cartesianMembers (domains.map fun entry => entry.2.members)
  if let .relation _ constraints _ := transitionDecl.body then
    for constraint in constraints do
      let (left, right, token) := match constraint with
        | .equal left right token | .notEqual left right token => (left, right, token)
      let leftName := identText left
      let rightName := identText right
      let leftIndex ← match transitionDecl.binders.findIdx? (·.name == leftName) with
        | some found => pure found
        | none => throwErrorAt left "unknown relation constraint binder '{leftName}'"
      let rightIndex ← match transitionDecl.binders.findIdx? (·.name == rightName) with
        | some found => pure found
        | none => throwErrorAt right "unknown relation constraint binder '{rightName}'"
      let leftDomain := (transitionDecl.binders.get? leftIndex).bind (·.domainName)
      let rightDomain := (transitionDecl.binders.get? rightIndex).bind (·.domainName)
      unless leftDomain == rightDomain do
        throwErrorAt token "relation constraint operands must have the same domain"
      combinations := combinations.filter fun combination =>
        let equal := match combination.get? leftIndex, combination.get? rightIndex with
          | some leftMember, some rightMember => leftMember == rightMember
          | _, _ => false
        match constraint with | .equal .. => equal | .notEqual .. => !equal
  combinations.mapM fun combination => do
    let bindings := (domains.zip combination).map fun ((binder, _), member) =>
      { name := binder.name, token := binder.token, member,
        domainName := binder.domainName.getD binder.name, mode := binder.mode,
        matchedAttribute := binder.matchedAttribute }
    let components ← combination.mapM fun member => match canonicalIndexComponent member with
      | .ok value => pure value
      | .error message => throwErrorAt transitionDecl.token message
    pure (transitionDecl.name ++ "_" ++ String.intercalate "_" components, bindings)

private def transitionTerm (paramCtx : List SurfaceParam) (boxCtx : SurfaceBox)
    (families : List SurfaceParamFamily) (transitionDecl : SurfaceTransition)
    (generatedName : String) (bindings : List SurfaceIndexBinding)
    (domains : List SurfaceDomain) (functions : List SurfaceExprFunction)
    (partitions : List SurfacePartition) : TermElabM (TSyntax `term) := do
  let resolved ← resolveTransitionBody paramCtx boxCtx transitionDecl families bindings
    domains functions partitions
  let (hazardTerm, hazardTy) ← elaborateExpr resolved.selected resolved.selected.attrs
    paramCtx boxCtx.inputs resolved.hazardSyntax
    (familyCtx := families) (bindingCtx := bindings) (domainCtx := domains)
    (functionCtx := functions) (partitionCtx := partitions)
  unless hazardTy == .real do
    throwErrorAt resolved.hazardSyntax "hazard has type {typeName hazardTy}; expected Real"
  let mut guardTerm := resolved.guardTerm
  for binding in bindings do
    if binding.mode != .static then
      let attrName := binding.matchedAttribute.getD binding.name
      let indexGuard ← match binding.member with
        | .enum value => `(Expr.enumIs $(Lean.quote attrName) $(Lean.quote value))
        | .int value =>
            let valueTerm := Lean.quote value
            `(Expr.eq (Expr.selfAttr $(Lean.quote attrName))
              (Expr.int (Int.ofNat $valueTerm)))
      guardTerm ← `(Expr.and $guardTerm $indexGuard)
  let effects := resolved.effectTerms
  let contests := resolved.contestTerms
  `(Transition.mk $(Lean.quote generatedName) $(Lean.quote resolved.selected.irName)
      $guardTerm $hazardTerm [$effects,*] [$contests,*])

private def transitionTerms (indexes : List SurfaceIndex) (families : List SurfaceParamFamily)
    (paramCtx : List SurfaceParam) (boxCtx : SurfaceBox) (transitionDecl : SurfaceTransition)
    (domains : List SurfaceDomain) (functions : List SurfaceExprFunction)
    (partitions : List SurfacePartition) : TermElabM (Array (TSyntax `term)) := do
  let instances ← transitionInstances indexes boxCtx transitionDecl
  return (← instances.mapM fun (name, bindings) =>
    transitionTerm paramCtx boxCtx families transitionDecl name bindings
      domains functions partitions).toArray

private def outputTerm (paramCtx : List SurfaceParam) (boxCtx : SurfaceBox)
    (outputDecl : SurfaceOutput) : TermElabM (TSyntax `term) := do
  let selected ← lookupSystem boxCtx outputDecl.system
  ensureUnique "output schema field"
    (outputDecl.schema.map fun item => (item.name, item.nameToken))
  ensureUnique "output builder field" (outputDecl.fields.map fun item => (item.name, item.token))
  for outputField in outputDecl.fields do
    unless outputDecl.schema.any (·.name == outputField.name) do
      throwErrorAt outputField.token "output field '{outputField.name}' is absent from port schema"
  let mut fieldTerms : Array (TSyntax `term) := #[]
  -- Emit builders in schema order because the frozen IR contract is positional.
  for schemaField in outputDecl.schema do
    let outputField ← match outputDecl.fields.find? (·.name == schemaField.name) with
      | some builderField => pure builderField
      | none => throwErrorAt schemaField.nameToken
          "output schema field '{schemaField.name}' has no builder"
    match outputField.op, outputField.filter, outputField.value with
    | "count", some filterExpr, none =>
        unless schemaField.ty == .int do
          throwErrorAt outputField.token "count output field '{outputField.name}' must have type Int"
        let (filterTerm, filterTy) ←
          elaborateExpr selected selected.attrs paramCtx boxCtx.inputs filterExpr
        if filterTy != .bool then throwErrorAt filterExpr "output filter must have type Bool"
        fieldTerms := fieldTerms.push (← `(OutputField.mk $(Lean.quote outputField.name)
          AggOp.count (some $filterTerm)))
    | "sum", none, some valueExpr =>
        let (valueTerm, valueTy) ←
          elaborateExpr selected selected.attrs paramCtx boxCtx.inputs valueExpr
        unless isNumeric valueTy do
          throwErrorAt valueExpr "output sum value must be numeric"
        unless sameType schemaField.ty valueTy do
          throwErrorAt valueExpr "output sum value has incompatible type"
        fieldTerms := fieldTerms.push (← `(OutputField.mk $(Lean.quote outputField.name)
          (AggOp.sum $valueTerm) none))
    | _, _, _ => throwErrorAt outputField.token "invalid output builder"
  let schemaTerms ← outputDecl.schema.toArray.mapM (attrTerm boxCtx)
  `(OutputDecl.mk $(Lean.quote outputDecl.name) [$schemaTerms,*]
      (OutputBuilder.perTable $(Lean.quote selected.irName) [$fieldTerms,*]))

private def viewTerm (paramCtx : List SurfaceParam) (boxCtx : SurfaceBox)
    (viewDecl : SurfaceView) : TermElabM (TSyntax `term) := do
  let systemName := identText viewDecl.system
  let selected ← match boxCtx.systems.find? (·.logicalName == systemName) with
    | some found => pure found
    | none => throwErrorAt viewDecl.system
        "view '{viewDecl.name}' refers to unknown table '{systemName}'"
  let context := some s!"view '{viewDecl.name}'"
  let filterTerm ← match viewDecl.filter with
    | none => `(none)
    | some filterExpr =>
        let (term, ty) ← elaborateExpr selected selected.attrs paramCtx boxCtx.inputs
          filterExpr context
        if ty != .bool then
          throwErrorAt filterExpr
            "view '{viewDecl.name}' filter has type {typeName ty}; expected Bool"
        `(some $term)
  let valueTerm ← match viewDecl.reduce, viewDecl.value with
    | "count", none => `(none)
    | "count", some _ => throwErrorAt viewDecl.token
        "view '{viewDecl.name}' with reduce count cannot declare a value expression"
    | _, none => throwErrorAt viewDecl.token
        "view '{viewDecl.name}' with reduce {viewDecl.reduce} must declare a value expression"
    | _, some valueExpr =>
        let (term, ty) ← elaborateExpr selected selected.attrs paramCtx boxCtx.inputs
          valueExpr context
        unless isNumeric ty do
          throwErrorAt valueExpr
            "view '{viewDecl.name}' value has type {typeName ty}; expected Real or Int"
        `(some $term)
  let reduceTerm ← match viewDecl.reduce with
    | "sum" => `(ViewReduce.sum)
    | "count" => `(ViewReduce.count)
    | "min" => `(ViewReduce.min)
    | "max" => `(ViewReduce.max)
    | _ => throwErrorAt viewDecl.token "unsupported view reduction '{viewDecl.reduce}'"
  `(ViewDecl.mk $(Lean.quote viewDecl.name) $(Lean.quote selected.irName)
      $filterTerm $valueTerm $reduceTerm)

private partial def rejectGroupedFilterAggregates (stx : Syntax) : TermElabM Unit := do
  match stx with
  | `(semblaExpr| inputSum $_port:ident field $_field:ident)
  | `(semblaExpr| countBy $_countKey:ident ($_filter:semblaExpr))
  | `(semblaExpr| sizeBy $_sizeKey:ident)
  | `(semblaExpr| freq ($_predicate:semblaExpr) over $_freqKey:ident) =>
      throwErrorAt stx "aggregates are not supported in grouped view filters"
  | _ =>
      for child in stx.getArgs do
        rejectGroupedFilterAggregates child

/- Grouped views always elaborate completely: Lean authoring has no runtime flag
   context. Rust validation and execution enforce `grouped-observations` (K6). -/
private def groupedViewTerm (paramCtx : List SurfaceParam) (boxCtx : SurfaceBox)
    (viewDecl : SurfaceGroupedView) : TermElabM (TSyntax `term) := do
  let systemName := identText viewDecl.system
  let selected ← match boxCtx.systems.find? (·.logicalName == systemName) with
    | some found => pure found
    | none => throwErrorAt viewDecl.system
        "grouped view '{viewDecl.name}' refers to unknown table '{systemName}'"
  if viewDecl.keys.isEmpty then
    throwErrorAt viewDecl.token "grouped view '{viewDecl.name}' requires at least one key"
  if viewDecl.keys.length > 4 then
    let some fifth := viewDecl.keys.get? 4
      | throwErrorAt viewDecl.token "internal grouped key count mismatch"
    throwErrorAt fifth.attr
      "grouped view '{viewDecl.name}' supports at most 4 keys"
  let mut keyTerms : Array (TSyntax `term) := #[]
  for key in viewDecl.keys do
    let column ← lookupAttr selected.attrs key.attr
    let widthTerm ← match column.ty, key.bandWidth with
      | .int, some (0, token) =>
          throwErrorAt token "grouped band width must be greater than zero"
      | .int, some (width, _) => `(some $(Lean.quote width))
      | .int, none =>
          let expected := if viewDecl.scopedSyntax then
            s!"band({column.name}, <positive-width>)"
          else s!"band {column.name} <positive-width>"
          throwErrorAt key.attr "Int grouped key '{column.name}' requires '{expected}'"
      | .enum _, none | .ref _, none => `(none)
      | .enum _, some _ | .ref _, some _ => throwErrorAt key.attr
          "band is supported only for Int grouped keys; '{column.name}' has type {typeName column.ty}"
      | .real, _ => throwErrorAt key.attr
          "grouped key '{column.name}' has type Real; expected Enum, Ref, or banded Int"
      | .bool, _ => throwErrorAt key.attr "Bool grouped keys are not supported"
    keyTerms := keyTerms.push (← `(GroupKey.mk $(Lean.quote column.name) $widthTerm))
  let filterTerm ← match viewDecl.filter with
    | none => `(none)
    | some filterExpr =>
        rejectGroupedFilterAggregates filterExpr
        let (term, ty) ← elaborateExpr selected selected.attrs paramCtx boxCtx.inputs
          filterExpr (some s!"grouped view '{viewDecl.name}'")
        if ty != .bool then throwErrorAt filterExpr
          "grouped view '{viewDecl.name}' filter has type {typeName ty}; expected Bool"
        `(some $term)
  `(GroupedViewDecl.mk $(Lean.quote viewDecl.name) $(Lean.quote selected.irName)
      $filterTerm [$keyTerms,*])

private def summaryTerm (boxCtxs : List SurfaceBox) (summaryDecl : SurfaceSummary) :
    TermElabM (TSyntax `term) := do
  let boxName := identText summaryDecl.box
  let boxCtx ← match boxCtxs.find? (·.name == boxName) with
    | some found => pure found
    | none => throwErrorAt summaryDecl.box
        "summary '{summaryDecl.name}' refers to unknown box '{boxName}'"
  let viewName := identText summaryDecl.view
  unless boxCtx.views.any (·.name == viewName) do
    throwErrorAt summaryDecl.view
      "summary '{summaryDecl.name}' refers to undeclared view '{boxName}.{viewName}'"
  let reduceTerm ← match summaryDecl.reduce with
    | "sum" => `(SummaryReduce.sum)
    | "min" => `(SummaryReduce.min)
    | "max" => `(SummaryReduce.max)
    | "last" => `(SummaryReduce.last)
    | "argmax_tick" => `(SummaryReduce.argmaxTick)
    | _ => throwErrorAt summaryDecl.token
        "unsupported summary reduction '{summaryDecl.reduce}'"
  `(SummaryDecl.mk $(Lean.quote summaryDecl.name) $(Lean.quote boxName)
      $(Lean.quote viewName) $reduceTerm)

private def resolvedTy (boxCtx : SurfaceBox) : SurfaceTy → Option SurfaceTy
  | .ref logical => boxCtx.systems.find? (·.logicalName == logical) |>.map fun target => .ref target.irName
  | ty => some ty

private def schemasMatch (leftBox : SurfaceBox) (left : List SurfaceAttr)
    (rightBox : SurfaceBox) (right : List SurfaceAttr) : Bool :=
  left.length == right.length && (left.zip right).all fun (a, b) =>
    a.name == b.name && resolvedTy leftBox a.ty == resolvedTy rightBox b.ty

private unsafe def evalModelUnsafe (expr : Lean.Expr) : TermElabM Model :=
  Meta.evalExpr Model (mkConst ``Model) expr

@[implemented_by evalModelUnsafe]
private opaque evalModel (expr : Lean.Expr) : TermElabM Model

private def modelTerm (name : String) (stepWidth : TSyntax `term)
    (params boxes wires summaryTerms : Array (TSyntax `term)) : TermElabM (TSyntax `term) :=
  `(Model.mk $(Lean.quote name) $stepWidth [$params,*] [$boxes,*] [$wires,*] [$summaryTerms,*])

/-- Single shared path for validation, IR emission, and one-time evaluation.
    Widget attachment is a caller policy; all IR-building helpers stay private. -/
private def elaborateSurfaceModelCore (attachWidgets : Bool) (surface : SurfaceModel)
    (elaborateTerm : TSyntax `term → TermElabM Lean.Expr) : TermElabM Lean.Expr := do
  let modelName := match surface.runtimeName with
    | some runtime => runtime.1
    | none => surface.declarationName
  let stepWidth := surface.dt
  let paramCtx := surface.params
  let boxCtxs := surface.boxes
  let wireCtx := surface.wires
  let summaryCtx := surface.summaries
  let indexCtx := surface.indexes
  let familyCtx := surface.families
  let domainCtx := surface.domains
  let functionCtx := surface.exprFunctions
  let partitionCtx := surface.partitions

  -- Pass one: validate the complete collected declaration graph.
  validateStep stepWidth
  ensureUnique "parameter" (paramCtx.map fun p => (p.sourceName, p.token))
  ensureUniqueRuntimeNames "parameter" (paramCtx.map fun p => (p.name, p.sourceName, p.token))
  for paramDecl in paramCtx do
    match paramDecl.ty with
    | .real => validateRealTerm paramDecl.default
    | .int => validateIntTerm paramDecl.default
    | _ => throwErrorAt paramDecl.token "unsupported parameter type"
    match paramDecl.prior with
    | some priorArgs =>
        validateRealTerm priorArgs.1
        validateRealTerm priorArgs.2
    | none => pure ()
  validateExprFunctionBodies paramCtx familyCtx domainCtx functionCtx partitionCtx
  ensureUnique "box" (boxCtxs.map fun b => (b.name, b.token))
  for boxCtx in boxCtxs do
    ensureUnique "system" (boxCtx.systems.map fun s => (s.logicalName, s.token))
    ensureUniqueRuntimeNames "table" (boxCtx.systems.map fun s =>
      (s.irName, s.logicalName, s.irNameToken))
    for selected in boxCtx.systems do validateSize selected.size
    ensureUnique "input port" (boxCtx.inputs.map fun p => (p.name, p.token))
    ensureUnique "transition" (boxCtx.transitions.map fun t => (t.name, t.token))
    let mut generatedTransitionNames : List (String × Syntax) := []
    for transitionDecl in boxCtx.transitions do
      for (name, _) in (← transitionInstances indexCtx boxCtx transitionDecl) do
        generatedTransitionNames := generatedTransitionNames ++ [(name, transitionDecl.token)]
    ensureUnique "transition" generatedTransitionNames
    ensureUnique "output port" (boxCtx.outputs.map fun p => (p.name, p.token))
    ensureUnique "view" (
      boxCtx.views.map (fun declaration => (declaration.name, declaration.token)) ++
      boxCtx.groupedViews.map (fun declaration => (declaration.name, declaration.token)))
    for selected in boxCtx.systems do
      validateAttrs "attribute" selected.attrs
    for inputDecl in boxCtx.inputs do
      validateAttrs "input field" inputDecl.schema
    for outputDecl in boxCtx.outputs do
      validateAttrs "output schema field" outputDecl.schema
    validateStateAliases paramCtx boxCtx familyCtx domainCtx functionCtx partitionCtx
  ensureUnique "summary" (summaryCtx.map fun declaration =>
    (declaration.name, declaration.token))

  -- Pass two: resolve from the declarations above and emit one pure deep-IR term.
  let mut paramTerms : Array (TSyntax `term) := #[]
  for paramDecl in paramCtx do
    let term ← match paramDecl.ty, paramDecl.prior with
      | .real, some priorArgs =>
          let familyTerm ← match paramDecl.priorFamily with
            | .logNormal => `(PriorFamily.logNormal)
            | .normal => `(PriorFamily.normal)
          `(ParamDecl.mk $(Lean.quote paramDecl.name) ParamType.real
            (ParamValue.real $(paramDecl.default))
            (some (Prior.mk $familyTerm [$(priorArgs.1), $(priorArgs.2)])))
      | .real, none => `(ParamDecl.mk $(Lean.quote paramDecl.name) ParamType.real
          (ParamValue.real $(paramDecl.default)) none)
      | .int, none => `(ParamDecl.mk $(Lean.quote paramDecl.name) ParamType.int
          (ParamValue.int $(paramDecl.default)) none)
      | .int, some _ => throwErrorAt paramDecl.token
          "priors are not supported on Int parameters"
      | _, _ => throwErrorAt paramDecl.token "unsupported parameter type"
    paramTerms := paramTerms.push term

  let mut boxTerms : Array (TSyntax `term) := #[]
  for boxCtx in boxCtxs do
    -- Ref targets are checked only after all systems are collected, allowing forward refs.
    let mut tableTerms : Array (TSyntax `term) := #[]
    for selected in boxCtx.systems do
      let attrTerms ← selected.attrs.toArray.mapM (attrTerm boxCtx)
      tableTerms := tableTerms.push (← `(Table.mk $(Lean.quote selected.irName)
        $(selected.size) [$attrTerms,*]))
    let mut transitionTermList : Array (TSyntax `term) := #[]
    for transitionDecl in boxCtx.transitions do
      transitionTermList := transitionTermList ++
        (← transitionTerms indexCtx familyCtx paramCtx boxCtx transitionDecl
          domainCtx functionCtx partitionCtx)
    let mut inputTerms : Array (TSyntax `term) := #[]
    for inputDecl in boxCtx.inputs do
      let schemaTerms ← inputDecl.schema.toArray.mapM (attrTerm boxCtx)
      inputTerms := inputTerms.push (← `(PortDecl.mk $(Lean.quote inputDecl.name) [$schemaTerms,*]))
    let outputTerms ← boxCtx.outputs.toArray.mapM (outputTerm paramCtx boxCtx)
    let viewTerms ← boxCtx.views.toArray.mapM (viewTerm paramCtx boxCtx)
    let groupedViewTerms ← boxCtx.groupedViews.toArray.mapM (groupedViewTerm paramCtx boxCtx)
    boxTerms := boxTerms.push (← `(Box.mk $(Lean.quote boxCtx.name) [$tableTerms,*]
      [$transitionTermList,*] [$inputTerms,*] [$outputTerms,*] [$viewTerms,*]
      [$groupedViewTerms,*]))

  let mut wireTerms : Array (TSyntax `term) := #[]
  let mut deliveredInputs : List String := []
  for wireDecl in wireCtx do
    let fromBoxName := identText wireDecl.fromBox
    let toBoxName := identText wireDecl.toBox
    let fromBoxCtx ← match boxCtxs.find? (·.name == fromBoxName) with
      | some found => pure found
      | none => throwErrorAt wireDecl.fromBox "unknown wire source box '{fromBoxName}'"
    let toBoxCtx ← match boxCtxs.find? (·.name == toBoxName) with
      | some found => pure found
      | none => throwErrorAt wireDecl.toBox "unknown wire target box '{toBoxName}'"
    let fromPortName := identText wireDecl.fromPort
    let toPortName := identText wireDecl.toPort
    let deliveryKey := toBoxName ++ "." ++ toPortName
    if deliveredInputs.contains deliveryKey then
      throwErrorAt wireDecl.toPort "duplicate wire target '{deliveryKey}'"
    deliveredInputs := deliveryKey :: deliveredInputs
    let fromPort ← match fromBoxCtx.outputs.find? (·.name == fromPortName) with
      | some port => pure port
      | none => throwErrorAt wireDecl.fromPort "unknown output port '{fromBoxName}.{fromPortName}'"
    let toPort ← match toBoxCtx.inputs.find? (·.name == toPortName) with
      | some port => pure port
      | none => throwErrorAt wireDecl.toPort "unknown input port '{toBoxName}.{toPortName}'"
    unless schemasMatch fromBoxCtx fromPort.schema toBoxCtx toPort.schema do
      throwErrorAt wireDecl.toPort "wire schema mismatch for '{fromBoxName}.{fromPortName}' -> '{toBoxName}.{toPortName}'"
    wireTerms := wireTerms.push (← `(Wire.mk
      (WireEndpoint.mk $(Lean.quote fromBoxName) $(Lean.quote fromPortName))
      (WireEndpoint.mk $(Lean.quote toBoxName) $(Lean.quote toPortName))))

  let mut summaryTerms : Array (TSyntax `term) := #[]
  for summaryDecl in summaryCtx do
    summaryTerms := summaryTerms.push (← summaryTerm boxCtxs summaryDecl)

  let result ← modelTerm modelName stepWidth paramTerms boxTerms wireTerms summaryTerms
  let elaborated ← elaborateTerm result
  synthesizeSyntheticMVarsNoPostponing
  let modelValue ← evalModel elaborated

  if attachWidgets then
    -- Attach thin ProofWidgets panels to the original declaration-name ranges.
    -- The displayed JSON props come only from the pure IR builders.
    for boxCtx in boxCtxs do
      for selected in boxCtx.systems do
        if let some props := stateDiagramProps? modelValue boxCtx.name selected.irName then
          saveStateDiagram props selected.token
      for transitionDecl in boxCtx.transitions do
        let selected ← selectedSystemForTransition boxCtx transitionDecl
        if let some props := stateDiagramProps? modelValue boxCtx.name selected.irName then
          saveStateDiagram props transitionDecl.token
        if transitionDecl.binders.isEmpty then
          if let some props := hazardPanelProps? modelValue boxCtx.name transitionDecl.name then
            saveHazardPanel props transitionDecl.token

  pure elaborated

/-- Shared surface kernel with the existing model-widget behavior. -/
def elaborateSurfaceModel (surface : SurfaceModel)
    (elaborateTerm : TSyntax `term → TermElabM Lean.Expr) : TermElabM Lean.Expr :=
  elaborateSurfaceModelCore true surface elaborateTerm

/-- Shared surface kernel without widget anchors, for composition authoring. -/
def elaborateSurfaceModelNoWidgets (surface : SurfaceModel)
    (elaborateTerm : TSyntax `term → TermElabM Lean.Expr) : TermElabM Lean.Expr :=
  elaborateSurfaceModelCore false surface elaborateTerm

elab "model%" name:str "step" "(" stepWidth:term ")" "where"
    "params" "[" paramDecls:semblaParam,* "]"
    "boxes" "[" boxDecls:semblaBox,* "]"
    "wires" "[" wireDecls:semblaWire,* "]"
    summaryBlock:(semblaSummaryBlock)? : term => do
  let surface ← collectLegacySurfaceModel name stepWidth
    paramDecls.getElems.toList boxDecls.getElems.toList wireDecls.getElems.toList summaryBlock
  elaborateSurfaceModel surface fun result => elabTerm result none

private def defineCommandModel (declaration : TSyntax `ident)
    (runtimeOverride : Option (TSyntax `str)) (stepWidth : TSyntax `term)
    (items : List (TSyntax `semblaCommandModelItem)) : Command.CommandElabM Unit := do
  let currentNamespace ← getCurrNamespace
  let declarationName := currentNamespace ++ declaration.getId
  checkNotAlreadyDeclared declarationName
  Command.runTermElabM fun _ => Term.withDeclName declarationName do
    let surface ← collectCommandSurfaceModel declaration runtimeOverride stepWidth items
    let value ← elaborateSurfaceModel surface fun result =>
      elabTerm result (some (mkConst ``Model))
    let value ← instantiateMVars value
    let modelDeclaration : Declaration := .defnDecl {
      name := declarationName
      levelParams := []
      type := mkConst ``Model
      value := value
      hints := .regular 0
      safety := .safe }
    Term.ensureNoUnassignedMVars modelDeclaration
    addAndCompile modelDeclaration
    Term.addTermInfo' declaration (mkConst declarationName) (isBinder := true)

@[command_elab semblaModelCommand] private def elabSemblaModel : Command.CommandElab := fun stx => do
  match stx with
  | `(command| sembla_model $declaration:ident (dt := $stepWidth:term) where
        $items:semblaCommandModelItem*) =>
      defineCommandModel declaration none stepWidth items.toList
  | _ => throwUnsupportedSyntax

@[command_elab semblaNamedModelCommand] private def elabNamedSemblaModel :
    Command.CommandElab := fun stx => do
  match stx with
  | `(command| sembla_model $declaration:ident
        ($overrideKeyword:ident := $runtimeName:str) (dt := $stepWidth:term) where
        $items:semblaCommandModelItem*) =>
      unless identText overrideKeyword == "name" do
        throwErrorAt overrideKeyword "expected 'name' model override"
      defineCommandModel declaration (some runtimeName) stepWidth items.toList
  | _ => throwUnsupportedSyntax

@[command_elab semblaModelMissingDtCommand] private def elabMissingSemblaDt :
    Command.CommandElab := fun stx => do
  match stx with
  | `(command| sembla_model $declaration:ident where
        $_items:semblaCommandModelItem*) =>
      throwErrorAt declaration "sembla_model requires '(dt := <positive decimal>)'"
  | _ => throwUnsupportedSyntax

@[command_elab semblaNamedModelMissingDtCommand] private def elabNamedMissingSemblaDt :
    Command.CommandElab := fun stx => do
  match stx with
  | `(command| sembla_model $declaration:ident
        ($_overrideKeyword:ident := $_runtimeName:str) where
        $_items:semblaCommandModelItem*) =>
      throwErrorAt declaration "sembla_model requires '(dt := <positive decimal>)'"
  | _ => throwUnsupportedSyntax

@[command_elab semblaMisplacedSystemCommand] private def elabMisplacedSemblaSystem :
    Command.CommandElab := fun stx => do
  match stx with
  | `(command| system $name:ident (rows := $_size:term)) =>
      throwErrorAt name "system declaration must be indented inside a sembla_model box"
  | _ => throwUnsupportedSyntax

end Sembla.DSL
