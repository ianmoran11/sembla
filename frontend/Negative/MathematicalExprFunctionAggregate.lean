import Sembla.DSL

sembla_model ExprFunctionAggregate (dt := 1.0) where
  domain Area := {a}
  function F (area : Area) : Int where
    [a] := (sizeBy resource)
