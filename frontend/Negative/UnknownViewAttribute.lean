import Sembla.DSL

open Sembla.IR Sembla.DSL

def badUnknownViewAttribute : Model := model% "bad_unknown_view_attribute" step(1.0) where
  params []
  boxes [
    box population where
      systems [
        system Person as "Person" rows(1) where [state status : {active, inactive}]]
      inputs []
      transitions []
      outputs []
      views [view bad_attribute from Person where missing = active reduce count]]
  wires []
