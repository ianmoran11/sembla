import Sembla.DSL
open Sembla.IR Sembla.DSL

def badReference : Model := model% "bad" step(1.0) where
  params []
  boxes [box demo where
    systems [system Person as "person" rows(1) where [
      state health : {S, I, R}, ref workplace : Workplace]]
    inputs []
    transitions []
    outputs []]
  wires []
