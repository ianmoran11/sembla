import Sembla.Json
import Sembla.Models.AustralianPopulation.Surface
import Sembla.PlanExport
import Sembla.Semantics.CheckModel

/-! Public declarative assembly and frozen exports for the Australian population model. -/

namespace Sembla.Models

open Sembla.IR

private def canonicalRows : Nat := 352460

/-- Canonical one-in-a-hundred model used by the initial-state fixture. -/
def australianPopulation : Model := australianPopulationDeclarative

private def canonicalRowCountsHold : Bool :=
  match australianPopulation.boxes with
  | [modelBox] => modelBox.tables.map (fun modelTable => modelTable.sizeHint) ==
      [canonicalRows, canonicalRows]
  | _ => false

#guard australianPopulation.name == "australian_population"
#guard australianPopulation.boxes.length == 1
#guard canonicalRowCountsHold
#guard australianPopulation.params.length == 377
#guard (australianPopulation.boxes.bind (·.transitions)).length == 418
#guard (Sembla.Semantics.checkModel australianPopulation).isOk

/-- Canonical legacy-model bytes consumed by the Python state builder. -/
def australianPopulationJson : String := toJson australianPopulation

/-- Direct-stable plan bytes for parity and downstream run fixtures. -/
def australianPopulationPlanJson : String :=
  match PlanExport.directStablePlan australianPopulation with
  | .error message => message
  | .ok plan => PlanJson.planToCJson plan |>.render

end Sembla.Models
