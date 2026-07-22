import Sembla.Composition.Fixtures
import Sembla.Composition.Json
import Sembla.Composition.SpecStatements

namespace Sembla.Composition.SpecTests

open Sembla
open Sembla.Composition
open Sembla.Composition.Fixtures

/-!
# Executable static-preservation checks

These guards are the proved-by-evaluation status of the V1 static fragment:
the independent source denotation agrees with linked flat plans over the full
linkable corpus and PRD 0009 variants, while two hand-written meanings prevent
the two derivations from merely agreeing on the same mistake. Full behavioral
preservation remains stated-deferred. Its V1 quotient includes every observation
field, including draw coordinates; hashes remain artifact consequences under
DECISIONS.md §J10.
-/

private def sid (raw : String) : StableId := ⟨raw⟩

/-- Executable instance of the static preservation proposition for one source. -/
def checkStaticPreservation (source : CompositionSourceV1) : Bool :=
  let bytes := Json.render source
  match linkV1 source bytes, denoteSourceStatic source with
  | .ok linked, .ok sourceMeaning => denotePlanStatic linked.plan == sourceMeaning
  | _, _ => false

#guard checkStaticPreservation soloPopulation
#guard checkStaticPreservation independentEpidemicPolicy
#guard checkStaticPreservation twoIndependentRegions
#guard checkStaticPreservation epidemicPolicy
#guard checkStaticPreservation pingPong
#guard checkStaticPreservation twoRegions
#guard checkStaticPreservation regionalResponse
#guard checkStaticPreservation wrappedPingPong

private def permuteDefinitions (source : CompositionSourceV1) : CompositionSourceV1 :=
  { source with definitions := source.definitions.reverse }

private def permuteInstances (source : CompositionSourceV1) : CompositionSourceV1 :=
  { source with definitions := source.definitions.map fun definition =>
      match definition.body with
      | .primitive _ => definition
      | .composite body =>
          { definition with body := .composite { body with instances := body.instances.reverse } } }

private def permuteWires (source : CompositionSourceV1) : CompositionSourceV1 :=
  { source with definitions := source.definitions.map fun definition =>
      match definition.body with
      | .primitive _ => definition
      | .composite body =>
          { definition with body := .composite { body with «wires» := body.wires.reverse } } }

private def permuteExposures (source : CompositionSourceV1) : CompositionSourceV1 :=
  { source with definitions := source.definitions.map fun definition =>
      match definition.body with
      | .primitive _ => definition
      | .composite body =>
          { definition with body := .composite { body with exposures := body.exposures.reverse } } }

private def modifyRootComposite
    (source : CompositionSourceV1) (change : CompositeBodyV1 → CompositeBodyV1) :
    CompositionSourceV1 :=
  { source with definitions := source.definitions.map fun definition =>
      if definition.id == source.rootDefinition then
        match definition.body with
        | .primitive _ => definition
        | .composite body => { definition with body := .composite (change body) }
      else definition }

private def renameNorthInstance : CompositionSourceV1 :=
  let renamedRoot := modifyRootComposite twoRegions fun body =>
    { body with instances := body.instances.map fun item =>
        if item.id.raw == "inst:north" then { item with id := sid "inst:west" }
        else item }
  { renamedRoot with «summaries» := renamedRoot.summaries.map fun sourceSummary =>
      { sourceSummary with instancePath := sourceSummary.instancePath.map fun item =>
          if item.raw == "inst:north" then sid "inst:west" else item } }

#guard checkStaticPreservation twoRegionsDisplayRenamed
#guard checkStaticPreservation (permuteDefinitions twoRegions)
#guard checkStaticPreservation (permuteInstances twoRegions)
#guard checkStaticPreservation (permuteWires twoRegions)
#guard checkStaticPreservation (permuteExposures regionalResponse)
#guard checkStaticPreservation renameNorthInstance

private def hasStaticMeaning
    (source : CompositionSourceV1) (expected : StaticMeaning) : Bool :=
  match denoteSourceStatic source with
  | .ok actual => actual == expected
  | .error _ => false

private def expectedSoloPopulation : StaticMeaning := {
  leaves := [("population", sid "occ:population")]
  «transitions» := [
    ("occ:population#infect", 2501600445),
    ("occ:population#recover", 3552696863)]
  mailboxes := []
  boundary := [] }

private def expectedPingPong : StaticMeaning := {
  leaves := [
    ("ping", sid "occ:ping"),
    ("pong", sid "occ:pong")]
  «transitions» := [
    ("occ:ping#arm", 350567833),
    ("occ:ping#disarm", 3679717260),
    ("occ:pong#notice", 2831376099)]
  mailboxes := [
    "mbox:occ:#wire:pulse|occ:ping.port:pulse|occ:pong.port:pulse"]
  boundary := [] }

#guard hasStaticMeaning soloPopulation expectedSoloPopulation
#guard hasStaticMeaning pingPong expectedPingPong

private def duplicateDefinitionSource : CompositionSourceV1 :=
  match soloPopulation.definitions with
  | [] => soloPopulation
  | first :: _ => { soloPopulation with
      definitions := soloPopulation.definitions ++ [first] }

private def missingDefinitionSource : CompositionSourceV1 :=
  modifyRootComposite soloPopulation fun body =>
    { body with instances := body.instances.map fun item =>
        { item with definition := sid "def:missing" } }

private def staticErrorCodes (source : CompositionSourceV1) : Option (List LinkErrorCodeV1) :=
  match denoteSourceStatic source with
  | .ok _ => none
  | .error errors => some (errors.map (·.code))

private def linkerErrorCodes (source : CompositionSourceV1) : Option (List LinkErrorCodeV1) :=
  match linkV1 source (Json.render source) with
  | .ok _ => none
  | .error errors => some (errors.map (·.code))

#guard staticErrorCodes duplicateDefinitionSource == some [.duplicateStableId]
#guard linkerErrorCodes duplicateDefinitionSource == some [.duplicateStableId]
#guard staticErrorCodes missingDefinitionSource == some [.missingDefinition]
#guard linkerErrorCodes missingDefinitionSource == some [.missingDefinition]

end Sembla.Composition.SpecTests
