# PRD 0003: Congruence corollary, proof-hygiene guard, docs landing

## Context

Read the folder README first. PRD 0002 proved the plans equal. This PRD adds
the small corollary that carries the result to hazards, makes proof hygiene a
permanent CI-enforced property, and records the landing honestly in the
project's authority documents — including what was *not* proved.

## Goal

The transport corollary is proved; `sorry`-freedom is enforced by script and
CI forever; `DESIGN.md` and `docs/ROADMAP.md` record theorem target 1a as
landed and target 1b as open.

## Specification

**1. The transport corollary** (append to
`frontend/Sembla/LumpingProof.lean`):

```lean
/-- Transport: any per-row function of the coworker count — a hazard, a
probability, any downstream expression — is unchanged by the rewrite. With
coordinate-keyed randomness (DESIGN.md §4.2), identical hazard inputs give
identical draws, so the executed distributions coincide; that final step is
an argument about the runtime's RNG discipline, recorded here as
documentation, not formalized. -/
theorem plan_rewrite_congr {α : Type} (f : Nat → α)
    (table : List Person) (i : Nat) :
    f (groupedCount table i) = f (naiveCount table i) := by
  rw [groupedCount_eq_naiveCount]
```

**2. Proof-hygiene guard.** New `frontend/scripts/check-proofs.sh`
(mirroring the style of the existing `frontend/scripts/*.sh`):

- greps `frontend/Sembla/Lumping*.lean` (and is written so adding future
  proof modules to its file list is one line) for
  `sorry`, `admit`, `native_decide`, and lines beginning `axiom`; any hit
  ⇒ exit 1 with the offending line printed;
- verifies the three PRD-0002 theorem names plus `plan_rewrite_congr` are
  present in the source (a renamed-away theorem should fail loudly);
- is wired into the repo-root `./scripts/check.sh` alongside the existing
  Lean parity checks, so the CI `lean`/`Rust` jobs enforce it from now on.

**3. Docs landing — exact edits:**

- `DESIGN.md` §7, theorem-target list, item 1: annotate it as
  *"1a proved (2026-07, `frontend/Sembla/LumpingProof.lean`:
  `groupedCount_eq_naiveCount`) — specification-level plans over the
  ℕ-counting fragment; 1b, binding the theorem to the deep-embedding
  evaluator, remains open."* Keep the item; do not reword the rest of the
  list.
- `docs/ROADMAP.md`, proof track section, item 1: same annotation in one
  sentence, dated.
- `frontend/README.md`: a short "Proofs" section — what is proved, the exact
  build/check commands, the honest-scope sentence (specification level, not
  the evaluator; the folder README's scope paragraph may be condensed but
  its meaning must survive intact).
- Do **not** edit `DECISIONS.md` (no decision changed — this executes an
  existing one), and do not touch `docs/sembla-assessment.md` (a dated
  snapshot; the proof-gap risk it names is *reduced*, not gone, and the
  next assessment can say so).

## Non-goals

Formalizing the distributional step (RNG discipline stays a documented
argument). Target 1b. New proof targets. Workflow YAML edits (the existing
CI jobs already run `check.sh` and the frontend build — wiring into
`check.sh` is sufficient).

## Acceptance criteria

1. `cd frontend && lake build` green; `bash frontend/scripts/check-proofs.sh`
   green; repo-root `./scripts/check.sh` green and now *includes* the proof
   guard (verified by temporarily inserting `sorry` into a Lumping module
   and observing `check.sh` fail, then reverting — record this in the
   implementation notes).
2. `#print axioms plan_rewrite_congr` within the allowed set, recorded.
3. The three docs edits are exactly as specified — in particular, both the
   DESIGN and ROADMAP annotations contain the words "1b" and "open" (the
   honesty clause is part of acceptance, not garnish).
4. `git diff --stat` touches only: the two Lumping proof/test modules,
   `frontend/Sembla.lean` imports if needed, `frontend/scripts/`,
   `scripts/check.sh`, `frontend/README.md`, `DESIGN.md`, and
   `docs/ROADMAP.md`.
