import Sembla.DSL
open Sembla.IR Sembla.DSL

def oversizedRows : Model := model% "bad" step(1.0) where
  params []
  boxes [box demo where
    systems [system Person as "person" rows(18446744073709551616) where []]
    inputs []
    transitions []
    outputs []]
  wires []
