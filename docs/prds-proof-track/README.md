# Proof-track PRDs: the group-by lumping theorem

Three-PRD set landing Sembla's **first proof** — theorem target 1 of
`DESIGN.md` §7: the group-by lumping rewrite is correct. Run with
`/piprd run docs/prds-proof-track`. This README is excluded from runs and is
**required reading for every PRD** — it carries the constraints and the Lean
survival guide the PRDs assume.

## Why this exists

`DESIGN.md` §7 / `DECISIONS.md` §H6: the project's thesis is *optimization =
certified equivalence* — every backend rewrite should correspond to an
equivalence the theory can state. Zero proofs have landed; the thesis is
currently aspiration. The cheapest high-value first proof is the flagship
example: computing each person's *count of infectious coworkers* by a naive
self-join (quadratic) versus by group-by-then-broadcast (linear) gives
identical results. The Rust runtime already pins this operationally
(`crates/sembla-runtime/tests/eval.rs`,
`group_by_count_matches_naive_quadratic_lumping_reference`). These PRDs land
the *mathematical* counterpart in Lean.

## Honest scope — read carefully, it is part of the deliverable

The Lean frontend has a deep embedding of the IR (`frontend/Sembla/IR.lean`)
but **no meaning function yet**. These PRDs do *not* build the full IR
semantics. They define a small, faithful **specification-level model** of the
two query plans (tables as lists, counts as naturals) and prove the plans
equal there. That is a real theorem about the rewrite's mathematics — exact
lumping on the counting fragment — but it is **not** yet a theorem about the
deep embedding's evaluator. Every document written by these PRDs must say so
plainly: this is *target 1a (specification level, proved)*; *target 1b
(binding to the deep-embedding evaluator)* remains open on the proof track.
Overclaiming is a review-rejection offense in this repository
(`docs/sembla-assessment.md` lists the proof gap as risk #1 precisely because
claims outran proofs elsewhere).

## Run order

| # | PRD | Layer |
|---|-----|-------|
| 0001 | Specification model + executable fixtures (no proofs) | Lean defs |
| 0002 | The lumping theorem (three lemmas) | Lean proofs |
| 0003 | Congruence corollary, proof-hygiene guard, docs landing | Lean/docs/CI |

## Hard constraints (binding on all PRDs)

1. **Toolchain:** Lean `v4.13.0` exactly (`frontend/lean-toolchain`). Work
   only inside `frontend/`; no Rust changes anywhere in this set.
2. **No new dependencies.** The manifest has ProofWidgets4 only. **Mathlib,
   Std/Batteries, or any other library must NOT be added** — the proofs below
   are designed to need only Lean core. Adding a dependency fails review.
3. **No `sorry`, no `admit`, no new `axiom`, no `native_decide`.**
   `native_decide` is banned because it adds `Lean.ofReduceBool` to the
   trusted base. After each theorem, run `#print axioms <name>` — the output
   must list nothing beyond `propext`, `Classical.choice`, `Quot.sound`
   (fewer is fine; the intended proofs use none).
4. **Existing behavior untouched:** `lake build` must stay green for the
   whole library (widgets included); `bash scripts/test-negative.sh` and the
   repo-root `./scripts/check.sh` must pass unchanged. New modules are
   *added* to the `Sembla` lib; nothing existing is edited except the import
   list in `frontend/Sembla.lean` and (PRD 0003) the check scripts and docs.
5. **Build command:** `cd frontend && lake build`. First build may download
   ProofWidgets artifacts; that is normal. A single file can be checked
   faster with `lake env lean Sembla/Lumping.lean`.

## Lean 4 survival guide (for the implementing agent)

You do not have mathlib. You do have Lean core, which includes everything
these proofs need:

- **Types:** `Nat`, `Bool`, `List`, `Option`, `Prod`. Tables are
  `List Person`; totals are `List (Nat × Nat)` association lists.
- **Core list API:** `List.countP`, `List.length`, `l[i]?` (indexing into an
  `Option`, also spelled `l.get? i`). The key core lemma is `List.countP_cons`
  (`countP p (a :: l) = countP p l + if p a = true then 1 else 0`).
- **Tactic toolbox — in the order to reach for them:**
  - `rfl` / `decide` — closed computations (fixtures, small examples).
  - `simp [defName₁, defName₂]` — unfold your own definitions by naming them.
  - `omega` — finishes any linear `Nat`/`Int` goal (replaces linarith/ring
    for this work).
  - `induction xs with | nil => … | cons x rest ih => …` — the shape of every
    proof here.
  - `by_cases h : k = e`, `split`, `cases … with` — case analysis.
  - `exact?` / `simp?` — core library-search tactics; use them when a core
    lemma name is unknown, then replace with the found name.
- **`Bool` vs `Prop` — the one recurring trap.** `q.employer == e` is a
  `Bool` (`BEq`); `q.employer = e` is a `Prop`. Move between them with
  `beq_iff_eq` (simp lemma) or `by_cases` on the propositional form followed
  by `simp [h]`. When a goal contains `if (a == b) then …`, `by_cases
  h : a = b <;> simp [h]` usually collapses it; `omega` mops up.
- **When stuck:** shrink — prove the statement for `[]` and `[p]` as
  `example`s first to find the right simp set, then generalize. Do not
  weaken a theorem statement to pass; if a stated lemma seems false, stop
  and re-derive the expected values by hand with `#eval` — the statements in
  these PRDs have been checked against worked examples, so a mismatch means
  a definition was transcribed wrongly.
- **Style:** follow the existing modules (`Sembla/IR.lean`) — `namespace
  Sembla`, doc comments on every public def/theorem, `deriving Repr, BEq`
  where useful for `#eval`.

## Verification culture

Every PRD lands with: `lake build` green, the negative suite green,
`./scripts/check.sh` green from the repo root, zero occurrences of
`sorry|admit|axiom|native_decide` in the new modules (PRD 0003 turns this
grep into a permanent script), and `#print axioms` output recorded in the
implementation notes for each theorem.
