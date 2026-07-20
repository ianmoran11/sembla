import Sembla.DSL
open Sembla.DSL
sembla_model DupView (dt := 1.0) where
  box b where
    system A (rows := 1)
    view v := count A
    view v := count A
