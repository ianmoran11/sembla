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

/-- Workplace SIR with non-feedback observations and run-level summaries. -/
def observedWorkplaceSIR : Model := model% "tutorial_04_observed_workplace_sir" step(0.25) where
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
      inputs []
      transitions [
        transition infect on Person where
          guard health = S
          hazard parameter beta *
            (countBy employer (health = I) / sizeBy employer)
          set [health := I],
        transition recover on Person where
          guard health = I
          hazard parameter gamma
          set [health := R]]
      outputs []
      views [
        view susceptible from Person where health = S reduce count,
        view infectious from Person where health = I reduce count,
        view recovered from Person where health = R reduce count]]
  wires []
  summaries [
    summary peak_infectious from population view infectious reduce max,
    summary peak_tick from population view infectious reduce argmax_tick]

#guard observedWorkplaceSIR.boxes.map (fun modelBox => modelBox.views.map (·.name)) ==
  [["susceptible", "infectious", "recovered"]]
#guard observedWorkplaceSIR.summaries.map (·.name) ==
  ["peak_infectious", "peak_tick"]
#guard observedWorkplaceSIR.summaries.map (·.reduce) ==
  [SummaryReduce.max, .argmaxTick]

end Sembla.Tutorial.Step04
