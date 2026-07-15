import Sembla

open Sembla

private def usage : String :=
  "usage: sembla-export <sir|sir_policy|reversible_ctmc|radioactive_decay_chain|sis_importation|seirs_waning|noisy_voter> <out.json>"

private def lookupModel (name : String) : Option IR.Model :=
  match name with
  | "sir" | "Sembla.Models.sir" | "Sembla/Models/sir" => some Models.sir
  | "sirPolicy" | "sir_policy"
  | "Sembla.Models.sirPolicy" | "Sembla.Models.sir_policy"
  | "Sembla/Models/sirPolicy" | "Sembla/Models/sir_policy" => some Models.sirPolicy
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
  | _ => none

def main (args : List String) : IO UInt32 := do
  match args with
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
