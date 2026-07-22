import Sembla.Composition.SurfaceModels

/-!
# Composition showcase models

Four executable demonstrations of the composition pipeline:

* `counterfactualOutbreakModel` — repeated primitive components, explicit
  parameter bindings, hidden boundary inputs, and parallel counterfactuals;
* `coordinatedRegionsModel` — cross-region sensing and one controller output
  fanned out to two population inputs;
* `regionalSurveillanceModel` — nested epidemic-policy components connected
  through exposed ports to a surveillance dashboard; and
* `nationalNetworkModel` — repeated nested regional systems, exposure chains,
  a national dashboard, and deep occurrence identities.
-/

namespace Sembla.Demos.Composition

open Sembla.Composition.Surface

sembla_component RegionalCoordinator
    (display := "Shared regional policy coordinator") where
  system Coordinator (rows := 1) where
    mode : {Open, Restricted}
    modifier : ℝ

  input north_cases where
    infected : Int

  input south_cases where
    infected : Int

  transition restrict on Coordinator where
    guard mode = Open ∧
      inputSum north_cases field infected + inputSum south_cases field infected > 600
    hazard 1e300
    set mode := Restricted
    set modifier := 0.4

  transition reopen on Coordinator where
    guard mode = Restricted ∧
      inputSum north_cases field infected + inputSum south_cases field infected < 200
    hazard 1e300
    set mode := Open
    set modifier := 1.0

  output restriction_modifier from Coordinator where
    restriction : ℝ := sum (modifier - 1.0)

  view Restricted := count Coordinator where mode = Restricted

sembla_component SurveillanceDashboard
    (display := "Two-region surveillance dashboard") where
  system Dashboard (rows := 1) where
    burden : {Normal, High}

  input north_cases where
    infected : Int

  input south_cases where
    infected : Int

  transition raise_alert on Dashboard where
    guard burden = Normal ∧
      inputSum north_cases field infected + inputSum south_cases field infected > 700
    hazard 1e300
    set burden := High

  transition clear_alert on Dashboard where
    guard burden = High ∧
      inputSum north_cases field infected + inputSum south_cases field infected < 250
    hazard 1e300
    set burden := Normal

  view HighBurden := count Dashboard where burden = High

sembla_component NationalDashboard
    (display := "National four-region surveillance dashboard") where
  system Dashboard (rows := 1) where
    burden : {Normal, High}

  input east_north_cases where
    infected : Int

  input east_south_cases where
    infected : Int

  input west_north_cases where
    infected : Int

  input west_south_cases where
    infected : Int

  transition raise_alert on Dashboard where
    guard burden = Normal ∧
      inputSum east_north_cases field infected +
      inputSum east_south_cases field infected +
      inputSum west_north_cases field infected +
      inputSum west_south_cases field infected > 1200
    hazard 1e300
    set burden := High

  transition clear_alert on Dashboard where
    guard burden = High ∧
      inputSum east_north_cases field infected +
      inputSum east_south_cases field infected +
      inputSum west_north_cases field infected +
      inputSum west_south_cases field infected < 500
    hazard 1e300
    set burden := Normal

  view HighBurden := count Dashboard where burden = High

sembla_component Region
    (display := "Reusable open-loop epidemic region") where
  instance population := Sembla.Composition.SurfaceModels.Population
  expose infection_count : population.infection_count as infection_count
  expose restriction_modifier : population.restriction_modifier as restriction_modifier

sembla_component CounterfactualOutbreak
    (display := "Parallel outbreak counterfactuals") where
  instance control := Region (
    beta := control_beta,
    gamma := recovery_rate)
  instance high_contact := Region (
    beta := high_contact_beta,
    gamma := recovery_rate)
  hide control.restriction_modifier
  hide high_contact.restriction_modifier

sembla_component CoordinatedRegions
    (display := "Two regions with shared policy coordination") where
  instance north := Region (
    beta := north_beta,
    gamma := recovery_rate)
  instance south := Region (
    beta := south_beta,
    gamma := recovery_rate)
  instance coordinator := RegionalCoordinator
  wire north_cases : north.infection_count -> coordinator.north_cases
  wire south_cases : south.infection_count -> coordinator.south_cases
  wire north_restriction : coordinator.restriction_modifier -> north.restriction_modifier
  wire south_restriction : coordinator.restriction_modifier -> south.restriction_modifier
  expose north_cases : north.infection_count as north_cases
  expose south_cases : south.infection_count as south_cases

sembla_component RegionalSurveillance
    (display := "Nested regional policy with surveillance") where
  instance north := Sembla.Composition.SurfaceModels.Exposing.EpidemicPolicy (
    beta := north_beta,
    gamma := recovery_rate)
  instance south := Sembla.Composition.SurfaceModels.Exposing.EpidemicPolicy (
    beta := south_beta,
    gamma := recovery_rate)
  instance dashboard := SurveillanceDashboard
  wire north_cases : north.infection_count -> dashboard.north_cases
  wire south_cases : south.infection_count -> dashboard.south_cases

sembla_component NationalNetwork
    (display := "Nested national regional network") where
  instance east := CoordinatedRegions (
    north_beta := east_north_beta,
    south_beta := east_south_beta,
    recovery_rate := recovery_rate)
  instance west := CoordinatedRegions (
    north_beta := west_north_beta,
    south_beta := west_south_beta,
    recovery_rate := recovery_rate)
  instance dashboard := NationalDashboard
  wire east_north_cases : east.north_cases -> dashboard.east_north_cases
  wire east_south_cases : east.south_cases -> dashboard.east_south_cases
  wire west_north_cases : west.north_cases -> dashboard.west_north_cases
  wire west_south_cases : west.south_cases -> dashboard.west_south_cases

sembla_composition counterfactualOutbreakModel
    (name := "demo_counterfactual_outbreak") (dt := 0.25) where
  param control_beta : ℝ := 0.45
  param high_contact_beta : ℝ := 0.9
  param recovery_rate : ℝ := 0.1
  root CounterfactualOutbreak
  summary control_peak := max control.population.I
  summary high_contact_peak := max high_contact.population.I

sembla_composition coordinatedRegionsModel
    (name := "demo_coordinated_regions") (dt := 0.25) where
  param north_beta : ℝ := 0.8
  param south_beta : ℝ := 0.65
  param recovery_rate : ℝ := 0.1
  root CoordinatedRegions
  summary north_peak := max north.population.I
  summary south_peak := max south.population.I
  summary restricted_ticks := sum coordinator.Restricted

sembla_composition regionalSurveillanceModel
    (name := "demo_regional_surveillance") (dt := 0.25) where
  param north_beta : ℝ := 0.8
  param south_beta : ℝ := 0.65
  param recovery_rate : ℝ := 0.1
  root RegionalSurveillance
  summary north_peak := max north.population.I
  summary south_peak := max south.population.I
  summary alert_ticks := sum dashboard.HighBurden

sembla_composition nationalNetworkModel
    (name := "demo_national_network") (dt := 0.25) where
  param east_north_beta : ℝ := 0.8
  param east_south_beta : ℝ := 0.7
  param west_north_beta : ℝ := 0.6
  param west_south_beta : ℝ := 0.5
  param recovery_rate : ℝ := 0.1
  root NationalNetwork
  summary east_north_peak := max east.north.population.I
  summary east_south_peak := max east.south.population.I
  summary west_north_peak := max west.north.population.I
  summary west_south_peak := max west.south.population.I
  summary national_alert_ticks := sum dashboard.HighBurden

def lookup (name : String) : Option Sembla.Composition.CompositionSourceV1 :=
  match name with
  | "demo_counterfactual_outbreak" => some counterfactualOutbreakModel
  | "demo_coordinated_regions" => some coordinatedRegionsModel
  | "demo_regional_surveillance" => some regionalSurveillanceModel
  | "demo_national_network" => some nationalNetworkModel
  | _ => none

end Sembla.Demos.Composition
