import Sembla.DSL

/-!
# Step 02 — Parameters, priors, and a second transition

The model now exposes infection and recovery rates as parameters. Their
`LogNormal` priors remain first-class IR data for calibration and widgets;
defaults are not substituted into transition expressions.

At this stage infection is a deliberately simple constant external force:
every susceptible person has hazard `β`, independent of prevalence. This is
an epidemic-shaped parameter scaffold, not yet homogeneous-mixing SIR
transmission. Step 03 replaces it with workplace-dependent exposure.
-/

namespace Sembla.Tutorial.Step02

open Sembla.IR Sembla.DSL

/- Parameterized epidemic scaffold, not yet interaction-dependent SIR. -/
sembla_model parameterizedScaffold
    (name := "tutorial_02_parameterized_scaffold")
    (dt := 0.25) where
  param β : ℝ := 0.8 ~ LogNormal (-0.2231435513142097) 0.25
  param γ : ℝ := 0.1 ~ LogNormal (-2.302585092994046) 0.25

  box population where
    system Person (rows := 1_000) where
      health : {S, I, R}

    infect on Person : health: S →[β] I
    recover on Person : health: I →[γ] R

#guard parameterizedScaffold.params.map (·.name) == ["beta", "gamma"]
#guard parameterizedScaffold.params.all (·.prior.isSome)
#guard parameterizedScaffold.boxes.map (fun modelBox => modelBox.transitions.map (·.name)) ==
  [["infect", "recover"]]

end Sembla.Tutorial.Step02
