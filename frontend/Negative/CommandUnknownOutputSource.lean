import Sembla.DSL
open Sembla.DSL
sembla_model UnknownOutputSource (dt := 1.0) where
  box b where
    system A (rows := 1)
    output p from Missing where
      n : Int := count where 1 = 1
