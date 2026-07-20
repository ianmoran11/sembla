import Sembla.DSL

/-!
# Step 01 — One state machine and one transition

We begin with the smallest useful epidemic-shaped model: people have an
`S`/`I`/`R` health state, and infectious people recover at a constant hazard.
There are no parameters, interactions, observations, or wires yet.

`(rows := 1_000)` becomes an IR `sizeHint`; the runtime still owns concrete
population initialization.
-/

namespace Sembla.Tutorial.Step01

open Sembla.IR Sembla.DSL

/- Recovery-only baseline: one system, one guarded transition, one effect. -/
sembla_model recoveryOnly
    (name := "tutorial_01_recovery_only")
    (dt := 0.25) where
  box population where
    system Person (rows := 1_000) where
      health : {S, I, R}

    recover on Person : health: I →[0.1] R

#guard recoveryOnly.name == "tutorial_01_recovery_only"
#guard recoveryOnly.boxes.length == 1
#guard recoveryOnly.boxes.map (fun modelBox => modelBox.transitions.map (·.name)) ==
  [["recover"]]

end Sembla.Tutorial.Step01
