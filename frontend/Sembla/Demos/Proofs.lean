import Sembla.LumpingProof

/-!
# Proved rewrite feature tour

Sembla's first landed proof establishes target 1a: the grouped coworker-count
plan equals the naive self-join plan for every specification-level table and
row. The transport corollary carries that equality through any downstream
function of the count.

This is not yet target 1b: the theorem is not connected to the deep-embedding
evaluator. Coordinate-keyed runtime randomness makes identical hazards use
identical draws, but that final runtime argument is documented rather than
formalized here.
-/

namespace Sembla.Demos.Proofs

open Sembla.Lumping

/-- A small executable table for trying both plans. -/
def coworkers : List Person :=
  [ { employer := 0, infectious := true }
  , { employer := 1, infectious := false }
  , { employer := 0, infectious := false }
  , { employer := 1, infectious := true }
  , { employer := 0, infectious := true }
  ]

/-- Example downstream hazard depending only on the sufficient statistic. -/
def hazardFromCoworkers (infectiousCoworkers : Nat) : Nat :=
  2 * infectiousCoworkers + 1

example : (List.range coworkers.length).map (naiveCount coworkers) == [2, 1, 2, 1, 2] := by
  decide

example : (List.range coworkers.length).map (groupedCount coworkers) == [2, 1, 2, 1, 2] := by
  decide

/-- The rewrite-equivalence theorem is available for arbitrary tables and indices. -/
example (table : List Person) (i : Nat) :
    groupedCount table i = naiveCount table i :=
  groupedCount_eq_naiveCount table i

/-- Any hazard or other downstream function receives the same input. -/
example (table : List Person) (i : Nat) :
    hazardFromCoworkers (groupedCount table i) =
      hazardFromCoworkers (naiveCount table i) :=
  plan_rewrite_congr hazardFromCoworkers table i

/-- The corollary is polymorphic in the downstream result type. -/
example (label : Nat → String) (table : List Person) (i : Nat) :
    label (groupedCount table i) = label (naiveCount table i) :=
  plan_rewrite_congr label table i

end Sembla.Demos.Proofs
