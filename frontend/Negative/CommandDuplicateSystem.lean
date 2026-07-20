import Sembla.DSL
open Sembla.DSL
sembla_model DupSystem (dt := 1.0) where
  box b where
    system A (rows := 1)
    system A (rows := 2)
