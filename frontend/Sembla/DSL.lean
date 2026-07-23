import Lean.Elab.Term
import Lean.Elab.Command
import Sembla.IR
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

structure SurfaceParam where
  sourceName : String
  name : String
  token : Syntax
  ty : SurfaceTy := .real
  default : TSyntax `term
  prior : Option (TSyntax `term × TSyntax `term)

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

inductive SurfaceTransitionBody where
  | general
      (system : TSyntax `ident)
      (guard : TSyntax `semblaExpr)
      (hazard : TSyntax `semblaExpr)
      (sets : List (TSyntax `semblaSet))
  | reaction
      (system : Option (TSyntax `ident))
      (stateAttr : Option (TSyntax `ident))
      (source : TSyntax `ident)
      (hazard : TSyntax `semblaExpr)
      (destination : TSyntax `ident)

structure SurfaceTransition where
  name : String
  token : Syntax
  body : SurfaceTransitionBody

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

/-- Attribute declarations occur exactly once, inside their actual system or
    port declaration.  Transition and output contexts are derived from these
    declarations by the enclosing model elaborator. -/
declare_syntax_cat semblaAttr
syntax "state" ident ":" "{" ident,* "}" : semblaAttr
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
syntax "param" ident ":" "ℝ" ":=" term "~" "LogNormal" semblaRealTerm semblaRealTerm : semblaParam
syntax "param" ident ":" "ℝ" ":=" term : semblaParam
syntax "param" ident ":" "Real" ":=" term "prior" "LogNormal" "(" term "," term ")" : semblaParam
syntax "param" ident ":" "Real" ":=" term : semblaParam
syntax "param" ident ":" "Int" ":=" term "~" "LogNormal" semblaRealTerm semblaRealTerm : semblaParam
syntax "param" ident ":" "Int" ":=" term : semblaParam

declare_syntax_cat semblaExpr
syntax:max ident : semblaExpr
syntax:max "parameter" ident : semblaExpr
syntax:max num : semblaExpr
syntax:max scientific : semblaExpr
syntax:max "(" semblaExpr ")" : semblaExpr
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

declare_syntax_cat semblaArrowTail
syntax "guard" semblaExpr : semblaArrowTail
syntax "set" "[" semblaSet,* "]" : semblaArrowTail

declare_syntax_cat semblaTransition
syntax "transition" ident "on" ident "where" "guard" semblaExpr "hazard" semblaExpr
  "set" "[" semblaSet,* "]" : semblaTransition
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
  "views" "[" semblaView,* "]" : semblaBox

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

declare_syntax_cat semblaCommandTransitionItem
syntax "guard" semblaExpr : semblaCommandTransitionItem
syntax "hazard" semblaExpr : semblaCommandTransitionItem
syntax "set" semblaSet : semblaCommandTransitionItem

declare_syntax_cat semblaCommandGeneralTransition
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

declare_syntax_cat semblaCommandBoxItem
syntax semblaCommandSystem : semblaCommandBoxItem
syntax semblaCommandInput : semblaCommandBoxItem
syntax semblaCommandGeneralTransition : semblaCommandBoxItem
syntax semblaTransition : semblaCommandBoxItem
syntax semblaCommandOutput : semblaCommandBoxItem
syntax semblaCommandView : semblaCommandBoxItem
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

declare_syntax_cat semblaCommandModelItem
syntax semblaParam : semblaCommandModelItem
syntax semblaCommandBox : semblaCommandModelItem
syntax semblaWire : semblaCommandModelItem
syntax semblaCommandSummary : semblaCommandModelItem
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

/-- Derive the frozen runtime name at the original identifier token. Surface
    frontends reuse this entry point so name rules and positioned diagnostics
    remain identical. -/
def deriveRuntimeNameAt (token : TSyntax `ident) : TermElabM String := do
  match deriveRuntimeName (identText token) with
  | .ok name => pure name
  | .error message => throwErrorAt token message

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
  | `(semblaAttr| state $name:ident : { $variants:ident,* }) =>
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
  | `(semblaParam| param $name:ident : ℝ := $default:term ~ LogNormal
        $a:semblaRealTerm $b:semblaRealTerm) =>
      pure ⟨identText name, ← deriveRuntimeNameAt name, name.raw, .real, default,
        some (← realSurfaceTerm a, ← realSurfaceTerm b)⟩
  | `(semblaParam| param $name:ident : ℝ := $default:term) =>
      pure ⟨identText name, ← deriveRuntimeNameAt name, name.raw, .real, default, none⟩
  | `(semblaParam| param $name:ident : Real := $default:term prior LogNormal($a:term, $b:term)) =>
      pure ⟨identText name, identText name, name.raw, .real, default, some (a, b)⟩
  | `(semblaParam| param $name:ident : Real := $default:term) =>
      pure ⟨identText name, identText name, name.raw, .real, default, none⟩
  | `(semblaParam| param $name:ident : Int := $_default:term ~ LogNormal
        $_a:semblaRealTerm $_b:semblaRealTerm) =>
      throwErrorAt name "priors are not supported on Int parameters"
  | `(semblaParam| param $name:ident : Int := $default:term) =>
      pure ⟨identText name, ← deriveRuntimeNameAt name, name.raw, .int, default, none⟩
  | _ => throwUnsupportedSyntax

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

private def parseTransition (stx : TSyntax `semblaTransition) : TermElabM SurfaceTransition := do
  match stx with
  | `(semblaTransition| transition $name:ident on $onSystem:ident where
        guard $guardExpr:semblaExpr hazard $hazardExpr:semblaExpr set [$assignments:semblaSet,*]) =>
      pure ⟨identText name, name.raw,
        .general onSystem guardExpr hazardExpr assignments.getElems.toList⟩
  | `(semblaTransition| $name:ident : $source:ident → [$hazardExpr:semblaExpr]
        $destination:ident) =>
      pure ⟨identText name, name.raw,
        .reaction none none source hazardExpr destination⟩
  | `(semblaTransition| $name:ident on $onSystem:ident : $source:ident →
        [$hazardExpr:semblaExpr] $destination:ident) =>
      pure ⟨identText name, name.raw,
        .reaction (some onSystem) none source hazardExpr destination⟩
  | `(semblaTransition| $name:ident : $stateAttr:ident : $source:ident →
        [$hazardExpr:semblaExpr] $destination:ident) =>
      pure ⟨identText name, name.raw,
        .reaction none (some stateAttr) source hazardExpr destination⟩
  | `(semblaTransition| $name:ident on $onSystem:ident : $stateAttr:ident :
        $source:ident → [$hazardExpr:semblaExpr] $destination:ident) =>
      pure ⟨identText name, name.raw,
        .reaction (some onSystem) (some stateAttr) source hazardExpr destination⟩
  | `(semblaTransition| $_name:ident : $_source:ident → [$_hazardExpr:semblaExpr]
        $_destination:ident $tail:semblaArrowTail) =>
      throwErrorAt tail
        "reaction arrows cannot declare additional guards or effects; use 'transition ... where'"
  | `(semblaTransition| $_name:ident on $_onSystem:ident : $_source:ident →
        [$_hazardExpr:semblaExpr] $_destination:ident $tail:semblaArrowTail) =>
      throwErrorAt tail
        "reaction arrows cannot declare additional guards or effects; use 'transition ... where'"
  | `(semblaTransition| $_name:ident : $_stateAttr:ident : $_source:ident →
        [$_hazardExpr:semblaExpr] $_destination:ident $tail:semblaArrowTail) =>
      throwErrorAt tail
        "reaction arrows cannot declare additional guards or effects; use 'transition ... where'"
  | `(semblaTransition| $_name:ident on $_onSystem:ident : $_stateAttr:ident :
        $_source:ident → [$_hazardExpr:semblaExpr] $_destination:ident
        $tail:semblaArrowTail) =>
      throwErrorAt tail
        "reaction arrows cannot declare additional guards or effects; use 'transition ... where'"
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
        ← outputDecls.getElems.toList.mapM parseOutput, []⟩
  | `(semblaBox| box $name:ident where
        systems [$systemDecls:semblaSystem,*]
        inputs [$inputDecls:semblaInput,*]
        transitions [$transitionDecls:semblaTransition,*]
        outputs [$outputDecls:semblaOutput,*]
        views [$viewDecls:semblaView,*]) =>
      pure ⟨identText name, name.raw,
        ← systemDecls.getElems.toList.mapM parseSystem,
        ← inputDecls.getElems.toList.mapM parseInput,
        ← transitionDecls.getElems.toList.mapM parseTransition,
        ← outputDecls.getElems.toList.mapM parseOutput,
        ← viewDecls.getElems.toList.mapM parseView⟩
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
  match stx with
  | `(semblaCommandGeneralTransition| transition $name:ident on $selectedToken:ident where
        $items:semblaCommandTransitionItem*) =>
      let mut guardExpr : Option (TSyntax `semblaExpr) := none
      let mut hazardExpr : Option (TSyntax `semblaExpr) := none
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
        | _ => throwUnsupportedSyntax
      let resolvedGuard ← guardExpr.getDM
        (throwErrorAt name "general transition '{identText name}' requires exactly one guard")
      let resolvedHazard ← hazardExpr.getDM
        (throwErrorAt name "general transition '{identText name}' requires exactly one hazard")
      if assignments.isEmpty then
        throwErrorAt name "general transition '{identText name}' requires at least one set effect"
      pure ⟨identText name, name.raw,
        .general selectedToken resolvedGuard resolvedHazard assignments⟩
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
        | `(semblaCommandBoxItem| $decl:semblaTransition) =>
            transitionDecls := transitionDecls ++ [← parseTransition decl]
        | `(semblaCommandBoxItem| $decl:semblaCommandOutput) =>
            let parsed ← parseCommandOutput decl
            outputDecls := outputDecls ++ [parsed]
            portDecls := portDecls ++ [.output parsed]
        | `(semblaCommandBoxItem| $decl:semblaCommandView) =>
            viewDecls := viewDecls ++ [← parseCommandView decl]
        | `(semblaCommandBoxItem| contest $unsupported:ident) =>
            throwErrorAt unsupported "unsupported Sembla box declaration '{identText unsupported}'"
        | _ => throwUnsupportedSyntax
      let surfaceBox : SurfaceBox :=
        ⟨identText name, name.raw, systemDecls, inputDecls, transitionDecls,
          outputDecls, viewDecls⟩
      pure ⟨surfaceBox, portDecls⟩
  | _ => throwUnsupportedSyntax

/-- Existing command-box API; model elaboration intentionally ignores the
    additional port-order metadata. -/
def parseCommandBox (stx : TSyntax `semblaCommandBox) : TermElabM SurfaceBox := do
  pure (← parseCommandBoxWithPorts stx).surfaceBox

private def collectCommandSurfaceModel (declaration : TSyntax `ident)
    (runtimeOverride : Option (TSyntax `str)) (stepWidth : TSyntax `term)
    (items : List (TSyntax `semblaCommandModelItem)) : TermElabM SurfaceModel := do
  let mut paramDecls : List SurfaceParam := []
  let mut boxDecls : List SurfaceBox := []
  let mut wireDecls : List SurfaceWire := []
  let mut summaryDecls : List SurfaceSummary := []
  for item in items do
    match item with
    | `(semblaCommandModelItem| $decl:semblaParam) =>
        paramDecls := paramDecls ++ [← parseSurfaceParam decl]
    | `(semblaCommandModelItem| $decl:semblaCommandBox) =>
        boxDecls := boxDecls ++ [← parseCommandBox decl]
    | `(semblaCommandModelItem| $decl:semblaWire) =>
        wireDecls := wireDecls ++ [← parseWire decl]
    | `(semblaCommandModelItem| $decl:semblaCommandSummary) =>
        summaryDecls := summaryDecls ++ [← parseCommandSummary decl]
    | `(semblaCommandModelItem| contest $unsupported:ident) =>
        throwErrorAt unsupported "unsupported Sembla model declaration '{identText unsupported}'"
    | _ => throwUnsupportedSyntax
  let runtimeName ← match runtimeOverride with
    | some value => pure (value.getString, value.raw)
    | none => pure (← deriveRuntimeNameAt declaration, declaration.raw)
  pure ⟨identText declaration, declaration.raw, some runtimeName, stepWidth,
    paramDecls, boxDecls, wireDecls, summaryDecls⟩

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
    summaryCtx⟩

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

private partial def elaborateExpr (tableCtx : SurfaceSystem) (attrs : List SurfaceAttr)
    (paramCtx : List SurfaceParam) (inputCtx : List SurfaceInput) (stx : Syntax)
    (declaration : Option String := none) (frequencyPredicate : Bool := false) :
    TermElabM (TSyntax `term × SurfaceTy) := do
  let recur := fun expression =>
    elaborateExpr tableCtx attrs paramCtx inputCtx expression declaration frequencyPredicate
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
      match attrs.find? (·.name == value), paramCtx.find? (·.sourceName == value) with
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
        elaborateExpr tableCtx attrs paramCtx inputCtx predicate declaration true
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
                let variantName := identText variant
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
      | _ => `(Expr.gt $left $right)
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

private def selectedSystemForTransition (boxCtx : SurfaceBox)
    (transitionDecl : SurfaceTransition) : TermElabM SurfaceSystem := do
  match transitionDecl.body with
  | .general onSystem _ _ _ => lookupSystem boxCtx onSystem
  | .reaction onSystem stateAttr source _ destination =>
      return (← resolveReaction boxCtx transitionDecl.name transitionDecl.token
        onSystem stateAttr source destination).selected

structure ResolvedTransitionBody where
  selected : SurfaceSystem
  guardTerm : TSyntax `term
  hazardSyntax : TSyntax `semblaExpr
  effectTerms : Array (TSyntax `term)

/-- Shared identifier-assignment validation for expanded transitions and
    reaction arrows.  Reactions retain their original destination token while
    using the same enum-membership, Ref-write, and value-type checks. -/
private def identifierEffectTerm (paramCtx : List SurfaceParam) (boxCtx : SurfaceBox)
    (selected : SurfaceSystem) (attrName value : TSyntax `ident) :
    TermElabM (TSyntax `term) := do
  let destination ← lookupAttr selected.attrs attrName
  match destination.ty with
  | .ref _ => throwErrorAt attrName
      "writes to Ref attributes require resource claims, which are not supported by this DSL"
  | _ => pure ()
  let valueName := identText value
  let valueTerm ← match destination.ty with
    | .enum variants =>
        unless variants.contains valueName do
          throwErrorAt value "unknown variant '{valueName}' for attribute '{destination.name}'"
        `(Expr.enum $(Lean.quote valueName))
    | _ =>
        let (term, actualTy) ←
          elaborateExpr selected selected.attrs paramCtx boxCtx.inputs value
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
    (selected : SurfaceSystem) (attrName : TSyntax `ident) (value : TSyntax `semblaExpr) :
    TermElabM (TSyntax `term) := do
  let destination ← lookupAttr selected.attrs attrName
  match destination.ty with
  | .ref _ => throwErrorAt attrName
      "writes to Ref attributes require resource claims, which are not supported by this DSL"
  | .enum variants =>
      match value with
      | `(semblaExpr| $variant:ident) =>
          let variantName := identText variant
          unless variants.contains variantName do
            throwErrorAt variant
              "unknown variant '{variantName}' for attribute '{destination.name}'"
          `(Effect.setAttr $(Lean.quote destination.name) (Expr.enum $(Lean.quote variantName)))
      | _ => throwErrorAt value "enum effect values must be variant literals"
  | .real | .int =>
      rejectEffectAggregates value
      let (valueTerm, actualTy) ←
        elaborateExpr selected selected.attrs paramCtx boxCtx.inputs value
      unless sameType destination.ty actualTy do
        throwErrorAt value "effect value has incompatible type"
      `(Effect.setAttr $(Lean.quote destination.name) $valueTerm)
  | .bool => throwErrorAt value "effect value has incompatible type"

private def resolveTransitionBody (paramCtx : List SurfaceParam) (boxCtx : SurfaceBox)
    (transitionDecl : SurfaceTransition) : TermElabM ResolvedTransitionBody := do
  match transitionDecl.body with
  | .reaction onSystem stateAttr source hazardExpr destination =>
      let resolved ← resolveReaction boxCtx transitionDecl.name transitionDecl.token
        onSystem stateAttr source destination
      let guardTerm ← `(Expr.enumIs $(Lean.quote resolved.stateAttr.name)
        $(Lean.quote resolved.source))
      let attrName := stateAttr.getD ⟨resolved.stateAttr.nameToken⟩
      let effectTerm ← identifierEffectTerm paramCtx boxCtx resolved.selected attrName destination
      pure ⟨resolved.selected, guardTerm, hazardExpr, #[effectTerm]⟩
  | .general onSystem guardExpr hazardExpr assignments =>
      let selected ← lookupSystem boxCtx onSystem
      let (guardTerm, guardTy) ←
        elaborateExpr selected selected.attrs paramCtx boxCtx.inputs guardExpr
      if guardTy != .bool then
        throwErrorAt guardExpr "guard has type {typeName guardTy}; expected Bool"
      let mut effects : Array (TSyntax `term) := #[]
      for assignment in assignments do
        match assignment with
        | `(semblaSet| $attrName:ident := $value:semblaExpr) =>
            effects := effects.push (← effectTerm paramCtx boxCtx selected attrName value)
        | _ => throwUnsupportedSyntax
      pure ⟨selected, guardTerm, hazardExpr, effects⟩

private def transitionTerm (paramCtx : List SurfaceParam) (boxCtx : SurfaceBox)
    (transitionDecl : SurfaceTransition) : TermElabM (TSyntax `term) := do
  let resolved ← resolveTransitionBody paramCtx boxCtx transitionDecl
  let (hazardTerm, hazardTy) ← elaborateExpr resolved.selected resolved.selected.attrs
    paramCtx boxCtx.inputs resolved.hazardSyntax
  unless hazardTy == .real do
    throwErrorAt resolved.hazardSyntax "hazard has type {typeName hazardTy}; expected Real"
  let guardTerm := resolved.guardTerm
  let effects := resolved.effectTerms
  `(Transition.mk $(Lean.quote transitionDecl.name) $(Lean.quote resolved.selected.irName)
      $guardTerm $hazardTerm [$effects,*] [])

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
    | some (first, second) =>
        validateRealTerm first
        validateRealTerm second
    | none => pure ()
  ensureUnique "box" (boxCtxs.map fun b => (b.name, b.token))
  for boxCtx in boxCtxs do
    ensureUnique "system" (boxCtx.systems.map fun s => (s.logicalName, s.token))
    ensureUniqueRuntimeNames "table" (boxCtx.systems.map fun s =>
      (s.irName, s.logicalName, s.irNameToken))
    for selected in boxCtx.systems do validateSize selected.size
    ensureUnique "input port" (boxCtx.inputs.map fun p => (p.name, p.token))
    ensureUnique "transition" (boxCtx.transitions.map fun t => (t.name, t.token))
    ensureUnique "output port" (boxCtx.outputs.map fun p => (p.name, p.token))
    ensureUnique "view" (boxCtx.views.map fun declaration =>
      (declaration.name, declaration.token))
    for selected in boxCtx.systems do
      validateAttrs "attribute" selected.attrs
    for inputDecl in boxCtx.inputs do
      validateAttrs "input field" inputDecl.schema
    for outputDecl in boxCtx.outputs do
      validateAttrs "output schema field" outputDecl.schema
  ensureUnique "summary" (summaryCtx.map fun declaration =>
    (declaration.name, declaration.token))

  -- Pass two: resolve from the declarations above and emit one pure deep-IR term.
  let mut paramTerms : Array (TSyntax `term) := #[]
  for paramDecl in paramCtx do
    let term ← match paramDecl.ty, paramDecl.prior with
      | .real, some (a, b) => `(ParamDecl.mk $(Lean.quote paramDecl.name) ParamType.real
          (ParamValue.real $(paramDecl.default))
          (some (Prior.mk PriorFamily.logNormal [$a, $b])))
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
    let transitionTerms ← boxCtx.transitions.toArray.mapM (transitionTerm paramCtx boxCtx)
    let mut inputTerms : Array (TSyntax `term) := #[]
    for inputDecl in boxCtx.inputs do
      let schemaTerms ← inputDecl.schema.toArray.mapM (attrTerm boxCtx)
      inputTerms := inputTerms.push (← `(PortDecl.mk $(Lean.quote inputDecl.name) [$schemaTerms,*]))
    let outputTerms ← boxCtx.outputs.toArray.mapM (outputTerm paramCtx boxCtx)
    let viewTerms ← boxCtx.views.toArray.mapM (viewTerm paramCtx boxCtx)
    boxTerms := boxTerms.push (← `(Box.mk $(Lean.quote boxCtx.name) [$tableTerms,*]
      [$transitionTerms,*] [$inputTerms,*] [$outputTerms,*] [$viewTerms,*]))

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
