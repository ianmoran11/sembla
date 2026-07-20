import Sembla.DSL

/-!
# Step 02 — Parameters, priors, and a second transition

The model now exposes infection and recovery rates as parameters. Their
`LogNormal` priors remain first-class IR data for calibration and widgets;
defaults are not substituted into transition expressions.

At this stage infection is a deliberately simple constant external force:
every susceptible person has hazard `beta`, independent of prevalence. This is
an epidemic-shaped parameter scaffold, not yet homogeneous-mixing SIR
transmission. Step 03 replaces it with workplace-dependent exposure.
-/

namespace Sembla.Tutorial.Step02

open Sembla.IR Sembla.DSL

/-- Parameterized epidemic scaffold, not yet interaction-dependent SIR. -/
def parameterizedScaffold : Model := model% "tutorial_02_parameterized_scaffold" step(0.25) where
  params [
    param beta : Real := 0.8 prior LogNormal(-0.2231435513142097, 0.25),
    param gamma : Real := 0.1 prior LogNormal(-2.302585092994046, 0.25)]
  boxes [
    box population where
      systems [
        system Person as "person" rows(1000) where [
          state health : {S, I, R}]]
      inputs []
      transitions [
        transition infect on Person where
          guard health = S
          hazard parameter beta
          set [health := I],
        transition recover on Person where
          guard health = I
          hazard parameter gamma
          set [health := R]]
      outputs []]
  wires []

#guard parameterizedScaffold.params.map (·.name) == ["beta", "gamma"]
#guard parameterizedScaffold.params.all (·.prior.isSome)
#guard parameterizedScaffold.boxes.map (fun modelBox => modelBox.transitions.map (·.name)) ==
  [["infect", "recover"]]

end Sembla.Tutorial.Step02
