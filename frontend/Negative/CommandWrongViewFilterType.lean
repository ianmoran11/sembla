import Sembla.DSL
open Sembla.DSL
sembla_model ViewFilterType (dt := 1.0) where
  box b where
    system A (rows := 1) where
      x : Int
    view v := count A where x
