import Sembla.DSL
open Sembla.DSL
sembla_model UnknownAttr (dt := 1.0) where
  box b where
    system A (rows := 1) where
      mode : {X, Y}
    transition t on A where
      guard missing = X
      hazard 0.1
      set mode := Y
