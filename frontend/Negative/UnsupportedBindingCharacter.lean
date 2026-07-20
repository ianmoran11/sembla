import Sembla.DSL
open Sembla.IR Sembla.DSL

def unsupportedBindingCharacter : Model := model% "negative" step(1.0) where
  params [param «café» : ℝ := 0.1]
  boxes []
  wires []
