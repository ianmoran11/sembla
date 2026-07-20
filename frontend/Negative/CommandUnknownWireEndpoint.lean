import Sembla.DSL
open Sembla.DSL
sembla_model UnknownWire (dt := 1.0) where
  box a where
    system A (rows := 1)
  box b where
    system B (rows := 1)
    input incoming where
      x : Int
  wire a missing -> b incoming
