import Sembla.DSL

/-!
# Step 8: indexed parameter and transition families

Finite `index` declarations let one mathematical rule describe a deterministic
family of ordinary scalar parameters and transitions. The range is inclusive;
it is an authoring-time expansion domain, not a runtime bound on `age`.
Large tables may use the pinned CSV/JSON forms documented in the indexed-family
guide.
-/
namespace Sembla.Tutorial.Step08

open Sembla.IR

sembla_model AgeStructuredSIR (dt := 1.0) where
  index age := 0 .. 1
  index sex := {male, female}

  param β[age, sex] : ℝ where
    [0, male] := 0.10
    [0, female] := 0.09
    [1, male] := 0.08
    [1, female] := 0.07

  box population where
    system Person (rows := 100) where
      health : {S, I, R}
      age : Int
      sex : {male, female}
      employer : Employer
    system Employer (rows := 10)

    infect[age, sex] on Person : health: S →[
      β[age, sex] · freq (health = I) over employer
    ] I
    recover on Person : health: I →[0.2] R

#guard AgeStructuredSIR.params.map (·.name) ==
  ["beta_0_male", "beta_0_female", "beta_1_male", "beta_1_female"]
#guard AgeStructuredSIR.boxes.head?.map (fun modelBox => modelBox.transitions.length) == some 5

end Sembla.Tutorial.Step08
