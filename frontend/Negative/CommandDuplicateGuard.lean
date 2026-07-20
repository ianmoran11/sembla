import Sembla.DSL
open Sembla.DSL
sembla_model DuplicateGuard (dt := 1.0) where
  box b where
    system A (rows := 1) where
      x : Int
    transition t on A where
      guard x = 1
      guard x = 2
      hazard 0.1
      set x := 1
