import Sembla.DSL
open Sembla.DSL
sembla_model UnknownSystem (dt := 1.0) where
  box b where
    system A (rows := 1) where
      mode : {X, Y}
    transition t on Missing where
      guard mode = X
      hazard 0.1
      set mode := Y
