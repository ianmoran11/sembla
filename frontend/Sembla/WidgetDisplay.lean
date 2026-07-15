import ProofWidgets.Component.HtmlDisplay
import Sembla.Widgets

/-! Thin ProofWidgets display/registration layer for the pure props in
`Sembla.Widgets`.  No model execution or simulation is reachable here. -/
namespace Sembla.WidgetDisplay

open Lean Lean.Server ProofWidgets Sembla.Widgets

/-- Visual presets for structure widgets. All presets still inherit the active VS Code theme. -/
inductive WidgetTheme where
  | editor
  | academic
  | notebook
deriving BEq, Repr

def WidgetTheme.name : WidgetTheme → String
  | .editor => "editor"
  | .academic => "academic"
  | .notebook => "notebook"

def WidgetTheme.ofName : String → WidgetTheme
  | "editor" => .editor
  | "notebook" => .notebook
  | "professional" | "academic" => .academic
  | _ => .academic

register_option sembla.widget.theme : String := {
  defValue := "academic"
  descr := "Sembla widget theme: academic, editor, or notebook"
}

private structure ThemeTokens where
  shellBackground : String
  surfaceBackground : String
  codeBackground : String
  border : String
  foreground : String
  muted : String
  headingFont : String
  bodyFont : String
  codeFont : String
  shellRadius : String
  cardRadius : String
  pillRadius : String
  badgeBackground : String
  badgeForeground : String
  stateOne : String
  stateTwo : String
  stateThree : String
  stateFour : String
  stateFive : String
  prior : String
  probability : String

private def themeTokens : WidgetTheme → ThemeTokens
  | .academic => {
      shellBackground := "color-mix(in srgb, var(--vscode-editor-background, #f8f8f5) 97%, var(--vscode-charts-blue, #315d75) 3%)"
      surfaceBackground := "var(--vscode-editor-background, #fbfbf8)"
      codeBackground := "color-mix(in srgb, var(--vscode-editor-background, #fbfbf8) 94%, var(--vscode-editor-foreground, #1f2933) 6%)"
      border := "var(--vscode-contrastBorder, color-mix(in srgb, var(--vscode-editor-foreground, #1f2933) 22%, transparent))"
      foreground := "var(--vscode-editor-foreground, #1f2933)"
      muted := "var(--vscode-descriptionForeground, #626b73)"
      headingFont := "Iowan Old Style, Palatino Linotype, Palatino, Georgia, serif"
      bodyFont := "var(--vscode-font-family, -apple-system, BlinkMacSystemFont, sans-serif)"
      codeFont := "var(--vscode-editor-font-family, ui-monospace, monospace)"
      shellRadius := "4px"
      cardRadius := "3px"
      pillRadius := "2px"
      badgeBackground := "color-mix(in srgb, var(--vscode-charts-blue, #315d75) 14%, transparent)"
      badgeForeground := "var(--vscode-charts-blue, #315d75)"
      stateOne := "var(--vscode-charts-blue, #315d75)"
      stateTwo := "var(--vscode-charts-orange, #9a6435)"
      stateThree := "var(--vscode-charts-green, #466d5b)"
      stateFour := "var(--vscode-charts-purple, #685b78)"
      stateFive := "var(--vscode-charts-red, #8a4f4f)"
      prior := "var(--vscode-charts-purple, #685b78)"
      probability := "var(--vscode-charts-green, #466d5b)"
    }
  | .editor => {
      shellBackground := "var(--vscode-editorWidget-background, var(--vscode-editor-background, #1e1e1e))"
      surfaceBackground := "var(--vscode-editor-background, #1e1e1e)"
      codeBackground := "var(--vscode-textCodeBlock-background, var(--vscode-editor-background, #1e1e1e))"
      border := "var(--vscode-editorWidget-border, var(--vscode-panel-border, #555))"
      foreground := "var(--vscode-editor-foreground, currentColor)"
      muted := "var(--vscode-descriptionForeground, #888)"
      headingFont := "var(--vscode-font-family, sans-serif)"
      bodyFont := "var(--vscode-font-family, sans-serif)"
      codeFont := "var(--vscode-editor-font-family, monospace)"
      shellRadius := "8px"
      cardRadius := "6px"
      pillRadius := "999px"
      badgeBackground := "var(--vscode-badge-background, #4f6b88)"
      badgeForeground := "var(--vscode-badge-foreground, #fff)"
      stateOne := "var(--vscode-charts-blue, #5b8def)"
      stateTwo := "var(--vscode-charts-orange, #d99a2b)"
      stateThree := "var(--vscode-charts-green, #4caf7a)"
      stateFour := "var(--vscode-charts-purple, #9b72cf)"
      stateFive := "var(--vscode-charts-red, #d66a6a)"
      prior := "var(--vscode-charts-purple, #9b72cf)"
      probability := "var(--vscode-charts-green, #4caf7a)"
    }
  | .notebook => {
      shellBackground := "color-mix(in srgb, var(--vscode-editor-background, #fffaf0) 95%, var(--vscode-charts-yellow, #d7ba7d) 5%)"
      surfaceBackground := "var(--vscode-editor-background, #fffdf8)"
      codeBackground := "color-mix(in srgb, var(--vscode-editor-background, #fffdf8) 92%, var(--vscode-charts-yellow, #d7ba7d) 8%)"
      border := "var(--vscode-contrastBorder, color-mix(in srgb, var(--vscode-editor-foreground, #332f2a) 18%, transparent))"
      foreground := "var(--vscode-editor-foreground, #332f2a)"
      muted := "var(--vscode-descriptionForeground, #746e65)"
      headingFont := "var(--vscode-font-family, -apple-system, BlinkMacSystemFont, sans-serif)"
      bodyFont := "var(--vscode-font-family, -apple-system, BlinkMacSystemFont, sans-serif)"
      codeFont := "var(--vscode-editor-font-family, ui-monospace, monospace)"
      shellRadius := "12px"
      cardRadius := "8px"
      pillRadius := "999px"
      badgeBackground := "color-mix(in srgb, var(--vscode-charts-yellow, #d7ba7d) 28%, transparent)"
      badgeForeground := "var(--vscode-editor-foreground, #332f2a)"
      stateOne := "var(--vscode-charts-blue, #4d7ea8)"
      stateTwo := "var(--vscode-charts-orange, #c17c3a)"
      stateThree := "var(--vscode-charts-green, #5b8b6f)"
      stateFour := "var(--vscode-charts-purple, #8b6fa8)"
      stateFive := "var(--vscode-charts-red, #b65f5f)"
      prior := "var(--vscode-charts-purple, #8b6fa8)"
      probability := "var(--vscode-charts-blue, #4d7ea8)"
    }

private def attrs (values : List (String × String)) : Array (String × Json) :=
  values.toArray.map fun (name, value) => (name, .str value)

private def element (tag : String) (attributes : List (String × String) := [])
    (children : Array Html := #[]) : Html :=
  .element tag (attrs attributes) children

private def textElement (tag text : String) (attributes : List (String × String) := []) : Html :=
  element tag attributes #[.text text]

private def styleJson (values : List (String × String)) : Json :=
  Json.mkObj (values.map fun (name, value) => (name, .str value))

private def styledElement (tag : String) (styles : List (String × String))
    (children : Array Html := #[]) : Html :=
  .element tag #[("style", styleJson styles)] children

private def styledElementWithAttrs (tag : String) (attributes styles : List (String × String))
    (children : Array Html := #[]) : Html :=
  .element tag (attrs attributes ++ #[("style", styleJson styles)]) children

private def styledTextElement (tag text : String) (styles : List (String × String)) : Html :=
  styledElement tag styles #[.text text]

private def widgetShell (theme : WidgetTheme) (children : Array Html) : Html :=
  let tokens := themeTokens theme
  styledElementWithAttrs "section" [("data-sembla-theme", theme.name)] [
    ("--sembla-shell-bg", tokens.shellBackground),
    ("--sembla-surface", tokens.surfaceBackground),
    ("--sembla-code-bg", tokens.codeBackground),
    ("--sembla-border", tokens.border),
    ("--sembla-fg", tokens.foreground),
    ("--sembla-muted", tokens.muted),
    ("--sembla-heading-font", tokens.headingFont),
    ("--sembla-body-font", tokens.bodyFont),
    ("--sembla-code-font", tokens.codeFont),
    ("--sembla-shell-radius", tokens.shellRadius),
    ("--sembla-card-radius", tokens.cardRadius),
    ("--sembla-pill-radius", tokens.pillRadius),
    ("--sembla-badge-bg", tokens.badgeBackground),
    ("--sembla-badge-fg", tokens.badgeForeground),
    ("--sembla-state-1", tokens.stateOne),
    ("--sembla-state-2", tokens.stateTwo),
    ("--sembla-state-3", tokens.stateThree),
    ("--sembla-state-4", tokens.stateFour),
    ("--sembla-state-5", tokens.stateFive),
    ("--sembla-prior", tokens.prior),
    ("--sembla-probability", tokens.probability),
    ("boxSizing", "border-box"),
    ("width", "100%"),
    ("padding", "11px"),
    ("border", "1px solid var(--sembla-border)"),
    ("borderRadius", "var(--sembla-shell-radius)"),
    ("backgroundColor", "var(--sembla-shell-bg)"),
    ("color", "var(--sembla-fg)"),
    ("fontFamily", "var(--sembla-body-font)"),
    ("fontSize", "12px"),
    ("lineHeight", "1.4")
  ] children

private def eyebrow (label : String) : Html :=
  styledTextElement "div" label [
    ("marginBottom", "2px"),
    ("color", "var(--sembla-muted)"),
    ("fontSize", "10px"),
    ("fontWeight", "700"),
    ("letterSpacing", "0.08em"),
    ("textTransform", "uppercase")
  ]

private def badge (label : String) : Html :=
  styledTextElement "span" label [
    ("display", "inline-block"),
    ("maxWidth", "100%"),
    ("padding", "2px 7px"),
    ("border", "1px solid var(--sembla-border)"),
    ("borderRadius", "var(--sembla-pill-radius)"),
    ("backgroundColor", "var(--sembla-badge-bg)"),
    ("color", "var(--sembla-badge-fg)"),
    ("fontSize", "10px"),
    ("fontWeight", "700"),
    ("lineHeight", "1.4"),
    ("overflowWrap", "anywhere")
  ]

private def widgetHeader (kind title context : String) : Html :=
  styledElement "div" [
    ("display", "flex"),
    ("alignItems", "center"),
    ("justifyContent", "space-between"),
    ("flexWrap", "wrap"),
    ("gap", "6px 8px"),
    ("marginBottom", "10px")
  ] #[
    styledElement "div" [("flex", "1 1 150px"), ("minWidth", "0")] #[
      eyebrow kind,
      styledTextElement "h3" title [
        ("margin", "0"),
        ("color", "var(--sembla-fg)"),
        ("fontFamily", "var(--sembla-heading-font)"),
        ("fontSize", "15px"),
        ("fontWeight", "650"),
        ("lineHeight", "1.25"),
        ("overflowWrap", "anywhere")
      ]
    ],
    badge context
  ]

private def countLabel (count : Nat) (singular plural : String) : String :=
  s!"{count} {if count == 1 then singular else plural}"

private def mutedPill (label : String) : Html :=
  styledTextElement "span" label [
    ("display", "inline-block"),
    ("maxWidth", "100%"),
    ("padding", "1px 6px"),
    ("border", "1px solid var(--sembla-border)"),
    ("borderRadius", "var(--sembla-pill-radius)"),
    ("color", "var(--sembla-muted)"),
    ("fontSize", "9px"),
    ("fontWeight", "650"),
    ("letterSpacing", "0.03em"),
    ("lineHeight", "1.5"),
    ("overflowWrap", "anywhere")
  ]

private def sectionHeading (title context : String) : Html :=
  styledElement "div" [
    ("display", "flex"),
    ("alignItems", "center"),
    ("justifyContent", "space-between"),
    ("flexWrap", "wrap"),
    ("gap", "5px 8px"),
    ("marginTop", "10px"),
    ("marginBottom", "5px")
  ] #[
    styledTextElement "div" title [
      ("fontFamily", "var(--sembla-heading-font)"),
      ("fontSize", "11px"),
      ("fontWeight", "700"),
      ("letterSpacing", "0.02em"),
      ("overflowWrap", "anywhere")
    ],
    mutedPill context
  ]

private def trimDecimalZeros (value : String) : String :=
  if !value.toList.contains '.' then value
  else
    let reversed := value.toList.reverse.dropWhile (· == '0')
    let trimmed := match reversed with
      | '.' :: rest => rest
      | _ => reversed
    String.mk trimmed.reverse

private def formatFloat (value : Float) : String :=
  trimDecimalZeros (toString value)

private def roundedAt (value scale : Float) : Float :=
  if value < 0.0 then -(((-value * scale) + 0.5).floor / scale)
  else ((value * scale) + 0.5).floor / scale

private def formatTick (value : Float) : String :=
  let magnitude := Float.abs value
  let rounded :=
    if magnitude >= 100000.0 then roundedAt value 0.001
    else if magnitude >= 10000.0 then roundedAt value 0.01
    else if magnitude >= 1000.0 then roundedAt value 0.1
    else if magnitude >= 100.0 then roundedAt value 1.0
    else if magnitude >= 10.0 then roundedAt value 10.0
    else if magnitude >= 1.0 then roundedAt value 100.0
    else if magnitude >= 0.1 then roundedAt value 1000.0
    else if magnitude >= 0.01 then roundedAt value 10000.0
    else if magnitude >= 0.001 then roundedAt value 100000.0
    else value
  formatFloat rounded

private structure AxisScale where
  lower : Float
  upper : Float
  step : Float

private def niceStep (span : Float) : Float :=
  if span ≤ 0.0 then 1.0
  else
    let rough := span / 3.0
    let power := (Float.log rough / Float.log 10.0).floor
    let magnitude := Float.pow 10.0 power
    let fraction := rough / magnitude
    let niceFraction :=
      if fraction ≤ 1.0 then 1.0
      else if fraction ≤ 2.0 then 2.0
      else if fraction ≤ 2.5 then 2.5
      else if fraction ≤ 5.0 then 5.0
      else 10.0
    niceFraction * magnitude

private def niceAxis (dataMin dataMax : Float) : AxisScale :=
  let span := if dataMax > dataMin then dataMax - dataMin else 1.0
  let step := niceStep span
  let lower := (dataMin / step).floor * step
  let upper := (dataMax / step).ceil * step
  if upper > lower then { lower, upper, step }
  else { lower, upper := lower + step, step }

private def axisTicks (scale : AxisScale) : List Float :=
  let rawCount := (((scale.upper - scale.lower) / scale.step) + 0.5).floor.toUInt64.toNat
  let count := Nat.min 8 rawCount
  (List.range (count + 1)).map fun index => scale.lower + index.toFloat * scale.step

private def chartX (axis : AxisScale) (value : Float) : Float :=
  46.0 + 264.0 * (value - axis.lower) / (axis.upper - axis.lower)

private def chartY (axis : AxisScale) (value : Float) : Float :=
  86.0 - 76.0 * (value - axis.lower) / (axis.upper - axis.lower)

private def chartPointString (xAxis yAxis : AxisScale) (point : PlotPoint) : String :=
  s!"{chartX xAxis point.x},{chartY yAxis point.y}"

private def plotSvg (title xLabel yLabel accent : String) (points : List PlotPoint) : Html :=
  let dataXMin := points.head?.map (·.x) |>.getD 0.0
  let dataXMax := points.getLast?.map (·.x) |>.getD 1.0
  let dataYMax := points.foldl (fun value point => if point.y > value then point.y else value) 0.0
  let xAxis := niceAxis dataXMin dataXMax
  let yAxis := niceAxis 0.0 dataYMax
  let polyline := points.map (chartPointString xAxis yAxis) |> String.intercalate " "
  let baselineY := chartY yAxis 0.0
  let area := s!"{chartX xAxis dataXMin},{baselineY} {polyline} {chartX xAxis dataXMax},{baselineY}"
  let peak := points.foldl (fun best point => if point.y > best.y then point else best)
    (points.head?.getD { x := dataXMin, y := 0.0 })
  let peakX := chartX xAxis peak.x
  let peakY := chartY yAxis peak.y
  let domain := s!"{formatTick xAxis.lower} to {formatTick xAxis.upper}"
  let range := s!"{formatTick yAxis.lower} to {formatTick yAxis.upper}"
  let yGrid := axisTicks yAxis |>.bind fun tick =>
    let y := chartY yAxis tick
    [ element "line" [("x1", "46"), ("y1", toString y), ("x2", "310"),
        ("y2", toString y), ("stroke", "var(--sembla-border)"),
        ("strokeWidth", "1"), ("strokeDasharray", "2 5"), ("strokeLinecap", "round")],
      element "line" [("x1", "42"), ("y1", toString y), ("x2", "46"),
        ("y2", toString y), ("stroke", "var(--sembla-muted)"), ("strokeWidth", "1"),
        ("strokeLinecap", "round")],
      textElement "text" (formatTick tick) [("x", "38"), ("y", toString (y + 3.5)),
        ("textAnchor", "end"), ("fontSize", "10"),
        ("fill", "var(--sembla-muted)")] ]
  let xMarks := axisTicks xAxis |>.bind fun tick =>
    let x := chartX xAxis tick
    [ element "line" [("x1", toString x), ("y1", "86"), ("x2", toString x),
        ("y2", "90"), ("stroke", "var(--sembla-muted)"), ("strokeWidth", "1"),
        ("strokeLinecap", "round")],
      textElement "text" (formatTick tick) [("x", toString x), ("y", "103"),
        ("textAnchor", "middle"), ("fontSize", "10"),
        ("fill", "var(--sembla-muted)")] ]
  styledElement "div" [("marginTop", "8px")] #[
    styledElement "div" [
      ("display", "flex"),
      ("alignItems", "center"),
      ("justifyContent", "space-between"),
      ("flexWrap", "wrap"),
      ("gap", "4px 8px"),
      ("marginBottom", "2px")
    ] #[
      styledTextElement "div" title [
        ("fontFamily", "var(--sembla-heading-font)"),
        ("fontSize", "12px"),
        ("fontWeight", "650"),
        ("overflowWrap", "anywhere")
      ],
      mutedPill yLabel
    ],
    styledElementWithAttrs "svg" [
      ("viewBox", "0 0 320 120"),
      ("role", "img"),
      ("aria-label", s!"{title}; {xLabel} domain {domain}; {yLabel} range {range}")
    ] [
      ("display", "block"),
      ("width", "100%"),
      ("height", "auto")
    ] (#[] ++ yGrid.toArray ++ xMarks.toArray ++ #[
      element "line" [("x1", "46"), ("y1", "86"), ("x2", "310"), ("y2", "86"),
        ("stroke", "var(--sembla-muted)"), ("strokeWidth", "1")],
      element "line" [("x1", "46"), ("y1", "10"), ("x2", "46"), ("y2", "86"),
        ("stroke", "var(--sembla-muted)"), ("strokeWidth", "1")],
      element "polygon" [("points", area),
        ("fill", accent), ("fillOpacity", "0.12")],
      element "polyline" [("points", polyline), ("fill", "none"),
        ("stroke", accent), ("strokeWidth", "2"),
        ("strokeLinejoin", "round"), ("strokeLinecap", "round")],
      element "circle" [("cx", toString peakX), ("cy", toString peakY), ("r", "2.5"),
        ("fill", "var(--sembla-surface)"),
        ("stroke", accent), ("strokeWidth", "1.5")],
      textElement "text" xLabel [("x", "178"), ("y", "117"),
        ("textAnchor", "middle"), ("fontSize", "12"),
        ("fill", "var(--sembla-muted)"), ("fontWeight", "600")],
      textElement "text" yLabel [("x", "12"), ("y", "48"),
        ("textAnchor", "middle"), ("fontSize", "10"), ("fontWeight", "600"),
        ("transform", "rotate(-90 12 48)"),
        ("fill", "var(--sembla-muted)")]
    ])
  ]

private def stateColor (index : Nat) : String :=
  match index % 5 with
  | 0 => "var(--sembla-state-1)"
  | 1 => "var(--sembla-state-2)"
  | 2 => "var(--sembla-state-3)"
  | 3 => "var(--sembla-state-4)"
  | _ => "var(--sembla-state-5)"

private def formulaBlock (label value : String) : Html :=
  styledElement "div" [
    ("display", "flex"),
    ("alignItems", "flex-start"),
    ("gap", "8px"),
    ("padding", "7px 8px"),
    ("border", "1px solid var(--sembla-border)"),
    ("borderRadius", "var(--sembla-card-radius)"),
    ("backgroundColor", "var(--sembla-code-bg)")
  ] #[
    styledTextElement "span" label [
      ("flex", "0 0 auto"),
      ("minWidth", "42px"),
      ("paddingTop", "1px"),
      ("color", "var(--sembla-muted)"),
      ("fontSize", "9px"),
      ("fontWeight", "700"),
      ("letterSpacing", "0.06em"),
      ("textTransform", "uppercase")
    ],
    styledTextElement "code" value [
      ("minWidth", "0"),
      ("color", "var(--sembla-fg)"),
      ("fontFamily", "var(--sembla-code-font)"),
      ("fontSize", "11px"),
      ("whiteSpace", "pre-wrap"),
      ("overflowWrap", "anywhere"),
      ("wordBreak", "break-word")
    ]
  ]

private def transitionDetail (edge : StateEdge) : Html :=
  styledElement "div" [
    ("padding", "8px"),
    ("border", "1px solid var(--sembla-border)"),
    ("borderRadius", "var(--sembla-card-radius)"),
    ("backgroundColor", "var(--sembla-surface)")
  ] #[
    styledElement "div" [
      ("display", "flex"),
      ("alignItems", "center"),
      ("flexWrap", "wrap"),
      ("gap", "5px 7px"),
      ("marginBottom", "5px"),
      ("minWidth", "0")
    ] #[
      styledTextElement "span" edge.name [
        ("minWidth", "0"), ("fontWeight", "700"), ("overflowWrap", "anywhere")],
      styledTextElement "span" s!"{edge.source} → {edge.target}" [
        ("maxWidth", "100%"),
        ("padding", "1px 6px"),
        ("borderRadius", "var(--sembla-pill-radius)"),
        ("backgroundColor", "var(--sembla-code-bg)"),
        ("color", "var(--sembla-muted)"),
        ("fontFamily", "var(--sembla-code-font)"),
        ("fontSize", "10px"),
        ("overflowWrap", "anywhere")
      ]
    ],
    styledElement "div" [
      ("display", "grid"),
      ("gridTemplateColumns", "34px minmax(0, 1fr)"),
      ("alignItems", "start"),
      ("gap", "7px")
    ] #[
      styledTextElement "span" "Rate" [
        ("paddingTop", "1px"),
        ("color", "var(--sembla-muted)"),
        ("fontSize", "9px"),
        ("fontWeight", "700"),
        ("letterSpacing", "0.05em"),
        ("textTransform", "uppercase")
      ],
      styledTextElement "code" edge.hazard [
        ("display", "block"),
        ("minWidth", "0"),
        ("color", "var(--sembla-muted)"),
        ("fontFamily", "var(--sembla-code-font)"),
        ("fontSize", "10px"),
        ("whiteSpace", "pre-wrap"),
        ("overflowWrap", "anywhere"),
        ("wordBreak", "break-word")
      ]
    ]
  ]

private def stateHalfWidth (id : String) : Float :=
  let labelWidth := 16.0 + id.length.toFloat * 7.0
  if labelWidth < 44.0 then 22.0 else labelWidth / 2.0

/-- Render state-machine props as a compact dashboard card with an explicit visual preset. -/
def stateDiagramHtmlWithTheme (theme : WidgetTheme) (props : StateDiagramProps) : Html :=
  let count := props.nodes.length
  let markerId := s!"sembla-arrow-{hash props.system}"
  let markerUrl := s!"url(#{markerId})"
  let positions := props.nodes.enum.map fun (index, node) =>
    let x := if count ≤ 1 then 160.0 else 50.0 + index.toFloat * 220.0 / (count - 1).toFloat
    (node.id, x)
  let position (name : String) := positions.find? (·.1 == name) |>.map (·.2) |>.getD 50.0
  let hasReverse (edge : StateEdge) := edge.source != edge.target && props.edges.any fun other =>
    other.source == edge.target && other.target == edge.source
  let edges := props.edges.bind fun edge =>
    let sourceX := position edge.source
    let targetX := position edge.target
    let midpoint := (sourceX + targetX) / 2.0
    if edge.source == edge.target then
      let halfWidth := stateHalfWidth edge.source
      let d := s!"M {sourceX + halfWidth * 0.45},47 C {sourceX + halfWidth * 0.75},8 {sourceX - halfWidth * 0.75},8 {sourceX - halfWidth * 0.45},47"
      [ element "path" [("d", d), ("fill", "none"),
          ("stroke", "var(--sembla-muted)"),
          ("strokeWidth", "1.5"), ("markerEnd", markerUrl)],
        textElement "text" edge.name [("x", toString sourceX), ("y", "13"),
          ("textAnchor", "middle"), ("fontSize", "12"), ("fontWeight", "600"),
          ("fill", "var(--sembla-muted)")] ]
    else
      let direction : Float := if targetX > sourceX then 1.0 else -1.0
      let sourceEdgeX := sourceX + direction * stateHalfWidth edge.source
      let targetEdgeX := targetX - direction * (stateHalfWidth edge.target + 3.0)
      if hasReverse edge then
        let curveY := if targetX > sourceX then 27.0 else 97.0
        let labelY := if targetX > sourceX then 24.0 else 110.0
        let d := s!"M {sourceEdgeX},64 Q {midpoint},{curveY} {targetEdgeX},64"
        [ element "path" [("d", d), ("fill", "none"),
            ("stroke", "var(--sembla-muted)"),
            ("strokeWidth", "1.5"), ("markerEnd", markerUrl)],
          textElement "text" edge.name [("x", toString midpoint), ("y", toString labelY),
            ("textAnchor", "middle"), ("fontSize", "12"), ("fontWeight", "600"),
            ("fill", "var(--sembla-muted)")] ]
      else
        let d := s!"M {sourceEdgeX},64 L {targetEdgeX},64"
        [ element "path" [("d", d), ("fill", "none"),
            ("stroke", "var(--sembla-muted)"),
            ("strokeWidth", "1.5"), ("markerEnd", markerUrl)],
          textElement "text" edge.name [("x", toString midpoint), ("y", "46"),
            ("textAnchor", "middle"), ("fontSize", "12"), ("fontWeight", "600"),
            ("fill", "var(--sembla-muted)")] ]
  let nodes := props.nodes.enum.bind fun (index, node) =>
    let x := position node.id
    let halfWidth := stateHalfWidth node.id
    let color := stateColor index
    [ element "rect" [("x", toString (x - halfWidth)), ("y", "47"),
        ("width", toString (halfWidth * 2.0)), ("height", "34"), ("rx", "17"),
        ("fill", "var(--sembla-surface)"),
        ("stroke", color), ("strokeWidth", "2.5")],
      textElement "text" node.id [("x", toString x), ("y", "69"),
        ("textAnchor", "middle"), ("fontSize", "13"), ("fontWeight", "700"),
        ("fill", "var(--sembla-fg)")] ]
  let stateSummary := props.nodes.map (·.id) |> String.intercalate ", "
  let transitionSummary := props.edges.map (fun edge =>
    s!"{edge.name}: {edge.source} to {edge.target}") |> String.intercalate "; "
  let details := props.edges.map transitionDetail |>.toArray
  widgetShell theme (#[] ++ #[
    widgetHeader "State machine" props.system
      s!"{countLabel props.nodes.length "state" "states"} · {countLabel props.edges.length "transition" "transitions"}",
    styledElement "div" [
      ("padding", "2px"),
      ("border", "1px solid var(--sembla-border)"),
      ("borderRadius", "var(--sembla-card-radius)"),
      ("backgroundColor", "var(--sembla-surface)")
    ] #[
      styledElementWithAttrs "svg" [
        ("viewBox", "0 0 320 120"),
        ("role", "img"),
        ("aria-label", s!"State machine {props.system}; states {stateSummary}; transitions {transitionSummary}")
      ] [
        ("display", "block"),
        ("width", "100%"),
        ("height", "auto")
      ] (#[] ++ #[
        element "defs" [] #[
          element "marker" [("id", markerId), ("viewBox", "0 0 10 10"),
            ("refX", "9"), ("refY", "5"), ("markerWidth", "6"), ("markerHeight", "6"),
            ("orient", "auto-start-reverse")] #[
              element "path" [("d", "M 0 0 L 10 5 L 0 10 z"),
                ("fill", "var(--sembla-muted)")]
            ]
        ]
      ] ++ edges.toArray ++ nodes.toArray)
    ],
    if details.isEmpty then
      styledTextElement "div" "No state transitions" [
        ("marginTop", "8px"),
        ("color", "var(--sembla-muted)"),
        ("fontStyle", "italic")
      ]
    else
      styledElement "div" [
        ("display", "grid"),
        ("gridTemplateColumns", "minmax(0, 1fr)"),
        ("gap", "6px"),
        ("marginTop", "8px")
      ] details
  ])

/-- Render using the default academic/professional preset. -/
def stateDiagramHtml (props : StateDiagramProps) : Html :=
  stateDiagramHtmlWithTheme .academic props

private def paramHtml (param : ParamSummary) : Html :=
  let density := match param.density with
    | none => #[
        styledElement "div" [
          ("marginTop", "8px"),
          ("paddingTop", "7px"),
          ("borderTop", "1px solid var(--sembla-border)"),
          ("color", "var(--sembla-muted)"),
          ("fontSize", "10px")
        ] #[
          styledTextElement "span" "No prior specified" [("fontStyle", "italic")]
        ]
      ]
    | some curve => #[
        plotSvg s!"{curve.family} prior" param.name "density"
          "var(--sembla-prior)" curve.points
      ]
  styledElement "div" [
    ("marginTop", "8px"),
    ("padding", "8px"),
    ("border", "1px solid var(--sembla-border)"),
    ("borderRadius", "var(--sembla-card-radius)"),
    ("backgroundColor", "var(--sembla-surface)")
  ] (#[] ++ #[
    styledElement "div" [
      ("display", "flex"),
      ("alignItems", "center"),
      ("justifyContent", "space-between"),
      ("flexWrap", "wrap"),
      ("gap", "6px 8px"),
      ("minWidth", "0")
    ] #[
      styledElement "div" [("flex", "1 1 120px"), ("minWidth", "0")] #[
        eyebrow "Parameter",
        styledTextElement "code" param.name [
          ("fontFamily", "var(--sembla-code-font)"),
          ("fontSize", "12px"),
          ("fontWeight", "700"),
          ("overflowWrap", "anywhere")
        ]
      ],
      styledElement "div" [("flex", "0 1 auto"), ("minWidth", "0"), ("textAlign", "right")] #[
        styledTextElement "div" "default" [
          ("color", "var(--sembla-muted)"),
          ("fontSize", "9px"),
          ("textTransform", "uppercase")
        ],
        styledTextElement "code" (formatFloat param.defaultValue) [
          ("fontFamily", "var(--sembla-code-font)"),
          ("fontSize", "13px"),
          ("fontWeight", "700"),
          ("color", "var(--sembla-state-1)"),
          ("overflowWrap", "anywhere")
        ]
      ]
    ]
  ] ++ density)

private def emptyState (message : String) : Html :=
  styledTextElement "div" message [
    ("padding", "8px"),
    ("border", "1px dashed var(--sembla-border)"),
    ("borderRadius", "var(--sembla-card-radius)"),
    ("color", "var(--sembla-muted)"),
    ("fontSize", "10px"),
    ("fontStyle", "italic"),
    ("textAlign", "center"),
    ("overflowWrap", "anywhere")
  ]

private def probabilityUnavailable (reason : String) : Html :=
  styledElement "div" [
    ("display", "flex"),
    ("alignItems", "flex-start"),
    ("gap", "8px"),
    ("marginTop", "9px"),
    ("padding", "8px"),
    ("borderLeft", "3px solid var(--vscode-notificationsWarningIcon-foreground, #d99a2b)"),
    ("borderRadius", "var(--sembla-card-radius)"),
    ("backgroundColor", "var(--sembla-code-bg)")
  ] #[
    styledTextElement "span" "i" [
      ("flex", "0 0 auto"),
      ("width", "16px"),
      ("height", "16px"),
      ("border", "1px solid var(--vscode-notificationsWarningIcon-foreground, #d99a2b)"),
      ("borderRadius", "50%"),
      ("color", "var(--vscode-notificationsWarningIcon-foreground, #d99a2b)"),
      ("fontSize", "10px"),
      ("fontWeight", "700"),
      ("lineHeight", "15px"),
      ("textAlign", "center")
    ],
    styledElement "div" [("minWidth", "0")] #[
      styledTextElement "div" "Data-dependent rate" [("fontWeight", "650")],
      styledTextElement "div" reason [
        ("marginTop", "2px"),
        ("color", "var(--sembla-muted)"),
        ("fontSize", "10px"),
        ("overflowWrap", "anywhere")
      ]
    ]
  ]

/-- Render hazard props with an explicit visual preset. -/
def hazardPanelHtmlWithTheme (theme : WidgetTheme) (props : HazardPanelProps) : Html :=
  let probability := match props.probability, props.noProbabilityReason with
    | some points, _ => #[
        sectionHeading "Firing probability" "closed form",
        plotSvg "p(dt) = 1 − exp(−λ·dt)" "dt" "p(dt)"
          "var(--sembla-probability)" points
      ]
    | none, some reason => #[
        sectionHeading "Firing probability" "data-dependent",
        probabilityUnavailable reason
      ]
    | none, none => #[]
  let parameters := if props.params.isEmpty then
    #[sectionHeading "Parameters" "none", emptyState "This transition references no parameters."]
  else
    #[] ++ #[sectionHeading "Parameters" (countLabel props.params.length "referenced" "referenced")]
      ++ (props.params.map paramHtml).toArray
  widgetShell theme
    (#[] ++ #[
      widgetHeader "Transition" props.transition (countLabel props.params.length "parameter" "parameters"),
      styledElement "div" [
        ("display", "grid"),
        ("gridTemplateColumns", "minmax(0, 1fr)"),
        ("gap", "6px")
      ] #[
        formulaBlock "Guard" props.guard,
        formulaBlock "Rate" props.hazard
      ]
    ] ++ parameters ++ probability)

/-- Render using the default academic/professional preset. -/
def hazardPanelHtml (props : HazardPanelProps) : Html :=
  hazardPanelHtmlWithTheme .academic props

private def saveHtmlPanel (html : Html) (stx : Syntax) : CoreM Unit :=
  Widget.savePanelWidgetInfo
    (hash HtmlDisplayPanel.javascript)
    (return json% { html: $(← rpcEncode html) })
    stx

private def selectedTheme : CoreM WidgetTheme := do
  pure (WidgetTheme.ofName (sembla.widget.theme.get (← getOptions)))

/-- Attach the state diagram to the source range of a system or transition. -/
def saveStateDiagram (props : StateDiagramProps) (stx : Syntax) : CoreM Unit := do
  saveHtmlPanel (stateDiagramHtmlWithTheme (← selectedTheme) props) stx

/-- Attach the hazard panel to the source range of a transition. -/
def saveHazardPanel (props : HazardPanelProps) (stx : Syntax) : CoreM Unit := do
  saveHtmlPanel (hazardPanelHtmlWithTheme (← selectedTheme) props) stx

end Sembla.WidgetDisplay
