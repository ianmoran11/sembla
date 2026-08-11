import Sembla.DSL

sembla_model AliasIncompatibleAssignment (dt := 1.0) where
  domain Area := {a, b}
  domain Sex := {male, female}
  box population where
    system Person (rows := 1) where
      place : Area
    state Active (gender : Sex) on Person where
      place := gender
