import Sembla.DSL

open Sembla.IR Sembla.DSL

def bad : Model := model% "bad" step(1.0) where
  params []
  boxes [box population where
    systems [
      system Person (rows := 4) where [state health : {S, I}, ref employer : Employer],
      system Employer (rows := 2) where []]
    inputs [input metrics {score : ℝ}]
    transitions [infect on Person : health: S →[freq (inputSum metrics field score > 0.5) over employer] I]
    outputs []]
  wires []
