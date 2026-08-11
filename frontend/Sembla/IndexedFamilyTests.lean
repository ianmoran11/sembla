import Sembla.Json
import Sembla.DSL

namespace Sembla.IndexedFamilyTests
open Sembla.IR Sembla.DSL

sembla_model IndexedInline (name := "indexed_family_twin") (dt := 1.0) where
  index age := 0 .. 1
  index sex := {male, female}
  param β[age, sex] : ℝ where
    [1, female] := 0.4
    [0, male] := 0.10 ~ LogNormal -2.302585092994046 2.5e-1
    [1, male] := 6.25e-2
    [0, female] := 0.123456789012345678901234567890
  box population where
    system Person (rows := 10) where
      health : {S, I}
      age : Int
      sex : {male, female}
      employer : Employer
    system Employer (rows := 2)
    infect[age, sex] on Person : health: S →[
      β[age, sex] · freq (health = I) over employer
    ] I

sembla_model IndexedCsv (name := "indexed_family_twin") (dt := 1.0) where
  index age := 0 .. 1
  index sex := {male, female}
  param β[age, sex] : ℝ from csv "TestData/IndexedFamilies/beta.csv" sha256
    "613195059792a4054b01c98e083d375865c7cfaf14515285cdc90b6cf04ce3fc"
  box population where
    system Person (rows := 10) where
      health : {S, I}
      age : Int
      sex : {male, female}
      employer : Employer
    system Employer (rows := 2)
    infect[age, sex] on Person : health: S →[
      β[age, sex] · freq (health = I) over employer
    ] I

sembla_model IndexedJson (name := "indexed_family_twin") (dt := 1.0) where
  index age := 0 .. 1
  index sex := {male, female}
  param β[age, sex] : ℝ from json "TestData/IndexedFamilies/beta.json" sha256
    "8236ce78ea61f379f10919456a06b231dac5d22daadffc74ea12b6e0b30a30b7"
  box population where
    system Person (rows := 10) where
      health : {S, I}
      age : Int
      sex : {male, female}
      employer : Employer
    system Employer (rows := 2)
    infect[age, sex] on Person : health: S →[
      β[age, sex] · freq (health = I) over employer
    ] I

sembla_model IndexedManual (name := "indexed_family_twin") (dt := 1.0) where
  param beta_0_male : ℝ := 0.10 ~ LogNormal -2.302585092994046 2.5e-1
  param beta_0_female : ℝ := 0.123456789012345678901234567890
  param beta_1_male : ℝ := 6.25e-2
  param beta_1_female : ℝ := 0.4
  box population where
    system Person (rows := 10) where
      health : {S, I}
      age : Int
      sex : {male, female}
      employer : Employer
    system Employer (rows := 2)
    transition infect_0_male on Person where
      guard (health = S ∧ age = 0) ∧ sex = male
      hazard beta_0_male · freq (health = I) over employer
      set health := I
    transition infect_0_female on Person where
      guard (health = S ∧ age = 0) ∧ sex = female
      hazard beta_0_female · freq (health = I) over employer
      set health := I
    transition infect_1_male on Person where
      guard (health = S ∧ age = 1) ∧ sex = male
      hazard beta_1_male · freq (health = I) over employer
      set health := I
    transition infect_1_female on Person where
      guard (health = S ∧ age = 1) ∧ sex = female
      hazard beta_1_female · freq (health = I) over employer
      set health := I

sembla_model IndexedGeneral (name := "indexed_general") (dt := 1.0) where
  index age := 0 .. 1
  index sex := {male, female}
  param β[age, sex] : ℝ where
    [0, male] := 0.1
    [0, female] := 0.2
    [1, male] := 0.3
    [1, female] := 0.4
  box population where
    system Person (rows := 10) where
      health : {S, I}
      age : Int
      sex : {male, female}
      employer : Employer
    system Employer (rows := 2)
    transition expose[age, sex] on Person where
      guard health = S
      hazard β[age, sex] · freq (health = I) over employer
      set health := I

sembla_model IndexedIntDefaults (dt := 1.0) where
  index cohort := 0 .. 1
  param tally[cohort] : Int where
    [1] := -7
    [0] := 12

set_option sembla.maxFamilyExpansion 4 in
sembla_model IndexedExplicitCapOverride (dt := 1.0) where
  index row := 0 .. 1
  index column := {left, right}
  param small[row, column] : ℝ where
    [0, left] := 0.1
    [0, right] := 0.2
    [1, left] := 0.3
    [1, right] := 0.4

sembla_model MixedV2Inline (name := "mixed_v2") (dt := 1.0) where
  index group := {a, b}
  param mixed[group] : ℝ where
    [a] := 1.0 ~ Normal 1.0 0.25
    [b] := 2.0 ~ LogNormal 0.6931471805599453 0.5

sembla_model MixedV2Csv (name := "mixed_v2") (dt := 1.0) where
  index group := {a, b}
  param mixed[group] : ℝ from csv "TestData/IndexedFamilies/mixed-priors-v2.csv"
    schema "sembla.parameter-family/v2" sha256
    "63fd9f51d41dd99abaa79872c04e8a5470de998e82552d1cd1646247759444de"

sembla_model MixedV2Json (name := "mixed_v2") (dt := 1.0) where
  index group := {a, b}
  param mixed[group] : ℝ from json "TestData/IndexedFamilies/mixed-priors-v2.json" sha256
    "469e3885a9d010f7d63568e61c7db25771fe10d94ce546dd611be560b2a97879"

#guard MixedV2Csv == MixedV2Inline
#guard MixedV2Json == MixedV2Inline
#guard MixedV2Inline.params ==
  [ { name := "mixed_a", ty := .real, default := .real 1.0,
      «prior» := some { family := .normal, args := [1.0, 0.25] } }
  , { name := "mixed_b", ty := .real, default := .real 2.0,
      «prior» := some { family := .logNormal, args := [0.6931471805599453, 0.5] } }
  ]

/-- Regression: importing the DSL must not reserve ordinary Lean identifiers named `index`. -/
private def index (values : List Nat) (position : Nat) : Option Nat :=
  values[position]?

#guard index [10, 20] 1 == some 20

private def expectedParams : List ParamDecl :=
  [ { name := "beta_0_male", ty := .real, default := .real 0.10
      «prior» := some { family := .logNormal, args := [-2.302585092994046, 2.5e-1] } }
  , { name := "beta_0_female", ty := .real,
      default := .real 0.123456789012345678901234567890, «prior» := none }
  , { name := "beta_1_male", ty := .real, default := .real 6.25e-2, «prior» := none }
  , { name := "beta_1_female", ty := .real, default := .real 0.4, «prior» := none }
  ]

private def transitionNames (model : Model) : List String :=
  model.boxes.bind fun modelBox => modelBox.transitions.map (·.name)
private def allRules (model : Model) : List Transition :=
  model.boxes.bind (·.transitions)
private def withoutName (rule : Transition) : Transition := { rule with name := "" }

#guard IndexedInline.params == expectedParams
#guard IndexedIntDefaults.params ==
  [ { name := "tally_0", ty := .int, default := .int 12, «prior» := none }
  , { name := "tally_1", ty := .int, default := .int (-7), «prior» := none }
  ]
#guard IndexedExplicitCapOverride.params.length == 4
#guard transitionNames IndexedInline ==
  ["infect_0_male", "infect_0_female", "infect_1_male", "infect_1_female"]
#guard IndexedInline == IndexedManual
#guard IndexedCsv == IndexedInline
#guard IndexedJson == IndexedInline
#guard Sembla.IR.toJson IndexedInline == Sembla.IR.toJson IndexedManual
#guard Sembla.IR.toJson IndexedCsv == Sembla.IR.toJson IndexedManual
#guard Sembla.IR.toJson IndexedJson == Sembla.IR.toJson IndexedManual
#guard transitionNames IndexedGeneral ==
  ["expose_0_male", "expose_0_female", "expose_1_male", "expose_1_female"]
#guard (allRules IndexedGeneral).map withoutName == (allRules IndexedInline).map withoutName

end Sembla.IndexedFamilyTests
