import Sembla.DSL
open Sembla.IR Sembla.DSL

def refEffect : Model := model% "bad" step(1.0) where
  params []
  boxes [box demo where
    systems [
      system Person as "person" rows(1) where [ref employer : Employer],
      system Employer as "employer" rows(1) where []]
    inputs []
    transitions [transition bad on Person where
      guard employer = employer
      hazard 1.0
      set [employer := employer]]
    outputs []]
  wires []
