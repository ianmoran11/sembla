import Sembla.DSL
open Sembla.IR Sembla.DSL

def outOfRange : Model := model% "bad" step(1e10000) where
  params []
  boxes []
  wires []
