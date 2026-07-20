import Sembla.DSL
open Sembla.DSL
sembla_model ArrowMultipleAttrs (dt := 1.0) where
  box b where
    system A (rows := 1) where
      first : {X, Y}
      second : {X, Y}
    t on A : X →[0.1] Y
