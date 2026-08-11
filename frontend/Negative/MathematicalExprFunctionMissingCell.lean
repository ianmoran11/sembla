import Sembla.DSL

sembla_model ExprFunctionMissingCell (dt := 1.0) where
  domain Area := {a, b}
  function F (area : Area) : ℝ where
    [a] := 1.0
