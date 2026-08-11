import Sembla.DSL

sembla_model UnboundFamilyReference (dt := 1.0) where
  index age := 0 .. 0
  param beta[age] : ℝ where
    [0] := 0.1
  box population where
    system Person (rows := 1) where
      health : {S, I}
      age : Int
    transition infect on Person where
      guard health = S
      hazard beta[age]
      set health := I
