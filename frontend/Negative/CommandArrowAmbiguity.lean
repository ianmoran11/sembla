import Sembla.DSL
open Sembla.DSL
sembla_model ArrowAmbiguity (dt := 1.0) where
  box b where
    system A (rows := 1) where
      mode : {X, Y}
    system B (rows := 1) where
      mode : {X, Y}
    t : X →[0.1] Y
