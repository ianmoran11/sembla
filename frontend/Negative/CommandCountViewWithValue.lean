import Sembla.DSL
open Sembla.DSL
sembla_model CountValue (dt := 1.0) where
  box b where
    system A (rows := 1) where
      x : Int
    view v := count A using x
