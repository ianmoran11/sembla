import Sembla.DSL

sembla_model FamilyBinderType (dt := 1.0) where
  index age := 0 .. 1
  param beta[age] : ℝ where
    [0] := 0.1
    [1] := 0.2
  box population where
    system Person (rows := 1) where
      health : {S, I}
      age : {young, old}
    infect[age] on Person : health: S →[beta[age]] I
