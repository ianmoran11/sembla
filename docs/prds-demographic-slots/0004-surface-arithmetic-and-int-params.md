# PRD 0004: Surface arithmetic `set` effects and `Int` parameters

## Context

Read `docs/prds-demographic-slots/README.md` first; DECISIONS §K8 binds:
both constructs expose capability the IR and runtime have had since v0.1,
so neither needs a feature flag — these are pure frontend gaps.

Verified facts the implementation relies on:

- `Effect::SetAttr { attr, value: Expr }` takes a **full expression**
  (`crates/sembla-ir/src/model.rs:166`), and the runtime evaluates effect
  values over the old snapshot; `Expr` has `Add/Sub/Mul/Div`, `Int`,
  `Param`, `SelfAttr`. The demographic ageing transition
  (`set age_months := age_months + 1`) lowers to IR that executes today.
- `ParamType.int` / `ParamValue.int` exist in both IRs
  (`frontend/Sembla/IR.lean:21-22`, `crates/sembla-ir/src/model.rs:35,42`),
  but the surface hard-codes `ParamType.real` at every param elaboration
  site (`frontend/Sembla/DSL.lean:1639,1642`).
- The current surface `set` RHS accepts only enum variants and literals
  (the command-frontend contract in `docs/prds-surface-syntax/README.md`).

The surface-syntax track's rules bind here: one semantic kernel, no second
expression elaborator, byte-identical legacy elaboration, exact positioned
diagnostics, and the no-inert-syntax rule.

## Goal

`set <attr> := <expr>` accepts the existing row-local scalar expression
fragment for `Int`/`Real` attributes, and `param <name> : Int := <int>` is
accepted, both lowering to existing IR with byte-identical output for all
previously valid sources, full negative coverage, and no new semantics.

## Specification

### 1. Arithmetic `set` effects

In `frontend/Sembla/DSL.lean` (the single semantic kernel — locate the
current `set` elaboration and extend it in place):

- **Grammar.** The RHS of `set <attr> := <rhs>` becomes: the same scalar
  expression fragment the kernel already elaborates in guard/hazard
  position — identifiers (attribute/parameter resolution with the existing
  ambiguity/unknown diagnostics), `Int`/`Real` literals, `+ - · /` at the
  existing precedences, and parentheses. Do **not** write a new expression
  parser or elaborator; route through the existing one. Aggregates
  (`countBy`, `freq`, `inputSum`, …) in effect position: allow exactly
  what the IR/runtime accepts — determine this by reading the runtime
  effect evaluator first; if effect-position aggregates are exercised
  nowhere in the runtime today, **reject them at the surface** with a
  positioned diagnostic (`aggregates are not supported in effect
  expressions`) rather than shipping an untested lowering. Record the
  determination and evidence in the implementation notes.
- **Typing.** The RHS must typecheck to the attribute's type: `Int` attr ⇐
  Int-typed expression, `Real` attr ⇐ Real-typed (existing numeric typing
  rules; no implicit Int→Real coercion beyond what the expression fragment
  already does — mirror guard-position behavior exactly). Enum attributes
  keep the variant-literal-only rule; `Ref` attributes keep the existing
  rejection with the existing message (`DSL.lean:1397` — byte-identical,
  the negative suite proves it).
- **Legacy byte-identity.** `set mode := Restricted` and
  `set modifier := 0.4` and every other previously valid form elaborate to
  byte-identical IR JSON (canonical exports and parity prove it; also add
  a focused twin `#guard`: the literal form and the same value written as
  a trivial expression, e.g. `0.4` vs `(0.4)`, produce identical IR).
- **Self-reference works:** `set age_months := age_months + 1` reads the
  old snapshot by existing runtime semantics — add a runtime-facing test
  model exercising increment (see §3).

### 2. `Int` parameters

- **Grammar.** `param <name> : Int := <int-literal>` (negative literals
  allowed) alongside the existing `ℝ` form, in both the list kernel and
  command frontend. Lower to `ParamDecl` with `ParamType.int` /
  `ParamValue.int`.
- **No priors on Int.** `param n : Int := 5 ~ LogNormal …` is a positioned
  error (`priors are not supported on Int parameters`); the prior families
  are real-valued and nothing changes in the IR prior model.
- **References.** An `Int` parameter is referenced by bare identifier
  exactly like a real parameter, participates in the same
  ambiguity/unknown diagnostics, and typechecks as `Int` in expressions
  (usable in guards like `age_months < retirement_months` and in the new
  arithmetic effects).
- **Rust side.** Confirm (and test) that `--params` θ overrides accept
  Int parameter values through the existing resolution path — the IR
  supports it; if any CLI/manifest resolution site assumes real-only, fix
  it minimally and note it. Resolved Int θ appears in manifests via the
  existing `ResolvedValue` machinery.

### 3. Tests

- **Positive twins** (imported Lean test module): a model using arithmetic
  set effects and Int params, with an exact-IR `#guard` (the established
  twin pattern) proving the lowering — including `set x := x + 1`,
  `set y := β · x`? — no: cross-type `Real := Int·Real` only if guard
  position already allows it; mirror guard typing exactly. Keep the twin
  aligned with what §1 typing permits.
- **Runtime round-trip:** export a small increment model
  (single-row table, `set counter := counter + 1` at hazard `1e300`) via
  the canonical exporter path into a CLI test; run 5 ticks; assert the
  view trace is `1,2,3,4,5`. This is the end-to-end proof the surface gap
  is actually closed against the running system.
- **Negative suite** (exact `file:line:column: error:` expectations, the
  complete-set helper): type mismatch (`set age_months := 0.5`), enum RHS
  expression (`set mode := mode + 1`), Ref RHS unchanged message, unknown
  identifier in RHS, ambiguous identifier, aggregate-in-effect (if
  rejected per §1), prior on Int param, real literal for Int param
  (`param n : Int := 0.5`), Int param name collisions (existing
  model-global rule).

### 4. Documentation

Update the surface documentation that `docs/prds-surface-syntax/README.md`
PRD 0006 landed (find the authority doc it produced — likely
`docs/`-level or README surface sections) with the two new forms, marked
as additions with their typing rules. Do not modify the frozen historical
PRD folder itself.

## Allowed files

- `frontend/Sembla/DSL.lean`
- new/extended Lean test modules under `frontend/Sembla/`,
  `frontend/Sembla.lean` (imports)
- `frontend/Negative/**`, `frontend/scripts/test-negative.sh` (additions
  only)
- `crates/sembla-cli/src/**` and tests (only the minimal Int-θ resolution
  fix if §2 finds one), `crates/sembla-cli/tests/**` (the runtime
  round-trip test)
- surface authority documentation file(s)
- implementation notes/artifacts created by the managed run

## Non-goals

- New operators, coercions, `Expr::Tick`, date types, or any §K9 deferred
  construct.
- Aggregates in effects if the runtime evidence says untested (reject
  instead — no inert or untested syntax).
- Contest syntax (PRD 0005), grouped views (PRD 0006).
- Changing guard/hazard elaboration, canonical model bytes, or any
  existing diagnostic position.

## Acceptance criteria

1. Full check battery + negative suite + parity pass; all canonical
   exports byte-identical (legacy `set` and `param` forms unchanged).
2. The increment model's runtime round-trip produces `1,2,3,4,5`.
3. Exact-IR twins pin the lowering for arithmetic effects and Int params;
   review confirms one expression elaborator serves guard, hazard, and
   effect positions.
4. Every listed negative case fails at the right position with the exact
   expected message; the Ref-write message is byte-identical to before.
5. The aggregate-in-effect determination (allowed vs rejected) is recorded
   with runtime-code evidence in the implementation notes.
6. Int θ overrides resolve end-to-end (CLI test) and appear in the
   manifest; `git diff --check` passes.
