import Sembla.DSL
open Sembla.DSL
sembla_model UnknownInput (dt := 1.0) where
  box b where
    system A (rows := 1) where
      mode : {X, Y}
    transition t on A where
      guard mode = X
      hazard inputSum missing field x
      set mode := Y
