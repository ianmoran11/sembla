import Sembla.Models
import Sembla.PlanExport

namespace Sembla.PlanTests

open Sembla

private def numberVectors : List (IR.Scientific × String) := [
  (⟨-2302585092994046, -15⟩, "-2.302585092994046"),
  (⟨-2231435513142097, -16⟩, "-0.2231435513142097"),
  (⟨0, 0⟩, "0.0"),
  (⟨1, -9⟩, "1e-9"),
  (⟨1, -1⟩, "0.1"),
  (⟨25, -2⟩, "0.25"),
  (⟨4, -1⟩, "0.4"),
  (⟨5, -1⟩, "0.5"),
  (⟨8, -1⟩, "0.8"),
  (⟨10, -1⟩, "1.0"),
  (⟨20, -1⟩, "2.0"),
  (⟨5000, -1⟩, "500.0"),
  (⟨10000000, -1⟩, "1000000.0"),
  (⟨1, 300⟩, "1e+300")
]

private def numberVectorsMatch : Bool :=
  numberVectors.all fun (value, expected) => PlanJson.renderScientific value == expected

#guard numberVectorsMatch

private def canonicalEscapingAndSortingMatch : Bool :=
  (PlanJson.CJson.obj #[
    ("z", .int 1), ("a", .str "quote:\" slash:\\ control:\n")]).render ==
    "{\"a\":\"quote:\\\" slash:\\\\ control:\\n\",\"z\":1}"

#guard canonicalEscapingAndSortingMatch

private def invalidSlugRejected : Bool :=
  match PlanExport.directStablePlan { Models.sir with name := "Bad" } with
  | .error message => message == "model name 'Bad' is not a slug"
  | .ok _ => false

#guard invalidSlugRejected

private def mapBoxes (model : IR.Model) (transform : IR.Box → IR.Box) : IR.Model :=
  { model with «boxes» := model.boxes.map transform }

private def invalidBoxSlugRejected : Bool :=
  let model := mapBoxes Models.sir fun modelBox => { modelBox with name := "Bad" }
  match PlanExport.directStablePlan model with
  | .error message => message == "box name 'Bad' is not a slug"
  | .ok _ => false

#guard invalidBoxSlugRejected

private def invalidTransitionSlugRejected : Bool :=
  let model := mapBoxes Models.sir fun modelBox =>
    { modelBox with «transitions» := modelBox.transitions.map fun item =>
      { item with name := "Bad" } }
  match PlanExport.directStablePlan model with
  | .error message => message == "transition name 'Bad' in box 'sir' is not a slug"
  | .ok _ => false

#guard invalidTransitionSlugRejected

private def invalidInputPortSlugRejected : Bool :=
  let model := mapBoxes Models.sirPolicy fun modelBox =>
    if modelBox.name == "population" then
      { modelBox with «inputs» := modelBox.inputs.map fun port => { port with name := "Bad" } }
    else modelBox
  match PlanExport.directStablePlan model with
  | .error message => message == "input port name 'Bad' in box 'population' is not a slug"
  | .ok _ => false

#guard invalidInputPortSlugRejected

private def invalidOutputPortSlugRejected : Bool :=
  let model := mapBoxes Models.sirPolicy fun modelBox =>
    if modelBox.name == "population" then
      { modelBox with «outputs» := modelBox.outputs.map fun port => { port with name := "Bad" } }
    else modelBox
  match PlanExport.directStablePlan model with
  | .error message => message == "output port name 'Bad' in box 'population' is not a slug"
  | .ok _ => false

#guard invalidOutputPortSlugRejected

private def reservedWordRejected : Bool :=
  match PlanExport.directStablePlanWithRuleWord (fun _ => 0xFFFFFFFE) Models.sir with
  | .error message =>
      message == "rule word 4294967294 for 'occ:sir#infect' is reserved"
  | .ok _ => false

#guard reservedWordRejected

private def duplicateWordRejected : Bool :=
  match PlanExport.directStablePlanWithRuleWord (fun _ => 7) Models.sir with
  | .error message =>
      message == "duplicate rule word 7 for 'occ:sir#recover'; first used by 'occ:sir#infect'"
  | .ok _ => false

#guard duplicateWordRejected

private def sirSemanticDigest : String :=
  match PlanExport.directStablePlan Models.sir with
  | .error _ => ""
  | .ok plan =>
      let payload := (PlanJson.semanticPayloadToCJson plan).render
      (Hash.hashRecord Plan.planCoreDomain payload.toUTF8).digest

/- Shared literally with the Rust `sir_plan_semantic_hash_matches_lean` assertion. -/
#guard sirSemanticDigest ==
  "b702a2188de78b47ff0a23510ad8c736b9d1e93a1b79662a821bd3f491cb53da"

end Sembla.PlanTests
