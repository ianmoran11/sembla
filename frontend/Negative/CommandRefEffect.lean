import Sembla.DSL
open Sembla.DSL
sembla_model RefEffect (dt := 1.0) where
  box b where
    system Target (rows := 1)
    system A (rows := 1) where
      parent : Target
      x : Int
    transition t on A where
      guard x = 1
      hazard 0.1
      set parent := parent
