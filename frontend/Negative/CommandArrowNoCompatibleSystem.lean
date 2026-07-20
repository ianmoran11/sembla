import Sembla.DSL
open Sembla.DSL
sembla_model ArrowNoSystem (dt := 1.0) where
  box b where
    system A (rows := 1) where
      mode : {X, Z}
    t : X →[0.1] Y
