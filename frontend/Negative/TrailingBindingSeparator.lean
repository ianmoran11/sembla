import Sembla.DSL
open Sembla.IR Sembla.DSL

def trailingBindingSeparator : Model := model% "negative" step(1.0) where
  params [param «bad_» : ℝ := 0.1]
  boxes []
  wires []
