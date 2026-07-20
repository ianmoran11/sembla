# Surface-syntax options: from nested lists to mathematics

**Status:** Design proposal / discussion note, 2026-07-20. Exploratory — not
yet folded into `DESIGN.md`.
**Scope:** the Lean surface DSL only (`frontend/Sembla/DSL.lean`). The IR, its
JSON encoding, the validator, and both backends are untouched by everything
here — the frontend-agnostic IR (`DECISIONS.md` §A5) is what makes this note
cheap to act on or ignore.

---

## 1. Diagnosis: why models read as "a big nested list"

Today's `model%` term is one giant expression built from comma-separated
bracket blocks:

```lean
def sir : Model := model% "sir_workplace_frequency_dependent" step(0.25) where
  params [
    param beta : Real := 0.8 prior LogNormal(-0.2231435513142097, 0.25), ...]
  boxes [
    box sir where
      systems [
        system Person as "person" rows(1000000) where [
          state health : {S, I, R},
          ref employer : Employer], ...]
      transitions [
        transition infect on Person where
          guard health = S
          hazard parameter beta * (countBy employer (health = I) / sizeBy employer)
          set [health := I], ...] ...]
```

Five specific causes, each fixable independently:

1. **Bracket-and-comma block structure.** `params [...]`, `boxes [...]`,
   `transitions [...]` are lists whose entries are separated by commas —
   JSON with keywords. Lean users declare things with *declarations*, not
   comma-lists; mathematics does too.
2. **Keyword-marked references.** `parameter beta` inside a hazard is a
   parser tag, not notation. A mathematician writes `β`.
3. **Quoted plumbing.** `as "person"` leaks the wire format into the surface.
4. **Vertical keyword blocks where an arrow would do.** Every transition in
   every checked-in model is "from one enum state, at a rate, to another
   enum state" — four lines of `guard`/`hazard`/`set` for what the CTMC
   literature writes as `S →ᵦ I`.
5. **One syntax node.** The whole model is a single term, so errors point
   into the middle of a 30-line expression and widgets have one cursor
   target instead of one per declaration.

The DSL is already implemented with `declare_syntax_cat` and leveled
precedences, so every option below can be a **layered desugaring** — new
surface forms macro-expanding into the existing categories (or into the same
elaborator functions), with `model%` retained as the kernel format.

## 2. Constraints any option must respect

- **Byte-stable IR.** Golden fixtures and the Lean↔Rust parity checks pin
  exported JSON byte-for-byte. Sugar must elaborate to *identical* IR —
  including derived runtime names (see the `as "person"` note in §3.B).
- **No inert syntax** (`DESIGN.md` §5.5): every new form fully elaborates or
  errors with a named diagnostic. Each sugar ships with `Negative/` tests,
  matching the existing suite's pattern.
- **The closed fragment is the law.** Prettier aggregate notation must still
  only express join-on-declared-keys / commutative-monoid reductions
  (`DESIGN.md` §4.2). Notation that *looks* like arbitrary set comprehension
  must be rejected at elaboration when the predicate leaves the fragment —
  the diagnosis being the feature, not a limitation.
- **No new dependencies** (no mathlib for notation conveniences).
- Machine writers keep the low-level path: generated models (tests, tooling)
  construct IR terms directly; sugar is for humans.

## 3. The options

### A. Reaction arrows for transitions — the biggest mathematical win

CTMC/reaction-network notation, in the house style of mathlib's
`→ₗ[R]`-family arrows, inside our own syntax category (so no collision with
the function arrow):

```lean
infect  : S →[β · freq (health = I) over employer] I
recover : I →[γ] R
```

reading "from health-state `S`, at this hazard, to `I`". Desugars exactly to
today's `guard health = S / hazard … / set [health := I]` — the state
attribute is inferred when the system has one `state` column, named
explicitly otherwise (`infect : health: S →[…] I`). The general
`transition … where guard/hazard/set` form stays for multi-effect or
multi-guard transitions (e.g. `sirPolicy`'s controller, which also sets
`modifier`); the arrow is sugar for the dominant case — which is currently
*every transition in every canonical model*.

- **Fixes:** cause 4; collapses 4 lines to 1 that reads as mathematics.
- **Cost: S–M.** One syntax rule + desugaring + negative tests (unknown
  state, ambiguous state column, non-enum source).
- **Risk:** low. Scoped category; precedence already managed.

### B. Real binders instead of tags and strings

Three related subtractions, one addition:

- **Parameters become bindings.** `param β : ℝ := 0.8` introduces `β` into
  the model's scope; hazard expressions then use `β` bare, and elaboration
  emits `Expr.Param "beta"` when an identifier resolves to a declared
  parameter. `parameter beta` dies. (Within the DSL's own expression
  category this is a lookup-environment change, not term-level magic.)
- **Priors get the statistician's tilde.**
  `param β : ℝ := 0.8 ~ LogNormal (-0.2231) 0.25` — `~` is exactly how the
  model would appear in a methods section, and the prior stays declarative
  metadata (`DECISIONS.md` §G3).
- **Runtime names derive from identifiers.** `system Person` exports table
  name `person` by convention (documented snake_case mapping), with
  `(name := "…")` as the explicit override. `as "person"` dies. *Migration
  note:* derivation must reproduce the exact names in checked-in fixtures,
  or the override is used there — byte-stability is acceptance.
- **Unicode where Lean users already live:** `ℝ` for `Real`, greek idents
  (already legal), `∧`/`≠`/`≤` alongside `&&`/`=`/`<` in guards, `·` for
  multiplication in hazards. Pure notation aliases in the existing
  categories.

- **Fixes:** causes 2, 3.
- **Cost: S** for tilde/unicode/names; **M** for parameter-scope resolution
  (the elaborator gains an environment instead of a keyword).
- **Risk:** name-derivation collisions (two idents snake_casing identically)
  — reject with a diagnostic; that's a new negative test, not a design
  problem.

### C. Mathematical aggregates

Two candidate notations for the relational layer, both constrained to the
fragment:

```lean
-- (i) counting comprehension, explicitly keyed:
hazard β · #{q ∈ Person by employer | q.health = I} / #{q ∈ Person by employer}

-- (ii) the epidemiological idiom as a named combinator:
hazard β · freq (health = I) over employer
```

(i) is honest set-builder notation where `by employer` *is* the declared
join key — omitting it is an elaboration error naming the rule ("aggregates
join on declared keys only", `DESIGN.md` §4.2). (ii) reads best for the
frequency-dependent hazard that appears in five of seven checked-in models
and desugars to (i). Recommend shipping (ii) now and (i) when a model needs
a count that isn't a frequency.

- **Fixes:** the least-mathematical corner of expressions
  (`countBy`/`sizeBy`).
- **Cost: S** for (ii); **M** for (i) (dotted row-variable scoping).
- **Risk:** (i) tempts users toward predicates outside the fragment;
  the elaborator's rejection diagnostic must teach, not just refuse.

### D. Command-style declarations — the structural fix

Replace the single term with a *sequence of commands*, the way Lean itself
declares things:

```lean
sembla_model SirWorkplace (dt := 0.25) where

  param β : ℝ := 0.8 ~ LogNormal (-0.2231) 0.25
  param γ : ℝ := 0.1 ~ LogNormal (-2.3026) 0.25

  box sir where
    system Person (rows := 1_000_000) where
      health   : {S, I, R}
      employer : Employer
    system Employer (rows := 50_000)

    infect  : S →[β · freq (health = I) over employer] I
    recover : I →[γ] R

    view S := count Person where health = S
    view I := count Person where health = I
    view R := count Person where health = R

  summary peak_I    := max sir.I
  summary peak_tick := argmaxₜ sir.I
```

No comma-lists, no brackets; whitespace-structured `where` blocks like
`structure`/`inductive`. Every declaration is its own syntax node — errors
land on the offending line, and each `param`/`system`/transition is a
distinct cursor target for the infoview widgets (the project's #1 reason for
Lean, `DECISIONS.md` §A1). The command still ultimately defines
`SirWorkplace : Model`, so the exporter, fixtures, and parity checks are
untouched.

- **Fixes:** causes 1 and 5 — the "big nested list" feeling itself.
- **Cost: M–L.** This is real macro work (block-structured command syntax,
  or an environment-extension accumulator closed by the final `where`
  block's end). The existing elaborator core is reused; the surface parser
  is rebuilt.
- **Risk:** moderate — whitespace-sensitive blocks need care; mitigated by
  keeping `model%` as the kernel target so A–C land first and D desugars
  into forms already tested.

### E. do-notation builder — considered, not recommended for humans

```lean
def sir : Model := build do
  let β ← param 0.8 (prior := .logNormal …)
  …
```

Natural to Lean *programmers*, and let-bound identifiers give free
go-to-definition — but it reads as imperative construction, not
mathematics, which is the opposite of the request. Worth keeping in the back
pocket as the *programmatic* authoring API if generated-model tooling ever
wants more safety than raw IR constructors; not the human surface.

## 4. Recommended sequencing

| Phase | Contents | Size | Yield |
|---|---|---|---|
| 1 | B (binders, tilde, names, unicode) + A (arrows) | S–M | Transitions and parameters — the lines people actually read — become mathematics; zero structural risk |
| 2 | C(ii) `freq … over …`, then C(i) comprehensions on demand | S | The hazard expressions finish the job |
| 3 | D command-style blocks | M–L | Kills the nested-list shape and gives widgets per-declaration anchors |
| — | E | — | Only if programmatic authoring demands it |

Each phase is a layered macro over the previous surface, `model%` stays the
kernel, and every phase's acceptance is: all fixtures re-export
**byte-identical** IR, negative tests for each new diagnostic, parity checks
green. Phase 1+2 could be one PRD folder; Phase 3 deserves its own after the
sugars settle.

## 5. What this buys beyond aesthetics

- **"Easily parsed" is literal:** `declare_syntax_cat` rules *are* the
  grammar, checked by the compiler — the syntax definitions double as the
  authoritative surface-language reference, which no ad-hoc parser gives.
- The arrow form makes the **Poly-flavored reading** (`DESIGN.md` §4.1: "an
  Individual is a system with states and reactions") true on the page, not
  just in prose.
- Per-declaration syntax nodes are the missing substrate for richer
  **structure widgets** — hover a transition arrow, see the state diagram
  with that edge highlighted; hover `~ LogNormal …`, see the prior curve.
  The syntax work and the widget roadmap are the same investment.
