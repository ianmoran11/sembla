import Sembla.DSL

sembla_model ExprFunctionCall (dt := 1.0) where
  domain Area := {a}
  function F (area : Area) : ℝ where
    [a] := (G(area))
  function G (area : Area) : ℝ where
    [a] := (F(area))
