import Sembla.DSL
open Sembla.IR Sembla.DSL

def malformedBindingSeparator : Model := model% "negative" step(1.0) where
  params [param bad__name : ℝ := 0.1]
  boxes []
  wires []
