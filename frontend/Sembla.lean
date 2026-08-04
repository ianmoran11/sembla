import Sembla.IR
import Sembla.Semantics
import Sembla.Semantics.RawTests
import Sembla.Semantics.TypesTests
import Sembla.Semantics.SyntaxTests
import Sembla.Semantics.CheckDeclarationsTests
import Sembla.Semantics.CheckModelTests
import Sembla.Frontend.Builders
import Sembla.Frontend.Builders.CoreTests
import Sembla.Frontend.Builders.TransitionTests
import Sembla.Json
import Sembla.Hash
import Sembla.HashTests
import Sembla.Plan
import Sembla.PlanJson
import Sembla.PlanExport
import Sembla.Composition.Source
import Sembla.Composition.Json
import Sembla.Composition.Fixtures
import Sembla.Composition.SourceTests
import Sembla.Composition.Errors
import Sembla.Composition.Link
import Sembla.Composition.Bundle
import Sembla.Composition.LinkTests
import Sembla.Composition.SpecObservation
import Sembla.Composition.SpecStatic
import Sembla.Composition.SpecStatements
import Sembla.Composition.SpecTests
import Sembla.Composition.Widget
import Sembla.Composition.Surface
import Sembla.Composition.SurfaceModels
import Sembla.Composition.SurfaceTests
import Sembla.DSL
import Sembla.Models
import Sembla.PlanTests
import Sembla.CanonicalModelsTests
import Sembla.Widgets
import Sembla.WidgetDisplay
import Sembla.WidgetTests
import Sembla.Composition.WidgetTests
import Sembla.ScientificTests
import Sembla.SurfaceKernelTests
import Sembla.ReactionArrowTests
import Sembla.FrequencyTests
import Sembla.CommandFrontendTests
import Sembla.ArithmeticIntTests
import Sembla.ContestTests
import Sembla.GroupedObservationTests
import Sembla.Lumping
import Sembla.LumpingProof
import Sembla.LumpingTests
import Sembla.Demos
import Sembla.Tutorial

/-! Exact core-builder parity with a current command-frontend declaration. -/
namespace Sembla.Frontend.Builders.CanonicalParityTests

open Sembla

/-- Core shell derived from the actual emitted declaration fields. -/
private def coreShellOf (raw : IR.Model) : CoreModelShell :=
  CoreModelShell.mk raw.name raw.dt raw.params
    (raw.boxes.map fun entry => CoreBoxShell.mk entry.name entry.tables)

/-- Independent raw declaration-only projection used as the exact expected value. -/
private def coreRawOf (raw : IR.Model) : IR.Model :=
  IR.Model.mk raw.name raw.dt raw.params
    (raw.boxes.map fun entry =>
      IR.Box.mk entry.name entry.tables [] [] [] [] []) [] []

private def canonicalSirCoreParity : Bool :=
  match buildModelShell (coreShellOf Sembla.Models.sir) with
  | .ok built => built == coreRawOf Sembla.Models.sir
  | .error _ => false

#guard canonicalSirCoreParity
#guard (coreShellOf Sembla.Models.sir).parameterNames == ["beta", "gamma"]
#guard (coreShellOf Sembla.Models.sir).boxes.map CoreBoxShell.tableNames ==
  [["person", "employer"]]

/-- Independent pure-builder spelling of the current multiple-claim contest. -/
private def expectedContestTransition : IR.Transition :=
  TransitionRaw.transition "exit" "slot"
    (TransitionRaw.enumIs "occupancy" "present")
    (TransitionRaw.real 1.0)
    [TransitionRaw.setAttribute "occupancy" (TransitionRaw.enum "vacant")]
    [ TransitionRaw.raceClaim (TransitionRaw.selfAttribute "slot_resource")
    , TransitionRaw.raceClaim (TransitionRaw.selfAttribute "backup_resource") ]

private def contestTransitionSpec : TransitionOverlaySpec :=
  TransitionOverlaySpec.mk (coreShellOf Sembla.ContestTests.contestTwin) fun ordinal =>
    if ordinal.val = 0 then [expectedContestTransition] else []

private def canonicalContestTransitionParity : Bool :=
  contestTransitionSpec.toRaw == Sembla.ContestTests.contestTwin &&
    match buildTransitionOverlay contestTransitionSpec with
    | .ok checked => checked.erase == Sembla.ContestTests.contestTwin
    | .error _ => false

#guard canonicalContestTransitionParity
#guard expectedContestTransition.contests.length == 2

end Sembla.Frontend.Builders.CanonicalParityTests
