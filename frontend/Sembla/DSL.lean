import Lean.Elab.Term
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
  name : String
  token : Syntax
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

structure SurfaceTransition where
  name : String
  token : Syntax
  system : TSyntax `ident
  guard : TSyntax `semblaExpr
  hazard : TSyntax `semblaExpr
  sets : List (TSyntax `semblaSet)

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

structure SurfaceBox where
  name : String
  token : Syntax
  systems : List SurfaceSystem
  inputs : List SurfaceInput
  transitions : List SurfaceTransition
  outputs : List SurfaceOutput

structure SurfaceWire where
  fromBox : TSyntax `ident
  fromPort : TSyntax `ident
  toBox : TSyntax `ident
  toPort : TSyntax `ident

/-- Attribute declarations occur exactly once, inside their actual system or
    port declaration.  Transition and output contexts are derived from these
    declarations by the enclosing model elaborator. -/
declare_syntax_cat semblaAttr
syntax "state" ident ":" "{" ident,* "}" : semblaAttr
syntax "attr" ident ":" "Real" : semblaAttr
syntax "attr" ident ":" "Int" : semblaAttr
syntax ident ":" "Real" : semblaAttr
syntax ident ":" "Int" : semblaAttr
syntax "ref" ident ":" ident : semblaAttr

declare_syntax_cat semblaParam
syntax "param" ident ":" "Real" ":=" term "prior" "LogNormal" "(" term "," term ")" : semblaParam
syntax "param" ident ":" "Real" ":=" term : semblaParam

declare_syntax_cat semblaExpr
syntax:max ident : semblaExpr
syntax:max "parameter" ident : semblaExpr
syntax:max num : semblaExpr
syntax:max scientific : semblaExpr
syntax:max "(" semblaExpr ")" : semblaExpr
syntax:max "countBy " ident " (" semblaExpr ")" : semblaExpr
syntax:max "sizeBy " ident : semblaExpr
syntax:max "inputSum" ident "field" ident : semblaExpr
syntax:70 semblaExpr:70 " * " semblaExpr:71 : semblaExpr
syntax:70 semblaExpr:70 " / " semblaExpr:71 : semblaExpr
syntax:65 semblaExpr:65 " + " semblaExpr:66 : semblaExpr
syntax:65 semblaExpr:65 " - " semblaExpr:66 : semblaExpr
syntax:55 semblaExpr:56 " = " semblaExpr:55 : semblaExpr
syntax:55 semblaExpr:56 " < " semblaExpr:55 : semblaExpr
syntax:55 semblaExpr:56 " > " semblaExpr:55 : semblaExpr
syntax:40 semblaExpr:41 " && " semblaExpr:40 : semblaExpr

declare_syntax_cat semblaSet
syntax ident ":=" ident : semblaSet
syntax ident ":=" num : semblaSet
syntax ident ":=" scientific : semblaSet

declare_syntax_cat semblaSystem
syntax "system" ident "as" str "rows" "(" term ")" "where" "[" semblaAttr,* "]" : semblaSystem

declare_syntax_cat semblaInput
syntax "input" ident "{" semblaAttr,* "}" : semblaInput

declare_syntax_cat semblaTransition
syntax "transition" ident "on" ident "where" "guard" semblaExpr "hazard" semblaExpr
  "set" "[" semblaSet,* "]" : semblaTransition

declare_syntax_cat semblaOutputField
syntax "field" ident ":=" "count" "where" semblaExpr : semblaOutputField
syntax "field" ident ":=" "sum" "(" semblaExpr ")" : semblaOutputField

declare_syntax_cat semblaOutput
syntax "output" ident "{" semblaAttr,* "}" "from" ident "fields" "[" semblaOutputField,* "]" : semblaOutput

declare_syntax_cat semblaBox
syntax "box" ident "where"
  "systems" "[" semblaSystem,* "]"
  "inputs" "[" semblaInput,* "]"
  "transitions" "[" semblaTransition,* "]"
  "outputs" "[" semblaOutput,* "]" : semblaBox

declare_syntax_cat semblaWire
syntax "wire" ident ident "->" ident ident : semblaWire

private def identText (stx : TSyntax `ident) : String := stx.getId.toString

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
  | `(semblaAttr| attr $name:ident : Real) | `(semblaAttr| $name:ident : Real) =>
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

private def parseParam (stx : TSyntax `semblaParam) : TermElabM SurfaceParam := do
  match stx with
  | `(semblaParam| param $name:ident : Real := $default:term prior LogNormal($a:term, $b:term)) =>
      pure ⟨identText name, name.raw, default, some (a, b)⟩
  | `(semblaParam| param $name:ident : Real := $default:term) =>
      pure ⟨identText name, name.raw, default, none⟩
  | _ => throwUnsupportedSyntax

private def parseSystem (stx : TSyntax `semblaSystem) : TermElabM SurfaceSystem := do
  match stx with
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
      pure ⟨identText name, name.raw, onSystem, guardExpr, hazardExpr, assignments.getElems.toList⟩
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
        ← outputDecls.getElems.toList.mapM parseOutput⟩
  | _ => throwUnsupportedSyntax

private def parseWire (stx : TSyntax `semblaWire) : TermElabM SurfaceWire := do
  match stx with
  | `(semblaWire| wire $fromBox:ident $fromPort:ident -> $toBox:ident $toPort:ident) =>
      pure ⟨fromBox, fromPort, toBox, toPort⟩
  | _ => throwUnsupportedSyntax

private def ensureUnique (kind : String) (entries : List (String × Syntax)) : TermElabM Unit := do
  let mut seen : List String := []
  for (name, token) in entries do
    if seen.contains name then throwErrorAt token "duplicate {kind} '{name}'"
    seen := name :: seen

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

private partial def elaborateExpr (tableCtx : SurfaceSystem) (attrs : List SurfaceAttr)
    (paramCtx : List SurfaceParam) (inputCtx : List SurfaceInput) (stx : Syntax) :
    TermElabM (TSyntax `term × SurfaceTy) := do
  let recur := elaborateExpr tableCtx attrs paramCtx inputCtx
  match stx with
  | `(semblaExpr| ($inner:semblaExpr)) => recur inner
  | `(semblaExpr| $value:num) => pure (← `(Expr.int $value), .int)
  | `(semblaExpr| $value:scientific) =>
      validateScientific value false
      pure (← `(Expr.real $value), .real)
  | `(semblaExpr| parameter $name:ident) =>
      let value := identText name
      unless paramCtx.any (·.name == value) do
        throwErrorAt name "undeclared parameter '{value}'"
      pure (← `(Expr.param $(Lean.quote value)), .real)
  | `(semblaExpr| $name:ident) =>
      let column ← lookupAttr attrs name
      pure (← `(Expr.selfAttr $(Lean.quote column.name)), column.ty)
  | `(semblaExpr| countBy $fk:ident ($filter:semblaExpr)) =>
      let fkAttr ← lookupAttr attrs fk
      match fkAttr.ty with
      | .ref _ => pure ()
      | _ => throwErrorAt fk "countBy key '{identText fk}' must be a Ref attribute"
      let (filterTerm, filterTy) ← recur filter
      if filterTy != .bool then throwErrorAt filter "aggregate filter must have type Bool"
      pure (← `(Expr.agg AggOp.count $(Lean.quote tableCtx.irName) $(Lean.quote fkAttr.name)
        $(Lean.quote fkAttr.name) $filterTerm), .int)
  | `(semblaExpr| sizeBy $fk:ident) =>
      let fkAttr ← lookupAttr attrs fk
      match fkAttr.ty with
      | .ref _ => pure ()
      | _ => throwErrorAt fk "sizeBy key '{identText fk}' must be a Ref attribute"
      pure (← `(Expr.agg AggOp.count $(Lean.quote tableCtx.irName) $(Lean.quote fkAttr.name)
        $(Lean.quote fkAttr.name) (Expr.bool true)), .int)
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
  | `(semblaExpr| $lhs:semblaExpr / $rhs:semblaExpr) => elaborateNumericBinary "div" lhs rhs recur
  | `(semblaExpr| $lhs:semblaExpr + $rhs:semblaExpr) => elaborateNumericBinary "add" lhs rhs recur
  | `(semblaExpr| $lhs:semblaExpr - $rhs:semblaExpr) => elaborateNumericBinary "sub" lhs rhs recur
  | `(semblaExpr| $lhs:semblaExpr = $rhs:semblaExpr) =>
      match lhs, rhs with
      | `(semblaExpr| $attrName:ident), `(semblaExpr| $variant:ident) =>
          let column ← lookupAttr attrs attrName
          match column.ty with
          | .enum variants =>
              let variantName := identText variant
              unless variants.contains variantName do
                throwErrorAt variant "unknown variant '{variantName}' for attribute '{column.name}'"
              pure (← `(Expr.enumIs $(Lean.quote column.name) $(Lean.quote variantName)), .bool)
          | _ => elaborateComparison "eq" lhs rhs recur
      | _, _ => elaborateComparison "eq" lhs rhs recur
  | `(semblaExpr| $lhs:semblaExpr < $rhs:semblaExpr) => elaborateComparison "lt" lhs rhs recur
  | `(semblaExpr| $lhs:semblaExpr > $rhs:semblaExpr) => elaborateComparison "gt" lhs rhs recur
  | `(semblaExpr| $lhs:semblaExpr && $rhs:semblaExpr) =>
      let (left, leftTy) ← recur lhs
      let (right, rightTy) ← recur rhs
      if leftTy != .bool then throwErrorAt lhs "left operand of && must have type Bool"
      if rightTy != .bool then throwErrorAt rhs "right operand of && must have type Bool"
      pure (← `(Expr.and $left $right), .bool)
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
  elaborateComparison (kind : String) (lhs rhs : Syntax)
      (recur : Syntax → TermElabM (TSyntax `term × SurfaceTy)) : TermElabM (TSyntax `term × SurfaceTy) := do
    let (left, leftTy) ← recur lhs
    let (right, rightTy) ← recur rhs
    if kind == "eq" then
      unless equalityCompatible leftTy rightTy do
        throwErrorAt rhs "comparison operands have incompatible types"
    else
      unless isNumeric leftTy && isNumeric rightTy do
        throwErrorAt rhs "ordered comparison operands must be numeric"
    let term ← match kind with
      | "eq" => `(Expr.eq $left $right)
      | "lt" => `(Sembla.IR.Expr.lt $left $right)
      | _ => `(Expr.gt $left $right)
    pure (term, .bool)

private def transitionTerm (paramCtx : List SurfaceParam) (boxCtx : SurfaceBox)
    (transitionDecl : SurfaceTransition) : TermElabM (TSyntax `term) := do
  let selected ← lookupSystem boxCtx transitionDecl.system
  let (guardTerm, guardTy) ← elaborateExpr selected selected.attrs paramCtx boxCtx.inputs transitionDecl.guard
  if guardTy != .bool then
    throwErrorAt transitionDecl.guard "guard has type {typeName guardTy}; expected Bool"
  let (hazardTerm, hazardTy) ← elaborateExpr selected selected.attrs paramCtx boxCtx.inputs transitionDecl.hazard
  unless hazardTy == .real do
    throwErrorAt transitionDecl.hazard "hazard has type {typeName hazardTy}; expected Real"
  let mut effects : Array (TSyntax `term) := #[]
  for assignment in transitionDecl.sets do
    match assignment with
    | `(semblaSet| $attrName:ident := $value:ident) =>
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
              let (term, actualTy) ← elaborateExpr selected selected.attrs paramCtx boxCtx.inputs value
              unless sameType destination.ty actualTy do
                throwErrorAt value "effect value has incompatible type"
              pure term
        effects := effects.push (← `(Effect.setAttr $(Lean.quote destination.name) $valueTerm))
    | `(semblaSet| $attrName:ident := $value:num) =>
        let destination ← lookupAttr selected.attrs attrName
        match destination.ty with
        | .ref _ => throwErrorAt attrName
            "writes to Ref attributes require resource claims, which are not supported by this DSL"
        | _ => pure ()
        unless sameType destination.ty .int do throwErrorAt value "effect value has incompatible type"
        effects := effects.push (← `(Effect.setAttr $(Lean.quote destination.name) (Expr.int $value)))
    | `(semblaSet| $attrName:ident := $value:scientific) =>
        validateScientific value false
        let destination ← lookupAttr selected.attrs attrName
        match destination.ty with
        | .ref _ => throwErrorAt attrName
            "writes to Ref attributes require resource claims, which are not supported by this DSL"
        | _ => pure ()
        unless sameType destination.ty .real do throwErrorAt value "effect value has incompatible type"
        effects := effects.push (← `(Effect.setAttr $(Lean.quote destination.name) (Expr.real $value)))
    | _ => throwUnsupportedSyntax
  `(Transition.mk $(Lean.quote transitionDecl.name) $(Lean.quote selected.irName)
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

elab "model%" name:str "step" "(" dt:term ")" "where"
    "params" "[" paramDecls:semblaParam,* "]"
    "boxes" "[" boxDecls:semblaBox,* "]"
    "wires" "[" wireDecls:semblaWire,* "]" : term => do
  -- Pass one: collect every declaration and retain the original syntax tokens.
  validateStep dt
  let paramCtx ← paramDecls.getElems.toList.mapM parseParam
  let boxCtxs ← boxDecls.getElems.toList.mapM parseBox
  let wireCtx ← wireDecls.getElems.toList.mapM parseWire
  ensureUnique "parameter" (paramCtx.map fun p => (p.name, p.token))
  for paramDecl in paramCtx do
    validateRealTerm paramDecl.default
    match paramDecl.prior with
    | some (first, second) =>
        validateRealTerm first
        validateRealTerm second
    | none => pure ()
  ensureUnique "box" (boxCtxs.map fun b => (b.name, b.token))
  for boxCtx in boxCtxs do
    ensureUnique "system" (boxCtx.systems.map fun s => (s.logicalName, s.token))
    ensureUnique "table" (boxCtx.systems.map fun s => (s.irName, s.irNameToken))
    for selected in boxCtx.systems do validateSize selected.size
    ensureUnique "input port" (boxCtx.inputs.map fun p => (p.name, p.token))
    ensureUnique "transition" (boxCtx.transitions.map fun t => (t.name, t.token))
    ensureUnique "output port" (boxCtx.outputs.map fun p => (p.name, p.token))
    for selected in boxCtx.systems do
      validateAttrs "attribute" selected.attrs
    for inputDecl in boxCtx.inputs do
      validateAttrs "input field" inputDecl.schema
    for outputDecl in boxCtx.outputs do
      validateAttrs "output schema field" outputDecl.schema

  -- Pass two: resolve from the declarations above and emit one pure deep-IR term.
  let mut paramTerms : Array (TSyntax `term) := #[]
  for paramDecl in paramCtx do
    let term ← match paramDecl.prior with
      | some (a, b) => `(ParamDecl.mk $(Lean.quote paramDecl.name) ParamType.real
          (ParamValue.real $(paramDecl.default))
          (some (Prior.mk PriorFamily.logNormal [$a, $b])))
      | none => `(ParamDecl.mk $(Lean.quote paramDecl.name) ParamType.real
          (ParamValue.real $(paramDecl.default)) none)
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
    boxTerms := boxTerms.push (← `(Box.mk $(Lean.quote boxCtx.name) [$tableTerms,*]
      [$transitionTerms,*] [$inputTerms,*] [$outputTerms,*]))

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

  let result ← `(Model.mk $name $dt [$paramTerms,*] [$boxTerms,*] [$wireTerms,*])
  let elaborated ← elabTerm result none
  synthesizeSyntheticMVarsNoPostponing
  let modelValue ← evalModel elaborated

  -- Attach thin ProofWidgets panels to the original declaration-name ranges.
  -- The displayed JSON props come only from the pure IR builders.
  for boxCtx in boxCtxs do
    for selected in boxCtx.systems do
      if let some props := stateDiagramProps? modelValue boxCtx.name selected.irName then
        saveStateDiagram props selected.token
    for transitionDecl in boxCtx.transitions do
      if let some selected := boxCtx.systems.find? (·.logicalName == identText transitionDecl.system) then
        if let some props := stateDiagramProps? modelValue boxCtx.name selected.irName then
          saveStateDiagram props transitionDecl.token
      if let some props := hazardPanelProps? modelValue boxCtx.name transitionDecl.name then
        saveHazardPanel props transitionDecl.token

  pure elaborated

end Sembla.DSL
