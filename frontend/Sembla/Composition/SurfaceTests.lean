import Sembla.Composition.SurfaceModels
import Sembla.Composition.Fixtures
import Sembla.Composition.Link
import Sembla.PlanJson

namespace Sembla.Composition.SurfaceTests

open Sembla
open Sembla.Composition.SurfaceModels

sembla_component PortOrderProbe where
  system Row (rows := 1) where
    status : {A}
  output first from Row where
    n : Int := count where status = A
  input second where
    n : Int

#guard PortOrderProbe.ports.map (·.id.raw) == ["port:first", "port:second"]
#guard PortOrderProbe.ports.map (·.direction) == [.output, .input]

#guard epidemicPolicyModel == Fixtures.epidemicPolicy
#guard independentEpidemicAndPolicyModel == Fixtures.independentEpidemicPolicy
#guard twoRegionsModel == Fixtures.twoRegions

#guard Json.render epidemicPolicyModel == Json.render Fixtures.epidemicPolicy
#guard Json.render independentEpidemicAndPolicyModel ==
  Json.render Fixtures.independentEpidemicPolicy
#guard Json.render twoRegionsModel == Json.render Fixtures.twoRegions

private def linkedPlanBytes (source : CompositionSourceV1) : Option String :=
  match linkV1 source (Json.render source) with
  | .ok result => some (PlanJson.renderPlan result.plan)
  | .error _ => none

#guard linkedPlanBytes epidemicPolicyModel == linkedPlanBytes Fixtures.epidemicPolicy

private def everyBindingIsExplicit (source : CompositionSourceV1) : Bool :=
  source.definitions.all fun definition =>
    match definition.body with
    | .primitive _ => true
    | .composite body => body.instances.all fun instance_ =>
        match source.definitions.find? (·.id == instance_.definition) with
        | none => false
        | some child => instance_.parameterBindings.map (·.requirement) ==
            child.parameterRequirements

#guard everyBindingIsExplicit epidemicPolicyModel
#guard everyBindingIsExplicit independentEpidemicAndPolicyModel
#guard everyBindingIsExplicit twoRegionsModel

end Sembla.Composition.SurfaceTests
