import Sembla.DSL
open Sembla.DSL
sembla_model DupTransition (dt := 1.0) where
  box b where
    system A (rows := 1) where
      mode : {X, Y}
    one on A : mode: X →[0.1] Y
    one on A : mode: Y →[0.1] X
