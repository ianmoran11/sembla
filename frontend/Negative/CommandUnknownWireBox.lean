import Sembla.DSL
open Sembla.DSL
sembla_model UnknownWireBox (dt := 1.0) where
  box b where
    system B (rows := 1)
    input incoming where
      x : Int
  wire missing p -> b incoming
