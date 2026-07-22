import Sembla.Composition.Surface

namespace Sembla.Composition.SurfaceModels

open Sembla.Composition.Surface

sembla_component Population where
  requires beta
  requires gamma

  system Person (rows := 1000) where
    health : {S, I, R}
    employer : Employer
  system Employer (rows := 50)

  input restriction_modifier where
    restriction : ℝ

  infect on Person : health: S →[
    beta · freq (health = I) over employer ·
      (1.0 + inputSum restriction_modifier field restriction)
  ] I
  recover on Person : health: I →[gamma] R

  output infection_count from Person where
    infected : Int := count where health = I

  view S := count Person where health = S
  view I := count Person where health = I
  view R := count Person where health = R

sembla_component Policy where
  system Controller (rows := 1) where
    mode : {Open, Restricted}
    modifier : ℝ

  input infection_count where
    infected : Int

  transition restrict on Controller where
    guard mode = Open ∧ inputSum infection_count field infected > 500
    hazard 1e300
    set mode := Restricted
    set modifier := 0.4

  transition reopen on Controller where
    guard mode = Restricted ∧ inputSum infection_count field infected < 150
    hazard 1e300
    set mode := Open
    set modifier := 1.0

  output restriction_modifier from Controller where
    restriction : ℝ := sum (modifier - 1.0)

sembla_component IndependentEpidemicPolicy
    (display := "Independent epidemic and policy") where
  instance population := Population
  instance policy := Policy

namespace Plain

sembla_component EpidemicPolicy where
  instance population := Population
  instance policy := Policy
  wire count_to_policy : population.infection_count -> policy.infection_count
  wire restriction_to_population : policy.restriction_modifier -> population.restriction_modifier

end Plain

namespace Exposing

sembla_component EpidemicPolicy where
  instance population := Population
  instance policy := Policy
  wire count_to_policy : population.infection_count -> policy.infection_count
  wire restriction_to_population : policy.restriction_modifier -> population.restriction_modifier
  expose infection_count : population.infection_count as infection_count
  expose restriction_modifier : policy.restriction_modifier as restriction_modifier

end Exposing

sembla_component TwoRegions where
  instance north := Exposing.EpidemicPolicy
  instance south := Exposing.EpidemicPolicy

sembla_composition epidemicPolicyModel
    (name := "epidemic_policy") (dt := 0.25) where
  param beta : ℝ := 0.8 ~ LogNormal (-0.2231435513142097) 0.25
  param gamma : ℝ := 0.1 ~ LogNormal (-2.302585092994046) 0.25
  root Plain.EpidemicPolicy

sembla_composition independentEpidemicAndPolicyModel
    (name := "independent_epidemic_policy") (dt := 0.25) where
  param beta : ℝ := 0.8 ~ LogNormal (-0.2231435513142097) 0.25
  param gamma : ℝ := 0.1 ~ LogNormal (-2.302585092994046) 0.25
  root IndependentEpidemicPolicy

sembla_composition twoRegionsModel
    (name := "two_regions") (dt := 0.25) where
  param beta : ℝ := 0.8 ~ LogNormal (-0.2231435513142097) 0.25
  param gamma : ℝ := 0.1 ~ LogNormal (-2.302585092994046) 0.25
  root TwoRegions
  summary peak_i := max north.population.I

/-- Lean-authored composition registry. Surface-prefixed aliases let parity
    exercise this path independently while the canonical names replace the
    hand-value export path with byte-identical values. -/
def lookup (name : String) : Option CompositionSourceV1 :=
  match name with
  | "epidemic_policy" | "surface_epidemic_policy" => some epidemicPolicyModel
  | "independent_epidemic_policy" | "surface_independent_epidemic_policy" =>
      some independentEpidemicAndPolicyModel
  | "two_regions" | "surface_two_regions" => some twoRegionsModel
  | _ => none

end Sembla.Composition.SurfaceModels
