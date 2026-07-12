import Sembla.DSL
open Sembla.IR Sembla.DSL

def badEffect : Model := model% "bad" step(1.0) where
  params []
  boxes [box demo where
    systems [system Person as "person" rows(1) where [state health : {S, I, R}]]
    inputs []
    transitions [transition bad on Person where
      guard health = S
      hazard 1.0
      set [workplace := I]]
    outputs []]
  wires []
