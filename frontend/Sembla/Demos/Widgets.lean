import Sembla.Demos.Modeling
import Sembla.Widgets
import Sembla.WidgetDisplay

/-!
# Structure-widget feature tour

The widget layer is pure and split in two: `Sembla.Widgets` derives
JSON-encodable state-machine and hazard-panel props from an elaborated model;
`Sembla.WidgetDisplay` renders those props as responsive ProofWidgets HTML.
The DSL also attaches the panels to the original `system` and `transition`
source ranges automatically.

These widgets inspect structure only. They never invoke simulation or the Rust
runtime.
-/

namespace Sembla.Demos.Widgets

open Lean ProofWidgets
open Sembla.Demos.Modeling Sembla.WidgetDisplay Sembla.Widgets

/-- State graph derived from the feature-tour population system. -/
def populationDiagram : StateDiagramProps :=
  (stateDiagramProps? featureTour "population" "person").get!

/-- Closed hazard: the panel can plot firing probability over `dt`. -/
def recoveryPanel : HazardPanelProps :=
  (hazardPanelProps? featureTour "population" "recover").get!

/-- Aggregate/input-dependent hazard: the panel explains why no closed plot is shown. -/
def infectionPanel : HazardPanelProps :=
  (hazardPanelProps? featureTour "population" "infect").get!

/-- JSON props are the stable boundary between pure builders and display code. -/
def populationDiagramJson : Json := toJson populationDiagram

/-- The same props render under each supported visual preset. -/
def academicDiagram : Html := stateDiagramHtmlWithTheme .academic populationDiagram
def editorDiagram : Html := stateDiagramHtmlWithTheme .editor populationDiagram
def notebookDiagram : Html := stateDiagramHtmlWithTheme .notebook populationDiagram

def academicRecoveryPanel : Html := hazardPanelHtmlWithTheme .academic recoveryPanel
def editorRecoveryPanel : Html := hazardPanelHtmlWithTheme .editor recoveryPanel
def notebookRecoveryPanel : Html := hazardPanelHtmlWithTheme .notebook recoveryPanel
/-- Aggregate hazard rendering includes the prior chart and unavailable-plot explanation. -/
def academicInfectionPanel : Html := hazardPanelHtmlWithTheme .academic infectionPanel

#guard populationDiagram.nodes.map (·.id) == ["S", "I", "R"]
#guard populationDiagram.edges.map (·.name) == ["infect", "recover"]
#guard recoveryPanel.params.map (·.name) == ["gamma"]
#guard recoveryPanel.probability.isSome
#guard infectionPanel.params.map (·.name) == ["beta"]
#guard infectionPanel.params.head!.density.isSome
#guard infectionPanel.probability.isNone
#guard infectionPanel.noProbabilityReason == some
  "Per-tick probability plot unavailable: hazard depends on row state, inputs, or aggregates."
#guard WidgetTheme.ofName "professional" == .academic
#guard [WidgetTheme.academic, .editor, .notebook].map WidgetTheme.name ==
  ["academic", "editor", "notebook"]

end Sembla.Demos.Widgets
