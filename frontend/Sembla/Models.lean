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

end Sembla.Models
