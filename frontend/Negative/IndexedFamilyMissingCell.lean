import Sembla.DSL

sembla_model MissingFamilyCell (dt := 1.0) where
  index age := 0 .. 1
  param beta[age] : ℝ where
    [0] := 0.1
