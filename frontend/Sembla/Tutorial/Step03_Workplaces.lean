import Sembla.DSL

/-!
# Step 03 — References and grouped interaction

People now reference employers. Infection depends on the infectious fraction
inside the current person's workplace:

`countBy employer (health = I) / sizeBy employer`

The `Employer` declaration appears after `Person`, demonstrating the DSL's
multi-pass forward-reference resolution. This is the first genuinely
interaction-dependent model in the sequence.
-/

namespace Sembla.Tutorial.Step03

open Sembla.IR Sembla.DSL

/-- Frequency-dependent workplace SIR with reference-keyed aggregation. -/
def workplaceSIR : Model := model% "tutorial_03_workplace_sir" step(0.25) where
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
      outputs []]
  wires []

#guard workplaceSIR.boxes.map (fun modelBox => modelBox.tables.map (·.name)) ==
  [["person", "employer"]]
#guard workplaceSIR.boxes.map (fun modelBox => modelBox.tables.map (fun table => table.attrs.length)) ==
  [[2, 0]]
#guard workplaceSIR.boxes.map (fun modelBox => modelBox.transitions.map (·.name)) ==
  [["infect", "recover"]]

end Sembla.Tutorial.Step03
