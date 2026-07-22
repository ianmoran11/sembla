# PRD 0004: `compare` over plan envelopes — CRN policy contrasts for composed models

## Context

Read `docs/prds-composition-integration/README.md` first; its constraints
bind — especially decision 3 (mixed identity schemes never compare).

State of the code this PRD changes:

- `compare` (`crates/sembla-cli/src/main.rs:302,2459,2562`) supports two
  forms — `compare <modelA> <modelB> …` and
  `compare <model> --params-a a.json --params-b b.json …` — both
  legacy-only today. Both arms run at one shared seed, which is what makes
  the common-random-numbers contrast (DESIGN.md §5.3, DECISIONS §E5) work.

Why this is the headline PRD of the folder: under legacy positional
identity, CRN pairing across two *different* models was fragile — any
declaration-order difference silently de-paired the draws. Content-addressed
identity (DECISIONS §J4) makes cross-model CRN principled for the first
time: two plans that share a component share `occ:…#…` identities, hence
words, hence draws, at the same seed. Two plans that differ only in policy
parameters give the same simulated person the same shocks under both
policies — perfectly paired counterfactuals. This PRD wires that in and
proves it mechanically.

## Goal

Both `compare` forms accept plan envelopes; mixed legacy/plan arms reject
deterministically; a CRN noninterference test proves shared components
receive identical draws across different composed models; the counterfactual
demo is exercised end-to-end.

## Specification

### 1. Accept plans in `compare`

Route both arms through `parse_input`:

- **Two-file form.** Each arm independently accepts a legacy model or a
  plan envelope (validation + canonicality for plans, as `run` does). If
  the two arms' input kinds differ — one legacy, one plan — reject before
  executing anything with the deterministic error:
  `compare requires both inputs to use the same identity scheme; got legacy
  model '<a>' and plan envelope '<b>'` (README decision 3; wording may be
  adjusted, the *shape* — both paths named, no partial execution — may
  not). Two plans with different `identity_scheme` strings would be caught
  by validation already (only one scheme exists); do not special-case.
- **Params form.** `compare <plan> --params-a … --params-b …` runs one plan
  under two θ vectors at the shared seed — the pure-parameter
  counterfactual. Reuse the existing θ resolution against the derived
  model's params.
- Backend flag: `--backend cuda` follows PRD 0002's path; default CPU.
  No `--dt` flag exists on compare — do not add one.
- The comparison output (`compare.csv` columns, field diffing via
  `compare_field`) is **unchanged**. Compare output for plans keys rows
  exactly as it does for legacy models (box-qualified view/fired names);
  leaf names like `north/population` flow through as ordinary box names.

### 2. The CRN noninterference contrast test

This is the load-bearing test; make it exact, not statistical:

1. **Shared-component pairing across different models.** Arms:
   `fixtures/plans/linked/solo_population.plan.json` versus
   `fixtures/plans/linked/independent_epidemic_policy.plan.json` (the
   unwired product). Both contain identity `occ:population#…` transitions
   with equal words. At a shared seed, the population leaf's per-tick view
   and fired trajectories must be **bitwise identical** across the two
   arms, so the compare CSV must show zero difference on every
   population-derived column for every tick. This is CRN across *different
   composed models* — the property legacy identity could not offer.
2. **Wired divergence is delayed, then real.** Arms: linked
   `epidemic_policy` under `--params-a`/`--params-b` differing only in a
   policy-side parameter (e.g. the restriction threshold). Assert: tick 0
   population columns identical (policy influence needs one wire tick);
   divergence appears only at/after the first tick the changed parameter
   can causally reach population (two wire hops — assert the exact first
   differing tick at the chosen seed and pin it as a golden). This
   demonstrates paired counterfactuals with the delay discipline visible.
3. **Mixed-arm rejection.** `compare examples/two_box.json
   fixtures/plans/two_box.plan.json …` fails with the §1 error and creates
   no output file.

### 3. Counterfactual demo wiring

`fixtures/demos/composition/demo_counterfactual_outbreak/` (committed demo
work) is the narrative version of §2.2. Add one CLI test (or extend the
demo's existing golden test — read `frontend/Sembla/Demos/Composition*.lean`
and the demo goldens first to follow their conventions) running its plan
under two θ vectors via `compare` and checking the output against a
checked-in golden. Update `docs/composition.md`'s CRN section with the
exact commands. If the demo's fixtures make this awkward (e.g. no exported
plan file), fall back to documenting §2.2's fixture-based contrast and note
why in the implementation notes — do not restructure the demos to fit.

### 4. Tests and docs

- Extend `crates/sembla-cli/tests/` (a `compare.rs` exists? read the test
  tree first — `run.rs`/`sweep.rs` conventions apply; create `compare.rs`
  if none exists) with: §2's three cases, a plan-vs-plan two-file golden,
  a params-form golden, and determinism (same compare twice → identical
  bytes).
- `USAGE` mentions `<model-or-plan.json>` for both compare forms.
- `docs/composition.md` gains the CRN contrast walkthrough (§2.1 and §2.2
  narrated, with the "why content-addressing makes this principled"
  sentence pointing at DECISIONS §J4/§J14).

## Allowed files

- `crates/sembla-cli/src/main.rs`, `crates/sembla-cli/tests/**` (+ new
  golden files per existing conventions)
- `docs/composition.md`
- `frontend/Sembla/Demos/**` only if §3's demo test genuinely requires a
  hook (prefer not; never change demo goldens)
- implementation notes/artifacts created by the managed run

## Non-goals

- Statistical variance-reduction analysis, plotting, or new output formats.
- Cross-scheme (legacy↔plan) comparison bridges of any kind.
- Named experiment axes / coordinate seeds (v0.4).
- `verify-run`/`diff-backends` changes (done in 0002/earlier tracks).

## Acceptance criteria

1. `./scripts/check.sh` passes; no frozen artifact changed.
2. §2.1 passes: zero population-column differences across
   `solo_population` vs `independent_epidemic_policy` at a shared seed,
   asserted mechanically over every tick.
3. §2.2 passes with the exact first-divergence tick pinned; tick-0
   equality asserted.
4. Mixed-arm rejection: exact deterministic error, no partial output —
   covered by a test.
5. Both compare forms have plan goldens and a bitwise determinism test.
6. `docs/composition.md` documents the CRN contrast with runnable
   commands; `USAGE` updated.
7. `git diff --check` passes; no new dependencies; demos' goldens
   untouched.
