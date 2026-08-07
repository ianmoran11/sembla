import Sembla.Json
import Sembla.DSL
import Sembla.Models.DemographicSlots
import Sembla.Models.AustralianPopulation

namespace Sembla.Models
open Sembla.IR Sembla.DSL

/- The standalone SIR fixture, authored entirely with the command surface. -/
sembla_model sir
    (name := "sir_workplace_frequency_dependent")
    (dt := 0.25) where
  param β : ℝ := 0.8 ~ LogNormal (-0.2231435513142097) 0.25
  param γ : ℝ := 0.1 ~ LogNormal (-2.302585092994046) 0.25

  box sir where
    system Person (rows := 1_000_000) where
      health : {S, I, R}
      employer : Employer
    system Employer (rows := 50_000)

    infect on Person : health: S → [β · freq (health = I) over employer] I
    recover on Person : health: I → [γ] R

    view S := count Person where health = S
    view I := count Person where health = I
    view R := count Person where health = R

  summary peak_I := max sir.I
  summary peak_tick := argmaxₜ sir.I

/- The PRD-0002 observation fixture covers every view reduction. -/
sembla_model observations (dt := 1.0) where
  box population where
    system Person (name := "Person") (rows := 4) where
      status : {active, inactive}
      value : ℝ
      visits : Int

    view total_value := sum Person using value
    view active_count := count Person where status = active
    view minimum_visits := min Person using visits
    view maximum_value := max Person using value

  summary total_value_over_time := sum population.total_value
  summary peak_value_tick := argmaxₜ population.maximum_value

/- The two-box feedback fixture.  Declaration order intentionally preserves
   population rule IDs 0 and 1 for common-random-numbers parity. -/
sembla_model sirPolicy
    (name := "sir_workplace_policy_feedback")
    (dt := 0.25) where
  param β : ℝ := 0.8 ~ LogNormal (-0.2231435513142097) 0.25
  param γ : ℝ := 0.1 ~ LogNormal (-2.302585092994046) 0.25

  box population where
    system Person (rows := 1_000_000) where
      health : {S, I, R}
      employer : Employer
    system Employer (rows := 50_000)

    input restriction_modifier where
      modifier_offset : ℝ

    infect on Person : health: S → [
      β · freq (health = I) over employer ·
        (1.0 + inputSum restriction_modifier field modifier_offset)
    ] I
    recover on Person : health: I → [γ] R

    output infection_count from Person where
      infected : Int := count where health = I

    view S := count Person where health = S
    view I := count Person where health = I
    view R := count Person where health = R

  box policy where
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
      modifier_offset : ℝ := sum (modifier - 1.0)

  wire population infection_count -> policy infection_count
  wire policy restriction_modifier -> population restriction_modifier
  summary peak_I := max population.I
  summary peak_tick := argmaxₜ population.I

/- Canonical reversible two-state CTMC hazards, executed as fixed-dt tau leaps. -/
sembla_model reversibleCtmc
    (name := "reversible_two_state_ctmc")
    (dt := 0.1) where
  param rate_ab : ℝ := 0.4 ~ LogNormal (-0.916290731874155) 0.25
  param rate_ba : ℝ := 0.2 ~ LogNormal (-1.6094379124341003) 0.25

  box chain where
    system Particle (rows := 100_000) where
      phase : {A, B}

    move_ab on Particle : phase: A → [rate_ab] B
    move_ba on Particle : phase: B → [rate_ba] A

/- Bateman-chain hazards with a stable sink; stages advance on separate ticks. -/
sembla_model radioactiveDecayChain
    (name := "radioactive_decay_chain")
    (dt := 0.25) where
  param «λ_parent» : ℝ := 0.25 ~ LogNormal (-1.3862943611198906) 0.25
  param lambda_daughter : ℝ := 0.08 ~ LogNormal (-2.5257286443082556) 0.25

  box decay where
    system Atom (rows := 100_000) where
      nuclide : {Parent, Daughter, Stable}

    parent_decay on Atom : nuclide: Parent → [«λ_parent»] Daughter
    daughter_decay on Atom : nuclide: Daughter → [lambda_daughter] Stable

/- Frequency-dependent SIS hazards, frozen per tick, with exogenous importation. -/
sembla_model sisImportation
    (name := "sis_with_importation")
    (dt := 0.25) where
  param import_rate : ℝ := 0.02 ~ LogNormal (-3.912023005428146) 0.25
  param β : ℝ := 0.7 ~ LogNormal (-0.35667494393873245) 0.25
  param γ : ℝ := 0.2 ~ LogNormal (-1.6094379124341003) 0.25

  box epidemic where
    system Person (rows := 100_000) where
      health : {S, I}
      community : Community
    system Community (rows := 1_000)

    infect on Person : health: S → [
      import_rate + β · freq (health = I) over community
    ] I
    recover on Person : health: I → [γ] S

/- Markovian SEIRS hazards under fixed-dt tau-leaping, with importation and waning. -/
sembla_model seirsWaning
    (name := "seirs_with_waning_immunity")
    (dt := 0.25) where
  param import_rate : ℝ := 0.01 ~ LogNormal (-4.605170185988091) 0.25
  param β : ℝ := 0.8 ~ LogNormal (-0.2231435513142097) 0.25
  param σ : ℝ := 0.25 ~ LogNormal (-1.3862943611198906) 0.25
  param γ : ℝ := 0.1 ~ LogNormal (-2.302585092994046) 0.25
  param omega : ℝ := 0.02 ~ LogNormal (-3.912023005428146) 0.25

  box epidemic where
    system Person (rows := 100_000) where
      health : {S, E, I, R}
      community : Community
    system Community (rows := 1_000)

    expose on Person : health: S → [
      import_rate + β · freq (health = I) over community
    ] E
    progress on Person : health: E → [σ] I
    recover on Person : health: I → [γ] R
    wane on Person : health: R → [omega] S

/- Mean-field noisy voter hazards with per-tick snapshot mutation and imitation. -/
sembla_model noisyVoter
    (name := "noisy_voter_mean_field")
    (dt := 0.25) where
  param mutation_rate : ℝ := 0.02 ~ LogNormal (-3.912023005428146) 0.25
  param imitation_rate : ℝ := 0.8 ~ LogNormal (-0.2231435513142097) 0.25

  box opinions where
    system Agent (rows := 100_000) where
      opinion : {A, B}
      community : Community
    system Community (rows := 1_000)

    adopt_b on Agent : opinion: A → [
      mutation_rate + imitation_rate · freq (opinion = B) over community
    ] B
    adopt_a on Agent : opinion: B → [
      mutation_rate + imitation_rate · freq (opinion = A) over community
    ] A

end Sembla.Models
