import Sembla.DSL
open Sembla.IR Sembla.DSL

def leadingBindingSeparator : Model := model% "negative" step(1.0) where
  params [param «_bad» : ℝ := 0.1]
  boxes []
  wires []
