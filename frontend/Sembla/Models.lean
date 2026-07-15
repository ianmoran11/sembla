import Sembla.Json
import Sembla.DSL

namespace Sembla.Models
open Sembla.IR Sembla.DSL

/-- The standalone SIR fixture, authored entirely in one contextual DSL term. -/
def sir : Model := model% "sir_workplace_frequency_dependent" step(0.25) where
  params [
    param beta : Real := 0.8 prior LogNormal(-0.2231435513142097, 0.25),
    param gamma : Real := 0.1 prior LogNormal(-2.302585092994046, 0.25)]
  boxes [
    box sir where
      systems [
        system Person as "person" rows(1000000) where [
          state health : {S, I, R},
          ref employer : Employer],
        system Employer as "employer" rows(50000) where []]
      inputs []
      transitions [
        transition infect on Person where
          guard health = S
          hazard parameter beta * (countBy employer (health = I) / sizeBy employer)
          set [health := I],
        transition recover on Person where
          guard health = I
          hazard parameter gamma
          set [health := R]]
      outputs []]
  wires []

/-- The two-box feedback fixture.  Declaration order intentionally preserves
    population rule IDs 0 and 1 for common-random-numbers parity. -/
def sirPolicy : Model := model% "sir_workplace_policy_feedback" step(0.25) where
  params [
    param beta : Real := 0.8 prior LogNormal(-0.2231435513142097, 0.25),
    param gamma : Real := 0.1 prior LogNormal(-2.302585092994046, 0.25)]
  boxes [
    box population where
      systems [
        system Person as "person" rows(1000000) where [
          state health : {S, I, R},
          ref employer : Employer],
        system Employer as "employer" rows(50000) where []]
      inputs [
        input restriction_modifier {modifier_offset : Real}]
      transitions [
        transition infect on Person where
          guard health = S
          hazard parameter beta * (countBy employer (health = I) / sizeBy employer) *
            (1.0 + inputSum restriction_modifier field modifier_offset)
          set [health := I],
        transition recover on Person where
          guard health = I
          hazard parameter gamma
          set [health := R]]
      outputs [
        output infection_count {infected : Int} from Person fields [
          field infected := count where health = I]],
    box policy where
      systems [
        system Controller as "controller" rows(1) where [
          state mode : {Open, Restricted},
          attr modifier : Real]]
      inputs [
        input infection_count {infected : Int}]
      transitions [
        transition restrict on Controller where
          guard mode = Open && inputSum infection_count field infected > 500
          hazard 1e300
          set [mode := Restricted, modifier := 0.4],
        transition reopen on Controller where
          guard mode = Restricted && inputSum infection_count field infected < 150
          hazard 1e300
          set [mode := Open, modifier := 1.0]]
      outputs [
        output restriction_modifier {modifier_offset : Real} from Controller fields [
          field modifier_offset := sum (modifier - 1.0)]]]
  wires [
    wire population infection_count -> policy infection_count,
    wire policy restriction_modifier -> population restriction_modifier]

/-- Canonical reversible two-state CTMC hazards, executed as fixed-dt tau leaps. -/
def reversibleCtmc : Model := model% "reversible_two_state_ctmc" step(0.1) where
  params [
    param rate_ab : Real := 0.4 prior LogNormal(-0.916290731874155, 0.25),
    param rate_ba : Real := 0.2 prior LogNormal(-1.6094379124341003, 0.25)]
  boxes [
    box chain where
      systems [
        system Particle as "particle" rows(100000) where [
          state phase : {A, B}]]
      inputs []
      transitions [
        transition move_ab on Particle where
          guard phase = A
          hazard parameter rate_ab
          set [phase := B],
        transition move_ba on Particle where
          guard phase = B
          hazard parameter rate_ba
          set [phase := A]]
      outputs []]
  wires []

/-- Bateman-chain hazards with a stable sink; stages advance on separate ticks. -/
def radioactiveDecayChain : Model := model% "radioactive_decay_chain" step(0.25) where
  params [
    param lambda_parent : Real := 0.25 prior LogNormal(-1.3862943611198906, 0.25),
    param lambda_daughter : Real := 0.08 prior LogNormal(-2.5257286443082556, 0.25)]
  boxes [
    box decay where
      systems [
        system Atom as "atom" rows(100000) where [
          state nuclide : {Parent, Daughter, Stable}]]
      inputs []
      transitions [
        transition parent_decay on Atom where
          guard nuclide = Parent
          hazard parameter lambda_parent
          set [nuclide := Daughter],
        transition daughter_decay on Atom where
          guard nuclide = Daughter
          hazard parameter lambda_daughter
          set [nuclide := Stable]]
      outputs []]
  wires []

/-- Frequency-dependent SIS hazards, frozen per tick, with exogenous importation. -/
def sisImportation : Model := model% "sis_with_importation" step(0.25) where
  params [
    param import_rate : Real := 0.02 prior LogNormal(-3.912023005428146, 0.25),
    param beta : Real := 0.7 prior LogNormal(-0.35667494393873245, 0.25),
    param gamma : Real := 0.2 prior LogNormal(-1.6094379124341003, 0.25)]
  boxes [
    box epidemic where
      systems [
        system Person as "person" rows(100000) where [
          state health : {S, I},
          ref community : Community],
        system Community as "community" rows(1000) where []]
      inputs []
      transitions [
        transition infect on Person where
          guard health = S
          hazard parameter import_rate + parameter beta *
            (countBy community (health = I) / sizeBy community)
          set [health := I],
        transition recover on Person where
          guard health = I
          hazard parameter gamma
          set [health := S]]
      outputs []]
  wires []

/-- Markovian SEIRS hazards under fixed-dt tau-leaping, with importation and waning. -/
def seirsWaning : Model := model% "seirs_with_waning_immunity" step(0.25) where
  params [
    param import_rate : Real := 0.01 prior LogNormal(-4.605170185988091, 0.25),
    param beta : Real := 0.8 prior LogNormal(-0.2231435513142097, 0.25),
    param sigma : Real := 0.25 prior LogNormal(-1.3862943611198906, 0.25),
    param gamma : Real := 0.1 prior LogNormal(-2.302585092994046, 0.25),
    param omega : Real := 0.02 prior LogNormal(-3.912023005428146, 0.25)]
  boxes [
    box epidemic where
      systems [
        system Person as "person" rows(100000) where [
          state health : {S, E, I, R},
          ref community : Community],
        system Community as "community" rows(1000) where []]
      inputs []
      transitions [
        transition expose on Person where
          guard health = S
          hazard parameter import_rate + parameter beta *
            (countBy community (health = I) / sizeBy community)
          set [health := E],
        transition progress on Person where
          guard health = E
          hazard parameter sigma
          set [health := I],
        transition recover on Person where
          guard health = I
          hazard parameter gamma
          set [health := R],
        transition wane on Person where
          guard health = R
          hazard parameter omega
          set [health := S]]
      outputs []]
  wires []

/-- Mean-field noisy voter hazards with per-tick snapshot mutation and imitation. -/
def noisyVoter : Model := model% "noisy_voter_mean_field" step(0.25) where
  params [
    param mutation_rate : Real := 0.02 prior LogNormal(-3.912023005428146, 0.25),
    param imitation_rate : Real := 0.8 prior LogNormal(-0.2231435513142097, 0.25)]
  boxes [
    box opinions where
      systems [
        system Agent as "agent" rows(100000) where [
          state opinion : {A, B},
          ref community : Community],
        system Community as "community" rows(1000) where []]
      inputs []
      transitions [
        transition adopt_b on Agent where
          guard opinion = A
          hazard parameter mutation_rate + parameter imitation_rate *
            (countBy community (opinion = B) / sizeBy community)
          set [opinion := B],
        transition adopt_a on Agent where
          guard opinion = B
          hazard parameter mutation_rate + parameter imitation_rate *
            (countBy community (opinion = A) / sizeBy community)
          set [opinion := A]]
      outputs []]
  wires []

end Sembla.Models
