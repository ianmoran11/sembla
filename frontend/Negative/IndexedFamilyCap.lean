import Sembla.DSL

set_option sembla.maxFamilyExpansion 3 in
sembla_model FamilyCap (dt := 1.0) where
  index age := 0 .. 18446744073709551615
  index sex := {male, female}
  param beta[age, sex] : ℝ where
    [0, male] := 0.1
