import Sembla.DSL
open Sembla.IR Sembla.DSL

def duplicateDerivedTableName : Model := model% "negative" step(1.0) where
  params []
  boxes [box demo where
    systems [
      system HTTPServer (rows := 1) where [],
      system http_server (rows := 1) where []]
    inputs []
    transitions []
    outputs []]
  wires []
