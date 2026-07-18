import Sembla.DSL

namespace Sembla.Positive.ObservationOrder
open Sembla.IR Sembla.DSL

private def probe : Model := model% "observation_order" step(1.0) where
  params []
  boxes [
    box population where
      systems [
        system Person as "Person" rows(2) where [
          state status : {active, inactive},
          attr value : Real,
          attr visits : Int]]
      inputs []
      transitions []
      outputs []
      views [
        view total from Person using value reduce sum,
        view active from Person where status = active reduce count,
        view active_total from Person where status = active using value reduce sum,
        view least from Person using visits reduce min,
        view greatest from Person using value reduce max],
    box auxiliary where
      systems [system Row as "Row" rows(1) where [attr value : Int]]
      inputs []
      transitions []
      outputs []
      views [view row_count from Row reduce count]]
  wires []
  summaries [
    summary auxiliary_last from auxiliary view row_count reduce last,
    summary total_sum from population view total reduce sum,
    summary total_min from population view total reduce min,
    summary total_max from population view total reduce max,
    summary total_peak_tick from population view total reduce argmax_tick]

private def viewNames : List String :=
  match probe.boxes with
  | first :: _ => first.views.map (·.name)
  | [] => []

#guard viewNames == ["total", "active", "active_total", "least", "greatest"]
#guard probe.summaries.map (·.name) ==
  ["auxiliary_last", "total_sum", "total_min", "total_max", "total_peak_tick"]
#guard probe.summaries.map (·.reduce) ==
  [.last, .sum, .min, .max, .argmaxTick]

end Sembla.Positive.ObservationOrder
