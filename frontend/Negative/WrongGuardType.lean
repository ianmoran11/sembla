import Sembla.DSL
open Sembla.IR Sembla.DSL

def badGuardType : Model := model% "bad" step(1.0) where
  params []
  boxes [box demo where
    systems [system Controller as "controller" rows(1) where [
      state mode : {Open, Restricted}, attr modifier : Real]]
    inputs []
    transitions [transition bad on Controller where
      guard modifier
      hazard 1e300
      set [mode := Restricted]]
    outputs []]
  wires []
