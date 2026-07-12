import Sembla.DSL
open Sembla.IR Sembla.DSL

def zeroStep : Model := model% "bad" step(0.0) where
  params []
  boxes []
  wires []
