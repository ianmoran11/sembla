import Sembla.DSL

open Sembla.IR Sembla.DSL

def bad : Model := model% "bad" step(1.0) where
  params []
  boxes [box population where
    systems [system Person (rows := 4) where [
      state first : {S, X},
      state second : {Y, I}]]
    inputs []
    transitions [infect on Person : S →[0.5] I]
    outputs []]
  wires []
