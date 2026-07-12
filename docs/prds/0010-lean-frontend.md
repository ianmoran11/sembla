# PRD 0010: Lean frontend — surface DSL elaborating to IR

## Context

Sembla's frontend is Lean 4 (`DESIGN.md` §3): a surface DSL that reads as
"systems with states and hazard transitions" (Poly at the surface, §4.1) and
elaborates to the deep-embedded IR executed by the Rust runtime (PRDs
0002–0009). The IR JSON is the contract; the Rust side does not change in
this PRD. Keep dependencies minimal: **no mathlib**; ProofWidgets arrives in
PRD 0011.

## Goal

A Lean package in `frontend/` in which the SIR and SIR+policy models are
written in DSL syntax and exported as IR JSON that the Rust validator accepts
and that matches the checked-in fixtures semantically.

## Specification

- `frontend/` Lake package, `lean-toolchain` pinned to a current stable
  Lean 4 release. Build must work offline-ish: only Lake-resolvable deps.
- Core Lean types mirroring the IR (PRD 0002): `Model`, `Box`, `Table`,
  `Transition`, `Expr`, etc., with a JSON serializer. These are ordinary
  inductive types — this *is* the deep embedding `DESIGN.md` §3 commits to;
  write them cleanly, they are the future home of the semantics (out of
  scope here, but don't preclude it: pure data, no IO in the model types).
- Surface syntax via Lean `syntax`/`macro`/elaborators. Target shape
  (adjust concrete syntax as elaboration demands, but preserve the reading):

  ```
  system Person where
    state health : {S, I, R}
    ref employer : Employer

  transition infect on Person where
    guard  health = S
    hazard β * countBy employer (health = I) / size
    set    health := I

  transition recover on Person where
    guard  health = I
    hazard γ
    set    health := R

  model sir where
    box population : [Person, Employer]
    param β := 0.3
    param γ := 0.1
  ```

  `param` binds model-level constants substituted into expressions at
  elaboration (the IR has no parameter table in v0.1 — literals are inlined;
  record each param's name/value in a JSON `meta` field for tooling).
- Elaboration errors must be positioned: referencing an undeclared state
  or attribute is an error *at that token*, not a panic at export.
- `lake exe sembla-export <module-or-model-name> <out.json>` writes the IR.
- Parity testing: export the DSL-written SIR model and compare against
  `examples/sir.json` **normalized** (parse both with `sembla-ir`, compare
  canonical serializations; a `sembla diff-ir a.json b.json` subcommand in
  the Rust CLI does this and is added in this PRD). Same for
  `sir_policy.json`.
- `scripts/check.sh` extended: if `lake` is on PATH, build `frontend/` and
  run the parity check; otherwise print a skip warning (keeps Rust-only
  environments green).

## Non-goals

Widgets (PRD 0011), proofs or formal semantics, mathlib, parameter sliders,
pretty error recovery beyond positioned messages, supporting models beyond
the two fixtures.

## Acceptance criteria

1. `lake build` succeeds in `frontend/` from a fresh checkout with the
   pinned toolchain.
2. `lake exe sembla-export` produces IR for the DSL-written SIR model, and
   `sembla validate` accepts it (exit 0).
3. **Parity (v0.1 success criterion #1)**: `sembla diff-ir` reports the
   exported SIR and SIR+policy models semantically identical to
   `examples/sir.json` and `examples/sir_policy.json`.
4. End-to-end: running the *exported* SIR JSON through `sembla run` at a
   fixed seed produces the same results hash as running the fixture
   (scripted in a test or documented runnable commands executed in review).
5. Negative elaboration tests: at least 3 ill-formed DSL snippets (unknown
   state, type-mismatched guard, unknown ref target) fail with positioned
   Lean errors (as `#guard_msgs` tests or equivalent).
6. `frontend/README.md` documents setup (elan/toolchain), the DSL forms, and
   the export/parity workflow.
7. `./scripts/check.sh` passes with Lean present, and still passes (with the
   skip warning) when `lake` is absent.
