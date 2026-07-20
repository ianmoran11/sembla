import Sembla.DSL
open Sembla.IR Sembla.DSL

def duplicateDerivedParameterName : Model := model% "negative" step(1.0) where
  params [
    param β : ℝ := 0.1,
    param beta : ℝ := 0.2]
  boxes []
  wires []
