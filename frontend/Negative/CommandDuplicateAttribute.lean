import Sembla.DSL
open Sembla.DSL
sembla_model DupAttr (dt := 1.0) where
  box b where
    system A (rows := 1) where
      value : ℝ
      value : Int
