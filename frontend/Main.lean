import Sembla

open Sembla

private def usage : String :=
  "usage: sembla-export <sir|sirPolicy|Sembla.Models.sir|Sembla.Models.sirPolicy> <out.json>"

private def lookupModel (name : String) : Option IR.Model :=
  match name with
  | "sir" | "Sembla.Models.sir" | "Sembla/Models/sir" => some Models.sir
  | "sirPolicy" | "sir_policy" | "Sembla.Models.sirPolicy" | "Sembla/Models/sirPolicy" =>
      some Models.sirPolicy
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
