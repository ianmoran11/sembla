import Sembla.Composition.Bundle

open Sembla

private def usage : String :=
  "usage: sembla-link <source.json> (--plan <out.plan.json> [--report <out.report.json>] | --bundle <dir>)"

private def emitLinkErrors (errors : List Composition.LinkErrorV1) : IO Unit := do
  for error in errors do
    IO.eprintln s!"link error {error.code.toString} at {error.primary.raw}: {error.message}"

private def loadAndLink
    (sourcePath : String) : IO (Except UInt32 (String × Composition.LinkResultV1)) := do
  let bytes ← IO.FS.readFile sourcePath
  match Composition.Json.parse bytes with
  | .error message =>
      IO.eprintln s!"source error: {message}"
      pure (.error 1)
  | .ok source =>
      let canonicalSource := Composition.Json.render source
      match Composition.linkV1 source canonicalSource with
      | .error errors =>
          emitLinkErrors errors
          pure (.error 1)
      | .ok result => pure (.ok (canonicalSource, result))

private def runLink
    (sourcePath planPath : String) (reportPath : Option String) : IO UInt32 := do
  match ← loadAndLink sourcePath with
  | .error code => pure code
  | .ok (_, result) =>
      IO.FS.writeFile planPath (PlanJson.renderPlan result.plan)
      match reportPath with
      | none => pure ()
      | some path => IO.FS.writeFile path (Composition.Bundle.reportJson result.report).render
      pure 0

private def requireEmptyBundleDirectory (path : System.FilePath) : IO (Except String Unit) := do
  if ← path.pathExists then
    unless ← path.isDir do
      return .error s!"bundle output '{path}' exists and is not a directory"
    let entries ← path.readDir
    unless entries.isEmpty do
      return .error s!"refusing to overwrite non-empty bundle directory '{path}'"
  pure (.ok ())

private def runBundle (sourcePath bundlePath : String) : IO UInt32 := do
  let directory : System.FilePath := bundlePath
  match ← requireEmptyBundleDirectory directory with
  | .error message =>
      IO.eprintln message
      pure 1
  | .ok () =>
      match ← loadAndLink sourcePath with
      | .error code => pure code
      | .ok (canonicalSource, result) =>
          let artifacts := Composition.Bundle.build canonicalSource result
          IO.FS.createDirAll directory
          IO.FS.writeFile (directory / Composition.Bundle.sourcePath) artifacts.source
          IO.FS.writeFile (directory / Composition.Bundle.planPath) artifacts.plan
          IO.FS.writeFile (directory / Composition.Bundle.reportPath) artifacts.report
          IO.FS.writeFile (directory / Composition.Bundle.manifestPath) artifacts.manifest
          pure 0

def main (args : List String) : IO UInt32 := do
  match args with
  | [sourcePath, "--plan", planPath] => runLink sourcePath planPath none
  | [sourcePath, "--plan", planPath, "--report", reportPath] =>
      runLink sourcePath planPath (some reportPath)
  | [sourcePath, "--bundle", bundlePath] => runBundle sourcePath bundlePath
  | _ =>
      IO.eprintln usage
      pure 2
