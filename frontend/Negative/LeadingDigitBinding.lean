import Sembla.DSL
open Sembla.IR Sembla.DSL

def leadingDigitBinding : Model := model% "negative" step(1.0) where
  params [param «2beta» : ℝ := 0.1]
  boxes []
  wires []
