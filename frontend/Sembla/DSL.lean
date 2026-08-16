import Lean.Elab.Term
import Lean.Elab.Command
import Sembla.IR
import Sembla.Frontend.Builders
import Sembla.ParameterTable
import Sembla.WidgetDisplay

namespace Sembla.DSL
open Lean Elab Term Sembla.IR Sembla.Widgets Sembla.WidgetDisplay
open Sembla.Frontend.Builders Sembla.Semantics

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
-- Mathlib registers the adjacent `→[` token.  Keep explicit alternatives so
-- the established no-space surface spelling remains accepted when builders
-- are imported, while the spaced spelling above remains unchanged.
syntax ident "[" ident,* "]" "on" ident ":" ident ":" ident "→[" semblaExpr "]" ident : semblaTransition
syntax ident "on" ident ":" ident "[" ident,* "]" "→[" semblaExpr "]" ident : semblaTransition
syntax ident "on" ident ":" ident "[" ident,* "]" "→[" semblaExpr "]" ident "(" ident,* ")" : semblaTransition
syntax ident ":" ident "→[" semblaExpr "]" ident : semblaTransition
syntax ident "on" ident ":" ident "→[" semblaExpr "]" ident : semblaTransition
syntax ident ":" ident ":" ident "→[" semblaExpr "]" ident : semblaTransition
syntax ident "on" ident ":" ident ":" ident "→[" semblaExpr "]" ident : semblaTransition
syntax ident ":" ident "→[" semblaExpr "]" ident semblaArrowTail : semblaTransition
syntax ident "on" ident ":" ident "→[" semblaExpr "]" ident
  semblaArrowTail : semblaTransition
syntax ident ":" ident ":" ident "→[" semblaExpr "]" ident
  semblaArrowTail : semblaTransition
syntax ident "on" ident ":" ident ":" ident "→[" semblaExpr "]" ident
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
syntax ident ident "(" semblaTypedBinder,* ")" "on" ident Lean.Parser.rawIdent Lean.Parser.rawIdent
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
  | `(semblaTransition| $name:ident [$binders:ident,*] on $onSystem:ident :
        $stateAttr:ident : $source:ident →[$hazardExpr:semblaExpr] $destination:ident) =>
      if binders.getElems.isEmpty then
        throwErrorAt name "indexed transition family requires at least one index"
      pure ⟨identText name, name.raw,
        .reaction (some onSystem) (some stateAttr) source hazardExpr destination,
        binders.getElems.toList.map legacyTransitionBinder⟩
  | `(semblaTransition| $name:ident on $onSystem:ident : $sourceAlias:ident
        [$axes:ident,*] →[$hazardExpr:semblaExpr] $destinationAlias:ident) =>
      if axes.getElems.isEmpty then
        throwErrorAt sourceAlias "indexed named-state arrow requires at least one axis"
      pure ⟨identText name, name.raw,
        .namedReaction onSystem ⟨identText sourceAlias, sourceAlias.raw, []⟩
          axes.getElems.toList hazardExpr
          ⟨identText destinationAlias, destinationAlias.raw, []⟩, []⟩
  | `(semblaTransition| $name:ident on $onSystem:ident : $sourceAlias:ident
        [$axes:ident,*] →[$hazardExpr:semblaExpr] $destinationAlias:ident
        ($destinationArgs:ident,*)) =>
      if axes.getElems.isEmpty then
        throwErrorAt sourceAlias "indexed named-state arrow requires at least one axis"
      pure ⟨identText name, name.raw,
        .namedReaction onSystem ⟨identText sourceAlias, sourceAlias.raw, []⟩
          axes.getElems.toList hazardExpr
          ⟨identText destinationAlias, destinationAlias.raw, destinationArgs.getElems.toList⟩, []⟩
  | `(semblaTransition| $name:ident : $source:ident →[$hazardExpr:semblaExpr]
        $destination:ident) =>
      pure ⟨identText name, name.raw,
        .reaction none none source hazardExpr destination, []⟩
  | `(semblaTransition| $name:ident on $onSystem:ident : $source:ident →[$hazardExpr:semblaExpr]
        $destination:ident) =>
      pure ⟨identText name, name.raw,
        .reaction (some onSystem) none source hazardExpr destination, []⟩
  | `(semblaTransition| $name:ident : $stateAttr:ident : $source:ident →[$hazardExpr:semblaExpr]
        $destination:ident) =>
      pure ⟨identText name, name.raw,
        .reaction none (some stateAttr) source hazardExpr destination, []⟩
  | `(semblaTransition| $name:ident on $onSystem:ident : $stateAttr:ident :
        $source:ident →[$hazardExpr:semblaExpr] $destination:ident) =>
      pure ⟨identText name, name.raw,
        .reaction (some onSystem) (some stateAttr) source hazardExpr destination, []⟩
  | `(semblaTransition| $_name:ident : $_source:ident →[$_hazardExpr:semblaExpr]
        $_destination:ident $tail:semblaArrowTail) =>
      rejectReactionContest tail
  | `(semblaTransition| $_name:ident on $_onSystem:ident : $_source:ident →[$_hazardExpr:semblaExpr]
        $_destination:ident $tail:semblaArrowTail) =>
      rejectReactionContest tail
  | `(semblaTransition| $_name:ident : $_stateAttr:ident : $_source:ident →[$_hazardExpr:semblaExpr]
        $_destination:ident $tail:semblaArrowTail) =>
      rejectReactionContest tail
  | `(semblaTransition| $_name:ident on $_onSystem:ident : $_stateAttr:ident :
        $_source:ident →[$_hazardExpr:semblaExpr] $_destination:ident
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
  | _ =>
      match stx.raw with
      | .node _ _ relationArgs =>
          unless relationArgs.size == 12 do
            throwUnsupportedSyntax
          let keyword : TSyntax `ident := ⟨relationArgs[0]!⟩
          let name : TSyntax `ident := ⟨relationArgs[1]!⟩
          let selected : TSyntax `ident := ⟨relationArgs[6]!⟩
          let subject : TSyntax `ident := ⟨relationArgs[7]!⟩
          let targetKeyword : TSyntax `ident := ⟨relationArgs[8]!⟩
          unless identText subject == "subject" && identText targetKeyword == "to" do
            throwUnsupportedSyntax
          let binderSyntax : Array (TSyntax `semblaTypedBinder) :=
            relationArgs[3]!.getArgs.filterMap fun item =>
              if item.isAtom && item.getAtomVal == "," then none else some ⟨item⟩
          let constraintSyntax : Array (TSyntax `semblaRelationConstraint) :=
            relationArgs[9]!.getArgs.filterMap fun item =>
              if item.isAtom && item.getAtomVal == "," then none else some ⟨item⟩
          let itemSyntax : Array (TSyntax `semblaRelationItem) :=
            relationArgs[11]!.getArgs.map fun item => ⟨item⟩
          finish keyword name selected binderSyntax
            (← constraintSyntax.toList.mapM parseRelationConstraint) itemSyntax
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
    | some (_, firstSource) =>
        if firstSource != sourceName then
          throwErrorAt token
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

private def parameterDefaultValueTerm (stx : TSyntax `term) :
    TermElabM (TSyntax `term) := do
  match stx with
  | `(term| $value:scientific) =>
      validateScientific value false
      `(ParamValue.real $stx)
  | `(term| -$value:scientific) =>
      validateScientific value false
      `(ParamValue.real $stx)
  | `(term| $value:num) =>
      validateIntTerm stx
      `(ParamValue.int $stx)
  | `(term| -$value:num) =>
      validateIntTerm stx
      `(ParamValue.int $stx)
  | _ => throwErrorAt stx "parameter defaults require a numeric literal"

private def attrTerm (boxCtx : SurfaceBox) (column : SurfaceAttr) : TermElabM (TSyntax `term) := do
  let name := Lean.quote column.name
  match column.ty with
  | .real => `(attributeRaw $name AttrType.real)
  | .int => `(attributeRaw $name AttrType.int)
  | .enum variants =>
      let values : Array (TSyntax `term) := variants.toArray.map fun value => ⟨Syntax.mkStrLit value⟩
      `(attributeRaw $name (AttrType.enum [$values,*]))
  | .ref target =>
      let emittedTarget := (boxCtx.systems.find? (·.logicalName == target)).map (·.irName)
        |>.getD target
      `(attributeRaw $name (AttrType.ref $(Lean.quote emittedTarget)))
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
  `(TransitionRaw.relatedAggregate TransitionRaw.count $(Lean.quote tableCtx.irName)
    $(Lean.quote key.name) $(Lean.quote key.name) $filter)

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
  | `(semblaExpr| $value:num) => pure (← `(TransitionRaw.int $value), .int)
  | `(semblaExpr| $value:scientific) =>
      validateScientific value false
      pure (← `(TransitionRaw.real $value), .real)
  | `(semblaExpr| true) => pure (← `(TransitionRaw.bool true), .bool)
  | `(semblaExpr| false) => pure (← `(TransitionRaw.bool false), .bool)
  | `(semblaExpr| ¬$inner:semblaExpr) =>
      let (term, ty) ← recur inner
      unless ty == .bool do throwErrorAt inner "operand of ¬ must have type Bool"
      pure (← `(TransitionRaw.not $term), .bool)
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
          pure (← `(TransitionRaw.parameter $(Lean.quote runtimeName)), family.ty)
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
      pure (← `(TransitionRaw.parameter $(Lean.quote runtimeName)), family.ty)
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
      pure (← `(TransitionRaw.parameter $(Lean.quote paramDecl.name)), paramDecl.ty)
  | `(semblaExpr| $name:ident) =>
      let value := identText name
      if value == "true" then pure (← `(TransitionRaw.bool true), .bool)
      else if value == "false" then pure (← `(TransitionRaw.bool false), .bool)
      else if let some binding := bindingCtx.find? (·.name == value) then
        let domainTy := match domainCtx.find? (·.name == binding.domainName) with
          | some { domain := .enumeration variants, .. } => SurfaceTy.enum variants
          | _ => SurfaceTy.int
        match binding.member with
        | .enum member => pure (← `(TransitionRaw.enum $(Lean.quote member)), domainTy)
        | .int member =>
            let memberTerm := Lean.quote member
            pure (← `(TransitionRaw.int (Int.ofNat $memberTerm)), domainTy)
      else match attrs.find? (·.name == value), paramCtx.find? (·.sourceName == value) with
      | some _, some _ => throwErrorAt name
          "ambiguous identifier '{value}': both an attribute and parameter are in scope"
      | some column, none =>
          pure (← `(TransitionRaw.selfAttribute $(Lean.quote column.name)), column.ty)
      | none, some paramDecl =>
          pure (← `(TransitionRaw.parameter $(Lean.quote paramDecl.name)), paramDecl.ty)
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
      let trueTerm ← `(TransitionRaw.bool true)
      let denominator ← keyedCountTerm tableCtx keyAttr trueTerm
      pure (← `(TransitionRaw.div $numerator $denominator), .real)
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
      let trueTerm ← `(TransitionRaw.bool true)
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
              pure (← `(TransitionRaw.input $(Lean.quote portName)
                (TransitionRaw.aggregate
                  (TransitionRaw.sum (TransitionRaw.selfAttribute $(Lean.quote fieldName))) none)),
                inputField.ty)
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
      let lowerTerm ← `(TransitionRaw.int (Int.ofNat $lowerNat))
      let geTerm ← `(TransitionRaw.ge
        (TransitionRaw.selfAttribute $(Lean.quote partition.target.attrName)) $lowerTerm)
      match member.upper with
      | none => pure (geTerm, .bool)
      | some upper =>
          let upperNat := Lean.quote upper
          let upperTerm ← `(TransitionRaw.int (Int.ofNat $upperNat))
          pure (← `(TransitionRaw.and $geTerm
            (TransitionRaw.lt
              (TransitionRaw.selfAttribute $(Lean.quote partition.target.attrName)) $upperTerm)),
            .bool)
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
      | "mul" => `(TransitionRaw.mul $left $right)
      | "div" => `(TransitionRaw.div $left $right)
      | "add" => `(TransitionRaw.add $left $right)
      | _ => `(TransitionRaw.sub $left $right)
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
                  pure (← `(TransitionRaw.enumIs
                    $(Lean.quote column.name) $(Lean.quote variantName)), .bool)
                else
                  pure (← `(TransitionRaw.ne
                    (TransitionRaw.selfAttribute $(Lean.quote column.name))
                    (TransitionRaw.enum $(Lean.quote variantName))), .bool)
            | _ => elaborateComparison kind lhs rhs recur
        | none => elaborateComparison kind lhs rhs recur
    | _, _ => elaborateComparison kind lhs rhs recur
  elaborateAnd (operatorName : String) (lhs rhs : Syntax)
      (recur : Syntax → TermElabM (TSyntax `term × SurfaceTy)) : TermElabM (TSyntax `term × SurfaceTy) := do
    let (left, leftTy) ← recur lhs
    let (right, rightTy) ← recur rhs
    if leftTy != .bool then throwErrorAt lhs "left operand of {operatorName} must have type Bool"
    if rightTy != .bool then throwErrorAt rhs "right operand of {operatorName} must have type Bool"
    pure (← `(TransitionRaw.and $left $right), .bool)
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
      | "eq" => `(TransitionRaw.eq $left $right)
      | "ne" => `(TransitionRaw.ne $left $right)
      | "lt" => `(TransitionRaw.lt $left $right)
      | "le" => `(TransitionRaw.le $left $right)
      | "gt" => `(TransitionRaw.gt $left $right)
      | _ => `(TransitionRaw.ge $left $right)
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

structure ResolvedReactionChoice where
  selected : SurfaceSystem
  stateAttr : SurfaceAttr
  source : String
  destination : String

private def resolveReactionChoice (boxCtx : SurfaceBox) (transitionName : String)
    (transitionToken : Syntax) (systemToken : Option (TSyntax `ident))
    (attributeToken : Option (TSyntax `ident)) (sourceToken : TSyntax `ident)
    (destinationToken : TSyntax `ident) : TermElabM ResolvedReactionChoice := do
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
  pure ⟨selected, stateAttr, sourceName, destinationName⟩

private def resolveReaction (boxCtx : SurfaceBox) (transitionName : String)
    (transitionToken : Syntax) (systemToken : Option (TSyntax `ident))
    (attributeToken : Option (TSyntax `ident)) (sourceToken : TSyntax `ident)
    (destinationToken : TSyntax `ident) : TermElabM ResolvedReactionChoice := do
  let resolved ← resolveReactionChoice boxCtx transitionName transitionToken systemToken
    attributeToken sourceToken destinationToken
  let variants := match resolved.stateAttr.ty with
    | .enum values => values
    | _ => []
  unless variants.contains resolved.source do
    throwErrorAt sourceToken
      "unknown source variant '{resolved.source}' for state attribute '{resolved.stateAttr.name}'"
  unless variants.contains resolved.destination do
    throwErrorAt destinationToken
      "unknown destination variant '{resolved.destination}' for state attribute '{resolved.stateAttr.name}'"
  pure resolved

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

/-- The sanctioned declaration-only compatibility path for compile-time
expression-function cells, which have no Raw IR V1 declaration. It is invoked
once before substitution; emitted uses remain checker-owned. -/
private def trustedValidateExprFunctionCompatibility (paramCtx : List SurfaceParam)
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
  let lowerTerm ← `(TransitionRaw.int (Int.ofNat $lowerNat))
  let geTerm ← `(TransitionRaw.ge
    (TransitionRaw.selfAttribute $(Lean.quote partition.target.attrName)) $lowerTerm)
  match member.upper with
  | none => pure geTerm
  | some upper =>
      let upperNat := Lean.quote upper
      let upperTerm ← `(TransitionRaw.int (Int.ofNat $upperNat))
      `(TransitionRaw.and $geTerm
        (TransitionRaw.lt
          (TransitionRaw.selfAttribute $(Lean.quote partition.target.attrName)) $upperTerm))

private structure AliasDeclId where
  ordinal : Nat
  deriving BEq

private structure AliasGuardAtomProvenance where
  aliasId : AliasDeclId
  applicationToken : Syntax
  emittedGuardAtomOrdinal : Nat
  declarationAtomToken : Syntax
  destinationToken : Option Syntax
  valueToken : Syntax

private structure AliasEffectProvenance where
  aliasId : AliasDeclId
  applicationToken : Syntax
  emittedEffectOrdinal : Nat
  declarationAtomToken : Syntax
  destinationToken : Syntax
  valueToken : Syntax

private structure EmittedAliasProvenance where
  totalSourceAtomCount : Nat
  totalEffectCount : Nat
  guardAtoms : List AliasGuardAtomProvenance
  effects : List AliasEffectProvenance

private def EmittedAliasProvenance.empty : EmittedAliasProvenance := ⟨0, 0, [], []⟩

private structure LoweredAliasGuardAtoms where
  terms : List (TSyntax `term)
  provenance : List AliasGuardAtomProvenance

private structure LoweredAliasEffects where
  terms : List (TSyntax `term)
  provenance : List AliasEffectProvenance

private def lookupStateAliasWithId (boxCtx : SurfaceBox)
    (application : SurfaceStateApplication) : TermElabM (AliasDeclId × SurfaceStateAlias) := do
  let rec find (ordinal : Nat) : List SurfaceStateAlias → Option (AliasDeclId × SurfaceStateAlias)
    | [] => none
    | declaration :: rest =>
        if declaration.name == application.name then some (⟨ordinal⟩, declaration)
        else find (ordinal + 1) rest
  let found ← match find 0 boxCtx.aliases with
    | some found => pure found
    | none => throwErrorAt application.token "unknown state alias '{application.name}'"
  unless found.1.ordinal < boxCtx.aliases.length do
    throwError "internal state alias identity is out of range"
  pure found

private def trustedCheckedAliasGuardAtoms (paramCtx : List SurfaceParam) (boxCtx : SurfaceBox)
    (selected : SurfaceSystem) (application : SurfaceStateApplication)
    (families : List SurfaceParamFamily) (bindings : List SurfaceIndexBinding)
    (domains : List SurfaceDomain) (functions : List SurfaceExprFunction)
    (partitions : List SurfacePartition) (sourceAtomOffset : Nat := 0) :
    TermElabM LoweredAliasGuardAtoms := do
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
    return ⟨[← partitionGuardTerm boxCtx selected partition binding], []⟩
  let (aliasId, stateAlias) ← lookupStateAliasWithId boxCtx application
  unless identText stateAlias.system == selected.logicalName do
    throwErrorAt application.token
      "state alias '{stateAlias.name}' selects system '{identText stateAlias.system}', not '{selected.logicalName}'"
  let localBindings ← applicationBindings domains bindings stateAlias.args application
  let allBindings := localBindings ++ bindings
  let mut terms : List (TSyntax `term) := []
  let mut provenance : List AliasGuardAtomProvenance := []
  for atom in stateAlias.atoms do
    let ordinal := sourceAtomOffset + terms.length
    match atom with
    | .assignment attrName value token => do
        let destination ← lookupAttr selected.attrs attrName
        let term ← match destination.ty with
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
                  `(TransitionRaw.enumIs
                    $(Lean.quote destination.name) $(Lean.quote concrete))
              | _ => throwErrorAt value "enum state assignments require a value or variant"
          | .ref _ => throwErrorAt token "state aliases cannot match Ref attributes by assignment"
          | _ =>
              let (valueTerm, valueTy) ← elaborateExpr selected selected.attrs paramCtx boxCtx.inputs
                value (familyCtx := families) (bindingCtx := allBindings) (domainCtx := domains)
                (functionCtx := functions) (partitionCtx := partitions)
              unless sameType destination.ty valueTy do
                throwErrorAt token "state assignment has incompatible type"
              `(TransitionRaw.eq
                (TransitionRaw.selfAttribute $(Lean.quote destination.name)) $valueTerm)
        terms := terms ++ [term]
        provenance := provenance ++ [⟨aliasId, application.token, ordinal, token,
          some attrName.raw, value.raw⟩]
    | .predicate expression token => do
        let (term, ty) ← elaborateExpr selected selected.attrs paramCtx boxCtx.inputs expression
          (familyCtx := families) (bindingCtx := allBindings) (domainCtx := domains)
          (functionCtx := functions) (partitionCtx := partitions)
        unless ty == .bool do throwErrorAt token "state match expression must have type Bool"
        terms := terms ++ [term]
        provenance := provenance ++ [⟨aliasId, application.token, ordinal, token,
          none, expression.raw⟩]
  pure ⟨terms, provenance⟩

private def validateStateAliasExpansionShape (boxCtx : SurfaceBox)
    (domains : List SurfaceDomain) (partitions : List SurfacePartition) :
    TermElabM Unit := do
  ensureUnique "state alias" (boxCtx.aliases.map fun stateAlias =>
    (stateAlias.name, stateAlias.token))
  for stateAlias in boxCtx.aliases do
    if partitions.any (·.name == stateAlias.name) then
      throwErrorAt stateAlias.token
        "name '{stateAlias.name}' is ambiguous between a partition and a state alias"
    ensureUnique "state alias argument" (stateAlias.args.map fun arg =>
      (arg.name, arg.token))
    let _ ← lookupSystem boxCtx stateAlias.system
    for arg in stateAlias.args do
      let domain ← match domains.find? (·.name == arg.domainName) with
        | some found => pure found
        | none => throwErrorAt arg.domainToken "unknown domain '{arg.domainName}'"
      if domain.domain.members.isEmpty then
        throwErrorAt arg.domainToken "domain '{arg.domainName}' must not be empty"
    for atom in stateAlias.atoms do
      let expression := match atom with
        | .assignment _ value _ => value
        | .predicate value _ => value
      rejectAggregates "state aliases" expression

/-- The sole trusted semantic compatibility exception for raw-IR-V1 state
aliases. It is called exactly once only after emitted provenance proves that the
declaration is unused. -/
private def trustedValidateUnusedStateAliasCompatibility
    (paramCtx : List SurfaceParam) (boxCtx : SurfaceBox) (stateAlias : SurfaceStateAlias)
    (families : List SurfaceParamFamily) (domains : List SurfaceDomain)
    (functions : List SurfaceExprFunction) (partitions : List SurfacePartition) :
    TermElabM Unit := do
    let selected ← lookupSystem boxCtx stateAlias.system
    let mut bindings : List SurfaceIndexBinding := []
    for arg in stateAlias.args do
      let domain ← match domains.find? (·.name == arg.domainName) with
        | some found => pure found
        | none => throwErrorAt arg.domainToken "unknown domain '{arg.domainName}'"
      let member ← match domain.domain.members.head? with
        | some (.int lower) =>
            -- A trusted unused alias needs one concrete range-domain witness for
            -- syntax-independent compatibility checking; use the range's lower
            -- member rather than the synthetic `.members` enumeration origin.
            match domain.domain with
            | .range actualLower _ => pure (.int actualLower)
            | _ => pure (.int lower)
        | some found => pure found
        | none => throwErrorAt arg.domainToken "domain '{arg.domainName}' must not be empty"
      bindings := bindings ++ [SurfaceIndexBinding.mk arg.name arg.token member
        arg.domainName .static none]
    for atom in stateAlias.atoms do
      if let .assignment attrName value token := atom then
        let destination ← lookupAttr selected.attrs attrName
        if let `(semblaExpr| $identifier:ident) := value then
          if let some formal := stateAlias.args.find? (·.name == identText identifier) then
            let sourceDomain := (domains.find? (·.name == formal.domainName)).map (·.domain)
            let compatible := match sourceDomain, destination.ty with
              | some (.range _ _), .int => true
              | some (.enumeration members), .enum variants => members == variants
              | _, _ => false
            unless compatible do
              throwErrorAt token "state assignment has incompatible type"
    let application : SurfaceStateApplication :=
      { name := stateAlias.name, token := stateAlias.token,
        args := stateAlias.args.map fun arg => ⟨arg.token⟩ }
    let _ ← trustedCheckedAliasGuardAtoms paramCtx boxCtx selected application families bindings
      domains functions partitions

private partial def rightFoldAnd (atoms : List (TSyntax `term)) (token : Syntax) :
    TermElabM (TSyntax `term) := do
  match atoms with
  | [] => throwErrorAt token "source pattern expands to no guard atoms"
  | [only] => pure only
  | head :: tail => `(TransitionRaw.and $head $(← rightFoldAnd tail token))

private def selectedSystemForTransition (boxCtx : SurfaceBox)
    (transitionDecl : SurfaceTransition) : TermElabM SurfaceSystem := do
  match transitionDecl.body with
  | .general onSystem _ _ _ _ => lookupSystem boxCtx onSystem
  | .reaction (some onSystem) _ _ _ _ => lookupSystem boxCtx onSystem
  | .reaction none stateAttr source _ destination =>
      return (← resolveReactionChoice boxCtx transitionDecl.name transitionDecl.token
        none stateAttr source destination).selected
  | .namedReaction onSystem _ _ _ _ => lookupSystem boxCtx onSystem
  | .relation onSystem _ _ => lookupSystem boxCtx onSystem

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

private structure PlannedTransitionInstances where
  source : SurfaceTransition
  instances : List (String × List SurfaceIndexBinding)

private structure PlannedBoxTransitionInstances where
  plans : List PlannedTransitionInstances

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

inductive RawExprTokenTree where
  | node (token : Syntax) (children : List (ModelCheckPathSegment × RawExprTokenTree))

structure LoweredRawExpr where
  term : TSyntax `term
  tokens : RawExprTokenTree

private def RawExprTokenTree.at (tree : RawExprTokenTree) : Syntax :=
  match tree with | .node token _ => token

private def RawExprTokenTree.child? (tree : RawExprTokenTree)
    (segment : ModelCheckPathSegment) : Option RawExprTokenTree :=
  match tree with
  | .node _ children => (children.find? (·.1 == segment)).map (·.2)

/- Unchecked raw lowering for final observation expressions.  This routine may
   select raw constructors and perform surface-only expansion, but it does not
   decide checker-owned names, sorts, enum membership, aggregate validity, or
   filter typing. -/
private partial def lowerRawExpr (tableName : String) (attrs : List SurfaceAttr)
    (paramCtx : List SurfaceParam) (inputCtx : List SurfaceInput) (stx : Syntax)
    (familyCtx : List SurfaceParamFamily := [])
    (bindingCtx : List SurfaceIndexBinding := [])
    (domainCtx : List SurfaceDomain := [])
    (functionCtx : List SurfaceExprFunction := [])
    (partitionCtx : List SurfacePartition := []) : TermElabM LoweredRawExpr := do
  let recur := fun expression => lowerRawExpr tableName attrs paramCtx inputCtx expression
    familyCtx bindingCtx domainCtx functionCtx partitionCtx
  let leaf (term : TSyntax `term) (token : Syntax := stx) :
      TermElabM LoweredRawExpr :=
    pure ⟨term, .node token []⟩
  let unary (constructorName : String) (inner : Syntax) :
      TermElabM LoweredRawExpr := do
    let lowered ← recur inner
    let term ← match constructorName with
      | "not" => `(TransitionRaw.not $(lowered.term))
      | _ => throwErrorAt stx "unsupported Sembla expression"
    pure ⟨term, .node stx [(.operand, lowered.tokens)]⟩
  let binary (constructorName : String) (lhs rhs : Syntax) :
      TermElabM LoweredRawExpr := do
    let left ← recur lhs
    let right ← recur rhs
    let term ← match constructorName with
      | "mul" => `(TransitionRaw.mul $(left.term) $(right.term))
      | "div" => `(TransitionRaw.div $(left.term) $(right.term))
      | "add" => `(TransitionRaw.add $(left.term) $(right.term))
      | "sub" => `(TransitionRaw.sub $(left.term) $(right.term))
      | "eq" => `(TransitionRaw.eq $(left.term) $(right.term))
      | "ne" => `(TransitionRaw.ne $(left.term) $(right.term))
      | "lt" => `(TransitionRaw.lt $(left.term) $(right.term))
      | "le" => `(TransitionRaw.le $(left.term) $(right.term))
      | "gt" => `(TransitionRaw.gt $(left.term) $(right.term))
      | "ge" => `(TransitionRaw.ge $(left.term) $(right.term))
      | "and" => `(TransitionRaw.and $(left.term) $(right.term))
      | _ => throwErrorAt stx "unsupported Sembla expression"
    pure ⟨term, .node stx [(.lhs, left.tokens), (.rhs, right.tokens)]⟩
  let resolveArguments (functionToken : Syntax) (arguments : Array (TSyntax `semblaExpr))
      (expectedDomains : List String) := do
    unless arguments.size == expectedDomains.length do
      throwErrorAt functionToken
        "function expects {expectedDomains.length} arguments"
    (arguments.toList.zip expectedDomains).mapM fun (argument, domainName) => do
      let domainDecl ← match domainCtx.find? (·.name == domainName) with
        | some found => pure found
        | none => throwErrorAt functionToken "unknown domain '{domainName}'"
      let member ← match argument with
        | `(semblaExpr| $value:num) =>
            pure (ParameterTable.IndexMember.int (value.raw.isNatLit?.getD 0))
        | `(semblaExpr| $value:ident) =>
            let valueName := identText value
            match bindingCtx.find? (·.name == valueName) with
            | some binding =>
                unless binding.domainName == domainName do
                  throwErrorAt value
                    "argument '{valueName}' has domain '{binding.domainName}'; expected '{domainName}'"
                pure binding.member
            | none => pure (.enum valueName)
        | _ => throwErrorAt argument "function arguments must be bound values or domain literals"
      unless domainDecl.domain.members.contains member do
        throwErrorAt argument "function argument is outside domain '{domainName}'"
      pure member
  match stx with
  | `(semblaExpr| ($inner:semblaExpr)) => recur inner
  | `(semblaExpr| $value:num) => leaf (← `(TransitionRaw.int $value))
  | `(semblaExpr| $value:scientific) =>
      validateScientific value false
      leaf (← `(TransitionRaw.real $value))
  | `(semblaExpr| true) => leaf (← `(TransitionRaw.bool true))
  | `(semblaExpr| false) => leaf (← `(TransitionRaw.bool false))
  | `(semblaExpr| ¬$inner:semblaExpr) => unary "not" inner
  | `(semblaExpr| $functionName:ident ($arguments:semblaExpr,*)) =>
      let sourceName := identText functionName
      match familyCtx.find? fun family => family.callNotation && family.sourceName == sourceName with
      | some family =>
          let members ← resolveArguments functionName.raw arguments.getElems family.domains
          let runtimeName ← match familyRuntimeName family.name members with
            | .ok value => pure value
            | .error message => throwErrorAt functionName message
          leaf (← `(TransitionRaw.parameter $(Lean.quote runtimeName))) functionName.raw
      | none =>
          let functionDecl ← match functionCtx.find? (·.name == sourceName) with
            | some found => pure found
            | none => throwErrorAt functionName "unknown mathematical function '{sourceName}'"
          let members ← resolveArguments functionName.raw arguments.getElems
            (functionDecl.args.map (·.domainName))
          let cell ← match functionDecl.cells.find? (fun cell => cell.key.map (·.1) == members) with
            | some found => pure found
            | none => throwErrorAt functionName "expression-function cell is not declared"
          let formalBindings := (functionDecl.args.zip members).map fun (arg, member) =>
            SurfaceIndexBinding.mk arg.name arg.token member arg.domainName .static none
          lowerRawExpr tableName attrs paramCtx inputCtx cell.expression familyCtx
            (formalBindings ++ bindingCtx) domainCtx functionCtx partitionCtx
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
      leaf (← `(TransitionRaw.parameter $(Lean.quote runtimeName))) familyName.raw
  | `(semblaExpr| parameter $name:ident) =>
      let authored := identText name
      let emitted := (paramCtx.find? (·.sourceName == authored)).map (·.name) |>.getD authored
      leaf (← `(TransitionRaw.parameter $(Lean.quote emitted))) name.raw
  | `(semblaExpr| $name:ident) =>
      let authored := identText name
      if authored == "true" then leaf (← `(TransitionRaw.bool true)) name.raw
      else if authored == "false" then leaf (← `(TransitionRaw.bool false)) name.raw
      else if let some binding := bindingCtx.find? (·.name == authored) then
        match binding.member with
        | .enum member => leaf (← `(TransitionRaw.enum $(Lean.quote member))) name.raw
        | .int member =>
            let memberTerm := Lean.quote member
            leaf (← `(TransitionRaw.int (Int.ofNat $memberTerm))) name.raw
      else
        match attrs.find? (·.name == authored), paramCtx.find? (·.sourceName == authored) with
        | some _, some _ => throwErrorAt name
            "ambiguous identifier '{authored}': both an attribute and parameter are in scope"
        | some column, none =>
            leaf (← `(TransitionRaw.selfAttribute $(Lean.quote column.name))) name.raw
        | none, some parameterDecl =>
            leaf (← `(TransitionRaw.parameter $(Lean.quote parameterDecl.name))) name.raw
        | none, none => leaf (← `(TransitionRaw.selfAttribute $(Lean.quote authored))) name.raw
  | `(semblaExpr| freq ($predicate:semblaExpr) over $key:ident) =>
      -- Nested aggregates are a current surface-shape restriction rather than
      -- a semantic name/sort decision.  Keep this syntax-only compatibility
      -- check while leaving key and predicate validity to the final checker.
      validateFrequencyPredicate predicate
      let lowered ← recur predicate
      let keyName := identText key
      let numerator ← `(TransitionRaw.relatedAggregate TransitionRaw.count
        $(Lean.quote tableName) $(Lean.quote keyName) $(Lean.quote keyName) $(lowered.term))
      let denominator ← `(TransitionRaw.relatedAggregate TransitionRaw.count
        $(Lean.quote tableName) $(Lean.quote keyName) $(Lean.quote keyName)
        (TransitionRaw.bool true))
      let keyTokens := [
        (.tableTarget, .node key.raw []),
        (.joinForeignAttribute, .node key.raw []),
        (.joinSelfAttribute, .node key.raw [])]
      let tokens := .node stx [
        (.lhs, .node stx (keyTokens ++ [(.aggregateFilter, lowered.tokens)])),
        (.rhs, .node stx keyTokens)]
      let term ← `(TransitionRaw.div $numerator $denominator)
      pure ⟨term, tokens⟩
  | `(semblaExpr| freq ($_predicate:semblaExpr) over)
  | `(semblaExpr| freq ($_predicate:semblaExpr)) =>
      throwErrorAt stx "frequency syntax requires a key: use 'freq (<predicate>) over <ref>'"
  | `(semblaExpr| freq $_lhs:ident = $_rhs:ident over $_key:ident)
  | `(semblaExpr| freq $_value:ident over $_key:ident) =>
      throwErrorAt stx
        "frequency syntax requires parentheses around the predicate: use 'freq (<predicate>) over <ref>'"
  | `(semblaExpr| countBy $fk:ident ($filter:semblaExpr)) =>
      let lowered ← recur filter
      let keyName := identText fk
      let term ← `(TransitionRaw.relatedAggregate TransitionRaw.count $(Lean.quote tableName)
        $(Lean.quote keyName) $(Lean.quote keyName) $(lowered.term))
      pure ⟨term, .node stx [(.tableTarget, .node fk.raw []),
        (.joinForeignAttribute, .node fk.raw []), (.joinSelfAttribute, .node fk.raw []),
        (.aggregateFilter, lowered.tokens)]⟩
  | `(semblaExpr| sizeBy $fk:ident) =>
      let keyName := identText fk
      let term ← `(TransitionRaw.relatedAggregate TransitionRaw.count $(Lean.quote tableName)
        $(Lean.quote keyName) $(Lean.quote keyName) (TransitionRaw.bool true))
      pure ⟨term, .node stx [(.tableTarget, .node fk.raw []),
        (.joinForeignAttribute, .node fk.raw []), (.joinSelfAttribute, .node fk.raw [])]⟩
  | `(semblaExpr| inputSum $port:ident field $column:ident) =>
      let portName := identText port
      let fieldName := identText column
      let term ← `(TransitionRaw.input $(Lean.quote portName)
        (TransitionRaw.aggregate
          (TransitionRaw.sum (TransitionRaw.selfAttribute $(Lean.quote fieldName))) none))
      pure ⟨term, .node stx [(.inputPort, .node port.raw []),
        (.aggregateValue, .node column.raw [])]⟩
  | `(semblaExpr| $lhs:semblaExpr * $rhs:semblaExpr) => binary "mul" lhs rhs
  | `(semblaExpr| $lhs:semblaExpr · $rhs:semblaExpr) => binary "mul" lhs rhs
  | `(semblaExpr| $lhs:semblaExpr / $rhs:semblaExpr) => binary "div" lhs rhs
  | `(semblaExpr| $lhs:semblaExpr + $rhs:semblaExpr) => binary "add" lhs rhs
  | `(semblaExpr| $lhs:semblaExpr - $rhs:semblaExpr) => binary "sub" lhs rhs
  | `(semblaExpr| $lhs:semblaExpr = $rhs:semblaExpr) =>
      match lhs, rhs with
      | `(semblaExpr| $attrName:ident), `(semblaExpr| $variant:ident) =>
          match attrs.find? (·.name == identText attrName) with
          | some { ty := .enum _, .. } =>
              if paramCtx.any (·.sourceName == identText attrName) then
                throwErrorAt attrName
                  "ambiguous identifier '{identText attrName}': both an attribute and parameter are in scope"
              let variantName := match bindingCtx.find? (·.name == identText variant) with
                | some { member := .enum value, .. } => value
                | _ => identText variant
              let term ← `(TransitionRaw.enumIs $(Lean.quote (identText attrName))
                $(Lean.quote variantName))
              pure ⟨term, .node stx [(.lhs, .node attrName.raw []), (.rhs, .node variant.raw [])]⟩
          | _ => binary "eq" lhs rhs
      | _, _ => binary "eq" lhs rhs
  | `(semblaExpr| $lhs:semblaExpr ≠ $rhs:semblaExpr) =>
      match lhs, rhs with
      | `(semblaExpr| $attrName:ident), `(semblaExpr| $variant:ident) =>
          match attrs.find? (·.name == identText attrName) with
          | some { ty := .enum _, .. } =>
              if paramCtx.any (·.sourceName == identText attrName) then
                throwErrorAt attrName
                  "ambiguous identifier '{identText attrName}': both an attribute and parameter are in scope"
              let variantName := match bindingCtx.find? (·.name == identText variant) with
                | some { member := .enum value, .. } => value
                | _ => identText variant
              let term ← `(TransitionRaw.ne
                (TransitionRaw.selfAttribute $(Lean.quote (identText attrName)))
                (TransitionRaw.enum $(Lean.quote variantName)))
              pure ⟨term, .node stx [(.lhs, .node attrName.raw []),
                (.rhs, .node variant.raw [])]⟩
          | _ => binary "ne" lhs rhs
      | _, _ => binary "ne" lhs rhs
  | `(semblaExpr| $lhs:semblaExpr < $rhs:semblaExpr) => binary "lt" lhs rhs
  | `(semblaExpr| $lhs:semblaExpr ≤ $rhs:semblaExpr) => binary "le" lhs rhs
  | `(semblaExpr| $lhs:semblaExpr > $rhs:semblaExpr) => binary "gt" lhs rhs
  | `(semblaExpr| $lhs:semblaExpr ≥ $rhs:semblaExpr) => binary "ge" lhs rhs
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
      let targetName := Lean.quote partition.target.attrName
      let attributeTerm ← `(TransitionRaw.selfAttribute $targetName)
      let lowerTerm ← `(TransitionRaw.int (Int.ofNat $lowerNat))
      let ge ← `(TransitionRaw.ge $attributeTerm $lowerTerm)
      match member.upper with
      | none => pure ⟨ge, .node stx []⟩
      | some upper =>
          let upperNat := Lean.quote upper
          let term ← `(TransitionRaw.and $ge
            (TransitionRaw.lt (TransitionRaw.selfAttribute $targetName)
              (TransitionRaw.int (Int.ofNat $upperNat))))
          pure ⟨term, .node stx []⟩
  | `(semblaExpr| $lhs:semblaExpr && $rhs:semblaExpr) => binary "and" lhs rhs
  | `(semblaExpr| $lhs:semblaExpr ∧ $rhs:semblaExpr) => binary "and" lhs rhs
  | _ => throwErrorAt stx "unsupported Sembla expression"

/- Alias applications use the same unchecked raw-expression encoder as the
observation/transition surface. Schema inspection below selects only the legacy
raw enum encoding; all destination, membership, sort, Ref, and nested-expression
validity is owned by the authoritative transition adapter/checker. -/
private def lowerEmittedAliasGuardAtoms (paramCtx : List SurfaceParam)
    (boxCtx : SurfaceBox) (selected : SurfaceSystem)
    (application : SurfaceStateApplication) (families : List SurfaceParamFamily)
    (bindings : List SurfaceIndexBinding) (domains : List SurfaceDomain)
    (functions : List SurfaceExprFunction) (partitions : List SurfacePartition)
    (sourceAtomOffset : Nat := 0) : TermElabM LoweredAliasGuardAtoms := do
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
    return ⟨[← partitionGuardTerm boxCtx selected partition binding], []⟩
  let (aliasId, stateAlias) ← lookupStateAliasWithId boxCtx application
  unless identText stateAlias.system == selected.logicalName do
    throwErrorAt application.token
      "state alias '{stateAlias.name}' selects system '{identText stateAlias.system}', not '{selected.logicalName}'"
  let localBindings ← applicationBindings domains bindings stateAlias.args application
  let allBindings := localBindings ++ bindings
  let mut terms : List (TSyntax `term) := []
  let mut provenance : List AliasGuardAtomProvenance := []
  for atom in stateAlias.atoms do
    let ordinal := sourceAtomOffset + terms.length
    match atom with
    | .assignment attrName value token => do
        let attrNameText := identText attrName
        let destinationTy? := (selected.attrs.find? (·.name == attrNameText)).map (·.ty)
        let destinationIsEnum := match destinationTy? with
          | some (.enum _) => true
          | _ => false
        let lowered ← if destinationIsEnum then
          match value with
          | `(semblaExpr| $identifier:ident) =>
              let authored := identText identifier
              let concrete := match allBindings.find? (·.name == authored) with
                | some { member := .enum member, .. } => member
                | _ => authored
              pure (← `(TransitionRaw.enumIs $(Lean.quote attrNameText) $(Lean.quote concrete)))
          | _ =>
              let raw ← lowerRawExpr selected.irName selected.attrs paramCtx boxCtx.inputs
                value (familyCtx := families) (bindingCtx := allBindings)
                (domainCtx := domains) (functionCtx := functions)
                (partitionCtx := partitions)
              `(TransitionRaw.eq
                (TransitionRaw.selfAttribute $(Lean.quote attrNameText)) $(raw.term))
        else
              let loweredValue ← match value with
                | `(semblaExpr| $identifier:ident) =>
                    match allBindings.find? (·.name == identText identifier) with
                    | some { member := .int member, .. } =>
                        let memberTerm := Lean.quote member
                        pure (← `(TransitionRaw.int (Int.ofNat $memberTerm)))
                    | _ =>
                        let raw ← lowerRawExpr selected.irName selected.attrs paramCtx
                          boxCtx.inputs value (familyCtx := families)
                          (bindingCtx := allBindings) (domainCtx := domains)
                          (functionCtx := functions) (partitionCtx := partitions)
                        pure raw.term
                | _ =>
                    let raw ← lowerRawExpr selected.irName selected.attrs paramCtx boxCtx.inputs
                      value (familyCtx := families) (bindingCtx := allBindings)
                      (domainCtx := domains) (functionCtx := functions)
                      (partitionCtx := partitions)
                    pure raw.term
              `(TransitionRaw.eq
                (TransitionRaw.selfAttribute $(Lean.quote attrNameText)) $loweredValue)
        terms := terms ++ [lowered]
        provenance := provenance ++ [⟨aliasId, application.token, ordinal, token,
          some attrName.raw, value.raw⟩]
    | .predicate expression token => do
        let raw ← lowerRawExpr selected.irName selected.attrs paramCtx boxCtx.inputs expression
          (familyCtx := families) (bindingCtx := allBindings) (domainCtx := domains)
          (functionCtx := functions) (partitionCtx := partitions)
        terms := terms ++ [raw.term]
        provenance := provenance ++ [⟨aliasId, application.token, ordinal, token,
          none, expression.raw⟩]
  pure ⟨terms, provenance⟩

private def lowerEmittedAliasEffects (paramCtx : List SurfaceParam)
    (boxCtx : SurfaceBox) (selected : SurfaceSystem)
    (application : SurfaceStateApplication) (families : List SurfaceParamFamily)
    (bindings : List SurfaceIndexBinding) (domains : List SurfaceDomain)
    (functions : List SurfaceExprFunction) (partitions : List SurfacePartition)
    (effectOffset : Nat := 0) : TermElabM LoweredAliasEffects := do
  if partitions.any (·.name == application.name) then
    throwErrorAt application.token "projected partitions cannot appear after become"
  let (aliasId, stateAlias) ← lookupStateAliasWithId boxCtx application
  unless identText stateAlias.system == selected.logicalName do
    throwErrorAt application.token "state alias selects an incompatible system"
  let localBindings ← applicationBindings domains bindings stateAlias.args application
  let allBindings := localBindings ++ bindings
  let mut effects : List (TSyntax `term) := []
  let mut provenance : List AliasEffectProvenance := []
  for atom in stateAlias.atoms do
    match atom with
    | .predicate _ token =>
        throwErrorAt token "state alias containing match cannot be used after become"
    | .assignment attrName value token => do
        rejectAggregates "effect expressions" value
        let attrNameText := identText attrName
        let isEnum := (selected.attrs.find? (·.name == attrNameText)).any fun column =>
          match column.ty with | .enum _ => true | _ => false
        let valueTerm ← if isEnum then
          match value with
          | `(semblaExpr| $identifier:ident) =>
              let authored := identText identifier
              let concrete := match allBindings.find? (·.name == authored) with
                | some { member := .enum member, .. } => member
                | _ => authored
              `(TransitionRaw.enum $(Lean.quote concrete))
          | _ => do
              let raw ← lowerRawExpr selected.irName selected.attrs paramCtx boxCtx.inputs value
                (familyCtx := families) (bindingCtx := allBindings) (domainCtx := domains)
                (functionCtx := functions) (partitionCtx := partitions)
              pure raw.term
        else do
          let raw ← lowerRawExpr selected.irName selected.attrs paramCtx boxCtx.inputs value
            (familyCtx := families) (bindingCtx := allBindings) (domainCtx := domains)
            (functionCtx := functions) (partitionCtx := partitions)
          pure raw.term
        let term ← `(TransitionRaw.setAttribute $(Lean.quote attrNameText) $valueTerm)
        let ordinal := effectOffset + effects.length
        effects := effects ++ [term]
        provenance := provenance ++ [⟨aliasId, application.token, ordinal, token,
          attrName.raw, value.raw⟩]
  pure ⟨effects, provenance⟩

structure ExplicitEffectSidecar where
  destination : Syntax
  value : RawExprTokenTree

structure ExplicitClaimSidecar where
  resource : Syntax

structure EmittedExplicitEffectSidecar where
  ordinal : Nat
  detail : ExplicitEffectSidecar

/-- Token/provenance data accumulated by the same unchecked lowering pass that
emits a named-reaction or relation transition. -/
structure LegacyTransitionDetail where
  target : Syntax
  hazardTokens : RawExprTokenTree
  explicitEffects : List EmittedExplicitEffectSidecar
  claims : List ExplicitClaimSidecar
  aliasProvenance : EmittedAliasProvenance

structure ExplicitTransitionDetail where
  target : Syntax
  guardTokens : RawExprTokenTree
  hazardTokens : RawExprTokenTree
  effects : List ExplicitEffectSidecar
  claims : List ExplicitClaimSidecar

structure ExplicitReactionDetail where
  target : Syntax
  stateAttribute : Syntax
  source : Syntax
  destination : Syntax
  hazardTokens : RawExprTokenTree

structure InferredReactionDetail where
  targetToken : Syntax
  stateAttributeToken : Option Syntax
  resolvedStateAttributeName : String
  source : Syntax
  destination : Syntax
  hazardTokens : RawExprTokenTree
  resolvedTableName : String

structure GeneratedBindingProvenance where
  binderToken : Syntax
  matchedAttributeToken : Syntax
  memberToken : Syntax

structure GeneratedGeneralDetail where
  emittedName : String
  target : Syntax
  guardTokens : RawExprTokenTree
  hazardTokens : RawExprTokenTree
  effects : List ExplicitEffectSidecar
  claims : List ExplicitClaimSidecar
  bindings : List GeneratedBindingProvenance

structure GeneratedReactionDetail where
  emittedName : String
  target : Syntax
  stateAttribute : Syntax
  source : Syntax
  destination : Syntax
  guardTokens : RawExprTokenTree
  hazardTokens : RawExprTokenTree
  bindings : List GeneratedBindingProvenance

private structure LoweredIndexedGeneralTransition where
  term : TSyntax `term
  detail : GeneratedGeneralDetail

private structure LoweredIndexedReactionTransition where
  term : TSyntax `term
  detail : GeneratedReactionDetail

inductive EmittedTransitionSidecar where
  | explicit (source : SurfaceTransition) (detail : ExplicitTransitionDetail)
  | indexedGeneral (source : SurfaceTransition) (detail : GeneratedGeneralDetail)
  | explicitReaction (source : SurfaceTransition) (detail : ExplicitReactionDetail)
  | inferredReaction (source : SurfaceTransition) (detail : InferredReactionDetail)
  | indexedReaction (source : SurfaceTransition) (detail : GeneratedReactionDetail)
  | legacy (source : SurfaceTransition) (detail : LegacyTransitionDetail)

private def lowerExplicitEffect (tableName : String) (attrs : List SurfaceAttr)
    (paramCtx : List SurfaceParam) (inputCtx : List SurfaceInput)
    (families : List SurfaceParamFamily) (domains : List SurfaceDomain)
    (functions : List SurfaceExprFunction) (partitions : List SurfacePartition)
    (assignment : TSyntax `semblaSet) :
    TermElabM (TSyntax `term × ExplicitEffectSidecar) := do
  match assignment with
  | `(semblaSet| $attrName:ident := $value:semblaExpr) =>
      rejectEffectAggregates value
      let destination? := attrs.find? (·.name == identText attrName)
      let lowered ← match destination? with
        | some { ty := .enum _, .. } =>
            match value with
            | `(semblaExpr| $variant:ident) => do
                let term ← `(TransitionRaw.enum $(Lean.quote (identText variant)))
                pure (LoweredRawExpr.mk term (.node variant.raw []))
            | _ => throwErrorAt value "enum effect values must be variant literals"
        | _ => (lowerRawExpr tableName attrs paramCtx inputCtx value
            (familyCtx := families) (bindingCtx := []) (domainCtx := domains)
            (functionCtx := functions) (partitionCtx := partitions))
      let term ← `(TransitionRaw.setAttribute $(Lean.quote (identText attrName)) $(lowered.term))
      pure (term, ⟨attrName.raw, lowered.tokens⟩)
  | _ => throwUnsupportedSyntax

private def lowerExplicitGeneralTransition (paramCtx : List SurfaceParam)
    (boxCtx : SurfaceBox) (families : List SurfaceParamFamily)
    (transitionDecl : SurfaceTransition)
    (domains : List SurfaceDomain) (functions : List SurfaceExprFunction)
    (partitions : List SurfacePartition)
    (onSystem : TSyntax `ident) (guardExpr hazardExpr : TSyntax `semblaExpr)
    (contests : List SurfaceContest) (assignments : List (TSyntax `semblaSet)) :
    TermElabM (TSyntax `term × EmittedTransitionSidecar) := do
  let authoredTarget := identText onSystem
  let selected? := boxCtx.systems.find? (·.logicalName == authoredTarget)
  let tableName := selected?.map (·.irName) |>.getD authoredTarget
  let attrs := selected?.map (·.attrs) |>.getD []
  let loweredGuard ← lowerRawExpr tableName attrs paramCtx boxCtx.inputs guardExpr
    (familyCtx := families) (bindingCtx := []) (domainCtx := domains)
    (functionCtx := functions) (partitionCtx := partitions)
  let loweredHazard ← lowerRawExpr tableName attrs paramCtx boxCtx.inputs hazardExpr
    (familyCtx := families) (bindingCtx := []) (domainCtx := domains)
    (functionCtx := functions) (partitionCtx := partitions)
  let mut effectTerms : Array (TSyntax `term) := #[]
  let mut effectSidecars : List ExplicitEffectSidecar := []
  for assignment in assignments do
    let lowered ← lowerExplicitEffect tableName attrs paramCtx boxCtx.inputs
      families domains functions partitions assignment
    effectTerms := effectTerms.push lowered.1
    effectSidecars := effectSidecars ++ [lowered.2]
  let mut claimTerms : Array (TSyntax `term) := #[]
  let mut claimSidecars : List ExplicitClaimSidecar := []
  for claimDecl in contests do
    claimTerms := claimTerms.push (← `(TransitionRaw.raceClaim
      (TransitionRaw.selfAttribute $(Lean.quote (identText claimDecl.resource)))))
    claimSidecars := claimSidecars ++ [⟨claimDecl.resource.raw⟩]
  let term ← `(TransitionRaw.transition $(Lean.quote transitionDecl.name)
    $(Lean.quote tableName) $(loweredGuard.term) $(loweredHazard.term)
    [$effectTerms,*] [$claimTerms,*])
  let detail : ExplicitTransitionDetail :=
    ⟨onSystem.raw, loweredGuard.tokens, loweredHazard.tokens,
      effectSidecars, claimSidecars⟩
  pure (term, .explicit transitionDecl detail)

private def lowerExplicitReactionTransition (paramCtx : List SurfaceParam)
    (boxCtx : SurfaceBox) (families : List SurfaceParamFamily)
    (transitionDecl : SurfaceTransition)
    (domains : List SurfaceDomain) (functions : List SurfaceExprFunction)
    (partitions : List SurfacePartition)
    (onSystem stateAttr source : TSyntax `ident) (hazardExpr : TSyntax `semblaExpr)
    (destination : TSyntax `ident) :
    TermElabM (TSyntax `term × EmittedTransitionSidecar) := do
  let authoredTarget := identText onSystem
  let selected? := boxCtx.systems.find? (·.logicalName == authoredTarget)
  let tableName := selected?.map (·.irName) |>.getD authoredTarget
  let attrs := selected?.map (·.attrs) |>.getD []
  let guardTerm ← `(TransitionRaw.enumIs $(Lean.quote (identText stateAttr))
    $(Lean.quote (identText source)))
  let loweredHazard ← lowerRawExpr tableName attrs paramCtx boxCtx.inputs hazardExpr
    (familyCtx := families) (bindingCtx := []) (domainCtx := domains)
    (functionCtx := functions) (partitionCtx := partitions)
  let destinationEffect ← `(TransitionRaw.setAttribute $(Lean.quote (identText stateAttr))
    (TransitionRaw.enum $(Lean.quote (identText destination))))
  let term ← `(TransitionRaw.transition $(Lean.quote transitionDecl.name)
    $(Lean.quote tableName) $guardTerm $(loweredHazard.term) [$destinationEffect] [])
  let detail : ExplicitReactionDetail :=
    ⟨onSystem.raw, stateAttr.raw, source.raw, destination.raw, loweredHazard.tokens⟩
  pure (term, .explicitReaction transitionDecl detail)

private def lowerInferredReactionTransition (paramCtx : List SurfaceParam)
    (boxCtx : SurfaceBox) (families : List SurfaceParamFamily)
    (transitionDecl : SurfaceTransition)
    (domains : List SurfaceDomain) (functions : List SurfaceExprFunction)
    (partitions : List SurfacePartition)
    (onSystem : Option (TSyntax `ident)) (stateAttr : Option (TSyntax `ident))
    (source : TSyntax `ident) (hazardExpr : TSyntax `semblaExpr)
    (destination : TSyntax `ident) :
    TermElabM (TSyntax `term × EmittedTransitionSidecar) := do
  let choice ← resolveReactionChoice boxCtx transitionDecl.name transitionDecl.token
    onSystem stateAttr source destination
  let guardTerm ← `(TransitionRaw.enumIs $(Lean.quote choice.stateAttr.name)
    $(Lean.quote choice.source))
  let loweredHazard ← lowerRawExpr choice.selected.irName choice.selected.attrs paramCtx
    boxCtx.inputs hazardExpr (familyCtx := families) (bindingCtx := [])
    (domainCtx := domains) (functionCtx := functions) (partitionCtx := partitions)
  let destinationEffect ← `(TransitionRaw.setAttribute $(Lean.quote choice.stateAttr.name)
    (TransitionRaw.enum $(Lean.quote choice.destination)))
  let term ← `(TransitionRaw.transition $(Lean.quote transitionDecl.name)
    $(Lean.quote choice.selected.irName) $guardTerm $(loweredHazard.term)
    [$destinationEffect] [])
  let detail : InferredReactionDetail :=
    ⟨onSystem.map (·.raw) |>.getD transitionDecl.token, stateAttr.map (·.raw),
      choice.stateAttr.name, source.raw, destination.raw, loweredHazard.tokens,
      choice.selected.irName⟩
  pure (term, .inferredReaction transitionDecl detail)

private def lowerIndexedGeneralEffect (tableName : String) (attrs : List SurfaceAttr)
    (paramCtx : List SurfaceParam) (inputCtx : List SurfaceInput)
    (families : List SurfaceParamFamily) (bindings : List SurfaceIndexBinding)
    (domains : List SurfaceDomain) (functions : List SurfaceExprFunction)
    (partitions : List SurfacePartition) (assignment : TSyntax `semblaSet) :
    TermElabM (TSyntax `term × ExplicitEffectSidecar) := do
  match assignment with
  | `(semblaSet| $attrName:ident := $value:semblaExpr) =>
      rejectEffectAggregates value
      let lowered ← match attrs.find? (·.name == identText attrName) with
        | some { ty := .enum _, .. } =>
            match value with
            | `(semblaExpr| $variant:ident) =>
                let variantName := match bindings.find? (·.name == identText variant) with
                  | some { member := .enum member, .. } => member
                  | _ => identText variant
                pure (LoweredRawExpr.mk
                  (← `(TransitionRaw.enum $(Lean.quote variantName))) (.node variant.raw []))
            | _ => throwErrorAt value "enum effect values must be variant literals"
        | _ => (lowerRawExpr tableName attrs paramCtx inputCtx value
            (familyCtx := families) (bindingCtx := bindings) (domainCtx := domains)
            (functionCtx := functions) (partitionCtx := partitions))
      let term ← `(TransitionRaw.setAttribute $(Lean.quote (identText attrName)) $(lowered.term))
      pure (term, ⟨attrName.raw, lowered.tokens⟩)
  | _ => throwUnsupportedSyntax

private def generatedBindingProvenance (domains : List SurfaceDomain)
    (binding : SurfaceIndexBinding) : GeneratedBindingProvenance :=
  let memberToken := match binding.member with
    | .enum member =>
        (domains.find? (·.name == binding.domainName) >>= fun domain =>
          (domain.memberTokens.find? (·.1 == member)).map (·.2)).getD binding.token
    | .int _ => binding.token
  ⟨binding.token, binding.token, memberToken⟩

private def lowerIndexedGeneralTransition (paramCtx : List SurfaceParam)
    (boxCtx : SurfaceBox) (families : List SurfaceParamFamily)
    (transitionDecl : SurfaceTransition) (generatedName : String)
    (bindings : List SurfaceIndexBinding) (domains : List SurfaceDomain)
    (functions : List SurfaceExprFunction) (partitions : List SurfacePartition) :
    TermElabM LoweredIndexedGeneralTransition := do
  match transitionDecl.body with
  | .general onSystem guardExpr hazardExpr contests assignments =>
      -- Target resolution remains expansion-owned because matched binders project
      -- onto the selected system before the checker sees the raw transition.
      let selected ← lookupSystem boxCtx onSystem
      let loweredGuard ← lowerRawExpr selected.irName selected.attrs paramCtx boxCtx.inputs
        guardExpr (familyCtx := families) (bindingCtx := bindings) (domainCtx := domains)
        (functionCtx := functions) (partitionCtx := partitions)
      let loweredHazard ← lowerRawExpr selected.irName selected.attrs paramCtx boxCtx.inputs
        hazardExpr (familyCtx := families) (bindingCtx := bindings) (domainCtx := domains)
        (functionCtx := functions) (partitionCtx := partitions)
      let bindingDetails := bindings.map (generatedBindingProvenance domains)
      let mut guardTerm := loweredGuard.term
      let mut guardTokens := loweredGuard.tokens
      for (binding, provenance) in bindings.zip bindingDetails do
        if binding.mode != .static then
          let attrName := binding.matchedAttribute.getD binding.name
          let indexGuard ← match binding.member with
            | .enum member =>
                `(TransitionRaw.enumIs $(Lean.quote attrName) $(Lean.quote member))
            | .int member =>
                let memberTerm := Lean.quote member
                `(TransitionRaw.eq (TransitionRaw.selfAttribute $(Lean.quote attrName))
                  (TransitionRaw.int (Int.ofNat $memberTerm)))
          guardTerm ← `(TransitionRaw.and $guardTerm $indexGuard)
          let indexGuardTokens := .node provenance.binderToken [
            (.lhs, .node provenance.matchedAttributeToken []),
            (.rhs, .node provenance.memberToken [])]
          guardTokens := .node provenance.binderToken [
            (.lhs, guardTokens), (.rhs, indexGuardTokens)]
      let mut effectTerms : Array (TSyntax `term) := #[]
      let mut effectSidecars : List ExplicitEffectSidecar := []
      for assignment in assignments do
        let lowered ← lowerIndexedGeneralEffect selected.irName selected.attrs paramCtx boxCtx.inputs
          families bindings domains functions partitions assignment
        effectTerms := effectTerms.push lowered.1
        effectSidecars := effectSidecars ++ [lowered.2]
      let mut claimTerms : Array (TSyntax `term) := #[]
      let mut claimSidecars : List ExplicitClaimSidecar := []
      for claim in contests do
        claimTerms := claimTerms.push (← `(TransitionRaw.raceClaim
          (TransitionRaw.selfAttribute $(Lean.quote (identText claim.resource)))))
        claimSidecars := claimSidecars ++ [⟨claim.resource.raw⟩]
      let term ← `(TransitionRaw.transition $(Lean.quote generatedName)
        $(Lean.quote selected.irName) $guardTerm $(loweredHazard.term)
        [$effectTerms,*] [$claimTerms,*])
      let detail : GeneratedGeneralDetail :=
        ⟨generatedName, onSystem.raw, guardTokens, loweredHazard.tokens,
          effectSidecars, claimSidecars, bindingDetails⟩
      pure ⟨term, detail⟩
  | _ => throwError "internal indexed-general lowering requested for a non-general transition"

private def lowerIndexedReactionTransition (paramCtx : List SurfaceParam)
    (boxCtx : SurfaceBox) (families : List SurfaceParamFamily)
    (transitionDecl : SurfaceTransition) (generatedName : String)
    (bindings : List SurfaceIndexBinding) (domains : List SurfaceDomain)
    (functions : List SurfaceExprFunction) (partitions : List SurfacePartition) :
    TermElabM LoweredIndexedReactionTransition := do
  match transitionDecl.body with
  | .reaction (some onSystem) (some stateAttr) source hazardExpr destination =>
      -- Indexed projection needs the selected system before raw checking, so
      -- target resolution remains expansion-owned as it is in `transitionInstances`.
      let selected ← lookupSystem boxCtx onSystem
      let authoredGuard ← `(TransitionRaw.enumIs $(Lean.quote (identText stateAttr))
        $(Lean.quote (identText source)))
      let loweredHazard ← lowerRawExpr selected.irName selected.attrs paramCtx boxCtx.inputs
        hazardExpr (familyCtx := families) (bindingCtx := bindings) (domainCtx := domains)
        (functionCtx := functions) (partitionCtx := partitions)
      let destinationEffect ← `(TransitionRaw.setAttribute $(Lean.quote (identText stateAttr))
        (TransitionRaw.enum $(Lean.quote (identText destination))))
      let bindingDetails := bindings.map (generatedBindingProvenance domains)
      let mut guardTerm := authoredGuard
      let mut guardTokens := RawExprTokenTree.node stateAttr.raw [
        (.lhs, .node stateAttr.raw []), (.rhs, .node source.raw [])]
      for (binding, provenance) in bindings.zip bindingDetails do
        if binding.mode != .static then
          let attrName := binding.matchedAttribute.getD binding.name
          let indexGuard ← match binding.member with
            | .enum member =>
                `(TransitionRaw.enumIs $(Lean.quote attrName) $(Lean.quote member))
            | .int member =>
                let memberTerm := Lean.quote member
                `(TransitionRaw.eq (TransitionRaw.selfAttribute $(Lean.quote attrName))
                  (TransitionRaw.int (Int.ofNat $memberTerm)))
          guardTerm ← `(TransitionRaw.and $guardTerm $indexGuard)
          let indexGuardTokens := RawExprTokenTree.node provenance.binderToken [
            (.lhs, .node provenance.matchedAttributeToken []),
            (.rhs, .node provenance.memberToken [])]
          guardTokens := .node provenance.binderToken [
            (.lhs, guardTokens), (.rhs, indexGuardTokens)]
      let term ← `(TransitionRaw.transition $(Lean.quote generatedName)
        $(Lean.quote selected.irName) $guardTerm $(loweredHazard.term)
        [$destinationEffect] [])
      let detail : GeneratedReactionDetail :=
        ⟨generatedName, onSystem.raw, stateAttr.raw, source.raw, destination.raw,
          guardTokens, loweredHazard.tokens, bindingDetails⟩
      pure ⟨term, detail⟩
  | _ => throwError
      "internal indexed-reaction lowering requested for a non-explicit reaction transition"

/-- Unchecked raw lowering for the named-reaction/relation surface. Expansion
shape and raw encoding remain trusted here; all emitted term semantics are owned
by `buildSurfaceTransition` and the final model checker. -/
private def lowerLegacyTransition (paramCtx : List SurfaceParam)
    (boxCtx : SurfaceBox) (families : List SurfaceParamFamily)
    (transitionDecl : SurfaceTransition) (generatedName : String)
    (bindings : List SurfaceIndexBinding) (domains : List SurfaceDomain)
    (functions : List SurfaceExprFunction) (partitions : List SurfacePartition) :
    TermElabM (TSyntax `term × LegacyTransitionDetail) := do
  let lowerGuard := fun selected application offset =>
    lowerEmittedAliasGuardAtoms paramCtx boxCtx selected application families bindings
      domains functions partitions offset
  let lowerEffects := fun selected application offset =>
    lowerEmittedAliasEffects paramCtx boxCtx selected application families bindings
      domains functions partitions offset
  match transitionDecl.body with
  | .namedReaction onSystem source _axes hazardExpr destination =>
      let selected ← lookupSystem boxCtx onSystem
      let sourceAtoms ← lowerGuard selected source 0
      let mut guardTerm ← rightFoldAnd sourceAtoms.terms source.token
      let effectList ← lowerEffects selected destination 0
      if effectList.terms.isEmpty then
        throwErrorAt destination.token "named-state destination expands to no effects"
      let loweredHazard ← lowerRawExpr selected.irName selected.attrs paramCtx boxCtx.inputs
        hazardExpr (familyCtx := families) (bindingCtx := bindings) (domainCtx := domains)
        (functionCtx := functions) (partitionCtx := partitions)
      for binding in bindings do
        if binding.mode != .static then
          let attrName := binding.matchedAttribute.getD binding.name
          let indexGuard ← match binding.member with
            | .enum value =>
                `(TransitionRaw.enumIs $(Lean.quote attrName) $(Lean.quote value))
            | .int value =>
                let valueTerm := Lean.quote value
                `(TransitionRaw.eq (TransitionRaw.selfAttribute $(Lean.quote attrName))
                  (TransitionRaw.int (Int.ofNat $valueTerm)))
          guardTerm ← `(TransitionRaw.and $guardTerm $indexGuard)
      let effectTerms := effectList.terms.toArray
      let term ← `(TransitionRaw.transition $(Lean.quote generatedName)
        $(Lean.quote selected.irName) $guardTerm $(loweredHazard.term)
        [$effectTerms,*] [])
      let aliasProvenance : EmittedAliasProvenance :=
        ⟨sourceAtoms.terms.length, effectList.terms.length,
          sourceAtoms.provenance, effectList.provenance⟩
      pure (term, ⟨onSystem.raw, loweredHazard.tokens, [], [], aliasProvenance⟩)
  | .relation onSystem _constraints items =>
      let selected ← lookupSystem boxCtx onSystem
      let mut sourceAtoms : List (TSyntax `term) := []
      let mut aliasGuardProvenance : List AliasGuardAtomProvenance := []
      let mut hazardSyntax : Option (TSyntax `semblaExpr) := none
      let mut effects : Array (TSyntax `term) := #[]
      let mut explicitEffects : List EmittedExplicitEffectSidecar := []
      let mut aliasEffectProvenance : List AliasEffectProvenance := []
      let mut contests : Array (TSyntax `term) := #[]
      let mut claims : List ExplicitClaimSidecar := []
      for item in items do
        match item with
        | .source applications _ =>
            for application in applications do
              let lowered ← lowerGuard selected application sourceAtoms.length
              sourceAtoms := sourceAtoms ++ lowered.terms
              aliasGuardProvenance := aliasGuardProvenance ++ lowered.provenance
        | .hazard expression _ => hazardSyntax := some expression
        | .claim claimDecl _ =>
            contests := contests.push (← `(TransitionRaw.raceClaim
              (TransitionRaw.selfAttribute $(Lean.quote (identText claimDecl.resource)))))
            claims := claims ++ [⟨claimDecl.resource.raw⟩]
        | .set assignment _ =>
            let lowered ← lowerIndexedGeneralEffect selected.irName selected.attrs paramCtx
              boxCtx.inputs families bindings domains functions partitions assignment
            explicitEffects := explicitEffects ++ [⟨effects.size, lowered.2⟩]
            effects := effects.push lowered.1
        | .become application _ =>
            let lowered ← lowerEffects selected application effects.size
            effects := effects ++ lowered.terms.toArray
            aliasEffectProvenance := aliasEffectProvenance ++ lowered.provenance
      if effects.isEmpty then
        throwErrorAt transitionDecl.token "relation requires at least one effect after expansion"
      let hazardExpr ← hazardSyntax.getDM
        (throwErrorAt transitionDecl.token "relation requires exactly one hazard")
      let loweredHazard ← lowerRawExpr selected.irName selected.attrs paramCtx boxCtx.inputs
        hazardExpr (familyCtx := families) (bindingCtx := bindings) (domainCtx := domains)
        (functionCtx := functions) (partitionCtx := partitions)
      let mut guardTerm ← rightFoldAnd sourceAtoms transitionDecl.token
      for binding in bindings do
        if binding.mode != .static then
          let attrName := binding.matchedAttribute.getD binding.name
          let indexGuard ← match binding.member with
            | .enum value =>
                `(TransitionRaw.enumIs $(Lean.quote attrName) $(Lean.quote value))
            | .int value =>
                let valueTerm := Lean.quote value
                `(TransitionRaw.eq (TransitionRaw.selfAttribute $(Lean.quote attrName))
                  (TransitionRaw.int (Int.ofNat $valueTerm)))
          guardTerm ← `(TransitionRaw.and $guardTerm $indexGuard)
      let term ← `(TransitionRaw.transition $(Lean.quote generatedName)
        $(Lean.quote selected.irName) $guardTerm $(loweredHazard.term)
        [$effects,*] [$contests,*])
      let aliasProvenance : EmittedAliasProvenance :=
        ⟨sourceAtoms.length, effects.size, aliasGuardProvenance, aliasEffectProvenance⟩
      pure (term, ⟨onSystem.raw, loweredHazard.tokens, explicitEffects, claims,
        aliasProvenance⟩)
  | _ => throwError "internal legacy lowering requested for a non-legacy transition"

private def verifySingletonTransitionPlan (planned : PlannedTransitionInstances) :
    TermElabM Unit := do
  match planned.instances with
  | [(name, bindings)] =>
      unless name == planned.source.name && bindings.isEmpty do
        throwError "internal singleton transition plan mismatch"
  | _ => throwError "internal singleton transition plan mismatch"

private def transitionTerms (families : List SurfaceParamFamily)
    (paramCtx : List SurfaceParam) (boxCtx : SurfaceBox)
    (planned : PlannedTransitionInstances) (domains : List SurfaceDomain)
    (functions : List SurfaceExprFunction) (partitions : List SurfacePartition) :
    TermElabM (Array (TSyntax `term) × Array EmittedTransitionSidecar) := do
  let transitionDecl := planned.source
  match transitionDecl.binders.isEmpty, transitionDecl.body with
  | true, .general onSystem guardExpr hazardExpr contests assignments =>
      verifySingletonTransitionPlan planned
      let lowered ← lowerExplicitGeneralTransition paramCtx boxCtx families transitionDecl
        domains functions partitions onSystem guardExpr hazardExpr contests assignments
      pure (#[lowered.1], #[lowered.2])
  | false, .general .. =>
      let lowered ← planned.instances.mapM fun (name, bindings) => do
        let result ← lowerIndexedGeneralTransition paramCtx boxCtx families transitionDecl
          name bindings domains functions partitions
        pure (result.term,
          EmittedTransitionSidecar.indexedGeneral transitionDecl result.detail)
      pure (lowered.toArray.map (·.1), lowered.toArray.map (·.2))
  | true, .reaction (some onSystem) (some stateAttr) source hazardExpr destination =>
      verifySingletonTransitionPlan planned
      let lowered ← lowerExplicitReactionTransition paramCtx boxCtx families transitionDecl
        domains functions partitions onSystem stateAttr source hazardExpr destination
      pure (#[lowered.1], #[lowered.2])
  | true, .reaction none none source hazardExpr destination =>
      verifySingletonTransitionPlan planned
      let lowered ← lowerInferredReactionTransition paramCtx boxCtx families transitionDecl
        domains functions partitions none none source hazardExpr destination
      pure (#[lowered.1], #[lowered.2])
  | true, .reaction (some onSystem) none source hazardExpr destination =>
      verifySingletonTransitionPlan planned
      let lowered ← lowerInferredReactionTransition paramCtx boxCtx families transitionDecl
        domains functions partitions (some onSystem) none source hazardExpr destination
      pure (#[lowered.1], #[lowered.2])
  | true, .reaction none (some stateAttr) source hazardExpr destination =>
      verifySingletonTransitionPlan planned
      let lowered ← lowerInferredReactionTransition paramCtx boxCtx families transitionDecl
        domains functions partitions none (some stateAttr) source hazardExpr destination
      pure (#[lowered.1], #[lowered.2])
  | false, .reaction (some _) (some _) _ _ _ =>
      let lowered ← planned.instances.mapM fun (name, bindings) => do
        let result ← lowerIndexedReactionTransition paramCtx boxCtx families transitionDecl
          name bindings domains functions partitions
        pure (result.term,
          EmittedTransitionSidecar.indexedReaction transitionDecl result.detail)
      pure (lowered.toArray.map (·.1), lowered.toArray.map (·.2))
  | _, .namedReaction .. | _, .relation .. =>
      let lowered ← planned.instances.mapM fun (name, bindings) => do
        let result ← lowerLegacyTransition paramCtx boxCtx families transitionDecl name bindings
          domains functions partitions
        pure (result.1, EmittedTransitionSidecar.legacy transitionDecl result.2)
      pure (lowered.toArray.map (·.1), lowered.toArray.map (·.2))
  | _, _ => throwError "internal transition lowering did not classify the surface body"

structure OutputFieldSidecar where
  source : SurfaceOutputField
  value : Option RawExprTokenTree
  filter : Option RawExprTokenTree

structure OutputSidecar where
  source : SurfaceOutput
  authoredFields : List OutputFieldSidecar

structure ViewSidecar where
  source : SurfaceView
  filter : Option RawExprTokenTree
  value : Option RawExprTokenTree

structure GroupedViewSidecar where
  source : SurfaceGroupedView
  filter : Option RawExprTokenTree

private def EmittedTransitionSidecar.aliasProvenance : EmittedTransitionSidecar →
    EmittedAliasProvenance
  | .legacy _ detail => detail.aliasProvenance
  | _ => EmittedAliasProvenance.empty

private def validateAndCollectUsedAliasIds (boxCtx : SurfaceBox)
    (sidecars : List EmittedTransitionSidecar) : TermElabM (List AliasDeclId) := do
  let mut contributed : List AliasDeclId := []
  for sidecar in sidecars do
    let provenance := sidecar.aliasProvenance
    for atom in provenance.guardAtoms do
      unless atom.aliasId.ordinal < boxCtx.aliases.length do
        throwError "internal guard alias identity is out of range"
      unless atom.emittedGuardAtomOrdinal < provenance.totalSourceAtomCount do
        throwError "internal alias guard provenance ordinal is out of range"
      if !contributed.contains atom.aliasId then contributed := atom.aliasId :: contributed
    for effect in provenance.effects do
      unless effect.aliasId.ordinal < boxCtx.aliases.length do
        throwError "internal effect alias identity is out of range"
      unless effect.emittedEffectOrdinal < provenance.totalEffectCount do
        throwError "internal alias effect provenance ordinal is out of range"
      if !contributed.contains effect.aliasId then contributed := effect.aliasId :: contributed
  pure ((List.range boxCtx.aliases.length).filterMap fun ordinal =>
    let aliasId : AliasDeclId := ⟨ordinal⟩
    if contributed.contains aliasId then some aliasId else none)

structure BoxObservationSidecar where
  transitionSidecars : List EmittedTransitionSidecar
  usedAliasIds : List AliasDeclId
  outputSidecars : List OutputSidecar
  viewSidecars : List ViewSidecar
  groupedViewSidecars : List GroupedViewSidecar

structure ObservationSidecars where
  boxSidecars : List BoxObservationSidecar
  summarySidecars : List SurfaceSummary

private def outputTerm (paramCtx : List SurfaceParam) (boxCtx : SurfaceBox)
    (familyCtx : List SurfaceParamFamily) (bindingCtx : List SurfaceIndexBinding)
    (domainCtx : List SurfaceDomain) (functionCtx : List SurfaceExprFunction)
    (partitionCtx : List SurfacePartition) (outputDecl : SurfaceOutput) :
    TermElabM (TSyntax `term × OutputSidecar) := do
  let systemName := identText outputDecl.system
  let selected? := boxCtx.systems.find? (·.logicalName == systemName)
  let tableName := selected?.map (·.irName) |>.getD systemName
  let attrs := selected?.map (·.attrs) |>.getD []
  let mut fieldTerms : Array (TSyntax `term) := #[]
  let mut sidecarFields : List OutputFieldSidecar := []
  -- Preserve authored order here. `lowerSurfaceOutputFields` alone owns exact
  -- coverage and schema-order projection.
  for outputField in outputDecl.fields do
    match outputField.op, outputField.filter, outputField.value with
    | "count", some filterExpr, none =>
        let lowered ← lowerRawExpr tableName attrs paramCtx boxCtx.inputs filterExpr
          familyCtx bindingCtx domainCtx functionCtx partitionCtx
        fieldTerms := fieldTerms.push (← `(SurfaceOutputFieldSpec.mk
          $(Lean.quote outputField.name) TransitionRaw.count (some $(lowered.term))))
        let sidecar : OutputFieldSidecar := ⟨outputField, none, some lowered.tokens⟩
        sidecarFields := sidecarFields ++ [sidecar]
    | "sum", none, some valueExpr =>
        let lowered ← lowerRawExpr tableName attrs paramCtx boxCtx.inputs valueExpr
          familyCtx bindingCtx domainCtx functionCtx partitionCtx
        fieldTerms := fieldTerms.push (← `(SurfaceOutputFieldSpec.mk
          $(Lean.quote outputField.name) (TransitionRaw.sum $(lowered.term)) none))
        let sidecar : OutputFieldSidecar := ⟨outputField, some lowered.tokens, none⟩
        sidecarFields := sidecarFields ++ [sidecar]
    | _, _, _ => throwErrorAt outputField.token "invalid output builder"
  let schemaTerms ← outputDecl.schema.toArray.mapM (attrTerm boxCtx)
  let term ← `(SurfaceOutputSpec.mk $(Lean.quote outputDecl.name) [$schemaTerms,*]
    $(Lean.quote tableName) [$fieldTerms,*])
  pure (term, OutputSidecar.mk outputDecl sidecarFields)

private def viewTerm (paramCtx : List SurfaceParam) (boxCtx : SurfaceBox)
    (familyCtx : List SurfaceParamFamily) (bindingCtx : List SurfaceIndexBinding)
    (domainCtx : List SurfaceDomain) (functionCtx : List SurfaceExprFunction)
    (partitionCtx : List SurfacePartition) (viewDecl : SurfaceView) :
    TermElabM (TSyntax `term × ViewSidecar) := do
  let systemName := identText viewDecl.system
  let selected? := boxCtx.systems.find? (·.logicalName == systemName)
  let tableName := selected?.map (·.irName) |>.getD systemName
  let attrs := selected?.map (·.attrs) |>.getD []
  let (filterTerm, filterTokens) ← match viewDecl.filter with
    | none => do
        let term ← `(none)
        pure (term, none)
    | some filterExpr =>
        let lowered ← lowerRawExpr tableName attrs paramCtx boxCtx.inputs filterExpr
          familyCtx bindingCtx domainCtx functionCtx partitionCtx
        let term ← `(some $(lowered.term))
        pure (term, some lowered.tokens)
  let (valueTerm, valueTokens) ← match viewDecl.value with
    | none => do
        let term ← `(none)
        pure (term, none)
    | some valueExpr =>
        let lowered ← lowerRawExpr tableName attrs paramCtx boxCtx.inputs valueExpr
          familyCtx bindingCtx domainCtx functionCtx partitionCtx
        let term ← `(some $(lowered.term))
        pure (term, some lowered.tokens)
  let reduceTerm ← match viewDecl.reduce with
    | "sum" => `(ViewReduce.sum)
    | "count" => `(ViewReduce.count)
    | "min" => `(ViewReduce.min)
    | "max" => `(ViewReduce.max)
    | _ => throwErrorAt viewDecl.token "unsupported view reduction '{viewDecl.reduce}'"
  let term ← `(ObservationRaw.view $(Lean.quote viewDecl.name) $(Lean.quote tableName)
    $filterTerm $valueTerm $reduceTerm)
  pure (term, { source := viewDecl, filter := filterTokens, value := valueTokens })

/- Grouped views always elaborate completely: Lean authoring has no runtime flag
   context. Rust validation and execution enforce `grouped-observations` (K6). -/
private def groupedViewTerm (paramCtx : List SurfaceParam) (boxCtx : SurfaceBox)
    (familyCtx : List SurfaceParamFamily) (bindingCtx : List SurfaceIndexBinding)
    (domainCtx : List SurfaceDomain) (functionCtx : List SurfaceExprFunction)
    (partitionCtx : List SurfacePartition) (viewDecl : SurfaceGroupedView) :
    TermElabM (TSyntax `term × GroupedViewSidecar) := do
  let systemName := identText viewDecl.system
  let selected? := boxCtx.systems.find? (·.logicalName == systemName)
  let tableName := selected?.map (·.irName) |>.getD systemName
  let attrs := selected?.map (·.attrs) |>.getD []
  let mut keyTerms : Array (TSyntax `term) := #[]
  for key in viewDecl.keys do
    let widthTerm ← match key.bandWidth with
      | none => `(none)
      | some (width, _) => `(some $(Lean.quote width))
    keyTerms := keyTerms.push (← `(ObservationRaw.groupKey
      $(Lean.quote (identText key.attr)) $widthTerm))
  let (filterTerm, filterTokens) ← match viewDecl.filter with
    | none => do
        let term ← `(none)
        pure (term, none)
    | some filterExpr =>
        let lowered ← lowerRawExpr tableName attrs paramCtx boxCtx.inputs filterExpr
          familyCtx bindingCtx domainCtx functionCtx partitionCtx
        let term ← `(some $(lowered.term))
        pure (term, some lowered.tokens)
  let term ← `(ObservationRaw.groupedView $(Lean.quote viewDecl.name)
    $(Lean.quote tableName) $filterTerm [$keyTerms,*])
  pure (term, { source := viewDecl, filter := filterTokens })

private def summaryTerm (_boxCtxs : List SurfaceBox) (summaryDecl : SurfaceSummary) :
    TermElabM (TSyntax `term) := do
  let boxName := identText summaryDecl.box
  let viewName := identText summaryDecl.view
  let reduceTerm ← match summaryDecl.reduce with
    | "sum" => `(SummaryReduce.sum)
    | "min" => `(SummaryReduce.min)
    | "max" => `(SummaryReduce.max)
    | "last" => `(SummaryReduce.last)
    | "argmax_tick" => `(SummaryReduce.argmaxTick)
    | _ => throwErrorAt summaryDecl.token
        "unsupported summary reduction '{summaryDecl.reduce}'"
  `(ObservationRaw.summary $(Lean.quote summaryDecl.name) $(Lean.quote boxName)
      $(Lean.quote viewName) $reduceTerm)

private def resolvedTy (boxCtx : SurfaceBox) : SurfaceTy → Option SurfaceTy
  | .ref logical => boxCtx.systems.find? (·.logicalName == logical) |>.map fun target => .ref target.irName
  | ty => some ty

private def schemasMatch (leftBox : SurfaceBox) (left : List SurfaceAttr)
    (rightBox : SurfaceBox) (right : List SurfaceAttr) : Bool :=
  left.length == right.length && (left.zip right).all fun (a, b) =>
    a.name == b.name && resolvedTy leftBox a.ty == resolvedTy rightBox b.ty

private unsafe def evalCompleteBuilderUnsafe (expr : Lean.Expr) :
    TermElabM (Except ObservationBuilderError Sembla.Semantics.Checked.Model) := do
  Meta.evalExpr (Except ObservationBuilderError Sembla.Semantics.Checked.Model)
    (← Meta.inferType expr) expr

/-- Trusted metaprogram evaluation of the proved pure builder result.  This is
bookkeeping/diagnostic glue, not part of the verified builder boundary. -/
@[implemented_by evalCompleteBuilderUnsafe]
private opaque evalCompleteBuilder (expr : Lean.Expr) :
    TermElabM (Except ObservationBuilderError Sembla.Semantics.Checked.Model)

private def quoteList (elementType : Lean.Expr) (values : List α)
    (quote : α → MetaM Lean.Expr) : MetaM Lean.Expr := do
  Meta.mkListLit elementType (← values.mapM quote)

private def quoteOption (elementType : Lean.Expr) (value : Option α)
    (quote : α → MetaM Lean.Expr) : MetaM Lean.Expr :=
  match value with
  | none => pure (mkApp (mkConst ``Option.none [.zero]) elementType)
  | some value => do
      pure (mkApp2 (mkConst ``Option.some [.zero]) elementType (← quote value))

private def quoteIRScientific (value : IR.Scientific) : MetaM Lean.Expr :=
  Meta.mkAppM ``IR.Scientific.mk #[Lean.toExpr value.coefficient, Lean.toExpr value.exponent]

private def quoteIRParamType : IR.ParamType → MetaM Lean.Expr
  | .real => pure (mkConst ``IR.ParamType.real)
  | .int => pure (mkConst ``IR.ParamType.int)

private def quoteIRParamValue : IR.ParamValue → MetaM Lean.Expr
  | .real value => do Meta.mkAppM ``IR.ParamValue.real #[← quoteIRScientific value]
  | .int value => Meta.mkAppM ``IR.ParamValue.int #[Lean.toExpr value]

private def quoteIRPriorFamily : IR.PriorFamily → MetaM Lean.Expr
  | .normal => pure (mkConst ``IR.PriorFamily.normal)
  | .logNormal => pure (mkConst ``IR.PriorFamily.logNormal)
  | .uniform => pure (mkConst ``IR.PriorFamily.uniform)

private def quoteIRPrior (value : IR.Prior) : MetaM Lean.Expr := do
  Meta.mkAppM ``IR.Prior.mk #[← quoteIRPriorFamily value.family,
    ← quoteList (mkConst ``IR.Scientific) value.args quoteIRScientific]

private def quoteIRParamDecl (value : IR.ParamDecl) : MetaM Lean.Expr := do
  Meta.mkAppM ``IR.ParamDecl.mk #[Lean.toExpr value.name, ← quoteIRParamType value.ty,
    ← quoteIRParamValue value.default,
    ← quoteOption (mkConst ``IR.Prior) value.prior quoteIRPrior]

private def quoteIRAttrType : IR.AttrType → MetaM Lean.Expr
  | .real => pure (mkConst ``IR.AttrType.real)
  | .int => pure (mkConst ``IR.AttrType.int)
  | .enum variants => do
      Meta.mkAppM ``IR.AttrType.enum #[
        ← quoteList (mkConst ``String) variants fun value => pure (Lean.toExpr value)]
  | .ref table => Meta.mkAppM ``IR.AttrType.ref #[Lean.toExpr table]

private def quoteIRAttr (value : IR.Attr) : MetaM Lean.Expr := do
  Meta.mkAppM ``IR.Attr.mk #[Lean.toExpr value.name, ← quoteIRAttrType value.ty]

private def quoteIRTable (value : IR.Table) : MetaM Lean.Expr := do
  Meta.mkAppM ``IR.Table.mk #[Lean.toExpr value.name, Lean.toExpr value.sizeHint,
    ← quoteList (mkConst ``IR.Attr) value.attrs quoteIRAttr]

mutual
  private partial def quoteIRExpr : IR.Expr → MetaM Lean.Expr
    | .real value => do Meta.mkAppM ``IR.Expr.real #[← quoteIRScientific value]
    | .int value => Meta.mkAppM ``IR.Expr.int #[Lean.toExpr value]
    | .bool value => Meta.mkAppM ``IR.Expr.bool #[Lean.toExpr value]
    | .enum variant => Meta.mkAppM ``IR.Expr.enum #[Lean.toExpr variant]
    | .param name => Meta.mkAppM ``IR.Expr.param #[Lean.toExpr name]
    | .selfAttr name => Meta.mkAppM ``IR.Expr.selfAttr #[Lean.toExpr name]
    | .add lhs rhs => do Meta.mkAppM ``IR.Expr.add #[← quoteIRExpr lhs, ← quoteIRExpr rhs]
    | .sub lhs rhs => do Meta.mkAppM ``IR.Expr.sub #[← quoteIRExpr lhs, ← quoteIRExpr rhs]
    | .mul lhs rhs => do Meta.mkAppM ``IR.Expr.mul #[← quoteIRExpr lhs, ← quoteIRExpr rhs]
    | .div lhs rhs => do Meta.mkAppM ``IR.Expr.div #[← quoteIRExpr lhs, ← quoteIRExpr rhs]
    | .eq lhs rhs => do Meta.mkAppM ``IR.Expr.eq #[← quoteIRExpr lhs, ← quoteIRExpr rhs]
    | .ne lhs rhs => do Meta.mkAppM ``IR.Expr.ne #[← quoteIRExpr lhs, ← quoteIRExpr rhs]
    | .lt lhs rhs => do Meta.mkAppM ``IR.Expr.lt #[← quoteIRExpr lhs, ← quoteIRExpr rhs]
    | .le lhs rhs => do Meta.mkAppM ``IR.Expr.le #[← quoteIRExpr lhs, ← quoteIRExpr rhs]
    | .gt lhs rhs => do Meta.mkAppM ``IR.Expr.gt #[← quoteIRExpr lhs, ← quoteIRExpr rhs]
    | .ge lhs rhs => do Meta.mkAppM ``IR.Expr.ge #[← quoteIRExpr lhs, ← quoteIRExpr rhs]
    | .and lhs rhs => do Meta.mkAppM ``IR.Expr.and #[← quoteIRExpr lhs, ← quoteIRExpr rhs]
    | .or lhs rhs => do Meta.mkAppM ``IR.Expr.or #[← quoteIRExpr lhs, ← quoteIRExpr rhs]
    | .not value => do Meta.mkAppM ``IR.Expr.not #[← quoteIRExpr value]
    | .enumIs attributeName variant =>
        Meta.mkAppM ``IR.Expr.enumIs #[Lean.toExpr attributeName, Lean.toExpr variant]
    | .input port aggregate => do
        Meta.mkAppM ``IR.Expr.input #[Lean.toExpr port, ← quoteIRAggregate aggregate]
    | .agg op table fkAttr selfFkAttr filter => do
        Meta.mkAppM ``IR.Expr.agg #[← quoteIRAggOp op, Lean.toExpr table,
          Lean.toExpr fkAttr, Lean.toExpr selfFkAttr, ← quoteIRExpr filter]

  private partial def quoteIRAggOp : IR.AggOp → MetaM Lean.Expr
    | .count => pure (mkConst ``IR.AggOp.count)
    | .sum value => do Meta.mkAppM ``IR.AggOp.sum #[← quoteIRExpr value]

  private partial def quoteIRAggregate : IR.Aggregate → MetaM Lean.Expr
    | .mk op filter => do
        Meta.mkAppM ``IR.Aggregate.mk #[← quoteIRAggOp op,
          ← quoteOption (mkConst ``IR.Expr) filter quoteIRExpr]
end

private def quoteIREffect : IR.Effect → MetaM Lean.Expr
  | .setAttr attributeName value => do
      Meta.mkAppM ``IR.Effect.setAttr #[Lean.toExpr attributeName, ← quoteIRExpr value]

private def quoteIRClaimOrdering : IR.ClaimOrdering → MetaM Lean.Expr
  | .raceTime => pure (mkConst ``IR.ClaimOrdering.raceTime)
  | .key value => do Meta.mkAppM ``IR.ClaimOrdering.key #[← quoteIRExpr value]

private def quoteIRResourceClaim (value : IR.ResourceClaim) : MetaM Lean.Expr := do
  Meta.mkAppM ``IR.ResourceClaim.mk #[← quoteIRExpr value.resource,
    ← quoteIRClaimOrdering value.ordering]

private def quoteIRTransition (value : IR.Transition) : MetaM Lean.Expr := do
  Meta.mkAppM ``IR.Transition.mk #[Lean.toExpr value.name, Lean.toExpr value.table,
    ← quoteIRExpr value.guard, ← quoteIRExpr value.hazard,
    ← quoteList (mkConst ``IR.Effect) value.effects quoteIREffect,
    ← quoteList (mkConst ``IR.ResourceClaim) value.contests quoteIRResourceClaim]

private def quoteIRPortDecl (value : IR.PortDecl) : MetaM Lean.Expr := do
  Meta.mkAppM ``IR.PortDecl.mk #[Lean.toExpr value.name,
    ← quoteList (mkConst ``IR.Attr) value.schema quoteIRAttr]

private def quoteIROutputField (value : IR.OutputField) : MetaM Lean.Expr := do
  Meta.mkAppM ``IR.OutputField.mk #[Lean.toExpr value.name, ← quoteIRAggOp value.op,
    ← quoteOption (mkConst ``IR.Expr) value.filter quoteIRExpr]

private def quoteIROutputBuilder : IR.OutputBuilder → MetaM Lean.Expr
  | .perTable table fieldValues => do
      Meta.mkAppM ``IR.OutputBuilder.perTable #[Lean.toExpr table,
        ← quoteList (mkConst ``IR.OutputField) fieldValues quoteIROutputField]

private def quoteIROutputDecl (value : IR.OutputDecl) : MetaM Lean.Expr := do
  Meta.mkAppM ``IR.OutputDecl.mk #[Lean.toExpr value.name,
    ← quoteList (mkConst ``IR.Attr) value.schema quoteIRAttr,
    ← quoteIROutputBuilder value.builder]

private def quoteIRViewReduce : IR.ViewReduce → MetaM Lean.Expr
  | .sum => pure (mkConst ``IR.ViewReduce.sum)
  | .count => pure (mkConst ``IR.ViewReduce.count)
  | .min => pure (mkConst ``IR.ViewReduce.min)
  | .max => pure (mkConst ``IR.ViewReduce.max)

private def quoteIRViewDecl (value : IR.ViewDecl) : MetaM Lean.Expr := do
  Meta.mkAppM ``IR.ViewDecl.mk #[Lean.toExpr value.name, Lean.toExpr value.table,
    ← quoteOption (mkConst ``IR.Expr) value.filter quoteIRExpr,
    ← quoteOption (mkConst ``IR.Expr) value.value quoteIRExpr,
    ← quoteIRViewReduce value.reduce]

private def quoteIRGroupKey (value : IR.GroupKey) : MetaM Lean.Expr := do
  Meta.mkAppM ``IR.GroupKey.mk #[Lean.toExpr value.attr,
    ← quoteOption (mkConst ``Nat) value.bandWidth fun width => pure (Lean.toExpr width)]

private def quoteIRGroupedViewDecl (value : IR.GroupedViewDecl) : MetaM Lean.Expr := do
  Meta.mkAppM ``IR.GroupedViewDecl.mk #[Lean.toExpr value.name, Lean.toExpr value.table,
    ← quoteOption (mkConst ``IR.Expr) value.filter quoteIRExpr,
    ← quoteList (mkConst ``IR.GroupKey) value.keys quoteIRGroupKey]

private def quoteIRBox (value : IR.Box) : MetaM Lean.Expr := do
  Meta.mkAppM ``IR.Box.mk #[Lean.toExpr value.name,
    ← quoteList (mkConst ``IR.Table) value.tables quoteIRTable,
    ← quoteList (mkConst ``IR.Transition) value.transitions quoteIRTransition,
    ← quoteList (mkConst ``IR.PortDecl) value.inputs quoteIRPortDecl,
    ← quoteList (mkConst ``IR.OutputDecl) value.outputs quoteIROutputDecl,
    ← quoteList (mkConst ``IR.ViewDecl) value.views quoteIRViewDecl,
    ← quoteList (mkConst ``IR.GroupedViewDecl) value.groupedViews quoteIRGroupedViewDecl]

private def quoteIRWireEndpoint (value : IR.WireEndpoint) : MetaM Lean.Expr :=
  Meta.mkAppM ``IR.WireEndpoint.mk #[Lean.toExpr value.box, Lean.toExpr value.port]

private def quoteIRWire (value : IR.Wire) : MetaM Lean.Expr := do
  Meta.mkAppM ``IR.Wire.mk #[← quoteIRWireEndpoint value.source,
    ← quoteIRWireEndpoint value.target]

private def quoteIRSummaryReduce : IR.SummaryReduce → MetaM Lean.Expr
  | .sum => pure (mkConst ``IR.SummaryReduce.sum)
  | .min => pure (mkConst ``IR.SummaryReduce.min)
  | .max => pure (mkConst ``IR.SummaryReduce.max)
  | .last => pure (mkConst ``IR.SummaryReduce.last)
  | .argmaxTick => pure (mkConst ``IR.SummaryReduce.argmaxTick)

private def quoteIRSummaryDecl (value : IR.SummaryDecl) : MetaM Lean.Expr := do
  Meta.mkAppM ``IR.SummaryDecl.mk #[Lean.toExpr value.name, Lean.toExpr value.box,
    Lean.toExpr value.view, ← quoteIRSummaryReduce value.reduce]

private def quoteIRModel (value : IR.Model) : TermElabM Lean.Expr := do
  let result ← Meta.mkAppM ``IR.Model.mk #[Lean.toExpr value.name,
    ← quoteIRScientific value.dt,
    ← quoteList (mkConst ``IR.ParamDecl) value.params quoteIRParamDecl,
    ← quoteList (mkConst ``IR.Box) value.boxes quoteIRBox,
    ← quoteList (mkConst ``IR.Wire) value.wires quoteIRWire,
    ← quoteList (mkConst ``IR.SummaryDecl) value.summaries quoteIRSummaryDecl]
  let resultType ← Meta.inferType result
  unless ← Meta.isDefEq resultType (mkConst ``IR.Model) do
    throwError "internal IR model quotation produced type {resultType}, expected Sembla.IR.Model"
  synthesizeSyntheticMVarsNoPostponing
  instantiateMVars result

private structure ParsedCompleteBox where
  core : CoreBoxShell
  transitionValues : List IR.Transition
  observations : SurfaceBoxObservationPayload

/-- Trusted parser-to-dependent-spec bookkeeping. The sole public owner is the
resulting overlay; this private record has no raw box/model projection. -/
private def trustedSurfaceSpecFromFragments (name : String) (step : Scientific)
    (params : List ParamDecl) (boxes : List ParsedCompleteBox)
    (wires : List Wire) (summaryValues : List SummaryDecl) : SurfaceCompleteModelSpec :=
  let core : CoreModelShell :=
    CoreModelShell.mk name step params (boxes.map ParsedCompleteBox.core)
  let overlay : TransitionOverlaySpec :=
    TransitionOverlaySpec.mk core fun ordinal =>
      (boxes.get ⟨ordinal.val, by simpa [core] using ordinal.isLt⟩).transitionValues
  @SurfaceCompleteModelSpec.mk overlay (fun ordinal =>
    (boxes.get ⟨ordinal.val, by simpa [overlay, core] using ordinal.isLt⟩).observations)
    summaryValues wires

private def modelSpecTerm (name : String) (stepWidth : TSyntax `term)
    (params boxes wires summaryTerms : Array (TSyntax `term)) : TermElabM (TSyntax `term) :=
  `(trustedSurfaceSpecFromFragments $(Lean.quote name) $stepWidth
      [$params,*] [$boxes,*] [$wires,*] [$summaryTerms,*])

private def boxTokenAt (surface : SurfaceModel) (index : Nat) : Syntax :=
  (surface.boxes.get? index).map (·.token) |>.getD surface.declarationToken

private def rawExprPathToken (tree : RawExprTokenTree) :
    List ModelCheckPathSegment → Syntax
  | [] => tree.at
  | segment :: rest =>
      match tree.child? segment with
      | some child => rawExprPathToken child rest
      | none =>
          -- Aggregate owners add structural prefixes around the expression
          -- children retained by the surface token tree.
          match segment with
          | .aggregate | .fieldOperation => rawExprPathToken tree rest
          | _ => tree.at

private def rawExprPathTree? (tree : RawExprTokenTree) :
    List ModelCheckPathSegment → Option RawExprTokenTree
  | [] => some tree
  | segment :: rest =>
      match tree.child? segment with
      | some child => rawExprPathTree? child rest
      | none =>
          match segment with
          | .aggregate | .fieldOperation => rawExprPathTree? tree rest
          | _ => none

private def comparisonRhsToken? (tree : RawExprTokenTree)
    (path : List ModelCheckPathSegment) : Option Syntax := do
  let comparison ← rawExprPathTree? tree path
  match comparison.at with
  | `(semblaExpr| $_lhs:semblaExpr = $_rhs:semblaExpr)
  | `(semblaExpr| $_lhs:semblaExpr ≠ $_rhs:semblaExpr)
  | `(semblaExpr| $_lhs:semblaExpr < $_rhs:semblaExpr)
  | `(semblaExpr| $_lhs:semblaExpr ≤ $_rhs:semblaExpr)
  | `(semblaExpr| $_lhs:semblaExpr > $_rhs:semblaExpr)
  | `(semblaExpr| $_lhs:semblaExpr ≥ $_rhs:semblaExpr) =>
      some (rawExprPathToken comparison [.rhs])
  | _ => none

private def orderedComparisonRhsToken? (tree : RawExprTokenTree)
    (path : List ModelCheckPathSegment) : Option Syntax := do
  let comparison ← rawExprPathTree? tree path
  match comparison.at with
  | `(semblaExpr| $_lhs:semblaExpr < $_rhs:semblaExpr)
  | `(semblaExpr| $_lhs:semblaExpr ≤ $_rhs:semblaExpr)
  | `(semblaExpr| $_lhs:semblaExpr > $_rhs:semblaExpr)
  | `(semblaExpr| $_lhs:semblaExpr ≥ $_rhs:semblaExpr) =>
      some (rawExprPathToken comparison [.rhs])
  | _ => none

private def isOrderedComparisonAtPath (tree : RawExprTokenTree)
    (path : List ModelCheckPathSegment) : Bool :=
  match orderedComparisonRhsToken? tree path with
  | some _ => true
  | none => false

/-- Render logical operand failures from the authoritative checker path and the
retained authored token tree.  This inspects syntax provenance only; it does
not infer or recheck operand types. -/
private def logicalBoolOperandMessage? (tree : RawExprTokenTree) :
    List ModelCheckPathSegment → Option String
  | [] => none
  | [segment] =>
      match segment with
      | .lhs =>
          match tree.at with
          | `(semblaExpr| $_lhs:semblaExpr && $_rhs:semblaExpr) =>
              some "left operand of && must have type Bool"
          | `(semblaExpr| $_lhs:semblaExpr ∧ $_rhs:semblaExpr) =>
              some "left operand of ∧ must have type Bool"
          | _ => none
      | .rhs =>
          match tree.at with
          | `(semblaExpr| $_lhs:semblaExpr && $_rhs:semblaExpr) =>
              some "right operand of && must have type Bool"
          | `(semblaExpr| $_lhs:semblaExpr ∧ $_rhs:semblaExpr) =>
              some "right operand of ∧ must have type Bool"
          | _ => none
      | .operand =>
          match tree.at with
          | `(semblaExpr| ¬$_inner:semblaExpr) =>
              some "operand of ¬ must have type Bool"
          | _ => none
      | _ => none
  | segment :: rest =>
      match tree.child? segment with
      | some child => logicalBoolOperandMessage? child rest
      | none => none

private def transitionSidecarForPath? (sidecars : ObservationSidecars) :
    List ModelCheckPathSegment → Option EmittedTransitionSidecar
  | .model :: rest => transitionSidecarForPath? sidecars rest
  | .box boxIndex :: .transition transitionIndex :: _ =>
      sidecars.boxSidecars.get? boxIndex >>= (·.transitionSidecars.get? transitionIndex)
  | _ => none

private partial def inputSumFieldContextAtPath? (tree : RawExprTokenTree) :
    List ModelCheckPathSegment → Option (String × String)
  | path =>
      match tree.at with
      | `(semblaExpr| inputSum $port:ident field $column:ident) =>
          if path.any (· == .aggregateValue) then
            some (identText port, identText column)
          else none
      | _ =>
          match path with
          | segment :: rest =>
              match tree.child? segment with
              | some child => inputSumFieldContextAtPath? child rest
              | none =>
                  match segment with
                  | .aggregate | .fieldOperation => inputSumFieldContextAtPath? tree rest
                  | _ => none
          | [] => none

private partial def frequencyKeyNameAtPath? (tree : RawExprTokenTree) :
    List ModelCheckPathSegment → Option String
  | path =>
      match tree.at with
      | `(semblaExpr| freq ($_predicate:semblaExpr) over $key:ident) =>
          if path.any fun segment =>
              segment == .joinForeignAttribute || segment == .joinSelfAttribute then
            some (identText key)
          else none
      | _ =>
          match path with
          | segment :: rest =>
              match tree.child? segment with
              | some child => frequencyKeyNameAtPath? child rest
              | none =>
                  match segment with
                  | .aggregate | .fieldOperation => frequencyKeyNameAtPath? tree rest
                  | _ => none
          | [] => none

private partial def isFrequencyPredicatePath (tree : RawExprTokenTree) :
    List ModelCheckPathSegment → Bool
  | path =>
      match tree.at with
      | `(semblaExpr| freq ($_predicate:semblaExpr) over $_key:ident) =>
          path.any (· == .aggregateFilter)
      | _ =>
          match path with
          | segment :: rest =>
              match tree.child? segment with
              | some child => isFrequencyPredicatePath child rest
              | none =>
                  match segment with
                  | .aggregate | .fieldOperation => isFrequencyPredicatePath tree rest
                  | _ => false
          | [] => false

private def transitionExpressionProvenance? (sidecar : EmittedTransitionSidecar)
    (path : List ModelCheckPathSegment) :
    Option (Syntax × RawExprTokenTree × List ModelCheckPathSegment) :=
  match path.dropWhile fun segment => segment != .guard && segment != .hazard with
  | .guard :: rest =>
      match sidecar with
      | .explicit _ detail => some (detail.target, detail.guardTokens, rest)
      | .indexedGeneral _ detail => some (detail.target, detail.guardTokens, rest)
      | .indexedReaction _ detail => some (detail.target, detail.guardTokens, rest)
      | _ => none
  | .hazard :: rest =>
      match sidecar with
      | .explicit _ detail => some (detail.target, detail.hazardTokens, rest)
      | .indexedGeneral _ detail => some (detail.target, detail.hazardTokens, rest)
      | .explicitReaction _ detail => some (detail.target, detail.hazardTokens, rest)
      | .inferredReaction _ detail => some (detail.targetToken, detail.hazardTokens, rest)
      | .indexedReaction _ detail => some (detail.target, detail.hazardTokens, rest)
      | .legacy _ detail => some (detail.target, detail.hazardTokens, rest)
  | _ => none

private def transitionInputSumFieldContext? (sidecars : ObservationSidecars)
    (path : List ModelCheckPathSegment) : Option (String × String) := do
  let sidecar ← transitionSidecarForPath? sidecars path
  let (_, tree, expressionPath) ← transitionExpressionProvenance? sidecar path
  inputSumFieldContextAtPath? tree expressionPath

private def transitionFrequencyKeyContext? (sidecars : ObservationSidecars)
    (path : List ModelCheckPathSegment) : Option (String × String) := do
  let sidecar ← transitionSidecarForPath? sidecars path
  let (target, tree, expressionPath) ← transitionExpressionProvenance? sidecar path
  let keyName ← frequencyKeyNameAtPath? tree expressionPath
  pure (keyName, target.getId.getString!)

private def isTransitionFrequencyPredicatePath (sidecars : ObservationSidecars)
    (path : List ModelCheckPathSegment) : Bool :=
  match transitionSidecarForPath? sidecars path >>= fun sidecar =>
      transitionExpressionProvenance? sidecar path with
  | some (_, tree, expressionPath) => isFrequencyPredicatePath tree expressionPath
  | none => false

private def authoredOutputField? (sidecar : OutputSidecar) (schemaIndex : Nat) :
    Option OutputFieldSidecar := do
  let schemaField ← sidecar.source.schema.get? schemaIndex
  sidecar.authoredFields.find? (·.source.name == schemaField.name)

private def outputPathToken (sidecar : OutputSidecar) :
    List ModelCheckPathSegment → Syntax
  | .outputBuilder :: .tableTarget :: _ => sidecar.source.system.raw
  | .outputSchema :: rest =>
      match rest with
      | .outputField index :: _ =>
          (sidecar.source.schema.get? index).map (·.nameToken) |>.getD sidecar.source.token
      | _ => sidecar.source.token
  | .outputFields :: .outputField index :: rest =>
      match authoredOutputField? sidecar index with
      | none => sidecar.source.token
      | some fieldSidecar =>
          match rest with
          | .fieldFilter :: expressionPath =>
              fieldSidecar.filter.map (rawExprPathToken · expressionPath) |>.getD fieldSidecar.source.token
          | .fieldOperation :: .aggregateValue :: expressionPath
          | .fieldValue :: expressionPath =>
              fieldSidecar.value.map (rawExprPathToken · expressionPath) |>.getD fieldSidecar.source.token
          | .fieldOperation :: [] =>
              fieldSidecar.value.map (rawExprPathToken · []) |>.getD fieldSidecar.source.token
          | .fieldName :: _ | .fieldOperation :: _ | [] => fieldSidecar.source.token
          | _ => fieldSidecar.source.token
  | _ => sidecar.source.token

private def viewPathToken (sidecar : ViewSidecar) :
    List ModelCheckPathSegment → Syntax
  | .viewTable :: _ => sidecar.source.system.raw
  | .viewFilter :: rest =>
      sidecar.filter.map (rawExprPathToken · rest) |>.getD sidecar.source.token
  | .viewValue :: rest =>
      sidecar.value.map (rawExprPathToken · rest) |>.getD sidecar.source.token
  | .viewReducer :: _ | [] => sidecar.source.token
  | _ => sidecar.source.token

private def groupedPathToken (category? : Option ModelTermErrorCategory)
    (sidecar : GroupedViewSidecar) : List ModelCheckPathSegment → Syntax
  | .viewTable :: _ => sidecar.source.system.raw
  | .viewFilter :: rest =>
      sidecar.filter.map (rawExprPathToken · rest) |>.getD sidecar.source.token
  | .groupedKeys :: .groupedKey index :: .groupedBand :: _ =>
      match sidecar.source.keys.get? index with
      | some groupKey =>
          match category? with
          | some .unexpectedGroupedBand => groupKey.attr.raw
          | _ => groupKey.bandWidth.map (·.2) |>.getD groupKey.attr.raw
      | none => sidecar.source.token
  | .groupedKeys :: .groupedKey index :: _ =>
      (sidecar.source.keys.get? index).map (·.attr.raw) |>.getD sidecar.source.token
  | .groupedKeys :: _ =>
      if sidecar.source.keys.length > 4 then
        (sidecar.source.keys.get? 4).map (·.attr.raw) |>.getD sidecar.source.token
      else sidecar.source.token
  | _ => sidecar.source.token

private def transitionPathToken (category? : Option TermCheckErrorCategory)
    (source : SurfaceTransition) (detail : ExplicitTransitionDetail) :
    List ModelCheckPathSegment → Syntax
  | .guard :: rest =>
      if category? == some .expectedNumeric then
        (orderedComparisonRhsToken? detail.guardTokens rest).getD
          (rawExprPathToken detail.guardTokens rest)
      else if category? == some .incompatibleEquality then
        (comparisonRhsToken? detail.guardTokens rest).getD
          (rawExprPathToken detail.guardTokens rest)
      else rawExprPathToken detail.guardTokens rest
  | .hazard :: rest => rawExprPathToken detail.hazardTokens rest
  | .effects :: .effect index :: .destination :: _ =>
      (detail.effects.get? index).map (·.destination) |>.getD source.token
  | .effects :: .effect index :: .value :: rest =>
      match detail.effects.get? index with
      | some effect =>
          if category? == some .unclaimedRefWrite then effect.destination
          else rawExprPathToken effect.value rest
      | none => source.token
  | .contests :: .claim index :: .resource :: _
  | .contests :: .claim index :: .orderingKey :: _ =>
      (detail.claims.get? index).map (·.resource) |>.getD source.token
  | _ => source.token

private def indexedGeneralTransitionPathToken (category? : Option TermCheckErrorCategory)
    (source : SurfaceTransition) (detail : GeneratedGeneralDetail) :
    List ModelCheckPathSegment → Syntax :=
  transitionPathToken category? source
    ⟨detail.target, detail.guardTokens, detail.hazardTokens, detail.effects, detail.claims⟩

private def explicitReactionTransitionPathToken (category? : Option TermCheckErrorCategory)
    (sourceDecl : SurfaceTransition) (detail : ExplicitReactionDetail) :
    List ModelCheckPathSegment → Syntax
  | .guard :: _ =>
      if category? == some .unknownEnumVariant then detail.source
      else if category? == some .unknownAttribute || category? == some .sortMismatch then
        detail.stateAttribute
      else sourceDecl.token
  | .hazard :: rest => rawExprPathToken detail.hazardTokens rest
  | .effects :: .effect 0 :: .destination :: _ => detail.stateAttribute
  | .effects :: .effect 0 :: .value :: _ => detail.destination
  | _ => sourceDecl.token

private def inferredReactionTransitionPathToken (category? : Option TermCheckErrorCategory)
    (sourceDecl : SurfaceTransition) (detail : InferredReactionDetail) :
    List ModelCheckPathSegment → Syntax
  | .guard :: _ =>
      if category? == some .unknownEnumVariant then detail.source
      else if category? == some .unknownAttribute || category? == some .sortMismatch then
        detail.stateAttributeToken.getD sourceDecl.token
      else sourceDecl.token
  | .hazard :: rest => rawExprPathToken detail.hazardTokens rest
  | .effects :: .effect 0 :: .destination :: _ =>
      detail.stateAttributeToken.getD sourceDecl.token
  | .effects :: .effect 0 :: .value :: _ =>
      if category? == some .unknownEnumVariant then detail.destination else sourceDecl.token
  | _ => sourceDecl.token

private partial def reactionBaseGuardPath (tree : RawExprTokenTree) :
    List ModelCheckPathSegment → Bool
  | path =>
      match tree with
      | .node _ children =>
          match (children.find? (·.1 == .lhs)).map (·.2) with
          | some lhs =>
              match lhs with
              | .node _ (_ :: _) =>
                  match path with
                  | .lhs :: rest => reactionBaseGuardPath lhs rest
                  | _ => false
              | .node _ [] => path.isEmpty
          | none => false

private def indexedReactionBaseGuardFailure (detail : GeneratedReactionDetail)
    (path : List ModelCheckPathSegment) : Bool :=
  match path.dropWhile (· != .guard) with
  | .guard :: rest => reactionBaseGuardPath detail.guardTokens rest
  | _ => false

private def indexedReactionTransitionPathToken (category? : Option TermCheckErrorCategory)
    (sourceDecl : SurfaceTransition) (detail : GeneratedReactionDetail) :
    List ModelCheckPathSegment → Syntax
  | .guard :: rest =>
      if reactionBaseGuardPath detail.guardTokens rest then
        if category? == some .unknownEnumVariant then detail.source
        else if category? == some .unknownAttribute || category? == some .sortMismatch then
          detail.stateAttribute
        else sourceDecl.token
      else rawExprPathToken detail.guardTokens rest
  | .hazard :: rest => rawExprPathToken detail.hazardTokens rest
  | .effects :: .effect 0 :: .destination :: _ => detail.stateAttribute
  | .effects :: .effect 0 :: .value :: _ =>
      if category? == some .unknownEnumVariant then detail.destination else sourceDecl.token
  | _ => sourceDecl.token

private def aliasGuardOrdinal (atomCount : Nat) :
    List ModelCheckPathSegment → Nat
  | .lhs :: _ => 0
  | .rhs :: rest =>
      if atomCount ≤ 1 then 0 else 1 + aliasGuardOrdinal (atomCount - 1) rest
  | _ => 0

private def legacyTransitionPathToken (category? : Option TermCheckErrorCategory)
    (source : SurfaceTransition) (detail : LegacyTransitionDetail) :
    List ModelCheckPathSegment → Syntax
  | .guard :: rest =>
      let provenance := detail.aliasProvenance
      let generatedGuardCount := source.binders.filter (·.mode != .static) |>.length
      let ordinal := aliasGuardOrdinal provenance.totalSourceAtomCount
        (rest.drop generatedGuardCount)
      match provenance.guardAtoms.find? (·.emittedGuardAtomOrdinal == ordinal) with
      | some atom =>
          match category?, atom.destinationToken with
          | some .unknownAttribute, some destination => destination
          | _, _ => atom.valueToken
      | none => source.token
  | .hazard :: rest => rawExprPathToken detail.hazardTokens rest
  | .effects :: .effect index :: .destination :: _ =>
      match detail.explicitEffects.find? (·.ordinal == index) with
      | some effect => effect.detail.destination
      | none => (detail.aliasProvenance.effects.find?
          (·.emittedEffectOrdinal == index)).map (·.destinationToken) |>.getD source.token
  | .effects :: .effect index :: .value :: rest =>
      match detail.explicitEffects.find? (·.ordinal == index) with
      | some effect =>
          if category? == some .unclaimedRefWrite then effect.detail.destination
          else rawExprPathToken effect.detail.value rest
      | none =>
          match detail.aliasProvenance.effects.find? (·.emittedEffectOrdinal == index) with
          | some effect =>
              if category? == some .unclaimedRefWrite then effect.destinationToken
              else effect.valueToken
          | none => source.token
  | .contests :: .claim index :: _ =>
      (detail.claims.get? index).map (·.resource) |>.getD source.token
  | _ => source.token

private def modelPathToken (surface : SurfaceModel) (sidecars : ObservationSidecars)
    (category? : Option ModelTermErrorCategory := none)
    (termCategory? : Option TermCheckErrorCategory := none) :
    List ModelCheckPathSegment → Syntax
  | .model :: rest => modelPathToken surface sidecars category? termCategory? rest
  | .box index :: .output outputIndex :: rest =>
      match sidecars.boxSidecars.get? index >>= (·.outputSidecars.get? outputIndex) with
      | some sidecar => outputPathToken sidecar rest
      | none => boxTokenAt surface index
  | .box index :: .view viewIndex :: rest =>
      match sidecars.boxSidecars.get? index >>= (·.viewSidecars.get? viewIndex) with
      | some sidecar => viewPathToken sidecar rest
      | none => boxTokenAt surface index
  | .box index :: .groupedView viewIndex :: rest =>
      match sidecars.boxSidecars.get? index >>= (·.groupedViewSidecars.get? viewIndex) with
      | some sidecar => groupedPathToken category? sidecar rest
      | none => boxTokenAt surface index
  | .box index :: .transition transitionIndex :: rest =>
      match sidecars.boxSidecars.get? index >>= (·.transitionSidecars.get? transitionIndex) with
      | some (.explicit source detail) => transitionPathToken termCategory? source detail rest
      | some (.indexedGeneral source detail) =>
          indexedGeneralTransitionPathToken termCategory? source detail rest
      | some (.explicitReaction source detail) =>
          explicitReactionTransitionPathToken termCategory? source detail rest
      | some (.inferredReaction source detail) =>
          inferredReactionTransitionPathToken termCategory? source detail rest
      | some (.indexedReaction source detail) =>
          indexedReactionTransitionPathToken termCategory? source detail rest
      | some (.legacy source detail) =>
          legacyTransitionPathToken termCategory? source detail rest
      | none => boxTokenAt surface index
  | .summary index :: rest =>
      match sidecars.summarySidecars.get? index with
      | some declaration =>
          match rest with
          | .summaryBox :: _ => declaration.box.raw
          | .summaryView :: _ => declaration.view.raw
          | .summaryReducer :: _ | [] => declaration.token
          | _ => declaration.token
      | none => surface.declarationToken
  | .box index :: _ => boxTokenAt surface index
  | _ => surface.declarationToken

private def observationSurfacePathToken (surface : SurfaceModel)
    (sidecars : ObservationSidecars) : List ObservationSurfacePathSegment → Syntax
  | .box boxIndex :: .output outputIndex :: .outputField fieldIndex :: _ =>
      match sidecars.boxSidecars.get? boxIndex >>= (·.outputSidecars.get? outputIndex) with
      | some outputSidecar =>
          (outputSidecar.authoredFields.get? fieldIndex).map (·.source.token) |>.getD outputSidecar.source.token
      | none => boxTokenAt surface boxIndex
  | .box boxIndex :: .output outputIndex :: .outputSchema schemaIndex :: _ =>
      match sidecars.boxSidecars.get? boxIndex >>= (·.outputSidecars.get? outputIndex) with
      | some outputSidecar =>
          (outputSidecar.source.schema.get? schemaIndex).map (·.nameToken) |>.getD outputSidecar.source.token
      | none => boxTokenAt surface boxIndex
  | .box boxIndex :: .output outputIndex :: _ =>
      match sidecars.boxSidecars.get? boxIndex >>= (·.outputSidecars.get? outputIndex) with
      | some outputSidecar => outputSidecar.source.token
      | none => boxTokenAt surface boxIndex
  | .box boxIndex :: .view viewIndex :: _ =>
      match sidecars.boxSidecars.get? boxIndex >>= (·.viewSidecars.get? viewIndex) with
      | some viewSidecar => viewSidecar.source.token
      | none => boxTokenAt surface boxIndex
  | .box boxIndex :: .groupedView viewIndex :: .groupedKey keyIndex :: .groupedBand :: _ =>
      match sidecars.boxSidecars.get? boxIndex >>= (·.groupedViewSidecars.get? viewIndex)
          >>= (·.source.keys.get? keyIndex) with
      | some groupKey => groupKey.bandWidth.map (·.2) |>.getD groupKey.attr.raw
      | none => boxTokenAt surface boxIndex
  | .box boxIndex :: .groupedView viewIndex :: .groupedKey keyIndex :: _ =>
      match sidecars.boxSidecars.get? boxIndex >>= (·.groupedViewSidecars.get? viewIndex)
          >>= (·.source.keys.get? keyIndex) with
      | some groupKey => groupKey.attr.raw
      | none => boxTokenAt surface boxIndex
  | .box boxIndex :: .groupedView viewIndex :: _ =>
      match sidecars.boxSidecars.get? boxIndex >>= (·.groupedViewSidecars.get? viewIndex) with
      | some viewSidecar => viewSidecar.source.token
      | none => boxTokenAt surface boxIndex
  | .summary index :: .summaryBox :: _ =>
      (sidecars.summarySidecars.get? index).map (·.box.raw) |>.getD surface.declarationToken
  | .summary index :: .summaryView :: _ =>
      (sidecars.summarySidecars.get? index).map (·.view.raw) |>.getD surface.declarationToken
  | .summary index :: _ =>
      (sidecars.summarySidecars.get? index).map (·.token) |>.getD surface.declarationToken
  | .box boxIndex :: .input inputIndex :: _ =>
      match surface.boxes.get? boxIndex with
      | some currentBox =>
          (currentBox.inputs.get? inputIndex).map (·.token) |>.getD currentBox.token
      | none => surface.declarationToken
  | .box boxIndex :: _ => boxTokenAt surface boxIndex
  | _ => surface.declarationToken

private def termCategoryMessage : TermCheckErrorCategory → String
  | .unknownParameter => "undeclared parameter"
  | .unknownAttribute => "unknown state or attribute"
  | .unknownEnumVariant => "unknown enum variant"
  | .unknownInput => "unknown input port"
  | .unknownTable => "unknown table"
  | .unknownJoinAttribute => "unknown join attribute"
  | .nestedInputAggregate => "nested input aggregates are not supported"
  | .cannotInferEnumOwner => "cannot infer enum owner"
  | .expectedBool => "expression must have type Bool"
  | .expectedReal => "expression must have type Real"
  | .expectedNumeric => "expression must be numeric"
  | .expectedReference => "expression must be a reference"
  | .expectedOrderable => "expression must be orderable"
  | .sortMismatch => "expression sorts do not match"
  | .incompatibleEquality => "equality operands have incompatible types"
  | .incompatibleJoinTargets => "join targets are incompatible"
  | .duplicateResourceClaim => "duplicate resource claim"
  | .unclaimedRefWrite => "writes to Ref attributes require a resource claim"

private def modelCategoryMessage : ModelTermErrorCategory → String
  | .term category => termCategoryMessage category
  | .unresolvedOutputTable => "output refers to an unknown table"
  | .duplicateOutputField => "duplicate output field"
  | .outputFieldCountMismatch => "output field count does not match schema"
  | .outputFieldNameMismatch => "output field name does not match schema"
  | .outputFieldSortMismatch => "output field sort does not match schema"
  | .unresolvedViewTable => "view refers to an unknown table"
  | .invalidViewReducerShape => "view reducer has an invalid value shape"
  | .invalidGroupedKeyCount => "grouped view requires one to four keys"
  | .unresolvedGroupedKey => "grouped view key is unresolved"
  | .invalidGroupedKeySort => "grouped view key has an invalid sort"
  | .missingGroupedBand => "Int grouped key requires a positive band"
  | .unexpectedGroupedBand => "band is supported only for Int grouped keys"
  | .nonpositiveGroupedBand => "grouped band width must be greater than zero"
  | .aggregateInGroupedFilter => "aggregates are not supported in grouped view filters"
  | .unresolvedSummaryBox => "summary refers to an unknown box"
  | .unresolvedSummaryView => "summary refers to an undeclared view"

private def diagnosticSortName : DiagnosticScalarSort → String
  | .real => "Real"
  | .int => "Int"
  | .bool => "Bool"
  | .enum => "Enum"
  | .ref => "Ref"

private def diagnosticReducerName : DiagnosticViewReducer → String
  | .count => "count"
  | .sum => "sum"
  | .min => "min"
  | .max => "max"

private def pathHasEffectValue : List ModelCheckPathSegment → Bool
  | .effects :: .effect _ :: .value :: _ => true
  | _ :: rest => pathHasEffectValue rest
  | [] => false

private def pathHasGuard : List ModelCheckPathSegment → Bool
  | .guard :: _ => true
  | _ :: rest => pathHasGuard rest
  | [] => false

private def pathHasHazard : List ModelCheckPathSegment → Bool
  | .hazard :: _ => true
  | _ :: rest => pathHasHazard rest
  | [] => false

private def isOrderedComparisonSyntax (stx : Syntax) : Bool :=
  match stx with
  | `(semblaExpr| $_lhs:semblaExpr < $_rhs:semblaExpr)
  | `(semblaExpr| $_lhs:semblaExpr ≤ $_rhs:semblaExpr)
  | `(semblaExpr| $_lhs:semblaExpr > $_rhs:semblaExpr)
  | `(semblaExpr| $_lhs:semblaExpr ≥ $_rhs:semblaExpr) => true
  | _ => false

/- Lowered general-transition errors are rendered only from checker metadata
   and syntax provenance retained in emitted order. -/
private def loweredGeneralTransitionTermErrorMessage (surface : SurfaceModel)
    (sidecars : ObservationSidecars) (error : TermCheckError) : String :=
  let metadata := error.metadata
  let offending := metadata.offendingName.getD ""
  let actual := metadata.actualSort.map diagnosticSortName |>.getD ""
  let frequencyKeyContext? := transitionFrequencyKeyContext? sidecars error.path
  let inFrequencyPredicate := isTransitionFrequencyPredicatePath sidecars error.path
  match frequencyKeyContext?, error.category with
  | some (keyName, systemName), .unknownJoinAttribute =>
      s!"unknown frequency key attribute '{keyName}' on system '{systemName}'"
  | some (keyName, systemName), .expectedReference =>
      s!"frequency key attribute '{keyName}' on system '{systemName}' must have type Ref; found {actual}"
  | _, _ => if pathHasEffectValue error.path then
    match error.category with
    | .unknownParameter => s!"undeclared parameter '{offending}'"
    | .unknownAttribute => s!"unknown state or attribute '{offending}'"
    | .unknownInput => s!"unknown input port '{offending}'"
    | .unknownEnumVariant =>
        s!"unknown variant '{offending}' for attribute '{metadata.contextName.getD ""}'"
    | .nestedInputAggregate => "nested input aggregates are not supported"
    | .unclaimedRefWrite =>
        "writes to Ref attributes require resource claims, which are not supported by this DSL"
    | _ => "effect value has incompatible type"
  else
    match error.category with
    | .unknownParameter =>
        if inFrequencyPredicate then
          s!"unknown row attribute or model parameter '{offending}' in frequency predicate; {frequencyRowLocalMessage}"
        else s!"undeclared parameter '{offending}'"
    | .unknownAttribute =>
        if inFrequencyPredicate then
          s!"unknown row attribute or model parameter '{offending}' in frequency predicate; {frequencyRowLocalMessage}"
        else s!"unknown state or attribute '{offending}'"
    | .unknownInput => s!"unknown input port '{offending}'"
    | .unknownEnumVariant =>
        s!"unknown variant '{offending}' for attribute '{metadata.contextName.getD ""}'"
    | .expectedBool =>
        if inFrequencyPredicate then
          s!"frequency predicate has type {actual}; expected Bool"
        else if pathHasGuard error.path then
          let logicalMessage? : Option String :=
            match transitionSidecarForPath? sidecars error.path with
            | some (.explicit _ detail) =>
                match error.path.dropWhile (· != .guard) with
                | .guard :: rest => logicalBoolOperandMessage? detail.guardTokens rest
                | _ => none
            | some (.indexedGeneral _ detail) =>
                match error.path.dropWhile (· != .guard) with
                | .guard :: rest => logicalBoolOperandMessage? detail.guardTokens rest
                | _ => none
            | _ => none
          logicalMessage?.getD s!"guard has type {actual}; expected Bool"
        else termCategoryMessage error.category
    | .expectedReal =>
        if pathHasHazard error.path then s!"hazard has type {actual}; expected Real"
        else termCategoryMessage error.category
    | .expectedReference =>
        s!"contest attribute '{offending}' must have type Ref"
    | .duplicateResourceClaim => s!"duplicate contest for attribute '{offending}'"
    | .unclaimedRefWrite =>
        "writes to Ref attributes require resource claims, which are not supported by this DSL"
    | .expectedNumeric =>
        let ordered :=
          match transitionSidecarForPath? sidecars error.path with
          | some (.explicit _ detail) =>
              match error.path.dropWhile (· != .guard) with
              | .guard :: rest => isOrderedComparisonAtPath detail.guardTokens rest
              | _ => false
          | some (.indexedGeneral _ detail) =>
              match error.path.dropWhile (· != .guard) with
              | .guard :: rest => isOrderedComparisonAtPath detail.guardTokens rest
              | _ => false
          | some (.explicitReaction ..) | some (.inferredReaction ..)
          | some (.indexedReaction ..) | _ =>
              let token :=
                (modelPathToken surface sidecars (termCategory? := some error.category)) error.path
              isOrderedComparisonSyntax token
        match ordered with
        | true => "ordered comparison operands must be numeric"
        | false => "numeric operator requires numeric operands"
    | .incompatibleEquality => "comparison operands have incompatible types"
    | _ => termCategoryMessage error.category

private def transitionFrequencyTermErrorMessage? (sidecars : ObservationSidecars)
    (error : TermCheckError) : Option String :=
  let offending := error.metadata.offendingName.getD ""
  let actual := error.metadata.actualSort.map diagnosticSortName |>.getD ""
  match transitionInputSumFieldContext? sidecars error.path, error.category with
  | some (portName, fieldName), .expectedNumeric =>
      some s!"input sum field '{portName}.{fieldName}' must be numeric"
  | _, _ =>
      match transitionFrequencyKeyContext? sidecars error.path, error.category with
      | some (keyName, systemName), .unknownJoinAttribute =>
          some s!"unknown frequency key attribute '{keyName}' on system '{systemName}'"
      | some (keyName, systemName), .expectedReference =>
          some s!"frequency key attribute '{keyName}' on system '{systemName}' must have type Ref; found {actual}"
      | _, .unknownParameter | _, .unknownAttribute =>
          if isTransitionFrequencyPredicatePath sidecars error.path then
            some s!"unknown row attribute or model parameter '{offending}' in frequency predicate; {frequencyRowLocalMessage}"
          else none
      | _, .expectedBool =>
          if isTransitionFrequencyPredicatePath sidecars error.path then
            some s!"frequency predicate has type {actual}; expected Bool"
          else none
      | _, _ => none

private def reactionTermErrorMessage (stateAttributeToken : Syntax)
    (fallbackStateAttributeName : Option String) (baseGuardFailure : Bool)
    (error : TermCheckError) : String :=
  let metadata := error.metadata
  let offending := metadata.offendingName.getD ""
  let actual := metadata.actualSort.map diagnosticSortName |>.getD ""
  let fallback := fallbackStateAttributeName.getD stateAttributeToken.getId.getString!
  let stateAttribute := metadata.contextName.getD fallback
  if baseGuardFailure then
    match error.category with
    | .unknownAttribute => s!"unknown state or attribute '{offending}'"
    | .sortMismatch =>
        s!"reaction state attribute '{offending}' must have type Enum"
    | .unknownEnumVariant =>
        s!"unknown source variant '{offending}' for state attribute '{stateAttribute}'"
    | _ => termCategoryMessage error.category
  else if pathHasEffectValue error.path then
    match error.category with
    | .unknownEnumVariant =>
        s!"unknown destination variant '{offending}' for state attribute '{stateAttribute}'"
    | .unknownParameter => s!"undeclared parameter '{offending}'"
    | .unknownAttribute => s!"unknown state or attribute '{offending}'"
    | .unknownInput => s!"unknown input port '{offending}'"
    | _ => termCategoryMessage error.category
  else if pathHasHazard error.path then
    match error.category with
    | .unknownParameter => s!"undeclared parameter '{offending}'"
    | .unknownAttribute => s!"unknown state or attribute '{offending}'"
    | .unknownInput => s!"unknown input port '{offending}'"
    | .expectedReal => s!"hazard has type {actual}; expected Real"
    | _ => termCategoryMessage error.category
  else
    match error.category with
    | .unknownAttribute => s!"unknown state or attribute '{offending}'"
    | _ => termCategoryMessage error.category

private def explicitReactionTermErrorMessage (detail : ExplicitReactionDetail)
    (error : TermCheckError) : String :=
  reactionTermErrorMessage detail.stateAttribute none (pathHasGuard error.path) error

private def inferredReactionTermErrorMessage (detail : InferredReactionDetail)
    (error : TermCheckError) : String :=
  reactionTermErrorMessage (detail.stateAttributeToken.getD detail.source)
    (some detail.resolvedStateAttributeName) (pathHasGuard error.path) error

private def indexedReactionTermErrorMessage (detail : GeneratedReactionDetail)
    (error : TermCheckError) : String :=
  reactionTermErrorMessage detail.stateAttribute none
    (indexedReactionBaseGuardFailure detail error.path) error

private def viewSidecarForPath? (sidecars : ObservationSidecars) :
    List ModelCheckPathSegment → Option ViewSidecar
  | .model :: rest => viewSidecarForPath? sidecars rest
  | .box boxIndex :: .view viewIndex :: _ =>
      sidecars.boxSidecars.get? boxIndex >>= (·.viewSidecars.get? viewIndex)
  | _ => none

private def groupedSidecarForPath? (sidecars : ObservationSidecars) :
    List ModelCheckPathSegment → Option GroupedViewSidecar
  | .model :: rest => groupedSidecarForPath? sidecars rest
  | .box boxIndex :: .groupedView viewIndex :: _ =>
      sidecars.boxSidecars.get? boxIndex >>= (·.groupedViewSidecars.get? viewIndex)
  | _ => none

private def pathHasOutputFilter : List ModelCheckPathSegment → Bool
  | .fieldFilter :: _ => true
  | _ :: rest => pathHasOutputFilter rest
  | [] => false

private def pathHasViewFilter : List ModelCheckPathSegment → Bool
  | .viewFilter :: _ => true
  | _ :: rest => pathHasViewFilter rest
  | [] => false

private def pathHasAggregateFilter : List ModelCheckPathSegment → Bool
  | .aggregateFilter :: _ => true
  | _ :: rest => pathHasAggregateFilter rest
  | [] => false

private def pathHasLhsAggregate : List ModelCheckPathSegment → Bool
  | .lhs :: rest => pathHasAggregateFilter rest
  | _ :: rest => pathHasLhsAggregate rest
  | [] => false

/- Observation errors are rendered only from authoritative category/metadata
   plus parser sidecars.  This adapter does not repeat name, type, schema, or
   reducer validity decisions. -/
private def observationModelErrorMessage (sidecars : ObservationSidecars)
    (error : ModelTermError) : String :=
  let metadata := error.metadata
  let offending := metadata.offendingName.getD ""
  let context := metadata.contextName.getD ""
  let actual := metadata.actualSort.map diagnosticSortName |>.getD ""
  let viewName := (viewSidecarForPath? sidecars error.path).map (·.source.name) |>.getD context
  let groupedName :=
    (groupedSidecarForPath? sidecars error.path).map (·.source.name) |>.getD context
  let frequencyKeyContext? := transitionFrequencyKeyContext? sidecars error.path
  let inFrequencyPredicate := isTransitionFrequencyPredicatePath sidecars error.path
  match frequencyKeyContext?, error.category with
  | some (keyName, systemName), .term .unknownJoinAttribute =>
      s!"unknown frequency key attribute '{keyName}' on system '{systemName}'"
  | some (keyName, systemName), .term .expectedReference =>
      s!"frequency key attribute '{keyName}' on system '{systemName}' must have type Ref; found {actual}"
  | _, category => match category with
  | .term .unknownParameter =>
      if inFrequencyPredicate then
        s!"unknown row attribute or model parameter '{offending}' in frequency predicate; {frequencyRowLocalMessage}"
      else s!"undeclared parameter '{offending}'"
  | .term .unknownAttribute =>
      if inFrequencyPredicate then
        s!"unknown row attribute or model parameter '{offending}' in frequency predicate; {frequencyRowLocalMessage}"
      else if viewName != "" then s!"view '{viewName}': unknown state or attribute '{offending}'"
      else if groupedName != "" then
        s!"grouped view '{groupedName}': unknown state or attribute '{offending}'"
      else s!"unknown state or attribute '{offending}'"
  | .term .unknownEnumVariant =>
      s!"unknown variant '{offending}' for attribute '{metadata.contextName.getD ""}'"
  | .term .unknownInput => s!"unknown input port '{offending}'"
  | .term .unknownTable => s!"unknown table '{offending}'"
  | .term .unknownJoinAttribute => s!"unknown join attribute '{offending}'"
  | .term .expectedReference =>
      s!"join attribute '{offending}' has type {actual}; expected Ref"
  | .term .incompatibleJoinTargets =>
      s!"join targets '{context}' and '{metadata.expectedName.getD ""}' are incompatible"
  | .term .expectedBool =>
      if inFrequencyPredicate then
        s!"frequency predicate has type {actual}; expected Bool"
      else if pathHasLhsAggregate error.path then
        s!"frequency predicate has type {actual}; expected Bool"
      else if pathHasAggregateFilter error.path then
        s!"aggregate filter has type {actual}; expected Bool"
      else if pathHasOutputFilter error.path then "output filter must have type Bool"
      else if viewName != "" && pathHasViewFilter error.path then
        s!"view '{viewName}' filter has type {actual}; expected Bool"
      else if groupedName != "" && pathHasViewFilter error.path then
        s!"grouped view '{groupedName}' filter has type {actual}; expected Bool"
      else termCategoryMessage .expectedBool
  | .unresolvedOutputTable => s!"unknown system '{offending}'"
  | .outputFieldSortMismatch =>
      match metadata.aggregateKind with
      | some .count => s!"count output field '{offending}' must have type Int"
      | some .sum => "output sum value has incompatible type"
      | none => modelCategoryMessage error.category
  | .invalidViewReducerShape =>
      match metadata.viewReducer, metadata.valuePresent, metadata.actualSort with
      | some .count, some true, _ =>
          s!"view '{context}' with reduce count cannot declare a value expression"
      | some reducer, some false, _ =>
          s!"view '{context}' with reduce {diagnosticReducerName reducer} must declare a value expression"
      | _, _, some sort =>
          s!"view '{context}' value has type {diagnosticSortName sort}; expected Real or Int"
      | _, _, _ => modelCategoryMessage error.category
  | .unresolvedViewTable =>
      if (groupedSidecarForPath? sidecars error.path).isSome then
        s!"grouped view '{context}' refers to unknown table '{offending}'"
      else s!"view '{context}' refers to unknown table '{offending}'"
  | .invalidGroupedKeyCount =>
      if metadata.actualCount.getD 0 == 0 then
        s!"grouped view '{context}' requires at least one key"
      else s!"grouped view '{context}' supports at most 4 keys"
  | .unresolvedGroupedKey => s!"unknown state or attribute '{offending}'"
  | .invalidGroupedKeySort =>
      s!"grouped key '{offending}' has type {actual}; expected Enum, Ref, or banded Int"
  | .missingGroupedBand =>
      let isScoped := (groupedSidecarForPath? sidecars error.path).map (·.source.scopedSyntax) |>.getD false
      let expected := if isScoped then s!"band({offending}, <positive-width>)"
        else s!"band {offending} <positive-width>"
      s!"Int grouped key '{offending}' requires '{expected}'"
  | .unexpectedGroupedBand =>
      s!"band is supported only for Int grouped keys; '{offending}' has type {actual}"
  | .nonpositiveGroupedBand => "grouped band width must be greater than zero"
  | .aggregateInGroupedFilter => "aggregates are not supported in grouped view filters"
  | .unresolvedSummaryBox =>
      s!"summary '{context}' refers to unknown box '{offending}'"
  | .unresolvedSummaryView =>
      s!"summary '{context}' refers to undeclared view '{metadata.expectedName.getD ""}.{offending}'"
  | _ => modelCategoryMessage error.category

private def observationDeclarationPathToken (surface : SurfaceModel)
    (sidecars : ObservationSidecars) : List CheckPathSegment → Option Syntax
  | [.dt] => some surface.dt.raw
  | .parameters :: .parameter parameterIndex :: rest => do
      let parameterDecl ← surface.params.get? parameterIndex
      match rest with
      | [] | [.name] => some parameterDecl.token
      | [.default] => some parameterDecl.default.raw
      | .prior :: .argument argumentIndex :: _ =>
          parameterDecl.prior.map fun arguments => if argumentIndex = 0 then arguments.1.raw
            else arguments.2.raw
      | .prior :: _ => some parameterDecl.token
      | _ => some parameterDecl.token
  | .boxes :: .box boxIndex :: .tables :: .table tableIndex :: rest => do
      let currentBox ← surface.boxes.get? boxIndex
      let table ← currentBox.systems.get? tableIndex
      match rest with
      | [] | [.name] => some table.irNameToken
      | .schema :: .attribute attributeIndex :: attributeRest => do
          let attrDecl ← table.attrs.get? attributeIndex
          match attributeRest with
          | [] | [.name] | [.ty] => some attrDecl.nameToken
          | [.tableTarget] => some (attrDecl.refTargetToken.getD attrDecl.nameToken)
          | [.enumVariant variantIndex] =>
              some ((attrDecl.variantTokens.get? variantIndex).map (·.2) |>.getD attrDecl.nameToken)
          | _ => some attrDecl.nameToken
      | _ => some table.token
  | .boxes :: .box boxIndex :: .transitions :: .transition transitionIndex ::
      .tableTarget :: [] => do
      let sidecar ← sidecars.boxSidecars.get? boxIndex >>=
        (·.transitionSidecars.get? transitionIndex)
      match sidecar with
      | .explicit _ detail => some detail.target
      | .indexedGeneral _ detail => some detail.target
      | .explicitReaction _ detail => some detail.target
      | .inferredReaction _ detail => some detail.targetToken
      | .indexedReaction _ detail => some detail.target
      | .legacy _ detail => some detail.target
  | .boxes :: .box boxIndex :: .inputs :: .input inputIndex :: rest => do
      let currentBox ← surface.boxes.get? boxIndex
      let inputDecl ← currentBox.inputs.get? inputIndex
      match rest with
      | [] | [.name] => some inputDecl.token
      | .schema :: .attribute attributeIndex :: attributeRest => do
          let attrDecl ← inputDecl.schema.get? attributeIndex
          match attributeRest with
          | [] | [.name] | [.ty] => some attrDecl.nameToken
          | [.tableTarget] => some (attrDecl.refTargetToken.getD attrDecl.nameToken)
          | [.enumVariant variantIndex] =>
              some ((attrDecl.variantTokens.get? variantIndex).map (·.2) |>.getD attrDecl.nameToken)
          | _ => some attrDecl.nameToken
      | _ => some inputDecl.token
  | .boxes :: .box boxIndex :: .outputs :: .output outputIndex :: rest => do
      let currentBox ← surface.boxes.get? boxIndex
      let outputDecl ← currentBox.outputs.get? outputIndex
      match rest with
      | [] | [.name] => some outputDecl.token
      | .schema :: .attribute attributeIndex :: attributeRest => do
          let attrDecl ← outputDecl.schema.get? attributeIndex
          match attributeRest with
          | [] | [.name] | [.ty] => some attrDecl.nameToken
          | [.tableTarget] => some (attrDecl.refTargetToken.getD attrDecl.nameToken)
          | [.enumVariant variantIndex] =>
              some ((attrDecl.variantTokens.get? variantIndex).map (·.2) |>.getD attrDecl.nameToken)
          | _ => some attrDecl.nameToken
      | _ => some outputDecl.token
  | .boxes :: .box boxIndex :: .views :: .view viewIndex :: _ => do
      let currentBox ← surface.boxes.get? boxIndex
      some ((currentBox.views.get? viewIndex).map (·.token) |>.getD currentBox.token)
  | .boxes :: .box boxIndex :: .groupedViews :: .groupedView viewIndex :: _ => do
      let currentBox ← surface.boxes.get? boxIndex
      some ((currentBox.groupedViews.get? viewIndex).map (·.token) |>.getD currentBox.token)
  | .summaries :: .summary summaryIndex :: _ =>
      some ((surface.summaries.get? summaryIndex).map (·.token) |>.getD surface.declarationToken)
  | _ => none

/- Declaration diagnostics below are a syntax-only compatibility adapter.  The
   authoritative category/path selects the failed observation declaration; the
   parser sidecar supplies only its authored spelling and token. -/
private def observationDeclarationMessage? (surface : SurfaceModel)
    (error : CheckError) : Option String :=
  match error.category, error.path with
  | .duplicateName, .boxes :: .box boxIndex :: .inputs :: .input inputIndex :: .name :: [] => do
      let inputDecl ← surface.boxes.get? boxIndex >>= (·.inputs.get? inputIndex)
      some s!"duplicate input port '{inputDecl.name}'"
  | .duplicateName, .boxes :: .box boxIndex :: .outputs :: .output outputIndex :: .name :: [] => do
      let outputDecl ← surface.boxes.get? boxIndex >>= (·.outputs.get? outputIndex)
      some s!"duplicate output port '{outputDecl.name}'"
  | .duplicateName, .boxes :: .box boxIndex :: .views :: .view viewIndex :: .name :: [] => do
      let viewDecl ← surface.boxes.get? boxIndex >>= (·.views.get? viewIndex)
      some s!"duplicate view '{viewDecl.name}'"
  | .duplicateName,
      .boxes :: .box boxIndex :: .groupedViews :: .groupedView viewIndex :: .name :: [] => do
      let viewDecl ← surface.boxes.get? boxIndex >>= (·.groupedViews.get? viewIndex)
      some s!"duplicate view '{viewDecl.name}'"
  | .duplicateName, .summaries :: .summary summaryIndex :: .name :: [] => do
      let summaryDecl ← surface.summaries.get? summaryIndex
      some s!"duplicate summary '{summaryDecl.name}'"
  | .duplicateName,
      .boxes :: .box boxIndex :: .inputs :: .input inputIndex :: .schema ::
        .attribute attributeIndex :: .name :: [] => do
      let attrDecl ← surface.boxes.get? boxIndex >>= (·.inputs.get? inputIndex) >>=
        (·.schema.get? attributeIndex)
      some s!"duplicate input field '{attrDecl.name}'"
  | .duplicateName,
      .boxes :: .box boxIndex :: .outputs :: .output outputIndex :: .schema ::
        .attribute attributeIndex :: .name :: [] => do
      let attrDecl ← surface.boxes.get? boxIndex >>= (·.outputs.get? outputIndex) >>=
        (·.schema.get? attributeIndex)
      some s!"duplicate output schema field '{attrDecl.name}'"
  | .emptyEnum,
      .boxes :: .box boxIndex :: .inputs :: .input inputIndex :: .schema ::
        .attribute attributeIndex :: [] => do
      let attrDecl ← surface.boxes.get? boxIndex >>= (·.inputs.get? inputIndex) >>=
        (·.schema.get? attributeIndex)
      some s!"enum attribute '{attrDecl.name}' must declare at least one variant"
  | .emptyEnum,
      .boxes :: .box boxIndex :: .outputs :: .output outputIndex :: .schema ::
        .attribute attributeIndex :: [] => do
      let attrDecl ← surface.boxes.get? boxIndex >>= (·.outputs.get? outputIndex) >>=
        (·.schema.get? attributeIndex)
      some s!"enum attribute '{attrDecl.name}' must declare at least one variant"
  | .duplicateEnumVariant,
      .boxes :: .box boxIndex :: .inputs :: .input inputIndex :: .schema ::
        .attribute attributeIndex :: .enumVariant variantIndex :: [] => do
      let attrDecl ← surface.boxes.get? boxIndex >>= (·.inputs.get? inputIndex) >>=
        (·.schema.get? attributeIndex)
      let variant ← attrDecl.variantTokens.get? variantIndex
      some s!"duplicate enum variant '{variant.1}'"
  | .duplicateEnumVariant,
      .boxes :: .box boxIndex :: .outputs :: .output outputIndex :: .schema ::
        .attribute attributeIndex :: .enumVariant variantIndex :: [] => do
      let attrDecl ← surface.boxes.get? boxIndex >>= (·.outputs.get? outputIndex) >>=
        (·.schema.get? attributeIndex)
      let variant ← attrDecl.variantTokens.get? variantIndex
      some s!"duplicate enum variant '{variant.1}'"
  | _, _ => none

private def declarationCategoryMessage : CheckErrorCategory → String
  | .nonpositiveDt => "tick width must be greater than zero"
  | .duplicateName => "duplicate declaration name"
  | .parameterDefaultMismatch => "parameter default has incompatible type"
  | .integerPrior => "priors are not supported on Int parameters"
  | .priorArity => "prior has invalid arity"
  | .unorderedUniform => "uniform prior bounds must be strictly ordered"
  | .emptyEnum => "enum must declare at least one variant"
  | .duplicateEnumVariant => "duplicate enum variant"
  | .unresolvedTableReference => "unknown reference target"
  | .unresolvedTransitionTable => "transition refers to an unknown table"

private def declarationErrorTokenAndMessage (surface : SurfaceModel)
    (sidecars : ObservationSidecars) (error : CheckError) : Syntax × String :=
  let token := (observationDeclarationPathToken surface sidecars error.path).getD
    surface.declarationToken
  let explicitTarget? := match error.path with
    | .boxes :: .box boxIndex :: .transitions :: .transition transitionIndex ::
        .tableTarget :: [] => do
        let sidecar ← sidecars.boxSidecars.get? boxIndex >>=
          (·.transitionSidecars.get? transitionIndex)
        match sidecar with
        | .explicit _ detail => some detail.target
        | .indexedGeneral _ detail => some detail.target
        | .explicitReaction _ detail => some detail.target
        | .inferredReaction _ detail => some detail.targetToken
        | .indexedReaction _ detail => some detail.target
        | .legacy _ detail => some detail.target
    | _ => none
  let message := match error.category, explicitTarget? with
    | .unresolvedTransitionTable, some target =>
        s!"unknown system '{target.getId.getString!}'"
    | _, _ => (observationDeclarationMessage? surface error).getD
        (declarationCategoryMessage error.category)
  (token, message)

private def coreBuilderPathToken (surface : SurfaceModel) :
    List CoreBuilderPathSegment → Syntax
  | .modelMetadata :: rest => coreBuilderPathToken surface rest
  | .dt :: _ => surface.dt.raw
  | .parameter parameterIndex :: rest =>
      match surface.params.get? parameterIndex with
      | none => surface.declarationToken
      | some parameterDecl =>
          match rest with
          | .defaultValue :: _ => parameterDecl.default.raw
          | .prior :: .priorArgument argumentIndex :: _ =>
              parameterDecl.prior.map (fun arguments =>
                if argumentIndex = 0 then arguments.1.raw else arguments.2.raw) |>.getD
                  parameterDecl.token
          | .prior :: _ | .name :: _ | [] => parameterDecl.token
          | _ => parameterDecl.token
  | .box boxIndex :: rest =>
      match surface.boxes.get? boxIndex with
      | none => surface.declarationToken
      | some currentBox =>
          match rest with
          | .table tableIndex :: tableRest =>
              match currentBox.systems.get? tableIndex with
              | none => currentBox.token
              | some table =>
                  match tableRest with
                  | .attribute attributeIndex :: attributeRest =>
                      match table.attrs.get? attributeIndex with
                      | none => table.token
                      | some attributeDecl =>
                          match attributeRest with
                          | .enumVariant variantIndex :: _ =>
                              (attributeDecl.variantTokens.get? variantIndex).map (·.2) |>.getD
                                attributeDecl.nameToken
                          | .tableReference :: _ =>
                              attributeDecl.refTargetToken.getD attributeDecl.nameToken
                          | _ => attributeDecl.nameToken
                  | .name :: _ | [] => table.irNameToken
                  | _ => table.token
          | .name :: _ | [] => currentBox.token
          | _ => currentBox.token
  | [] => surface.declarationToken
  | _ => surface.declarationToken

private def transitionBuilderPathToken (surface : SurfaceModel)
    (sidecars : ObservationSidecars) : List TransitionBuilderPathSegment → Syntax
  | .box boxIndex :: .transition transitionIndex :: .claim claimIndex :: _ =>
      match sidecars.boxSidecars.get? boxIndex >>= (·.transitionSidecars.get? transitionIndex) with
      | some (.explicit source detail) =>
          (detail.claims.get? claimIndex).map (·.resource) |>.getD source.token
      | some (.indexedGeneral source detail) =>
          (detail.claims.get? claimIndex).map (·.resource) |>.getD source.token
      | some (.legacy source detail) =>
          (detail.claims.get? claimIndex).map (·.resource) |>.getD source.token
      | some (.explicitReaction source _) | some (.inferredReaction source _)
      | some (.indexedReaction source _) => source.token
      | none => boxTokenAt surface boxIndex
  | .box boxIndex :: .transition transitionIndex :: _ =>
      match sidecars.boxSidecars.get? boxIndex >>= (·.transitionSidecars.get? transitionIndex) with
      | some (.explicit source _) | some (.indexedGeneral source _)
      | some (.explicitReaction source _) | some (.inferredReaction source _)
      | some (.indexedReaction source _) | some (.legacy source _) => source.token
      | none => boxTokenAt surface boxIndex
  | .box boxIndex :: _ => boxTokenAt surface boxIndex
  | _ => surface.declarationToken

private def coreCategoryMessage : CoreBuilderErrorCategory → String
  | .nonpositiveDt => "tick width must be greater than zero"
  | .duplicateParameterName => "duplicate parameter"
  | .duplicateBoxName => "duplicate box"
  | .duplicateTableName => "duplicate table"
  | .duplicateAttributeName => "duplicate attribute"
  | .parameterDefaultTypeMismatch => "parameter default has incompatible type"
  | .integerPrior => "priors are not supported on Int parameters"
  | .invalidPriorArity => "prior has invalid arity"
  | .unorderedUniformBounds => "uniform prior bounds must be strictly ordered"
  | .emptyEnum => "enum must declare at least one variant"
  | .duplicateEnumVariant => "duplicate enum variant"
  | .unresolvedTableReference => "unknown reference target"


private def coreErrorMessage (surface : SurfaceModel) (error : CoreBuilderError) : String :=
  let fallback := coreCategoryMessage error.category
  match error.category, error.path with
  | .duplicateParameterName, .parameter index :: _ =>
      (surface.params.get? index).map (fun declaration =>
        s!"duplicate parameter '{declaration.sourceName}'") |>.getD fallback
  | .parameterDefaultTypeMismatch, .parameter index :: _ =>
      match (surface.params.get? index).map (·.ty) with
      | some SurfaceTy.int => "Int parameter defaults require an integer literal"
      | _ => fallback
  | .duplicateBoxName, .box index :: _ =>
      (surface.boxes.get? index).map (fun declaration =>
        s!"duplicate box '{declaration.name}'") |>.getD fallback
  | .duplicateTableName, .box boxIndex :: .table tableIndex :: _ =>
      (surface.boxes.get? boxIndex >>= (·.systems.get? tableIndex)).map (fun declaration =>
        s!"duplicate system '{declaration.logicalName}'") |>.getD fallback
  | .duplicateAttributeName, .box boxIndex :: .table tableIndex ::
      .attribute attributeIndex :: _ =>
      (surface.boxes.get? boxIndex >>= (·.systems.get? tableIndex) >>=
        (·.attrs.get? attributeIndex)).map (fun declaration =>
          s!"duplicate attribute '{declaration.name}'") |>.getD fallback
  | .duplicateEnumVariant, .box boxIndex :: .table tableIndex ::
      .attribute attributeIndex :: .enumVariant variantIndex :: _ =>
      (surface.boxes.get? boxIndex >>= (·.systems.get? tableIndex) >>=
        (·.attrs.get? attributeIndex) >>= (·.variantTokens.get? variantIndex)).map
          (fun variant => s!"duplicate enum variant '{variant.1}'") |>.getD fallback
  | .emptyEnum, .box boxIndex :: .table tableIndex :: .attribute attributeIndex :: _ =>
      (surface.boxes.get? boxIndex >>= (·.systems.get? tableIndex) >>=
        (·.attrs.get? attributeIndex)).map (fun declaration =>
          s!"enum attribute '{declaration.name}' must declare at least one variant") |>.getD fallback
  | .unresolvedTableReference, .box boxIndex :: .table tableIndex ::
      .attribute attributeIndex :: _ =>
      (surface.boxes.get? boxIndex >>= (·.systems.get? tableIndex) >>=
        (·.attrs.get? attributeIndex)).bind (fun declaration =>
          match declaration.ty with
          | .ref target => some s!"unknown reference target '{target}'"
          | _ => none) |>.getD fallback
  | _, _ => fallback

private def builderErrorTokenAndMessage (surface : SurfaceModel)
    (sidecars : ObservationSidecars) : ObservationBuilderError → Syntax × String
  | .core error =>
      (coreBuilderPathToken surface error.path, coreErrorMessage surface error)
  | .transition (.core error) =>
      (coreBuilderPathToken surface error.path, coreErrorMessage surface error)
  | .transition (.declaration error) =>
      declarationErrorTokenAndMessage surface sidecars error
  | .transition (.term error) =>
      let token := (modelPathToken surface sidecars (termCategory? := some error.category)) error.path
      match transitionFrequencyTermErrorMessage? sidecars error with
      | some message => (token, message)
      | none =>
          match transitionSidecarForPath? sidecars error.path with
          | some (.explicit ..) | some (.indexedGeneral ..) | some (.legacy ..) =>
              (token, loweredGeneralTransitionTermErrorMessage surface sidecars error)
          | some (.explicitReaction _ detail) =>
              (token, explicitReactionTermErrorMessage detail error)
          | some (.inferredReaction _ detail) =>
              (token, inferredReactionTermErrorMessage detail error)
          | some (.indexedReaction _ detail) =>
              (token, indexedReactionTermErrorMessage detail error)
          | _ => (token, termCategoryMessage error.category)
  | .transition (.modelCheck (.declaration error)) =>
      declarationErrorTokenAndMessage surface sidecars error
  | .transition (.modelCheck (.model error)) =>
      let termCategory? := match error.category with | .term category => some category | _ => none
      (modelPathToken surface sidecars (some error.category) termCategory? error.path,
        observationModelErrorMessage sidecars error)
  | .transition (.unsupportedSurfaceKeyOrdering path) =>
      (transitionBuilderPathToken surface sidecars path,
        "current transition surface supports race-time ordering only")
  | .lowering error =>
      let message := match error.category with
        | .duplicateOutputField => "duplicate output builder field"
        | .extraOutputField => "output field is absent from port schema"
        | .missingOutputField => "output schema field has no builder"
      (observationSurfacePathToken surface sidecars error.path, message)
  | .modelCheck (.declaration error) =>
      declarationErrorTokenAndMessage surface sidecars error
  | .modelCheck (.model error) =>
      let termCategory? := match error.category with | .term category => some category | _ => none
      (modelPathToken surface sidecars (some error.category) termCategory? error.path,
        observationModelErrorMessage sidecars error)

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

  -- Pass one performs only syntax/representation and expansion planning. Raw
  -- declaration and term semantics are owned by the pure builders/checkers.
  match stepWidth with
  | `(term| $value:scientific) => validateScientific value false
  | _ => throwErrorAt stepWidth "tick width must be a decimal or scientific literal"
  ensureUniqueRuntimeNames "parameter" (paramCtx.map fun p => (p.name, p.sourceName, p.token))
  for paramDecl in paramCtx do
    let _ ← parameterDefaultValueTerm paramDecl.default
    match paramDecl.prior with
    | some priorArgs =>
        validateRealTerm priorArgs.1
        validateRealTerm priorArgs.2
    | none => pure ()
  -- Expression functions are compile-time expansion declarations with no raw
  -- IR node of their own. Validate each cell once before substitution so the
  -- declared result sort remains an expansion/raw-encoding invariant; emitted
  -- uses are still checked by the authoritative term/model checkers.
  trustedValidateExprFunctionCompatibility paramCtx familyCtx domainCtx functionCtx partitionCtx
  let mut plannedBoxes : List PlannedBoxTransitionInstances := []
  for boxCtx in boxCtxs do
    -- Logical-name uniqueness is a surface target-selection compatibility rule;
    -- emitted runtime-name uniqueness remains expansion bookkeeping.
    ensureUnique "system" (boxCtx.systems.map fun s => (s.logicalName, s.token))
    ensureUniqueRuntimeNames "table" (boxCtx.systems.map fun s =>
      (s.irName, s.logicalName, s.irNameToken))
    for selected in boxCtx.systems do validateSize selected.size
    let mut generatedTransitionNames : List (String × Syntax) := []
    let mut plannedTransitions : List PlannedTransitionInstances := []
    for transitionDecl in boxCtx.transitions do
      let instances ← transitionInstances indexCtx boxCtx transitionDecl
      plannedTransitions := plannedTransitions ++ [⟨transitionDecl, instances⟩]
      for (name, _) in instances do
        generatedTransitionNames := generatedTransitionNames ++ [(name, transitionDecl.token)]
    ensureUnique "transition" generatedTransitionNames
    validateStateAliasExpansionShape boxCtx domainCtx partitionCtx
    plannedBoxes := plannedBoxes ++ [⟨plannedTransitions⟩]

  unless plannedBoxes.length == boxCtxs.length do
    throwError "internal box transition plan length mismatch"

  -- Pass two: resolve from the declarations above and emit one pure deep-IR term.
  let mut paramTerms : Array (TSyntax `term) := #[]
  for paramDecl in paramCtx do
    let typeTerm ← match paramDecl.ty with
      | .real => `(ParamType.real)
      | .int => `(ParamType.int)
      | _ => throwErrorAt paramDecl.token "unsupported parameter type"
    let defaultTerm ← parameterDefaultValueTerm paramDecl.default
    let priorTerm ← match paramDecl.prior with
      | some priorArgs =>
          let familyTerm ← match paramDecl.priorFamily with
            | .logNormal => `(PriorFamily.logNormal)
            | .normal => `(PriorFamily.normal)
          `(some (priorRaw $familyTerm [$(priorArgs.1), $(priorArgs.2)]))
      | none => `(none)
    let term ← `(parameterRaw $(Lean.quote paramDecl.name) $typeTerm $defaultTerm $priorTerm)
    paramTerms := paramTerms.push term

  let mut boxTerms : Array (TSyntax `term) := #[]
  let mut observationBoxSidecars : List BoxObservationSidecar := []
  let mut remainingBoxPlans := plannedBoxes
  for boxCtx in boxCtxs do
    let boxPlan ← match remainingBoxPlans with
      | plan :: rest =>
          remainingBoxPlans := rest
          pure plan
      | [] => throwError "internal box transition plan length mismatch"
    unless boxPlan.plans.length == boxCtx.transitions.length do
      throwError "internal transition plan length mismatch"
    -- Ref targets are checked only after all systems are collected, allowing forward refs.
    let mut tableTerms : Array (TSyntax `term) := #[]
    for selected in boxCtx.systems do
      let attrTerms ← selected.attrs.toArray.mapM (attrTerm boxCtx)
      tableTerms := tableTerms.push (← `(tableRaw $(Lean.quote selected.irName)
        $(selected.size) [$attrTerms,*]))
    let mut transitionTermList : Array (TSyntax `term) := #[]
    let mut transitionSidecarList : Array EmittedTransitionSidecar := #[]
    let mut remainingTransitionPlans := boxPlan.plans
    for _transitionDecl in boxCtx.transitions do
      let planned ← match remainingTransitionPlans with
        | plan :: rest =>
            remainingTransitionPlans := rest
            pure plan
        | [] => throwError "internal transition plan length mismatch"
      let loweredTransitions ← transitionTerms familyCtx paramCtx boxCtx planned
        domainCtx functionCtx partitionCtx
      transitionTermList := transitionTermList ++ loweredTransitions.1
      transitionSidecarList := transitionSidecarList ++ loweredTransitions.2
    unless remainingTransitionPlans.isEmpty do
      throwError "internal transition plan length mismatch"
    unless transitionTermList.size == transitionSidecarList.size do
      throwError "internal transition term/sidecar ordering mismatch"
    let mut inputTerms : Array (TSyntax `term) := #[]
    for inputDecl in boxCtx.inputs do
      let schemaTerms ← inputDecl.schema.toArray.mapM (attrTerm boxCtx)
      inputTerms := inputTerms.push (← `(ObservationRaw.input
        $(Lean.quote inputDecl.name) [$schemaTerms,*]))
    let loweredOutputs ← boxCtx.outputs.toArray.mapM
      (outputTerm paramCtx boxCtx familyCtx [] domainCtx functionCtx partitionCtx)
    let loweredViews ← boxCtx.views.toArray.mapM
      (viewTerm paramCtx boxCtx familyCtx [] domainCtx functionCtx partitionCtx)
    let loweredGroupedViews ← boxCtx.groupedViews.toArray.mapM
      (groupedViewTerm paramCtx boxCtx familyCtx [] domainCtx functionCtx partitionCtx)
    let outputTerms := loweredOutputs.map (·.1)
    let viewTerms := loweredViews.map (·.1)
    let groupedViewTerms := loweredGroupedViews.map (·.1)
    let usedAliasIds ← validateAndCollectUsedAliasIds boxCtx transitionSidecarList.toList
    let mut aliasOrdinal := 0
    for stateAlias in boxCtx.aliases do
      let aliasId : AliasDeclId := ⟨aliasOrdinal⟩
      unless usedAliasIds.contains aliasId do
        trustedValidateUnusedStateAliasCompatibility paramCtx boxCtx stateAlias familyCtx
          domainCtx functionCtx partitionCtx
      aliasOrdinal := aliasOrdinal + 1
    let boxObservationSidecar : BoxObservationSidecar := ⟨
      transitionSidecarList.toList, usedAliasIds, loweredOutputs.toList.map (·.2),
      loweredViews.toList.map (·.2), loweredGroupedViews.toList.map (·.2)⟩
    observationBoxSidecars := observationBoxSidecars ++ [boxObservationSidecar]
    boxTerms := boxTerms.push (← `(ParsedCompleteBox.mk
      (CoreBoxShell.mk $(Lean.quote boxCtx.name) [$tableTerms,*])
      [$transitionTermList,*]
      (SurfaceBoxObservationPayload.mk [$inputTerms,*] [$outputTerms,*]
        [$viewTerms,*] [$groupedViewTerms,*])))

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

  let observationSidecars : ObservationSidecars :=
    ⟨observationBoxSidecars, summaryCtx⟩
  let specTerm ← modelSpecTerm modelName stepWidth paramTerms boxTerms wireTerms summaryTerms
  let builderTerm ← `(buildSurfaceCompleteModel $specTerm)
  let builderExpr ← elaborateTerm builderTerm
  synthesizeSyntheticMVarsNoPostponing
  match ← evalCompleteBuilder builderExpr with
  | .error error =>
      let (token, message) := builderErrorTokenAndMessage surface observationSidecars error
      throwErrorAt token message
  | .ok checked =>
      let modelValue : IR.Model := checked.erase
      let resultExpr ← quoteIRModel modelValue

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

      pure resultExpr

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
