import Sembla.DSL
open Sembla.DSL
sembla_model HugeRows (dt := 1.0) where
  box b where
    system A (rows := 18446744073709551616)
