import Sembla.DSL

sembla_model DuplicateFamilyCell (dt := 1.0) where
  index age := 0 .. 0
  param beta[age] : ℝ where
    [0] := 0.1
    [0] := 0.2
