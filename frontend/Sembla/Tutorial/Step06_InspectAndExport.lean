import Sembla.Tutorial.Step05_PolicyFeedback
import Sembla.Json
import Sembla.Widgets
import Sembla.WidgetDisplay

/-!
# Step 06 — Inspect, render, and serialize the final model

No new dynamics are added. Instead, we consume the elaborated Step 05 model as
ordinary Lean data:

* pure widget builders derive state diagrams and hazard panels;
* ProofWidgets render those props under a selected theme; and
* `Sembla.IR.toJson` produces the canonical Rust-facing IR document.

The widgets inspect structure only. The JSON must still be validated and
executed by the Rust CLI outside Lean. A runtime population must initialize the
single controller row as `Open` with restriction `0.0`; empty tick-zero input is
also neutral because its sum is zero.

A tested workflow is: make a temporary Lean driver importing this module whose
`main` calls `writeFinalJson "/tmp/tutorial.json"`; run it with
`cd frontend && lake env lean --run /tmp/ExportTutorial.lean`; then use
`target/debug/sembla validate /tmp/tutorial.json`, `synth-pop`, and `run` from
the repository root. The synthetic-population path supplies the required
neutral controller row as well as people and employers.
-/

namespace Sembla.Tutorial.Step06

open Lean ProofWidgets
open Sembla.Tutorial.Step05 Sembla.WidgetDisplay Sembla.Widgets

/-- The completed tutorial model, now viewed through downstream tooling. -/
def finalModel := policyFeedbackSIR

/-- Canonical, newline-terminated JSON for the Rust validation boundary. -/
def finalJson : String := Sembla.IR.toJson finalModel

/-- Write the final IR so the Rust CLI can validate and execute it. -/
def writeFinalJson (path : System.FilePath) : IO Unit :=
  IO.FS.writeFile path finalJson

/-- Population and controller state-machine diagrams. -/
def populationDiagram : StateDiagramProps :=
  (stateDiagramProps? finalModel "population" "person").get!

def controllerDiagram : StateDiagramProps :=
  (stateDiagramProps? finalModel "policy" "controller").get!

/-- A closed recovery hazard and an aggregate/input-dependent infection hazard. -/
def recoveryPanel : HazardPanelProps :=
  (hazardPanelProps? finalModel "population" "recover").get!

def infectionPanel : HazardPanelProps :=
  (hazardPanelProps? finalModel "population" "infect").get!

/-- Rendered values are what the Lean infoview displays at source locations. -/
def populationHtml : Html := stateDiagramHtmlWithTheme .academic populationDiagram
def controllerHtml : Html := stateDiagramHtmlWithTheme .academic controllerDiagram
def recoveryHtml : Html := hazardPanelHtmlWithTheme .academic recoveryPanel
def infectionHtml : Html := hazardPanelHtmlWithTheme .academic infectionPanel

#guard finalJson.startsWith "{\"name\":\"tutorial_05_policy_feedback_sir\",\"dt\":0.25"
#guard finalJson.endsWith "}\n"
#guard populationDiagram.nodes.map (·.id) == ["S", "I", "R"]
#guard controllerDiagram.nodes.map (·.id) == ["Open", "Restricted"]
#guard recoveryPanel.probability.isSome
#guard infectionPanel.params.map (·.name) == ["beta"]
#guard infectionPanel.params.head!.density.isSome
#guard infectionPanel.probability.isNone
#guard infectionPanel.noProbabilityReason == some
  "Per-tick probability plot unavailable: hazard depends on row state, inputs, or aggregates."

end Sembla.Tutorial.Step06
