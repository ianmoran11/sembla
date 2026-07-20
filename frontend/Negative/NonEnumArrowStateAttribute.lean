import Sembla.DSL

open Sembla.IR Sembla.DSL

def bad : Model := model% "bad" step(1.0) where
  params []
  boxes [box population where
    systems [system Person (rows := 4) where [
      state health : {S, I},
      attr score : ℝ]]
    inputs []
    transitions [infect on Person : score: S →[0.5] I]
    outputs []]
  wires []
