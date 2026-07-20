import Sembla.DSL
open Sembla.DSL
sembla_model ViewType (dt := 1.0) where
  box b where
    system A (rows := 1) where
      mode : {X, Y}
    view v := sum A using mode
