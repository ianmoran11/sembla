import Sembla.Models
import Sembla.Widgets
import Sembla.WidgetDisplay

namespace Sembla.WidgetTests

open Lean ProofWidgets Sembla.IR Sembla.Models Sembla.Widgets Sembla.WidgetDisplay

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

-- React requires its `style` prop to be a JSON object rather than a CSS string.
private def stylesAreObjects : Nat → Html → Bool
  | 0, _ => false
  | _ + 1, .text _ => true
  | fuel + 1, .component _ _ _ children => children.all (stylesAreObjects fuel)
  | fuel + 1, .element _ attributes children =>
      attributes.all (fun (name, value) =>
        name != "style" || match value with
          | .obj _ => true
          | _ => false) &&
      children.all (stylesAreObjects fuel)

#guard stylesAreObjects 20 (stateDiagramHtml sirDiagram)
#guard stylesAreObjects 20 (hazardPanelHtml recoverPanel)
#guard stylesAreObjects 20 (hazardPanelHtml infectPanel)

private def htmlTexts : Nat → Bool → Html → List String
  | 0, _, _ => []
  | _ + 1, insideSvg, .text value => if insideSvg then [value] else []
  | fuel + 1, insideSvg, .component _ _ _ children =>
      children.toList.bind (htmlTexts fuel insideSvg)
  | fuel + 1, insideSvg, .element tag _ children =>
      children.toList.bind (htmlTexts fuel (insideSvg || tag == "svg"))

private def allHtmlTexts : Nat → Html → List String
  | 0, _ => []
  | _ + 1, .text value => [value]
  | fuel + 1, .component _ _ _ children => children.toList.bind (allHtmlTexts fuel)
  | fuel + 1, .element _ _ children => children.toList.bind (allHtmlTexts fuel)

private def diagramSvgTexts := htmlTexts 20 false (stateDiagramHtml sirDiagram)

-- SVG labels stay compact; full hazard expressions are rendered in wrapping HTML below the graph.
#guard diagramSvgTexts.contains "infect"
#guard diagramSvgTexts.contains "recover"
#guard !diagramSvgTexts.contains sirDiagram.edges.head!.hazard
#guard (allHtmlTexts 20 (stateDiagramHtml sirDiagram)).contains sirDiagram.edges.head!.hazard

-- Parameter defaults are presented without Float.toString's trailing zero padding.
#guard (allHtmlTexts 20 (hazardPanelHtml infectPanel)).contains "0.8"
#guard !(allHtmlTexts 20 (hazardPanelHtml infectPanel)).contains "0.800000"

private def sirDiagramTexts := allHtmlTexts 20 (stateDiagramHtml sirDiagram)
private def recoverPanelTexts := allHtmlTexts 20 (hazardPanelHtml recoverPanel)
private def priorlessPanelTexts := allHtmlTexts 20 (hazardPanelHtml priorlessRecover)
private def noParamPanel := HazardPanelProps.mk "constant" "true" "0.1" [] none none
private def noParamPanelTexts := allHtmlTexts 20 (hazardPanelHtml noParamPanel)

-- Summary badges and explicit empty states make the panels useful at a glance.
#guard sirDiagramTexts.contains "3 states · 2 transitions"
#guard sirDiagramTexts.contains "Rate"
#guard recoverPanelTexts.contains "1 parameter"
#guard recoverPanelTexts.contains "1 referenced"
#guard priorlessPanelTexts.contains "No prior specified"
#guard noParamPanelTexts.contains "This transition references no parameters."

private def attrsForTag : Nat → String → Html → List (Array (String × Json))
  | 0, _, _ => []
  | _ + 1, _, .text _ => []
  | fuel + 1, tag, .component _ _ _ children =>
      children.toList.bind (attrsForTag fuel tag)
  | fuel + 1, tag, .element actual attributes children =>
      let nested := children.toList.bind (attrsForTag fuel tag)
      if actual == tag then attributes :: nested else nested

private def attrJson? (attributes : Array (String × Json)) (name : String) : Option Json :=
  attributes.find? (·.1 == name) |>.map (·.2)

private def attrString? (attributes : Array (String × Json)) (name : String) : Option String :=
  match attrJson? attributes name with
  | some (.str value) => some value
  | _ => none

private def styleString? (attributes : Array (String × Json)) (name : String) : Option String :=
  match attrJson? attributes "style" with
  | some style => match style.getObjVal? name with
    | .ok (.str value) => some value
    | _ => none
  | none => none

private def rootStyle? (html : Html) (name : String) : Option String :=
  (attrsForTag 20 "section" html).head? >>= fun attributes => styleString? attributes name

private def rootTheme? (html : Html) : Option String :=
  (attrsForTag 20 "section" html).head? >>= fun attributes => attrString? attributes "data-sembla-theme"

private def academicThemeHtml := stateDiagramHtmlWithTheme .academic sirDiagram
private def editorThemeHtml := stateDiagramHtmlWithTheme .editor sirDiagram
private def notebookThemeHtml := stateDiagramHtmlWithTheme .notebook sirDiagram

-- Presets remain pure render choices and expose distinct geometry/color tokens.
#guard rootTheme? academicThemeHtml == some "academic"
#guard rootTheme? editorThemeHtml == some "editor"
#guard rootTheme? notebookThemeHtml == some "notebook"
#guard rootStyle? academicThemeHtml "--sembla-shell-radius" == some "4px"
#guard rootStyle? editorThemeHtml "--sembla-shell-radius" == some "8px"
#guard rootStyle? notebookThemeHtml "--sembla-shell-radius" == some "12px"
#guard WidgetTheme.ofName "professional" == .academic

private def responsiveSvgs : Nat → Html → Bool
  | 0, _ => false
  | _ + 1, .text _ => true
  | fuel + 1, .component _ _ _ children => children.all (responsiveSvgs fuel)
  | fuel + 1, .element tag attributes children =>
      let responsive := if tag == "svg" then
        (attrJson? attributes "height").isNone &&
        styleString? attributes "width" == some "100%" &&
        styleString? attributes "height" == some "auto" &&
        styleString? attributes "display" == some "block"
      else true
      responsive && children.all (responsiveSvgs fuel)

private def routingDiagram : StateDiagramProps :=
  StateDiagramProps.mk "policy mode"
    [StateNode.mk "Open", StateNode.mk "Restricted"]
    [ StateEdge.mk "restrict" "Open" "Restricted" "threshold",
      StateEdge.mk "reopen" "Restricted" "Open" "cooldown",
      StateEdge.mk "remain" "Restricted" "Restricted" "persistence" ]

private def routingHtml := stateDiagramHtml routingDiagram
private def routingPaths := (attrsForTag 20 "path" routingHtml).filterMap (attrString? · "d")
private def routingMarkerIds := (attrsForTag 20 "marker" routingHtml).filterMap
  (attrString? · "id")
private def sirMarkerIds := (attrsForTag 20 "marker" (stateDiagramHtml sirDiagram)).filterMap
  (attrString? · "id")
private def routingAria := (attrsForTag 20 "svg" routingHtml).filterMap
  (attrString? · "aria-label")
private def routingNodeStrokes := (attrsForTag 20 "rect" routingHtml).filterMap
  (attrString? · "stroke")

-- Opposing transitions use separate quadratic arcs, self-transitions use cubic loops,
-- and long labels render inside rounded rectangles rather than fixed circles.
#guard routingPaths.any (fun path => (path.splitOn " Q ").length > 1)
#guard routingPaths.any (fun path => (path.splitOn " C ").length > 1)
#guard (attrsForTag 20 "rect" routingHtml).length == 2
#guard routingNodeStrokes.length == 2
#guard routingNodeStrokes.head? != routingNodeStrokes.getLast?
#guard (htmlTexts 20 false routingHtml).contains "Restricted"
#guard routingMarkerIds.length == 1
#guard routingMarkerIds != sirMarkerIds
#guard routingAria.any (fun label =>
  (label.splitOn "states Open, Restricted").length > 1 &&
  (label.splitOn "remain: Restricted to Restricted").length > 1)

private def compactChartPanel : HazardPanelProps :=
  HazardPanelProps.mk "compact" "true" "rate" []
    (some [PlotPoint.mk 0.0 0.0, PlotPoint.mk 1.234567 1.234567]) none

private def compactChartHtml := hazardPanelHtml compactChartPanel
private def compactChartTexts := allHtmlTexts 20 compactChartHtml
private def compactChartAria := (attrsForTag 20 "svg" compactChartHtml).filterMap
  (attrString? · "aria-label")

-- SVG dimensions follow their viewBox, axes use rounded "nice" ticks, and metadata is exposed.
#guard responsiveSvgs 20 routingHtml
#guard responsiveSvgs 20 compactChartHtml
#guard compactChartTexts.contains "0.5"
#guard compactChartTexts.contains "1.5"
#guard !compactChartTexts.contains "1.23"
#guard !compactChartTexts.contains "1.234567"
#guard (compactChartTexts.filter (· == "dt")).length == 1
#guard (htmlTexts 20 false compactChartHtml).contains "p(dt)"
#guard (attrsForTag 20 "circle" compactChartHtml).length == 1
#guard compactChartAria.any (fun label =>
  (label.splitOn "dt domain 0 to 1.5").length > 1 &&
  (label.splitOn "p(dt) range 0 to 1.5").length > 1)

end Sembla.WidgetTests
