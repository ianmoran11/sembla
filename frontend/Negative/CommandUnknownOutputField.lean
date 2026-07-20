import Sembla.DSL
open Sembla.DSL
sembla_model UnknownOutputField (dt := 1.0) where
  box b where
    system A (rows := 1) where
      x : ℝ
    output p from A where
      total : ℝ := sum (missing)
