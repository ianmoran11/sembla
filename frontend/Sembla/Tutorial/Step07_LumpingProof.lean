import Sembla.Tutorial.Step06_InspectAndExport
import Sembla.LumpingProof

/-!
# Step 07 — The proved workplace-count rewrite

The final step isolates the mathematical fragment behind Step 03's workplace
infection term. The specification presents a naive coworker scan and an
association-list grouped formulation. Sembla proves their returned counts equal
for every specification-level table and index, then transports equality
through any downstream function of the count.

This theorem is extensional: the Lean function rebuilds `groupTotals` for each
queried row, so it does not prove shared aggregation or a linear-time
implementation. Such reuse belongs to the compiler/runtime plan. This is
theorem target 1a, not yet a theorem connecting the tutorial's deep-embedded
`Model` to an evaluator (target 1b), and it does not formalize the runtime's
coordinate-keyed RNG discipline.
-/

namespace Sembla.Tutorial.Step07

open Sembla.Lumping

/-- A small workplace table: employer 0 has two infectious people; employer 1 has one. -/
def coworkers : List Person :=
  [ { employer := 0, infectious := true }
  , { employer := 1, infectious := false }
  , { employer := 0, infectious := false }
  , { employer := 1, infectious := true }
  , { employer := 0, infectious := true }
  ]

/-- A toy downstream infection-pressure function of the sufficient statistic. -/
def infectionPressure (infectiousCoworkers : Nat) : Nat :=
  3 * infectiousCoworkers + 1

example :
    (List.range coworkers.length).map (naiveCount coworkers)
      = [2, 1, 2, 1, 2] := by
  decide

example :
    (List.range coworkers.length).map (groupedCount coworkers)
      = [2, 1, 2, 1, 2] := by
  decide

/-- The two counting plans agree for every table and row. -/
example (table : List Person) (i : Nat) :
    groupedCount table i = naiveCount table i :=
  groupedCount_eq_naiveCount table i

/-- Therefore a hazard or any other downstream function receives the same input. -/
example (table : List Person) (i : Nat) :
    infectionPressure (groupedCount table i) =
      infectionPressure (naiveCount table i) :=
  plan_rewrite_congr infectionPressure table i

end Sembla.Tutorial.Step07
