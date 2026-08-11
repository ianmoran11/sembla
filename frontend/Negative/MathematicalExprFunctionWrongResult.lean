import Sembla.DSL

sembla_model ExprFunctionWrongResult (dt := 1.0) where
  domain Area := {a}
  function F (area : Area) : ℝ where
    [a] := 1
