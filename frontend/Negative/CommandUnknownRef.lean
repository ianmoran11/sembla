import Sembla.DSL
open Sembla.DSL
sembla_model UnknownRef (dt := 1.0) where
  box b where
    system A (rows := 1) where
      parent : Missing
