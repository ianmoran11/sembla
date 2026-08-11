import Sembla
import Sembla.Semantics.RawTests
import Sembla.Semantics.TypesTests
import Sembla.Semantics.SyntaxTests
import Sembla.Semantics.CheckDeclarationsTests
import Sembla.Semantics.CheckModelTests
import Sembla.Frontend.Builders.CoreTests
import Sembla.Frontend.Builders.TransitionTests
import Sembla.HashTests
import Sembla.Composition.SourceTests
import Sembla.Composition.LinkTests
import Sembla.Composition.SpecTests
import Sembla.Composition.SurfaceTests
import Sembla.ParameterTableTests
import Sembla.IndexedFamilyTests
import Sembla.MathematicalSurfaceTests
import Sembla.Demos.CompositionTests
import Sembla.Models.AustralianPopulation.Validation
import Sembla.PlanTests
import Sembla.CanonicalModelsTests
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
import Sembla.LumpingTests

/-!
Compile-time test import surface. It remains a default Lake target so `lake
build` preserves the repository's existing test contract, while production
executables depend only on `Sembla`.
-/

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
