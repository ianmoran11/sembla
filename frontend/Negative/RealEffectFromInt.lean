import Sembla.DSL
open Sembla.IR Sembla.DSL

def realEffectFromInt : Model := model% "bad" step(1.0) where
  params []
  boxes [box demo where
    systems [system Person as "person" rows(1) where [attr value : Real]]
    inputs []
    transitions [transition bad on Person where
      guard value > 0.0
      hazard 1.0
      set [value := 1]]
    outputs []]
  wires []
