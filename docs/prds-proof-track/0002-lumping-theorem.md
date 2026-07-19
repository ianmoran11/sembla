# PRD 0002: The lumping theorem

## Context

Read the folder README first; its constraints bind. PRD 0001 landed the two
plans as Lean functions with pinned fixtures. This PRD proves them equal —
the first landed proof of the project, theorem target 1a. The proof
decomposes into two auxiliary lemmas and a main theorem; **the statements
below are the deliverable and have been checked against the worked
fixtures** — if one appears unprovable, suspect a transcription drift in the
PRD-0001 definitions before suspecting the statement.

## Goal

`frontend/Sembla/LumpingProof.lean` proving
`groupedCount table i = naiveCount table i` for every table and index, from
Lean core only, with no `sorry`/`axiom`/`native_decide`.

## Specification

Create `frontend/Sembla/LumpingProof.lean` (namespace `Sembla.Lumping`,
imported from `frontend/Sembla.lean`) containing exactly these three
results, in this order.

**Lemma 1 — `bump` shifts one lookup by one.**

```lean
theorem lookupTotal_bump (totals : List (Nat × Nat)) (e e' : Nat) :
    lookupTotal (bump totals e) e'
      = lookupTotal totals e' + (if e == e' then 1 else 0)
```

Proof sketch: induction on `totals`.
- `nil`: both sides reduce over `bump [] e = [(e, 1)]`;
  `simp [bump, lookupTotal]` then `by_cases h : e = e' <;> simp [h]`
  (or finish with `omega`).
- `cons (k, v) rest ih`: `by_cases hk : k = e`.
  - If `k = e`: `bump` returns `(k, v+1) :: rest`. Split again on
    `by_cases he : k = e'`; each branch is
    `simp [bump, lookupTotal, hk, he]` plus `omega`. Note when `k = e` and
    `k ≠ e'`, the `if e == e'` on the right is false because `e = k ≠ e'`
    — rewrite with `hk ▸ he` or `simp` with both hypotheses.
  - If `k ≠ e`: `bump` recurses; the goal steps by `lookupTotal` on the
    unchanged head, then closes with `ih` (again splitting on
    `k = e'`).
- Bool/Prop reminder: goals will contain `k == e` (`Bool`); convert with
  `beq_iff_eq` in the simp set or case on the propositional equality and
  `simp [h]`.

**Lemma 2 — the grouping pass computes `countP` (the heart).**

```lean
theorem lookupTotal_groupTotals (table : List Person) (e : Nat) :
    lookupTotal (groupTotals table) e = table.countP (matches e)
```

Proof sketch: induction on `table`.
- `nil`: `simp [groupTotals, lookupTotal, List.countP]` (or `rfl`).
- `cons q rest ih`: unfold one step of `groupTotals` and use
  `List.countP_cons` (core:
  `countP p (a :: l) = countP p l + if p a = true then 1 else 0`).
  Split on `hq : q.infectious`:
  - `q.infectious = true`: the left side is
    `lookupTotal (bump (groupTotals rest) q.employer) e`; rewrite with
    **Lemma 1**, then `ih`. The right side's
    `if matches e q then 1 else 0` reduces, given `hq`, to
    `if q.employer == e then 1 else 0` — `simp [matches, hq]`. The two
    `if`s now agree up to the symmetry `q.employer == e` vs
    `e == q.employer` *if written carelessly* — Lemma 1 was deliberately
    stated with `bump … q.employer` on the left and `if q.employer == e`
    emerging after `simp [beq_iff_eq]`-normalizing both to the
    propositional `q.employer = e`; `by_cases h : q.employer = e <;>
    simp [h] <;> omega` closes.
  - `q.infectious = false`: `groupTotals` skips the row and
    `matches e q = false`; `simp [matches, hq, ih]`.

**Main theorem — the plans agree.**

```lean
/-- DESIGN.md §7, theorem target 1a: the group-by lumping rewrite is exact
on the counting fragment — the linear grouped plan equals the quadratic
self-join plan on every table and row. -/
theorem groupedCount_eq_naiveCount (table : List Person) (i : Nat) :
    groupedCount table i = naiveCount table i := by
  unfold groupedCount naiveCount
  cases h : table[i]? with
  | none => rfl
  | some p => exact lookupTotal_groupTotals table p.employer
```

(If `unfold` fights the match syntax, `simp only [groupedCount,
naiveCount]` then `cases h : table[i]?` achieves the same shape.)

After each theorem add `#print axioms <name>` temporarily, record the output
in the implementation notes, and remove the directive (or leave it — it is
harmless at build time; if left, note that it prints during compilation).
Expected: `does not depend on any axioms` or a subset of
`propext, Classical.choice, Quot.sound`.

## Working method (binding guidance)

- Land Lemma 1 completely before touching Lemma 2. Commit-worthy state after
  each lemma: `lake build` green.
- If a case will not close, reproduce it concretely: state an `example` with
  a 2-element `totals` list or 2-person table and `decide` it — if the
  concrete instance is *false*, a definition drifted from PRD 0001; stop and
  fix the definition, do not bend the lemma.
- `exact?` and `simp?` are available for finding core lemma names
  (`List.countP_cons`, `beq_iff_eq`, `if_pos`, `if_neg`).
- Do not introduce helper axioms, `partial def`s, or well-founded recursion;
  everything here is structural.

## Non-goals

Congruence corollary, guard script, docs edits (PRD 0003). Generalizing
beyond `Person`/`Nat` (a polymorphic key type is a tempting refactor —
refuse it; it adds `BEq`/`LawfulBEq` bookkeeping for zero payoff here).
Any change outside the two Lumping proof/test modules and the root import
list.

## Acceptance criteria

1. `cd frontend && lake build` green; negative suite and repo-root
   `./scripts/check.sh` green.
2. The three theorems exist with exactly the stated names and statements
   (statement changes require stopping and flagging, not silent weakening).
3. `grep -rn "sorry\|admit\|native_decide\|axiom" frontend/Sembla/Lumping*.lean`
   → no hits.
4. `#print axioms` output for all three theorems recorded in implementation
   notes and within the allowed set.
5. The PRD-0001 fixtures still pass unchanged (they now double as instance
   checks of the theorem).
