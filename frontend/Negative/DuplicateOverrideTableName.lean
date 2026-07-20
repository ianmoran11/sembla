import Sembla.DSL
open Sembla.IR Sembla.DSL

def duplicateOverrideTableName : Model := model% "negative" step(1.0) where
  params []
  boxes [box demo where
    systems [
      system Person (rows := 1) where [],
      system LegacyPerson (name := "person") (rows := 1) where []]
    inputs []
    transitions []
    outputs []]
  wires []
