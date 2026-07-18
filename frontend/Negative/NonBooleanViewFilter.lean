import Sembla.DSL

open Sembla.IR Sembla.DSL

def badNonBooleanViewFilter : Model := model% "bad_non_boolean_view_filter" step(1.0) where
  params []
  boxes [
    box population where
      systems [system Person as "Person" rows(1) where [attr visits : Int]]
      inputs []
      transitions []
      outputs []
      views [view bad_filter from Person where visits reduce count]]
  wires []
