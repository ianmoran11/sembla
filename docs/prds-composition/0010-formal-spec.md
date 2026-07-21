# PRD 0010: Observation contract, denotation skeletons, preservation statements

## Context

Read `docs/prds-composition/README.md` first; its constraints bind. The
linker is functionally complete for the first release (PRDs 0007–0009). The
architecture doc §14 requires, **before surface syntax is considered
stable**: an observation contract, *independent* source and plan denotations
(source meaning must not be defined as "whatever the linker produces"), and
preservation statements that typecheck. Proofs are deferred per project
policy (DESIGN.md §3, DECISIONS §A3) — but statements are paid for now, and
whatever *can* be cheaply proved or executably checked is.

Honest scoping, restated: a full executable denotational semantics of
stochastic tau-leaped execution is out of scope. What is in scope and
genuinely checkable now is the **static/structural denotation**: the sets of
leaf, transition, wire/mailbox, and identity objects a source *means*,
defined by structural recursion on the source — independently of `linkV1` —
and compared against what the plan *contains*. That is exactly the fragment
the byte-level laws of PRD 0009 rest on, and it is where linker bugs would
live.

## Goal

Lean modules defining the observation contract, an independent static source
denotation, a plan denotation, named preservation propositions that
typecheck, a proved validity-by-construction theorem for `linkV1`, executable
preservation checks over the whole fixture corpus, and proof-hygiene guards
extended to cover the new spec modules.

## Specification

### 1. Observation contract — `frontend/Sembla/Composition/SpecObservation.lean`

Define the *type* of observations the eventual full preservation theorem
quantifies over (architecture doc §14.2), as a Lean structure over abstract
index types — it must typecheck, it is not yet computed:

```lean
structure CompositionObservation where
  leafState : StableId → Nat → CanonicalTableState
  mailboxState : StableId → Nat → CanonicalTableState
  externalOutputs : StableId → Nat → CanonicalTable
  fired : Nat → List StableId
  drawCoordinates : Nat → List DrawCoordinate
  observations : StableId → Nat → IR.Scientific
```

with `CanonicalTableState`, `CanonicalTable`, `DrawCoordinate` declared as
structures/abbreviations with documented fields (`DrawCoordinate` must be the
Philox tuple: `(tick, ruleWord : UInt32, entityId, drawIdx)`). Document in
module comments which fields the V1 equivalence includes (all of them) and
that hashes are consequences of canonical artifacts, not part of the
observation (doc §14.2).

### 2. Static denotations — `frontend/Sembla/Composition/SpecStatic.lean`

Two **independent** functions with the same result type:

```lean
structure StaticMeaning where
  leaves : List (String × StableId)          -- (leaf box name, occurrence)
  transitions : List (String × UInt32)       -- (identity, rule word)
  mailboxes : List String                    -- mailbox identities
  boundary : List (String × String)          -- (outer port, resolved leaf.port)
  deriving Repr, BEq

def denoteSourceStatic (src : CompositionSourceV1) :
    Except (List LinkErrorV1) StaticMeaning

def denotePlanStatic (plan : Plan.ExecutablePlanV1) : StaticMeaning
```

Hard requirement (doc §14.1): `denoteSourceStatic` is written by **direct
structural recursion on the source** — expansion, boundary resolution, and
identity construction re-derived here, *without calling* `linkV1` or sharing
its expansion code. Yes, this duplicates logic; the duplication is the point
— two independent derivations that must agree. Put a module comment saying
exactly that so a future refactor does not "deduplicate" the check away.
`denotePlanStatic` merely reads the plan's identity map and model. All lists
sorted; `StaticMeaning` equality is decidable `BEq`.

### 3. Statements — `frontend/Sembla/Composition/SpecStatements.lean`

```lean
/-- Full behavioral preservation: the obligation, stated but not proved. -/
def preservationStatement : Prop :=
  ∀ (src : CompositionSourceV1) (bytes : String) (r : LinkResultV1),
    linkV1 src bytes = .ok r →
    denoteSourceObs src = denotePlanObs r.plan

/-- Static preservation: the V1-checkable core of the above. -/
def staticPreservationStatement : Prop :=
  ∀ (src : CompositionSourceV1) (bytes : String) (r : LinkResultV1) (m : StaticMeaning),
    linkV1 src bytes = .ok r →
    denoteSourceStatic src = .ok m →
    denotePlanStatic r.plan = m
```

where `denoteSourceObs`/`denotePlanObs` are declared `opaque` constants of
type `CompositionSourceV1 → CompositionObservation` /
`ExecutablePlanV1 → CompositionObservation` (signatures typecheck; the full
behavioral denotations are future work and their opacity is documented as
such). Named `def … : Prop` declarations assert nothing — no `sorry`, no
`axiom` — while freezing the obligation's exact shape.

Prove the one theorem that is cheap because PRD 0007 §2.11 structured
`linkV1` for it:

```lean
theorem linkV1_produces_valid_plan
    (src : CompositionSourceV1) (bytes : String) (r : LinkResultV1)
    (h : linkV1 src bytes = .ok r) :
    planValidCheck r.plan = true
```

Proof strategy: `linkV1` ends in
`if planValidCheck plan then .ok … else .error …`; unfold/split on that
final branch (restructure `linkV1`'s tail into a small named `finalize`
function first if that makes the proof tractable — behavior-preserving,
parity re-proves it). If after genuine effort the general proof does not
close, the fallback is: make `LinkResultV1.plan` a subtype
`{ p : ExecutablePlanV1 // planValidCheck p = true }` so validity is carried
by construction and the theorem is its projection — an API change to
`LinkResultV1` that PRDs 0007–0009 call sites must absorb without behavior
change. One of the two must land; `sorry` may not.

### 4. Executable preservation over the corpus

In `frontend/Sembla/Composition/SpecTests.lean`, for **every** fixture the
corpus can link (`solo_population`, `independent_epidemic_policy`,
`two_independent_regions`, `epidemic_policy`, `ping_pong`, `two_regions`,
`regional_response`, `wrapped_ping_pong` and the PRD 0009 variants):

```lean
#guard checkStaticPreservation fixtureName = true
-- where checkStaticPreservation links, denotes both sides, and compares
```

plus targeted guards that `denoteSourceStatic` agrees with hand-written
expected `StaticMeaning` values for the two smallest fixtures
(`solo_population`, `ping_pong`) — hand-expected values catch the failure
mode where both derivations share a bug. Also negative: a source that fails
to link must also fail `denoteSourceStatic` with the same error codes for at
least the duplicate-id and missing-definition cases.

### 5. Proof hygiene extension

Extend `frontend/scripts/check-proofs.sh`: add
`"$frontend_root"/Sembla/Composition/Spec*.lean` to `proof_sources` (so
`sorry`/`admit`/`native_decide`/`axiom` are forbidden there) and add a second
required-name loop checking these exist: theorem
`linkV1_produces_valid_plan`; defs `preservationStatement`,
`staticPreservationStatement`, `denoteSourceStatic`, `denotePlanStatic`
(grep for `^[[:space:]]*(def|theorem)[[:space:]]+<name>` analogous to the
existing loop). Do not modify the existing Lumping entries.

### 6. Documentation

Module doc-comments must record: proof status of each statement (proved /
stated-deferred), the V1 observation-quotient decision (all fields, draw
coordinates included), and a pointer to DECISIONS §J10. Add one short
subsection to `DECISIONS.md` §J (e.g. `### J13. Observation quotient and
proof obligations (2026-07)`) recording that the static fragment is
executable-checked, full behavioral preservation is stated-deferred, and
rollout gates on the executable checks per architecture doc §14.3.

## Allowed files

- `frontend/Sembla/Composition/SpecObservation.lean`, `SpecStatic.lean`,
  `SpecStatements.lean`, `SpecTests.lean` (new)
- `frontend/Sembla/Composition/Link.lean` (only the §3 `finalize`
  restructure or subtype fallback; byte-identical linked goldens prove no
  behavior change)
- `frontend/Sembla.lean`, `frontend/scripts/check-proofs.sh`
- `DECISIONS.md` (append §J13 only)
- implementation notes/artifacts created by the managed run

## Non-goals

- Proving behavioral preservation, product/associativity theorems, or
  anything about Rust/CUDA (trust boundary: doc §4.3).
- Executable stochastic denotational semantics.
- Changing linker behavior, plan bytes, fixtures, or hashes — if any linked
  golden changes, the restructure went wrong.
- New laws beyond those PRD 0009 already tests.

## Acceptance criteria

1. `lake build` passes; all `checkStaticPreservation` guards over the full
   linkable corpus hold, plus the two hand-expected `StaticMeaning` guards.
2. `linkV1_produces_valid_plan` is proved with no `sorry`, `admit`, `axiom`,
   or `native_decide` (enforced by the extended `check-proofs.sh`, which
   passes).
3. `preservationStatement` and `staticPreservationStatement` typecheck as
   `Prop` definitions; `denoteSourceObs`/`denotePlanObs` are opaque with the
   frozen signatures; `DrawCoordinate` carries the Philox tuple shape.
4. Code inspection confirms `denoteSourceStatic` does not call `linkV1` or
   its expansion helpers (independent recursion, with the module comment).
5. Every linked golden from PRDs 0007–0009 is byte-unchanged
   (parity script proves it).
6. DECISIONS §J13 exists; `./scripts/check.sh` and `git diff --check` pass.
