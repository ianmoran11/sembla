import Sembla.DSL
open Sembla.IR Sembla.DSL

def escapedWhitespaceBinding : Model := model% "negative" step(1.0) where
  params [param «bad name» : ℝ := 0.1]
  boxes []
  wires []
