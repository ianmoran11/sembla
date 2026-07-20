import Sembla.DSL
open Sembla.DSL
sembla_model UnknownViewValue (dt := 1.0) where
  box b where
    system A (rows := 1) where
      x : ℝ
    view v := sum A using missing
