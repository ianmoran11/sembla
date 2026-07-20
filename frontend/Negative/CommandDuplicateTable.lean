import Sembla.DSL
open Sembla.DSL
sembla_model DupTable (dt := 1.0) where
  box b where
    system A (name := "same") (rows := 1)
    system B (name := "same") (rows := 2)
