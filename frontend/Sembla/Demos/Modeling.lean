import Sembla.DSL
import Sembla.Json

/-!
# Modeling feature tour

This executable tutorial model demonstrates the public `model%` surface:

* exact decimal steps, parameters, optional `LogNormal` priors, and symbolic
  parameter references;
* enum, real, integer, and forward-reference attributes;
* guarded transitions, arithmetic, `countBy`, `sizeBy`, `inputSum`, and effects;
* per-table count/sum outputs, typed inputs, and two-box feedback wiring;
* filtered and valued views with every view reduction; and
* every summary reduction.

Put the cursor on a `system` or `transition` declaration in VS Code's Lean
infoview to see the structure widgets attached by the elaborator. `rows(...)`
becomes an IR `Table.sizeHint`; runtime population initialization remains
external. Wires carry one-tick-delayed tables; the Lean frontend elaborates the
model to the deep IR but does not execute the Rust runtime.
-/

namespace Sembla.Demos.Modeling

open Sembla.IR Sembla.DSL

set_option sembla.widget.theme "notebook"

/-- A compact two-box model that exercises the complete supported DSL surface. -/
def featureTour : Model := model% "lean_feature_tour" step(0.25) where
  params [
    param beta : Real := 0.8 prior LogNormal(-0.2231435513142097, 0.25),
    param gamma : Real := 0.1]
  boxes [
    box population where
      systems [
        system Person as "person" rows(1000) where [
          state health : {S, I, R},
          attr risk : Real,
          attr visits : Int,
          ref employer : Employer],
        system Employer as "employer" rows(50) where []]
      inputs [
        input restriction {modifier : Real}]
      transitions [
        transition infect on Person where
          guard health = S && risk > 0.0
          hazard parameter beta *
            (countBy employer (health = I) / sizeBy employer) *
            (1.0 + inputSum restriction field modifier)
          set [health := I, risk := 0.5],
        transition recover on Person where
          guard health = I
          hazard parameter gamma
          set [health := R, visits := 1]]
      outputs [
        output activity {infected : Int, total_risk : Real} from Person fields [
          field infected := count where health = I,
          field total_risk := sum (risk)]]
      views [
        view infectious from Person where health = I reduce count,
        view total_risk from Person using risk reduce sum,
        view minimum_visits from Person using visits reduce min,
        view maximum_infectious_risk from Person where health = I using risk reduce max],
    box policy where
      systems [
        system Controller as "controller" rows(1) where [
          state mode : {Open, Restricted},
          attr modifier : Real]]
      inputs [
        input activity {infected : Int, total_risk : Real}]
      transitions [
        transition restrict on Controller where
          guard mode = Open && inputSum activity field infected > 100
          hazard 1e300
          set [mode := Restricted, modifier := 0.4],
        transition reopen on Controller where
          guard mode = Restricted && inputSum activity field infected < 25
          hazard 1e300
          set [mode := Open, modifier := 1.0]]
      outputs [
        output restriction {modifier : Real} from Controller fields [
          field modifier := sum (modifier - 1.0)]]
      views [
        view controllers from Controller reduce count]]
  wires [
    wire population activity -> policy activity,
    wire policy restriction -> population restriction]
  summaries [
    summary cumulative_risk from population view total_risk reduce sum,
    summary lowest_visit_count from population view minimum_visits reduce min,
    summary peak_infectious_risk from population view maximum_infectious_risk reduce max,
    summary final_infectious from population view infectious reduce last,
    summary peak_infectious_tick from population view infectious reduce argmax_tick]

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
