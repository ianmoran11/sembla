import Sembla.DSL
open Sembla.DSL
sembla_model OutputType (dt := 1.0) where
  box b where
    system A (rows := 1) where
      x : Int
    output p from A where
      n : ℝ := count where x = 1
