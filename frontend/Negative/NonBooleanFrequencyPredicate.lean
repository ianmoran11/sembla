import Sembla.DSL

open Sembla.IR Sembla.DSL

def bad : Model := model% "bad" step(1.0) where
  params []
  boxes [box population where
    systems [
      system Person (rows := 4) where [state health : {S, I}, attr risk : ℝ, ref employer : Employer],
      system Employer (rows := 2) where []]
    inputs []
    transitions [infect on Person : health: S →[freq (risk) over employer] I]
    outputs []]
  wires []
