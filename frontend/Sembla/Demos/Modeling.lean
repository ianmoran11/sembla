import Sembla.DSL
import Sembla.Json

/-!
# Modeling feature tour

This executable tutorial model demonstrates the public `sembla_model` command surface:

* exact decimal steps, parameters, optional `LogNormal` priors, and symbolic
  parameter references;
* enum, real, integer, and forward-reference attributes;
* guarded transitions, arithmetic, `freq`, `inputSum`, and effects;
* per-table count/sum outputs, typed inputs, and two-box feedback wiring;
* filtered and valued views with every view reduction; and
* every summary reduction.

Put the cursor on a `system` or transition-name declaration in VS Code's Lean
infoview to see the structure widgets attached by the elaborator. `(rows := ...)`
becomes an IR `Table.sizeHint`; runtime population initialization remains
external. Wires carry one-tick-delayed tables; the Lean frontend elaborates the
model to the deep IR but does not execute the Rust runtime.
-/

namespace Sembla.Demos.Modeling

open Sembla.IR Sembla.DSL

set_option sembla.widget.theme "notebook"

/- A compact two-box model that exercises the complete supported DSL surface. -/
sembla_model featureTour
    (name := "lean_feature_tour")
    (dt := 0.25) where
  param β : ℝ := 0.8 ~ LogNormal (-0.2231435513142097) 0.25
  param γ : ℝ := 0.1

  box population where
    system Person (rows := 1_000) where
      health : {S, I, R}
      risk : ℝ
      visits : Int
      employer : Employer
    system Employer (rows := 50)

    input restriction where
      modifier : ℝ

    transition infect on Person where
      guard health = S ∧ risk > 0.0
      hazard β · freq (health = I) over employer ·
        (1.0 + inputSum restriction field modifier)
      set health := I
      set risk := 0.5

    transition recover on Person where
      guard health = I
      hazard γ
      set health := R
      set visits := 1

    output activity from Person where
      infected : Int := count where health = I
      total_risk : ℝ := sum (risk)

    view infectious := count Person where health = I
    view total_risk := sum Person using risk
    view minimum_visits := min Person using visits
    view maximum_infectious_risk := max Person where health = I using risk

  box policy where
    system Controller (rows := 1) where
      mode : {Open, Restricted}
      modifier : ℝ

    input activity where
      infected : Int
      total_risk : ℝ

    transition restrict on Controller where
      guard mode = Open ∧ inputSum activity field infected > 100
      hazard 1e300
      set mode := Restricted
      set modifier := 0.4

    transition reopen on Controller where
      guard mode = Restricted ∧ inputSum activity field infected < 25
      hazard 1e300
      set mode := Open
      set modifier := 1.0

    output restriction from Controller where
      modifier : ℝ := sum (modifier - 1.0)

    view controllers := count Controller

  wire population activity -> policy activity
  wire policy restriction -> population restriction
  summary cumulative_risk := sum population.total_risk
  summary lowest_visit_count := min population.minimum_visits
  summary peak_infectious_risk := max population.maximum_infectious_risk
  summary final_infectious := last population.infectious
  summary peak_infectious_tick := argmaxₜ population.infectious

#guard featureTour.name == "lean_feature_tour"
#guard featureTour.params.length == 2
#guard featureTour.boxes.map (·.name) == ["population", "policy"]
#guard featureTour.wires.length == 2
#guard featureTour.summaries.map (·.name) == [
  "cumulative_risk",
  "lowest_visit_count",
  "peak_infectious_risk",
  "final_infectious",
  "peak_infectious_tick"
]

end Sembla.Demos.Modeling
