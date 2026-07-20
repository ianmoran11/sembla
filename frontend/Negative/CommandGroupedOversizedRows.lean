import Sembla.DSL
open Sembla.DSL
sembla_model GroupedHugeRows (dt := 1.0) where
  box b where
    system A (rows := 18_446_744_073_709_551_616)
