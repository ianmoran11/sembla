import Sembla

open Sembla

private def usage : String :=
  "usage: sembla-export [--plan|--source] <name> <out.json>"

private structure ModelRegistration where
  aliases : List String
  model : IR.Model

private def modelAliases (camel snake : String) : List String :=
  [ camel
  , snake
  , s!"Sembla.Models.{camel}"
  , s!"Sembla.Models.{snake}"
  , s!"Sembla/Models/{camel}"
  , s!"Sembla/Models/{snake}" ].eraseDups

private def modelRegistry : List ModelRegistration :=
  [ { aliases := modelAliases "sir" "sir", model := Models.sir }
  , { aliases := modelAliases "sirPolicy" "sir_policy", model := Models.sirPolicy }
  , { aliases := modelAliases "observations" "observations", model := Models.observations }
  , { aliases := modelAliases "reversibleCtmc" "reversible_ctmc", model := Models.reversibleCtmc }
  , { aliases := modelAliases "radioactiveDecayChain" "radioactive_decay_chain",
      model := Models.radioactiveDecayChain }
  , { aliases := modelAliases "sisImportation" "sis_importation", model := Models.sisImportation }
  , { aliases := modelAliases "seirsWaning" "seirs_waning", model := Models.seirsWaning }
  , { aliases := modelAliases "noisyVoter" "noisy_voter", model := Models.noisyVoter }
  , { aliases := modelAliases "demographicSlots" "demographic_slots",
      model := Models.demographicSlots }
  , { aliases := modelAliases "australianPopulation" "australian_population",
      model := Models.australianPopulation } ]

private def lookupRegisteredModel (name : String) : List ModelRegistration → Option IR.Model
  | [] => none
  | registration :: rest =>
      if registration.aliases.contains name then some registration.model
      else lookupRegisteredModel name rest

private def lookupModel (name : String) : Option IR.Model :=
  lookupRegisteredModel name modelRegistry

private def modelAliasesAreUnique : Bool :=
  let aliases := (modelRegistry.map fun registration => registration.aliases).join
  aliases.eraseDups.length == aliases.length

private def modelNamesAreUnique : Bool :=
  let names := modelRegistry.map fun registration => registration.model.name
  names.eraseDups.length == names.length

#guard modelAliasesAreUnique
#guard modelNamesAreUnique

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
