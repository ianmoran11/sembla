import Sembla.DSL
open Sembla.IR Sembla.DSL

def badSystem : Model := model% "bad" step(1.0) where
  params []
  boxes [box demo where
    systems [system Person as "person" rows(1) where [state health : {S, I, R}]]
    inputs []
    transitions [transition bad on Workplace where
      guard health = S
      hazard 1.0
      set [health := I]]
    outputs []]
  wires []
