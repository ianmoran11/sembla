import Sembla.DSL

sembla_model ExprFunctionDuplicateCell (dt := 1.0) where
  domain Area := {a}
  function F (area : Area) : ℝ where
    [a] := 1.0
    [a] := 2.0
