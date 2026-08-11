import Sembla.DSL

sembla_model FamilyEmptyBinders (dt := 1.0) where
  box population where
    system Person (rows := 1) where
      health : {S, I}
    infect[] on Person : health: S →[0.1] I
