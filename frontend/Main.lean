import Sembla

open Sembla

private def usage : String :=
  "usage: sembla-export [--plan|--source] <name> <out.json>"

private def lookupModel (name : String) : Option IR.Model :=
  match name with
  | "sir" | "Sembla.Models.sir" | "Sembla/Models/sir" => some Models.sir
  | "sirPolicy" | "sir_policy"
  | "Sembla.Models.sirPolicy" | "Sembla.Models.sir_policy"
  | "Sembla/Models/sirPolicy" | "Sembla/Models/sir_policy" => some Models.sirPolicy
  | "observations" | "Sembla.Models.observations" | "Sembla/Models/observations" =>
      some Models.observations
  | "reversibleCtmc" | "reversible_ctmc"
  | "Sembla.Models.reversibleCtmc" | "Sembla.Models.reversible_ctmc"
  | "Sembla/Models/reversibleCtmc" | "Sembla/Models/reversible_ctmc" =>
      some Models.reversibleCtmc
  | "radioactiveDecayChain" | "radioactive_decay_chain"
  | "Sembla.Models.radioactiveDecayChain" | "Sembla.Models.radioactive_decay_chain"
  | "Sembla/Models/radioactiveDecayChain" | "Sembla/Models/radioactive_decay_chain" =>
      some Models.radioactiveDecayChain
  | "sisImportation" | "sis_importation"
  | "Sembla.Models.sisImportation" | "Sembla.Models.sis_importation"
  | "Sembla/Models/sisImportation" | "Sembla/Models/sis_importation" =>
      some Models.sisImportation
  | "seirsWaning" | "seirs_waning"
  | "Sembla.Models.seirsWaning" | "Sembla.Models.seirs_waning"
  | "Sembla/Models/seirsWaning" | "Sembla/Models/seirs_waning" => some Models.seirsWaning
  | "noisyVoter" | "noisy_voter"
  | "Sembla.Models.noisyVoter" | "Sembla.Models.noisy_voter"
  | "Sembla/Models/noisyVoter" | "Sembla/Models/noisy_voter" => some Models.noisyVoter
  | "demographicSlots" | "demographic_slots"
  | "Sembla.Models.demographicSlots" | "Sembla.Models.demographic_slots"
  | "Sembla/Models/demographicSlots" | "Sembla/Models/demographic_slots" =>
      some Models.demographicSlots
  | _ => none

private def lookupSource (name : String) : Option Composition.CompositionSourceV1 :=
  (Demos.Composition.lookup name).orElse fun _ =>
    (Composition.SurfaceModels.lookup name).orElse fun _ =>
      Composition.Fixtures.lookup name

def main (args : List String) : IO UInt32 := do
  match args with
  | ["--source", name, outputPath] =>
      match lookupSource name with
      | none =>
          IO.eprintln s!"unknown composition source fixture '{name}'\n{usage}"
          pure 2
      | some source =>
          IO.FS.writeFile outputPath (Composition.Json.render source)
          pure 0
  | ["--plan", name, outputPath] =>
      match lookupModel name with
      | none =>
          IO.eprintln s!"unknown model '{name}'\n{usage}"
          pure 2
      | some model =>
          match PlanExport.directStablePlan model with
          | .error message =>
              IO.eprintln message
              pure 1
          | .ok plan =>
              IO.FS.writeFile outputPath (PlanJson.renderPlan plan)
              pure 0
  | [name, outputPath] =>
      match lookupModel name with
      | none =>
          IO.eprintln s!"unknown model '{name}'\n{usage}"
          pure 2
      | some model =>
          IO.FS.writeFile outputPath (IR.toJson model)
          pure 0
  | _ =>
      IO.eprintln usage
      pure 2
