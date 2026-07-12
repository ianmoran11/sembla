import Sembla.DSL
open Sembla.IR Sembla.DSL

def badParameter : Model := model% "bad" step(1.0) where
  params [param beta : Real := 1.0]
  boxes [box demo where
    systems [system Person as "person" rows(1) where [state health : {S, I, R}]]
    inputs []
    transitions [transition bad on Person where
      guard health = S
      hazard parameter delta
      set [health := I]]
    outputs []]
  wires []
