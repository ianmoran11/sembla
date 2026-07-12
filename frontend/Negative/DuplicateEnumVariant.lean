import Sembla.DSL
open Sembla.IR Sembla.DSL

def duplicateEnum : Model := model% "bad" step(1.0) where
  params []
  boxes [box demo where
    systems [system Person as "person" rows(1) where [state health : {S, I, S}]]
    inputs []
    transitions []
    outputs []]
  wires []
