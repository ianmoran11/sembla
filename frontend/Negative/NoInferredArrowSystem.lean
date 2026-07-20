import Sembla.DSL

open Sembla.IR Sembla.DSL

def bad : Model := model% "bad" step(1.0) where
  params []
  boxes [box population where
    systems [
      system Person (rows := 4) where [state health : {S, I}],
      system Controller (rows := 1) where [state mode : {Open, Closed}]]
    inputs []
    transitions [infect : A →[0.5] B]
    outputs []]
  wires []
