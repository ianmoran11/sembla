import Sembla.DSL

sembla_model ConstraintUnknownBinder (dt := 1.0) where
  domain Area := {a, b}
  box population where
    system Person (rows := 1) where
      place : Area
      status : {present, vacant}
    state Active on Person where
      status := present
    relation bad (origin : Area, target : Area) on Person subject to origin ≠ missing where
      from Active
      hazard 0.1
      set place := target
