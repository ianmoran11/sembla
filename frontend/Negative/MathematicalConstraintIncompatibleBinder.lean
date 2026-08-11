import Sembla.DSL

sembla_model ConstraintIncompatibleBinder (dt := 1.0) where
  domain Area := {a, b}
  domain Sex := {male, female}
  box population where
    system Person (rows := 1) where
      place : Area
      status : {present, vacant}
    state Active on Person where
      status := present
    relation bad (origin : Area, gender : Sex) on Person subject to origin ≠ gender where
      from Active
      hazard 0.1
      set place := origin
