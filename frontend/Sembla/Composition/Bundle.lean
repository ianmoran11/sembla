import Sembla.Composition.Link
import Sembla.Hash

namespace Sembla.Composition.Bundle

open Sembla

/-- Frozen bundle layout and schema strings. -/
def bundleSchema : String := "sembla.bundle/v1"
def sourcePath : String := "composition-source.json"
def planPath : String := "executable-plan.json"
def reportPath : String := "link-report.json"
def manifestPath : String := "bundle-manifest.json"

structure Artifacts where
  source : String
  plan : String
  report : String
  manifest : String

deriving Repr, BEq

private def hashRecordJson (record : Hash.HashRecord) : PlanJson.CJson :=
  .obj #[
    ("algorithm", .str record.algorithm),
    ("domain", .str record.domain),
    ("digest", .str record.digest)]

/-- Canonical, non-semantic link report used by both single-file and bundle modes. -/
def reportJson (report : LinkReportV1) : PlanJson.CJson :=
  .obj #[
    ("warnings", .arr (report.warnings.map PlanJson.CJson.str).toArray),
    ("statistics", .obj #[
      ("leaves", .int (Int.ofNat report.statistics.leaves)),
      ("transitions", .int (Int.ofNat report.statistics.transitions)),
      ("mailboxes", .int (Int.ofNat report.statistics.mailboxes))])]

private def originString : Plan.PlanOrigin → String
  | .linked => "linked"
  | .directStable => "direct_stable"

private def manifestFields
    (sourceHash semanticHash envelopeHash : Hash.HashRecord)
    (plan : Plan.ExecutablePlanV1) : Array (String × PlanJson.CJson) := #[
  ("bundle_schema", .str bundleSchema),
  ("canonical_encoding", .str Plan.canonicalEncoding),
  ("source", .obj #[
    ("schema", .str Plan.compositionSourceSchema),
    ("path", .str sourcePath),
    ("hash", hashRecordJson sourceHash)]),
  ("linker", .obj #[
    ("semantics", .str Plan.linkerSemantics),
    ("implementation", .str "lean4")]),
  ("plan", .obj #[
    ("schema", .str Plan.planSchema),
    ("identity_scheme", .str Plan.stableIdentityScheme),
    ("origin", .str (originString plan.origin)),
    ("enabled_features", .arr (plan.identity.enabledFeatures.map PlanJson.CJson.str).toArray),
    ("path", .str planPath),
    ("semantic_hash", hashRecordJson semanticHash),
    ("envelope_hash", hashRecordJson envelopeHash)]),
  ("source_map_schema", .str Plan.sourceMapSchema)]

/-- Frozen bundle-root payload: canonical manifest without `bundle_integrity`,
    followed by each named path, a zero byte, and raw SHA-256(file bytes), in
    lexicographic path order. The manifest is deliberately not a named input. -/
def integrityPayload
    (manifestWithoutIntegrity : String)
    (namedFiles : List (String × String)) : ByteArray :=
  let ordered := namedFiles.mergeSort fun left right => left.1 < right.1
  ordered.foldl (fun payload named =>
    let withPath := payload.append named.1.toUTF8
    let withSeparator := withPath.push 0
    withSeparator.append (Hash.sha256 named.2.toUTF8)) manifestWithoutIntegrity.toUTF8

/-- Construct all four canonical bundle files from one successful link result. -/
def build (canonicalSource : String) (result : LinkResultV1) : Artifacts :=
  let planBytes := PlanJson.renderPlan result.plan
  let reportBytes := (reportJson result.report).render
  let sourceHash := Hash.hashRecord Plan.sourceArtifactDomain canonicalSource.toUTF8
  let semanticBytes := (PlanJson.semanticPayloadToCJson result.plan).render
  let semanticHash := Hash.hashRecord Plan.planCoreDomain semanticBytes.toUTF8
  let envelopeHash := Hash.hashRecord Plan.planEnvelopeDomain planBytes.toUTF8
  let fields := manifestFields sourceHash semanticHash envelopeHash result.plan
  let manifestWithoutIntegrity := (PlanJson.CJson.obj fields).render
  let payload := integrityPayload manifestWithoutIntegrity [
    (sourcePath, canonicalSource),
    (planPath, planBytes),
    (reportPath, reportBytes)]
  let integrity := Hash.hashRecord Plan.bundleRootDomain payload
  let manifestBytes := (PlanJson.CJson.obj
    (fields.push ("bundle_integrity", hashRecordJson integrity))).render
  { source := canonicalSource
    plan := planBytes
    report := reportBytes
    manifest := manifestBytes }

end Sembla.Composition.Bundle
