import Sembla.DSL
open Sembla.IR Sembla.DSL

def badInput : Model := model% "bad" step(1.0) where
  params []
  boxes [box demo where
    systems [system Person as "person" rows(1) where [state health : {S, I, R}]]
    inputs [input modifier {value : Real}]
    transitions [transition bad on Person where
      guard health = S
      hazard inputSum missing field value
      set [health := I]]
    outputs []]
  wires []
