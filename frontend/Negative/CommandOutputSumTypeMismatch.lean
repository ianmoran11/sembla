import Sembla.DSL
open Sembla.DSL
sembla_model OutputSumType (dt := 1.0) where
  box b where
    system A (rows := 1) where
      x : Int
    output p from A where
      total : ℝ := sum (x)
