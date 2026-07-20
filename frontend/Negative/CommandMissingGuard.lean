import Sembla.DSL
open Sembla.DSL
sembla_model MissingGuard (dt := 1.0) where
  box b where
    system A (rows := 1) where
      x : Int
    transition t on A where
      hazard 0.1
      set x := 1
