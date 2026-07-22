import Sembla.Demos.Composition
import Sembla.Composition.Link

namespace Sembla.Demos.CompositionTests

open Sembla
open Sembla.Composition
open Sembla.Demos.Composition

private def linkedPlan? (source : CompositionSourceV1) : Option Plan.ExecutablePlanV1 :=
  match linkV1 source (Json.render source) with
  | .ok result => some result.plan
  | .error _ => none

private def everyBindingIsExplicit (source : CompositionSourceV1) : Bool :=
  source.definitions.all fun definition =>
    match definition.body with
    | .primitive _ => true
    | .composite body => body.instances.all fun instance_ =>
        match source.definitions.find? (·.id == instance_.definition) with
        | none => false
        | some child => instance_.parameterBindings.map (·.requirement) ==
            child.parameterRequirements

#guard counterfactualOutbreakModel.modelId.raw == "model:demo_counterfactual_outbreak"
#guard coordinatedRegionsModel.modelId.raw == "model:demo_coordinated_regions"
#guard regionalSurveillanceModel.modelId.raw == "model:demo_regional_surveillance"
#guard nationalNetworkModel.modelId.raw == "model:demo_national_network"

#guard everyBindingIsExplicit counterfactualOutbreakModel
#guard everyBindingIsExplicit coordinatedRegionsModel
#guard everyBindingIsExplicit regionalSurveillanceModel
#guard everyBindingIsExplicit nationalNetworkModel

#guard (linkedPlan? counterfactualOutbreakModel).isSome
#guard (linkedPlan? coordinatedRegionsModel).isSome
#guard (linkedPlan? regionalSurveillanceModel).isSome
#guard (linkedPlan? nationalNetworkModel).isSome

private def coordinatedIdentityGuard : Bool :=
  match linkedPlan? coordinatedRegionsModel with
  | none => false
  | some plan =>
      plan.identity.leaves.map (·.occurrence) ==
        ["occ:coordinator", "occ:north/population", "occ:south/population"] &&
      plan.identity.mailboxes.length == 4

private def surveillanceIdentityGuard : Bool :=
  match linkedPlan? regionalSurveillanceModel with
  | none => false
  | some plan =>
      plan.identity.leaves.map (·.occurrence) ==
        ["occ:dashboard", "occ:north/policy", "occ:north/population",
          "occ:south/policy", "occ:south/population"] &&
      plan.identity.mailboxes.length == 6

private def nationalIdentityGuard : Bool :=
  match linkedPlan? nationalNetworkModel with
  | none => false
  | some plan =>
      plan.identity.leaves.map (·.occurrence) ==
        ["occ:dashboard", "occ:east/coordinator", "occ:east/north/population",
          "occ:east/south/population", "occ:west/coordinator",
          "occ:west/north/population", "occ:west/south/population"] &&
      plan.identity.transitions.length == 14 &&
      plan.identity.mailboxes.length == 12

#guard coordinatedIdentityGuard
#guard surveillanceIdentityGuard
#guard nationalIdentityGuard

end Sembla.Demos.CompositionTests
