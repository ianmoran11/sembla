import Sembla.DSL
open Sembla.IR Sembla.DSL

def orderedEnum : Model := model% "bad" step(1.0) where
  params []
  boxes [box demo where
    systems [system Person as "person" rows(1) where [state health : {S, I}]]
    inputs []
    transitions [transition bad on Person where
      guard health < health
      hazard 1.0
      set [health := I]]
    outputs []]
  wires []
