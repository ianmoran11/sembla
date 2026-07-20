import Sembla.DSL
open Sembla.DSL
sembla_model DupInput (dt := 1.0) where
  box b where
    system A (rows := 1)
    input p where
      x : Int
    input p where
      x : Int
