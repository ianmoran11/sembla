import ProofWidgets.Component.HtmlDisplay
import Sembla.Widgets

/-! Thin ProofWidgets display/registration layer for the pure props in
`Sembla.Widgets`.  No model execution or simulation is reachable here. -/
namespace Sembla.WidgetDisplay

open Lean Lean.Server ProofWidgets Sembla.Widgets

private def attrs (values : List (String × String)) : Array (String × Json) :=
  values.toArray.map fun (name, value) => (name, .str value)

private def element (tag : String) (attributes : List (String × String) := [])
    (children : Array Html := #[]) : Html :=
  .element tag (attrs attributes) children

private def textElement (tag text : String) (attributes : List (String × String) := []) : Html :=
  element tag attributes #[.text text]

private def pointString (point : PlotPoint) : String :=
  s!"{point.x},{120.0 - point.y * 100.0}"

private def plotSvg (title : String) (points : List PlotPoint) : Html :=
  let xMin := points.head?.map (·.x) |>.getD 0.0
  let xMax := points.getLast?.map (·.x) |>.getD 1.0
  let yMax := points.foldl (fun value point => if point.y > value then point.y else value) 0.0
  let xSpan := if xMax == xMin then 1.0 else xMax - xMin
  let ySpan := if yMax ≤ 0.0 then 1.0 else yMax
  let scaled := points.map fun point =>
    { x := 12.0 + 296.0 * (point.x - xMin) / xSpan
      y := point.y / ySpan } 
  let polyline := scaled.map pointString |> String.intercalate " "
  element "div" [("style", "margin: 8px 0") ] #[
    textElement "div" title [("style", "font-weight: 600; margin-bottom: 3px")],
    element "svg" [("viewBox", "0 0 320 132"), ("width", "100%"), ("height", "132") ] #[
      element "line" [("x1", "12"), ("y1", "120"), ("x2", "310"), ("y2", "120"),
        ("stroke", "currentColor"), ("stroke-width", "1")],
      element "line" [("x1", "12"), ("y1", "8"), ("x2", "12"), ("y2", "120"),
        ("stroke", "currentColor"), ("stroke-width", "1")],
      element "polyline" [("points", polyline), ("fill", "none"),
        ("stroke", "#4f7cac"), ("stroke-width", "2")]
    ]
  ]

/-- Render state-machine props as a compact SVG graph. -/
def stateDiagramHtml (props : StateDiagramProps) : Html :=
  let count := props.nodes.length
  let positions := props.nodes.enum.map fun (index, node) =>
    let x := if count ≤ 1 then 260.0 else 55.0 + index.toFloat * 410.0 / (count - 1).toFloat
    (node.id, x)
  let position (name : String) := positions.find? (·.1 == name) |>.map (·.2) |>.getD 55.0
  let edges := props.edges.bind fun edge =>
    let sourceX := position edge.source
    let targetX := position edge.target
    let direction : Float := if targetX > sourceX then 1.0 else -1.0
    let nodeRadius := 25.0
    let sourceEdgeX := sourceX + direction * nodeRadius
    -- Keep the marker tip just beyond the circle stroke so later-rendered nodes
    -- cannot obscure the graph's direction.
    let targetEdgeX := targetX - direction * (nodeRadius + 2.0)
    let midpoint := (sourceX + targetX) / 2.0
    [ element "line" [("x1", toString sourceEdgeX), ("y1", "88"),
        ("x2", toString targetEdgeX), ("y2", "88"), ("stroke", "#4f7cac"),
        ("stroke-width", "2"), ("marker-end", "url(#arrow)")],
      textElement "text" s!"{edge.name}: {edge.hazard}"
        [("x", toString midpoint), ("y", "65"), ("text-anchor", "middle"),
          ("font-size", "11"), ("fill", "currentColor")] ]
  let nodes := props.nodes.bind fun node =>
    let x := position node.id
    [ element "circle" [("cx", toString x), ("cy", "88"), ("r", "25"),
        ("fill", "#eef4fb"), ("stroke", "#4f7cac"), ("stroke-width", "2")],
      textElement "text" node.id [("x", toString x), ("y", "93"),
        ("text-anchor", "middle"), ("font-size", "15"), ("fill", "#17202a")] ]
  element "section" [("style", "padding: 8px") ] #[
    textElement "h3" s!"State diagram — {props.system}" [("style", "margin: 0 0 6px")],
    element "svg" [("viewBox", "0 0 520 145"), ("width", "100%"), ("height", "145")]
      (#[] ++ #[element "defs" [] #[
        element "marker" [("id", "arrow"), ("viewBox", "0 0 10 10"),
          ("refX", "9"), ("refY", "5"), ("markerWidth", "6"), ("markerHeight", "6"),
          ("orient", "auto-start-reverse")] #[
            element "path" [("d", "M 0 0 L 10 5 L 0 10 z"), ("fill", "#4f7cac")]
          ]
      ]] ++ edges.toArray ++ nodes.toArray)
  ]

private def paramHtml (param : ParamSummary) : Html :=
  let density := match param.density with
    | none => #[]
    | some curve => #[plotSvg s!"{curve.family} prior density" curve.points]
  element "div" [("style", "border-left: 3px solid #4f7cac; padding-left: 8px; margin: 8px 0")]
    (#[textElement "div" s!"{param.name} = {param.defaultValue}"] ++ density)

/-- Render hazard props, prior density curves, and an optional p(dt) curve. -/
def hazardPanelHtml (props : HazardPanelProps) : Html :=
  let probability := match props.probability, props.noProbabilityReason with
    | some points, _ => #[plotSvg "Per-tick firing probability p(dt)" points]
    | none, some reason => #[textElement "p" reason [("style", "font-style: italic")]]
    | none, none => #[]
  element "section" [("style", "padding: 8px") ]
    (#[ textElement "h3" s!"Hazard — {props.transition}" [("style", "margin: 0 0 6px")],
        textElement "div" s!"guard: {props.guard}" [("style", "font-family: monospace")],
        textElement "div" s!"hazard: {props.hazard}" [("style", "font-family: monospace")]
      ] ++ (props.params.map paramHtml).toArray ++ probability)

private def saveHtmlPanel (html : Html) (stx : Syntax) : CoreM Unit :=
  Widget.savePanelWidgetInfo
    (hash HtmlDisplayPanel.javascript)
    (return json% { html: $(← rpcEncode html) })
    stx

/-- Attach the state diagram to the source range of a system or transition. -/
def saveStateDiagram (props : StateDiagramProps) (stx : Syntax) : CoreM Unit :=
  saveHtmlPanel (stateDiagramHtml props) stx

/-- Attach the hazard panel to the source range of a transition. -/
def saveHazardPanel (props : HazardPanelProps) (stx : Syntax) : CoreM Unit :=
  saveHtmlPanel (hazardPanelHtml props) stx

end Sembla.WidgetDisplay
