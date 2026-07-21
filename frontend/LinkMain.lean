import Sembla.Composition.Link

open Sembla

private def usage : String :=
  "usage: sembla-link <source.json> --plan <out.plan.json> [--report <out.report.json>]"

private def reportJson (report : Composition.LinkReportV1) : PlanJson.CJson :=
  .obj #[
    ("warnings", .arr (report.warnings.map PlanJson.CJson.str).toArray),
    ("statistics", .obj #[
      ("leaves", .int (Int.ofNat report.statistics.leaves)),
      ("transitions", .int (Int.ofNat report.statistics.transitions)),
      ("mailboxes", .int (Int.ofNat report.statistics.mailboxes))])]

private def emitLinkErrors (errors : List Composition.LinkErrorV1) : IO Unit := do
  for error in errors do
    IO.eprintln s!"link error {error.code.toString} at {error.primary.raw}: {error.message}"

private def runLink
    (sourcePath planPath : String) (reportPath : Option String) : IO UInt32 := do
  let bytes ← IO.FS.readFile sourcePath
  match Composition.Json.parse bytes with
  | .error message =>
      IO.eprintln s!"source error: {message}"
      pure 1
  | .ok source =>
      let canonicalSource := Composition.Json.render source
      match Composition.linkV1 source canonicalSource with
      | .error errors =>
          emitLinkErrors errors
          pure 1
      | .ok result =>
          IO.FS.writeFile planPath (PlanJson.renderPlan result.plan)
          match reportPath with
          | none => pure ()
          | some path => IO.FS.writeFile path (reportJson result.report).render
          pure 0

def main (args : List String) : IO UInt32 := do
  match args with
  | [sourcePath, "--plan", planPath] => runLink sourcePath planPath none
  | [sourcePath, "--plan", planPath, "--report", reportPath] =>
      runLink sourcePath planPath (some reportPath)
  | _ =>
      IO.eprintln usage
      pure 2
