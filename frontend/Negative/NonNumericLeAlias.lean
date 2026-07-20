import Sembla.DSL
open Sembla.IR Sembla.DSL

def nonNumericLeAlias : Model := model% "negative" step(1.0) where
  params []
  boxes [box demo where
    systems [system Person (rows := 1) where [state health : {S, I, R}]]
    inputs []
    transitions [transition bad on Person where
      guard health ≤ health
      hazard 0.1
      set [health := I]]
    outputs []]
  wires []
