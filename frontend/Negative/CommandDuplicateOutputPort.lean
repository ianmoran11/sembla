import Sembla.DSL
open Sembla.DSL
sembla_model DupOutput (dt := 1.0) where
  box b where
    system A (rows := 1) where
      x : Int
    output p from A where
      x : Int := count where x = 1
    output p from A where
      x : Int := count where x = 1
