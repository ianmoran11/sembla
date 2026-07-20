import Sembla.DSL
open Sembla.IR Sembla.DSL

def primeBindingIdentifier : Model := model% "negative" step(1.0) where
  params [param beta' : ℝ := 0.1]
  boxes []
  wires []
