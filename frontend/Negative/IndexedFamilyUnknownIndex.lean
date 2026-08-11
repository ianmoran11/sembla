import Sembla.DSL

sembla_model UnknownFamilyIndex (dt := 1.0) where
  param beta[age] : ℝ where
    [0] := 0.1
