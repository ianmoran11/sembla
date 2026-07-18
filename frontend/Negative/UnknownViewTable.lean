import Sembla.DSL

open Sembla.IR Sembla.DSL

def badUnknownViewTable : Model := model% "bad_unknown_view_table" step(1.0) where
  params []
  boxes [
    box population where
      systems [system Person as "Person" rows(1) where [attr visits : Int]]
      inputs []
      transitions []
      outputs []
      views [view bad_table from Missing reduce count]]
  wires []
