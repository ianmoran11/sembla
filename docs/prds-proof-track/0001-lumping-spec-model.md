# PRD 0001: Lumping specification model and executable fixtures

## Context

Read `docs/prds-proof-track/README.md` first — its constraints and survival
guide are binding. This PRD defines the two query plans as executable Lean
functions and pins them with computed fixtures. **No proofs beyond `rfl`/
`decide` on closed terms are in scope** — getting the definitions right and
building cleanly is the whole job. PRD 0002 proves them equal.

## Goal

A new module `frontend/Sembla/Lumping.lean` defining the naive and grouped
counting plans, plus `frontend/Sembla/LumpingTests.lean` with worked
fixtures, both built as part of the `Sembla` lib.

## Specification

Create `frontend/Sembla/Lumping.lean` with **exactly these definitions**
(doc comments may be expanded; semantics must not drift):

```lean
namespace Sembla
namespace Lumping

/-- One person row of the lumping fragment: an employer id and an
infectious flag. Mirrors the `infect` example of `DESIGN.md` §7. -/
structure Person where
  employer : Nat
  infectious : Bool
deriving Repr, BEq, DecidableEq

/-- The join predicate: `q` is an infectious member of employer `e`. -/
def matches (e : Nat) (q : Person) : Bool :=
  q.employer == e && q.infectious

/-- Naive self-join plan: for the row at index `i`, scan the whole table
and count infectious rows sharing its employer (the quadratic plan). Out
of range ⇒ 0. -/
def naiveCount (table : List Person) (i : Nat) : Nat :=
  match table[i]? with
  | none => 0
  | some p => table.countP (matches p.employer)

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
```

Notes: `table[i]?` may be spelled `table.get? i` if the bracket form fights
the elaborator on this toolchain — both are core; pick one and use it in both
plans. Do **not** "simplify" `groupTotals` into `countP` — the two plans must
remain visibly different computation strategies, or PRD 0002's theorem
becomes vacuous.

Create `frontend/Sembla/LumpingTests.lean` with:

1. **The 12-person worked fixture** (2 employers would be too degenerate;
   use this exact table over employers {0,1,2}):
   rows (employer, infectious) =
   `(0,T) (1,F) (0,F) (2,T) (1,T) (0,T) (2,F) (1,T) (0,F) (2,T) (1,F) (0,T)`.
   Hand-derived truth: employer 0 has 3 infectious (rows 0, 5, 11),
   employer 1 has 2 (rows 4, 7), employer 2 has 2 (rows 3, 9). Therefore the
   per-row count vector is `[3, 2, 3, 2, 2, 3, 2, 2, 3, 2, 2, 3]`.
   Pin **both** plans against it:
   ```lean
   example :
       (List.range fixture.length).map (Lumping.naiveCount fixture)
         = [3, 2, 3, 2, 2, 3, 2, 2, 3, 2, 2, 3] := by decide
   example :
       (List.range fixture.length).map (Lumping.groupedCount fixture)
         = [3, 2, 3, 2, 2, 3, 2, 2, 3, 2, 2, 3] := by decide
   ```
   plus out-of-range: `example : Lumping.naiveCount fixture 12 = 0 := by
   decide` (and the grouped twin).
2. **The Rust-twin fixture**, generated with the same formulas as
   `crates/sembla-runtime/tests/eval.rs`
   (`group_by_count_matches_naive_quadratic_lumping_reference`) but sized
   for kernel `decide`: 60 rows, 7 employers,
   `employer row = (row * 17 + row / 7 + 11) % 7`,
   `infectious row = decide ((row * 29 + row / 5 + 3) % 11 < 4)`. Build it
   with `(List.range 60).map`, then pin plan agreement:
   ```lean
   example :
       (List.range rustTwin.length).map (Lumping.naiveCount rustTwin)
         = (List.range rustTwin.length).map (Lumping.groupedCount rustTwin) := by
     decide
   ```
   with a comment naming the Rust test as this fixture's operational twin.
   If `decide` at 60 rows is unacceptably slow on this toolchain, reduce to
   40 rows — never switch to `native_decide`.
3. Register both new files in `frontend/Sembla.lean`'s import list.

## Non-goals

The equivalence theorem and all lemmas (PRD 0002). The congruence corollary,
guard script, and docs (PRD 0003). Any change to `IR.lean`, `DSL.lean`,
widgets, the exporter, or anything outside `frontend/`. Any new dependency.

## Acceptance criteria

1. `cd frontend && lake build` green; `bash scripts/test-negative.sh` green;
   repo-root `./scripts/check.sh` green.
2. All fixture `example`s above compile (they are checked at build time —
   that is the test).
3. `grep -rn "sorry\|admit\|native_decide" frontend/Sembla/Lumping*.lean`
   returns nothing; `grep -n "axiom" frontend/Sembla/Lumping*.lean` returns
   nothing.
4. Every public def has a doc comment; module headers cite `DESIGN.md` §7
   and state the honest scope (specification-level model; see the folder
   README's scope section).
5. Implementation notes record any spelling deviations taken (e.g. `get?`
   vs bracket indexing) and why.
