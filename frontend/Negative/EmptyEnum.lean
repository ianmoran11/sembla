import Sembla.DSL
open Sembla.IR Sembla.DSL

def emptyEnum : Model := model% "bad" step(1.0) where
  params []
  boxes [box demo where
    systems [system Person as "person" rows(1) where [state health : {}]]
    inputs []
    transitions []
    outputs []]
  wires []
