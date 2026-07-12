import Sembla.Models
import Sembla.Widgets

namespace Sembla.WidgetTests

open Lean Sembla.IR Sembla.Models Sembla.Widgets

private def close (lhs rhs : Float) (epsilon := 1e-9) : Bool :=
  Float.abs (lhs - rhs) < epsilon

private def monotone : List PlotPoint → Bool
  | [] | [_] => true
  | first :: second :: rest => first.y ≤ second.y && monotone (second :: rest)

private def sirDiagram : StateDiagramProps :=
  (stateDiagramProps? sir "sir" "person").get!

#guard sirDiagram.nodes == [{ id := "S" }, { id := "I" }, { id := "R" }]
#guard sirDiagram.edges.map (fun edge => (edge.name, edge.source, edge.target, edge.hazard)) == [
  ("infect", "S", "I",
    "(beta * (countBy person.employer = self.employer where health = I / countBy person.employer = self.employer where true))"),
  ("recover", "I", "R", "gamma")
]

private def recoverPanel : HazardPanelProps :=
  (hazardPanelProps? sir "sir" "recover").get!

#guard recoverPanel.guard == "health = I"
#guard recoverPanel.hazard == "gamma"
#guard recoverPanel.params.length == 1
#guard recoverPanel.params.head!.name == "gamma"
#guard close recoverPanel.params.head!.defaultValue 0.1
#guard recoverPanel.probability.isSome
#guard recoverPanel.probability.get!.length > 10
#guard close recoverPanel.probability.get!.head!.x 0.0
#guard close recoverPanel.probability.get!.head!.y 0.0
#guard monotone recoverPanel.probability.get!

private def infectPanel : HazardPanelProps :=
  (hazardPanelProps? sir "sir" "infect").get!

#guard infectPanel.probability.isNone
#guard infectPanel.noProbabilityReason == some
  "Per-tick probability plot unavailable: hazard depends on row state, inputs, or aggregates."
#guard infectPanel.params.length == 1
#guard infectPanel.params.head!.name == "beta"
#guard infectPanel.params.head!.density.isSome
#guard infectPanel.params.head!.density.get!.family == "LogNormal"
#guard infectPanel.params.head!.density.get!.points.length == 41

private def logNormalDensity (mu sigma x : Float) : Float :=
  Float.exp (-0.5 * Float.pow ((Float.log x - mu) / sigma) 2.0) /
    (x * sigma * Float.sqrt (2.0 * 3.141592653589793))

private def betaDensityPoints := infectPanel.params.head!.density.get!.points
private def betaMu := -0.2231435513142097
private def betaSigma := 0.25

#guard close betaDensityPoints[0]!.y (logNormalDensity betaMu betaSigma betaDensityPoints[0]!.x)
#guard close betaDensityPoints[20]!.y (logNormalDensity betaMu betaSigma betaDensityPoints[20]!.x)
#guard close betaDensityPoints[40]!.y (logNormalDensity betaMu betaSigma betaDensityPoints[40]!.x)

private def withoutGammaPrior (declaration : ParamDecl) : ParamDecl :=
  if declaration.name == "gamma" then
    ParamDecl.mk declaration.name declaration.ty declaration.default none
  else declaration

private def priorlessModel : Model :=
  Model.mk sir.name sir.dt (sir.params.map withoutGammaPrior) sir.boxes sir.wires

private def priorlessRecover : HazardPanelProps :=
  (hazardPanelProps? priorlessModel "sir" "recover").get!

#guard priorlessRecover.params.length == 1
#guard close priorlessRecover.params.head!.defaultValue 0.1
#guard priorlessRecover.params.head!.density.isNone

-- JSON encoding is part of the pure builder contract consumed by the display layer.
private def diagramJson := toJson sirDiagram
private def jsonFieldEq (document : Json) (name : String) (expected : Json) : Bool :=
  match document.getObjVal? name with
  | .ok actual => actual == expected
  | .error _ => false

#guard jsonFieldEq diagramJson "nodes" (json% [
  { "id": "S" }, { "id": "I" }, { "id": "R" }
])
#guard jsonFieldEq diagramJson "edges" (json% [
  { "name": "infect", "source": "S", "target": "I",
    "hazard": "(beta * (countBy person.employer = self.employer where health = I / countBy person.employer = self.employer where true))" },
  { "name": "recover", "source": "I", "target": "R", "hazard": "gamma" }
])
#guard ((toJson recoverPanel).getObjVal? "probability").isOk

end Sembla.WidgetTests
