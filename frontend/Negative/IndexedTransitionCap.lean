import Sembla.DSL

set_option sembla.maxFamilyExpansion 3 in
sembla_model TransitionFamilyCap (dt := 1.0) where
  index age := 0 .. 18446744073709551615
  index sex := {male, female}
  box population where
    system Person (rows := 1) where
      health : {S, I}
      age : Int
      sex : {male, female}
    infect[age, sex] on Person : health: S →[0.1] I
