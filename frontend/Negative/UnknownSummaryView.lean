import Sembla.DSL

open Sembla.IR Sembla.DSL

def badUnknownSummaryView : Model := model% "bad_unknown_summary_view" step(1.0) where
  params []
  boxes [
    box population where
      systems [system Person as "Person" rows(1) where [attr visits : Int]]
      inputs []
      transitions []
      outputs []
      views []]
  wires []
  summaries [summary bad_summary from population view absent reduce max]
