import Sembla.DSL
open Sembla.IR Sembla.DSL

/-- Employer is declared after Person; gamma deliberately has no prior. -/
def forwardRefPriorless : Model := model% "positive" step(1.0) where
  params [param gamma : Real := 0.1]
  boxes [box demo where
    systems [
      system Person as "person" rows(1) where [ref employer : Employer],
      system Employer as "employer" rows(1) where []]
    inputs []
    transitions []
    outputs []]
  wires []
