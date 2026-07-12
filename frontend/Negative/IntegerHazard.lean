import Sembla.DSL
open Sembla.IR Sembla.DSL

def integerHazard : Model := model% "bad" step(1.0) where
  params []
  boxes [box demo where
    systems [system Person as "person" rows(1) where [attr age : Int]]
    inputs []
    transitions [transition bad on Person where
      guard age > 0
      hazard 1
      set [age := 1]]
    outputs []]
  wires []
