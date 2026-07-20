import Sembla.DSL

/-!
# Step 01 — One state machine and one transition

We begin with the smallest useful epidemic-shaped model: people have an
`S`/`I`/`R` health state, and infectious people recover at a constant hazard.
There are no parameters, interactions, observations, or wires yet.

`rows(1000)` becomes an IR `sizeHint`; the runtime still owns concrete
population initialization.
-/

namespace Sembla.Tutorial.Step01

open Sembla.IR Sembla.DSL

/-- Recovery-only baseline: one system, one guarded transition, one effect. -/
def recoveryOnly : Model := model% "tutorial_01_recovery_only" step(0.25) where
  params []
  boxes [
    box population where
      systems [
        system Person as "person" rows(1000) where [
          state health : {S, I, R}]]
      inputs []
      transitions [
        transition recover on Person where
          guard health = I
          hazard 0.1
          set [health := R]]
      outputs []]
  wires []

#guard recoveryOnly.name == "tutorial_01_recovery_only"
#guard recoveryOnly.boxes.length == 1
#guard recoveryOnly.boxes.map (fun modelBox => modelBox.transitions.map (·.name)) ==
  [["recover"]]

end Sembla.Tutorial.Step01
