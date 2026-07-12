import Lean.Data.Json.FromToJson
import Sembla.IR

/-!
Pure data preparation for Sembla's structure widgets.

Every public builder in this module is a total, IO-free function from the
elaborated deep IR to JSON-encodable props.  Rendering and infoview
registration live in `Sembla.WidgetDisplay`.
-/
namespace Sembla.Widgets

open Lean Sembla.IR

structure PlotPoint where
  x : Float
  y : Float
deriving Repr, BEq, Inhabited, ToJson

structure StateNode where
  id : String
deriving Repr, BEq, Inhabited, ToJson

structure StateEdge where
  name : String
  source : String
  target : String
  hazard : String
deriving Repr, BEq, Inhabited, ToJson

structure StateDiagramProps where
  system : String
  nodes : List StateNode
  edges : List StateEdge
deriving Repr, BEq, Inhabited, ToJson

structure DensityCurve where
  family : String
  points : List PlotPoint
deriving Repr, BEq, Inhabited, ToJson

structure ParamSummary where
  name : String
  defaultValue : Float
  density : Option DensityCurve
deriving Repr, BEq, Inhabited, ToJson

structure HazardPanelProps where
  transition : String
  guard : String
  hazard : String
  params : List ParamSummary
  probability : Option (List PlotPoint)
  noProbabilityReason : Option String
deriving Repr, BEq, Inhabited, ToJson

private def scientificToFloat (value : Scientific) : Float :=
  Float.ofInt value.coefficient * Float.pow 10.0 (Float.ofInt value.exponent)

private def paramDefault? (param : ParamDecl) : Option Float :=
  match param.default with
  | .real value => some (scientificToFloat value)
  | .int value => some (Float.ofInt value)

mutual
partial def prettyExpr : Expr → String
  | .real value =>
      if value.exponent == 0 then toString value.coefficient
      else s!"{value.coefficient}e{value.exponent}"
  | .int value => toString value
  | .bool true => "true"
  | .bool false => "false"
  | .enum variant => variant
  | .param name => name
  | .selfAttr name => name
  | .add lhs rhs => s!"({prettyExpr lhs} + {prettyExpr rhs})"
  | .sub lhs rhs => s!"({prettyExpr lhs} - {prettyExpr rhs})"
  | .mul lhs rhs => s!"({prettyExpr lhs} * {prettyExpr rhs})"
  | .div lhs rhs => s!"({prettyExpr lhs} / {prettyExpr rhs})"
  | .eq lhs rhs => s!"({prettyExpr lhs} = {prettyExpr rhs})"
  | .ne lhs rhs => s!"({prettyExpr lhs} ≠ {prettyExpr rhs})"
  | .lt lhs rhs => s!"({prettyExpr lhs} < {prettyExpr rhs})"
  | .le lhs rhs => s!"({prettyExpr lhs} ≤ {prettyExpr rhs})"
  | .gt lhs rhs => s!"({prettyExpr lhs} > {prettyExpr rhs})"
  | .ge lhs rhs => s!"({prettyExpr lhs} ≥ {prettyExpr rhs})"
  | .and lhs rhs => s!"({prettyExpr lhs} ∧ {prettyExpr rhs})"
  | .or lhs rhs => s!"({prettyExpr lhs} ∨ {prettyExpr rhs})"
  | .not expr => s!"¬{prettyExpr expr}"
  | .enumIs attr variant => s!"{attr} = {variant}"
  | .input port agg => s!"input {port} {prettyAggregate agg}"
  | .agg op table fkAttr selfFkAttr filter =>
      s!"{prettyAggOp op}By {table}.{fkAttr} = self.{selfFkAttr} where {prettyExpr filter}"

partial def prettyAggOp : AggOp → String
  | .count => "count"
  | .sum value => s!"sum({prettyExpr value})"

partial def prettyAggregate : Aggregate → String
  | .mk op none => prettyAggOp op
  | .mk op (some filter) => s!"{prettyAggOp op} where {prettyExpr filter}"
end

mutual
private partial def referencedParams (expr : Expr) : List String :=
  let binary lhs rhs := referencedParams lhs ++ referencedParams rhs
  match expr with
  | .param name => [name]
  | .add lhs rhs | .sub lhs rhs | .mul lhs rhs | .div lhs rhs
  | .eq lhs rhs | .ne lhs rhs | .lt lhs rhs | .le lhs rhs
  | .gt lhs rhs | .ge lhs rhs | .and lhs rhs | .or lhs rhs => binary lhs rhs
  | .not inner => referencedParams inner
  | .input _ (.mk op filter) => referencedAggOp op ++ filter.toList.bind referencedParams
  | .agg op _ _ _ filter => referencedAggOp op ++ referencedParams filter
  | _ => []

private partial def referencedAggOp : AggOp → List String
  | .count => []
  | .sum value => referencedParams value
end

private def unique (values : List String) : List String :=
  values.foldl (fun acc value => if acc.contains value then acc else acc ++ [value]) []

private def findBox? (model : Model) (boxName : String) : Option Box :=
  model.boxes.find? (·.name == boxName)

private def findTransition? (modelBox : Box) (name : String) : Option Transition :=
  modelBox.transitions.find? (·.name == name)

private def enumSource? (attr : String) : Expr → Option String
  | .enumIs candidate variant => if candidate == attr then some variant else none
  | .and lhs rhs => enumSource? attr lhs <|> enumSource? attr rhs
  | _ => none

private def enumTarget? (attr : String) : List Effect → Option String
  | [] => none
  | .setAttr candidate (.enum variant) :: rest =>
      if candidate == attr then some variant else enumTarget? attr rest
  | _ :: rest => enumTarget? attr rest

/-- Build the state-machine graph for one elaborated system/table. -/
def stateDiagramProps? (model : Model) (boxName tableName : String) : Option StateDiagramProps := do
  let modelBox ← findBox? model boxName
  let table ← modelBox.tables.find? (·.name == tableName)
  let enumAttrs := table.attrs.filterMap fun attr =>
    match attr.ty with
    | .enum variants => some (attr.name, variants)
    | _ => none
  let nodes := unique (enumAttrs.bind (·.2)) |>.map fun id => ({ id := id } : StateNode)
  let edges := modelBox.transitions.filterMap fun transition =>
    if transition.table != tableName then none else
    enumAttrs.findSome? fun (attr, _) => do
      let source ← enumSource? attr transition.guard
      let target ← enumTarget? attr transition.effects
      pure { name := transition.name, source, target, hazard := prettyExpr transition.hazard }
  pure { system := tableName, nodes, edges }

private partial def evalClosed (params : List ParamDecl) : Expr → Option Float
  | .real value => some (scientificToFloat value)
  | .int value => some (Float.ofInt value)
  | .param name => params.find? (·.name == name) >>= paramDefault?
  | .add lhs rhs => return (← evalClosed params lhs) + (← evalClosed params rhs)
  | .sub lhs rhs => return (← evalClosed params lhs) - (← evalClosed params rhs)
  | .mul lhs rhs => return (← evalClosed params lhs) * (← evalClosed params rhs)
  | .div lhs rhs => return (← evalClosed params lhs) / (← evalClosed params rhs)
  | _ => none

private def sample (count : Nat) (lo hi : Float) (fn : Float → Float) : List PlotPoint :=
  (List.range count).map fun index =>
    let fraction := if count ≤ 1 then 0.0 else index.toFloat / (count - 1).toFloat
    let x := lo + (hi - lo) * fraction
    { x, y := fn x }

private def priorDensity? (prior : Prior) : Option DensityCurve := do
  let [first, second] := prior.args | none
  let a := scientificToFloat first
  let b := scientificToFloat second
  match prior.family with
  | .normal =>
      if b ≤ 0.0 then none else
      let normalizer := b * Float.sqrt (2.0 * 3.141592653589793)
      let density x := Float.exp (-0.5 * Float.pow ((x - a) / b) 2.0) / normalizer
      some { family := "Normal", points := sample 41 (a - 4.0 * b) (a + 4.0 * b) density }
  | .logNormal =>
      if b ≤ 0.0 then none else
      let lo := Float.exp (a - 4.0 * b)
      let hi := Float.exp (a + 4.0 * b)
      let normalizer := b * Float.sqrt (2.0 * 3.141592653589793)
      let density x :=
        if x ≤ 0.0 then 0.0
        else Float.exp (-0.5 * Float.pow ((Float.log x - a) / b) 2.0) / (x * normalizer)
      some { family := "LogNormal", points := sample 41 lo hi density }
  | .uniform =>
      if a >= b then none else
      some { family := "Uniform", points := sample 41 a b (fun _ => 1.0 / (b - a)) }

private def parameterSummary (param : ParamDecl) : Option ParamSummary := do
  let defaultValue ← paramDefault? param
  pure { name := param.name, defaultValue, density := param.prior >>= priorDensity? }

/-- Build guard, hazard, parameter/prior, and firing-probability props for a transition. -/
def hazardPanelProps? (model : Model) (boxName transitionName : String) : Option HazardPanelProps := do
  let modelBox ← findBox? model boxName
  let transition ← findTransition? modelBox transitionName
  let references := unique (referencedParams transition.guard ++ referencedParams transition.hazard)
  let summaries := model.params.filterMap fun param =>
    if references.contains param.name then parameterSummary param else none
  let probability := evalClosed model.params transition.hazard |>.map fun lambda =>
    let dtMax := scientificToFloat model.dt * 8.0
    sample 41 0.0 dtMax fun dt => 1.0 - Float.exp (-lambda * dt)
  let reason := if probability.isSome then none else
    some "Per-tick probability plot unavailable: hazard depends on row state, inputs, or aggregates."
  pure {
    transition := transition.name
    guard := prettyExpr transition.guard
    hazard := prettyExpr transition.hazard
    params := summaries
    probability
    noProbabilityReason := reason
  }

end Sembla.Widgets
