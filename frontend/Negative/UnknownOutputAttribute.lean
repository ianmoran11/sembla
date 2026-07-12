import Sembla.DSL
open Sembla.IR Sembla.DSL

def badOutput : Model := model% "bad" step(1.0) where
  params []
  boxes [box demo where
    systems [system Person as "person" rows(1) where [state health : {S, I, R}]]
    inputs []
    transitions []
    outputs [output counts {infected : Int} from Person fields [
      field infected := count where workplace = I]]]
  wires []
