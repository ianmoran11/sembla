import Sembla.DSL

sembla_model AliasDuplicateArgument (dt := 1.0) where
  domain Area := {a, b}
  box population where
    system Person (rows := 1) where
      place : Area
    state Active (x : Area, x : Area) on Person where
      place := x
