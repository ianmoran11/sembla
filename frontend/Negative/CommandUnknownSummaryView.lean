import Sembla.DSL
open Sembla.DSL
sembla_model UnknownSummary (dt := 1.0) where
  box b where
    system A (rows := 1)
    view v := count A
  summary s := max b.missing
