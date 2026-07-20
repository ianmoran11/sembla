import Sembla.DSL

/-!
# Step 04 — Views and summaries

The workplace model now reports susceptible, infectious, and recovered counts.
Views are per-tick observation sinks: they do not feed transitions. Summaries
fold those view streams after execution, here recording peak prevalence and
the earliest tick at which the peak occurs.
-/

namespace Sembla.Tutorial.Step04

open Sembla.IR Sembla.DSL

/- Workplace SIR with non-feedback observations and run-level summaries. -/
sembla_model observedWorkplaceSIR
    (name := "tutorial_04_observed_workplace_sir")
    (dt := 0.25) where
  param β : ℝ := 0.8 ~ LogNormal (-0.2231435513142097) 0.25
  param γ : ℝ := 0.1 ~ LogNormal (-2.302585092994046) 0.25

  box population where
    system Person (rows := 1_000) where
      health : {S, I, R}
      employer : Employer
    system Employer (rows := 50)

    infect on Person : health: S →[β · freq (health = I) over employer] I
    recover on Person : health: I →[γ] R

    view susceptible := count Person where health = S
    view infectious := count Person where health = I
    view recovered := count Person where health = R

  summary peak_infectious := max population.infectious
  summary peak_tick := argmaxₜ population.infectious

#guard observedWorkplaceSIR.boxes.map (fun modelBox => modelBox.views.map (·.name)) ==
  [["susceptible", "infectious", "recovered"]]
#guard observedWorkplaceSIR.summaries.map (·.name) ==
  ["peak_infectious", "peak_tick"]
#guard observedWorkplaceSIR.summaries.map (·.reduce) ==
  [SummaryReduce.max, .argmaxTick]

end Sembla.Tutorial.Step04
