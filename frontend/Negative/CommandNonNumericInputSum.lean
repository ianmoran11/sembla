import Sembla.DSL
open Sembla.DSL
sembla_model InputType (dt := 1.0) where
  box b where
    system A (rows := 1) where
      mode : {X, Y}
    input p where
      mode : {X, Y}
    transition t on A where
      guard mode = X
      hazard inputSum p field mode
      set mode := Y
