import Sembla.DSL

open Sembla.IR Sembla.DSL

def badCountViewWithValue : Model := model% "bad_count_view_with_value" step(1.0) where
  params []
  boxes [
    box population where
      systems [system Person as "Person" rows(1) where [attr visits : Int]]
      inputs []
      transitions []
      outputs []
      views [view bad_count from Person using visits reduce count]]
  wires []
