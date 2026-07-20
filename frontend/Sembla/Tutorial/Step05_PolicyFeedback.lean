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
owns those row values because `(rows := 1)` is only a size hint.
-/

namespace Sembla.Tutorial.Step05

open Sembla.IR Sembla.DSL

/- Composed workplace SIR with typed, one-tick-delayed policy feedback. -/
sembla_model policyFeedbackSIR
    (name := "tutorial_05_policy_feedback_sir")
    (dt := 0.25) where
  param β : ℝ := 0.8 ~ LogNormal (-0.2231435513142097) 0.25
  param γ : ℝ := 0.1 ~ LogNormal (-2.302585092994046) 0.25

  box population where
    system Person (rows := 1_000) where
      health : {S, I, R}
      employer : Employer
    system Employer (rows := 50)

    input restriction_modifier where
      restriction : ℝ

    infect on Person : health: S →[
      β · freq (health = I) over employer ·
        (1.0 - inputSum restriction_modifier field restriction)
    ] I
    recover on Person : health: I →[γ] R

    output infection_count from Person where
      infected : Int := count where health = I

    view susceptible := count Person where health = S
    view infectious := count Person where health = I
    view recovered := count Person where health = R

  box policy where
    system Controller (rows := 1) where
      mode : {Open, Restricted}
      restriction : ℝ

    input infection_count where
      infected : Int

    transition restrict on Controller where
      guard mode = Open ∧ inputSum infection_count field infected > 100
      hazard 1e300
      set mode := Restricted
      set restriction := 0.6

    transition reopen on Controller where
      guard mode = Restricted ∧ inputSum infection_count field infected < 25
      hazard 1e300
      set mode := Open
      set restriction := 0.0

    output restriction_modifier from Controller where
      restriction : ℝ := sum (restriction)

  wire population infection_count -> policy infection_count
  wire policy restriction_modifier -> population restriction_modifier
  summary peak_infectious := max population.infectious
  summary peak_tick := argmaxₜ population.infectious

#guard policyFeedbackSIR.boxes.map (·.name) == ["population", "policy"]
#guard policyFeedbackSIR.wires.length == 2
#guard policyFeedbackSIR.wires.map (fun connection =>
    (connection.source.box, connection.source.port,
      connection.target.box, connection.target.port)) ==
  [ ("population", "infection_count", "policy", "infection_count")
  , ("policy", "restriction_modifier", "population", "restriction_modifier")
  ]

end Sembla.Tutorial.Step05
