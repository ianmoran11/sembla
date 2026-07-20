import Sembla.DSL
open Sembla.IR Sembla.DSL

def tildePriorNonFinite : Model := model% "negative" step(1.0) where
  params [param β : ℝ := 0.8 ~ LogNormal nan 0.25]
  boxes []
  wires []
