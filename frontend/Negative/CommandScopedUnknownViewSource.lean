import Sembla.DSL
open Sembla.DSL
sembla_model ScopedUnknownViewSource (dt := 1.0) where
  box b where
    system A (rows := 1)
    views Missing where
      v := count
