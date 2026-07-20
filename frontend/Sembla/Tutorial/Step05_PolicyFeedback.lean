import Sembla.DSL

/-!
# Step 05 — Boxes, ports, outputs, and feedback wires

A policy controller is added as a second box. The population emits an
`infection_count` table; the controller reads it and emits a restriction
modifier back to the population. Schemas are checked during elaboration.

Every wire has a uniform one-tick delay. The controller therefore reacts to
the previous tick's population output rather than observing mutable global
state. Views remain observation-only; explicit ports and wires carry feedback.

The feedback carries a restriction amount, not a multiplier. Empty tick-zero
input sums to `0`, so `1 - 0 = 1` is neutral. A concrete population file must
initialize the controller as `mode = Open` and `restriction = 0.0`; the runtime
owns those row values because `rows(1)` is only a size hint.
-/

namespace Sembla.Tutorial.Step05

open Sembla.IR Sembla.DSL

/-- Composed workplace SIR with typed, one-tick-delayed policy feedback. -/
def policyFeedbackSIR : Model := model% "tutorial_05_policy_feedback_sir" step(0.25) where
  params [
    param beta : Real := 0.8 prior LogNormal(-0.2231435513142097, 0.25),
    param gamma : Real := 0.1 prior LogNormal(-2.302585092994046, 0.25)]
  boxes [
    box population where
      systems [
        system Person as "person" rows(1000) where [
          state health : {S, I, R},
          ref employer : Employer],
        system Employer as "employer" rows(50) where []]
      inputs [
        input restriction_modifier {restriction : Real}]
      transitions [
        transition infect on Person where
          guard health = S
          hazard parameter beta *
            (countBy employer (health = I) / sizeBy employer) *
            (1.0 - inputSum restriction_modifier field restriction)
          set [health := I],
        transition recover on Person where
          guard health = I
          hazard parameter gamma
          set [health := R]]
      outputs [
        output infection_count {infected : Int} from Person fields [
          field infected := count where health = I]]
      views [
        view susceptible from Person where health = S reduce count,
        view infectious from Person where health = I reduce count,
        view recovered from Person where health = R reduce count],
    box policy where
      systems [
        system Controller as "controller" rows(1) where [
          state mode : {Open, Restricted},
          attr restriction : Real]]
      inputs [
        input infection_count {infected : Int}]
      transitions [
        transition restrict on Controller where
          guard mode = Open && inputSum infection_count field infected > 100
          hazard 1e300
          set [mode := Restricted, restriction := 0.6],
        transition reopen on Controller where
          guard mode = Restricted && inputSum infection_count field infected < 25
          hazard 1e300
          set [mode := Open, restriction := 0.0]]
      outputs [
        output restriction_modifier {restriction : Real} from Controller fields [
          field restriction := sum (restriction)]]]
  wires [
    wire population infection_count -> policy infection_count,
    wire policy restriction_modifier -> population restriction_modifier]
  summaries [
    summary peak_infectious from population view infectious reduce max,
    summary peak_tick from population view infectious reduce argmax_tick]

#guard policyFeedbackSIR.boxes.map (·.name) == ["population", "policy"]
#guard policyFeedbackSIR.wires.length == 2
#guard policyFeedbackSIR.wires.map (fun connection =>
    (connection.source.box, connection.source.port,
      connection.target.box, connection.target.port)) ==
  [ ("population", "infection_count", "policy", "infection_count")
  , ("policy", "restriction_modifier", "population", "restriction_modifier")
  ]

end Sembla.Tutorial.Step05
