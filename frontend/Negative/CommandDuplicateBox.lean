import Sembla.DSL
open Sembla.DSL
sembla_model DupBox (dt := 1.0) where
  box b where
    system A (rows := 1)
  box b where
    system C (rows := 1)
