import Sembla.DSL

/-!
# Step 03 — References and grouped interaction

People now reference employers. Infection depends on the infectious fraction
inside the current person's workplace:

`freq (health = I) over employer`

The `Employer` declaration appears after `Person`, demonstrating the DSL's
multi-pass forward-reference resolution. This is the first genuinely
interaction-dependent model in the sequence.
-/

namespace Sembla.Tutorial.Step03

open Sembla.IR Sembla.DSL

/- Frequency-dependent workplace SIR with reference-keyed aggregation. -/
sembla_model workplaceSIR
    (name := "tutorial_03_workplace_sir")
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

#guard workplaceSIR.boxes.map (fun modelBox => modelBox.tables.map (·.name)) ==
  [["person", "employer"]]
#guard workplaceSIR.boxes.map (fun modelBox => modelBox.tables.map (fun table => table.attrs.length)) ==
  [[2, 0]]
#guard workplaceSIR.boxes.map (fun modelBox => modelBox.transitions.map (·.name)) ==
  [["infect", "recover"]]

end Sembla.Tutorial.Step03
