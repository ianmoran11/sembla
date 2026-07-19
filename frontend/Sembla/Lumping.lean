/-!
# Group-by lumping specification model

This module defines the two counting plans from `DESIGN.md` §7 as a small,
executable specification-level model over lists and natural numbers. It is proof
target 1a only: it does not connect these functions to the deep-embedded IR or
its future evaluator, so proof target 1b remains open as described in
`docs/prds-proof-track/README.md`.

Implementation notes: both plans use the specified `table[i]?` spelling. Lean
4.13.0 reserves `matches`, so its declaration and reference use the escaped
identifier `«matches»`; this preserves the public declaration name and semantics.
-/

namespace Sembla
namespace Lumping

/-- One person row of the lumping fragment: an employer id and an
infectious flag. Mirrors the `infect` example of `DESIGN.md` §7. -/
structure Person where
  employer : Nat
  infectious : Bool
deriving Repr, BEq, DecidableEq

/-- The join predicate: `q` is an infectious member of employer `e`. -/
def «matches» (e : Nat) (q : Person) : Bool :=
  q.employer == e && q.infectious

/-- Naive self-join plan: for the row at index `i`, scan the whole table
and count infectious rows sharing its employer (the quadratic plan). Out
of range ⇒ 0. -/
def naiveCount (table : List Person) (i : Nat) : Nat :=
  match table[i]? with
  | none => 0
  | some p => table.countP («matches» p.employer)

/-- Increment employer `e`'s entry in an association list of totals,
appending `(e, 1)` if absent. -/
def bump : List (Nat × Nat) → Nat → List (Nat × Nat)
  | [], e => [(e, 1)]
  | (k, v) :: rest, e =>
    if k == e then (k, v + 1) :: rest else (k, v) :: bump rest e

/-- Grouping pass: one scan of the table building
`employer ↦ infectious-count` (the linear plan's aggregation phase). -/
def groupTotals : List Person → List (Nat × Nat)
  | [] => []
  | q :: rest =>
    if q.infectious then bump (groupTotals rest) q.employer
    else groupTotals rest

/-- Look up an employer's total; absent ⇒ 0. -/
def lookupTotal (totals : List (Nat × Nat)) (e : Nat) : Nat :=
  match totals with
  | [] => 0
  | (k, v) :: rest => if k == e then v else lookupTotal rest e

/-- Optimized plan: group once, then broadcast a lookup per row. -/
def groupedCount (table : List Person) (i : Nat) : Nat :=
  match table[i]? with
  | none => 0
  | some p => lookupTotal (groupTotals table) p.employer

end Lumping
end Sembla
