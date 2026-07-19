import Sembla.Lumping

/-!
# Group-by lumping proof

This module proves theorem target 1a from `DESIGN.md` §7: exact agreement of
the specification-level grouped and naive counting plans. It does not connect
the model to the deep-embedded IR evaluator, so target 1b remains open as
stated in `docs/prds-proof-track/README.md`.

Implementation note: kernel dependency inspection reports `[propext,
Quot.sound]` for each of the three results below, within the folder README's
allowed set.
-/

namespace Sembla
namespace Lumping

/-- Incrementing employer `e`'s total shifts the lookup at `e'` by one
exactly when the two employer ids agree. -/
theorem lookupTotal_bump (totals : List (Nat × Nat)) (e e' : Nat) :
    lookupTotal (bump totals e) e'
      = lookupTotal totals e' + (if e == e' then 1 else 0) := by
  induction totals with
  | nil =>
      by_cases h : e = e' <;> simp [bump, lookupTotal, h]
  | cons head rest ih =>
      rcases head with ⟨k, v⟩
      by_cases hk : k = e
      · subst e
        by_cases he : k = e'
        · subst e'
          simp [bump, lookupTotal]
        · simp [bump, lookupTotal, he]
      · by_cases he : k = e'
        · subst e'
          have hek : e ≠ k := fun h => hk h.symm
          simp [bump, lookupTotal, hk, hek]
        · simp [bump, lookupTotal, hk, he, ih]

/-- Looking up employer `e` after grouping computes the same infectious-row
count as scanning the table with the join predicate. -/
theorem lookupTotal_groupTotals (table : List Person) (e : Nat) :
    lookupTotal (groupTotals table) e = table.countP («matches» e) := by
  induction table with
  | nil =>
      rfl
  | cons q rest ih =>
      by_cases hq : q.infectious = true
      · simp [groupTotals, List.countP_cons, «matches», hq, lookupTotal_bump, ih]
      · simp [groupTotals, List.countP_cons, «matches», hq, ih]

/-- DESIGN.md §7, theorem target 1a: the group-by lumping rewrite is exact
on the counting fragment — the linear grouped plan equals the quadratic
self-join plan on every table and row. -/
theorem groupedCount_eq_naiveCount (table : List Person) (i : Nat) :
    groupedCount table i = naiveCount table i := by
  unfold groupedCount naiveCount
  cases table[i]? with
  | none => rfl
  | some p => exact lookupTotal_groupTotals table p.employer

end Lumping
end Sembla
