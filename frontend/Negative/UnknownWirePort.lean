import Sembla.DSL
open Sembla.IR Sembla.DSL

def badWire : Model := model% "bad" step(1.0) where
  params []
  boxes [
    box source where systems [] inputs [] transitions [] outputs [],
    box target where systems [] inputs [input received {value : Int}] transitions [] outputs []]
  wires [wire source missing -> target received]
