# PRD 0001: Re-cut the roadmap and record the integration decisions

## Context

Read `docs/prds-composition-integration/README.md` first; its constraints
bind. This PRD is documentation-only. `docs/ROADMAP.md` is dated 2026-07-15
(amended 2026-07-18) and is now materially wrong about composition: it says
v0.3 ships "flat n-box first" and "nesting follows demand", but the
composition track (`docs/prds-composition/`, commits `26aac6c`…`470ccf6`,
DECISIONS.md §J) has since implemented the full Option D pipeline — reusable
components, product, wires, nesting/exposure, stable content-addressed
identity, the canonical Lean linker, plan envelopes, bundles, and surface
syntax. A stale roadmap that contradicts DECISIONS.md is exactly the
authority drift the project's own PFCLBS comparison warns against.

## Goal

`docs/ROADMAP.md` accurately reflects the post-composition state and
sequences the integration work; DECISIONS.md gains §J14 recording this
folder's frozen integration decisions; no other file changes.

## Specification

### 1. Re-cut `docs/ROADMAP.md`

Preserve the document's structure and voice (status header, "Where we are",
at-a-glance table, per-milestone sections, dependency summary). Required
changes — keep each edit surgical, do not rewrite prose that is still true:

1. **Status header.** Add an amendment line dated 2026-07-22: composition
   implemented far beyond the v0.3 cut via the Option D track
   (`docs/prds-composition/`, DECISIONS.md §J); integration track started
   (`docs/prds-composition-integration/`).
2. **"Where we are" section.** After the v0.1 summary, add a short
   "Composition (Option D) is complete" subsection listing what now exists,
   with pointers rather than re-explanation: serialized
   `CompositionSourceV1`, canonical Lean linker (`sembla-link`), versioned
   `ExecutablePlanV1` with content-addressed rule words, `sembla_component`/
   `sembla_composition` surface syntax, bundles + `bundle-verify`, the
   byte-equality composition laws, and the static-preservation spec modules.
   Note the known integration gaps this folder closes: plans run CPU-only;
   `sweep`/`compare`/`diff-backends` are legacy-only; no composition widget.
3. **v0.3 section.** Mark the "flat n-box" and "wiring language depth"
   items **superseded** (with a dated note pointing at §J) rather than
   deleting them — the roadmap's convention is to record resolutions, not
   erase history. Views/summaries remain accurately described as done.
   Birth/death and ODE/Kurtz remain deferred-to-demand, unchanged.
4. **v0.4 section.** Add one sentence: NPE sweeps must accept plan
   envelopes so composed models can be calibrated (this folder, PRD 0003);
   the `(θ, x)` export contract itself is unchanged.
5. **v0.5 section.** Add the dependency that was implicit: the
   courts/queueing hybrid requires heterogeneous scheduler domains —
   Option D Phase 8, explicitly deferred by DECISIONS §J12 — and that
   opening v0.5 therefore starts with a Phase-0-style decision PRD for the
   outer-boundary protocol, per-domain identity coordinates, and the
   co-simulation coupling-error caveat (a monolithic exact run validates a
   wired hybrid only up to coupling error, so that check is a convergence
   check, not a bitwise one).
6. **Dependency summary diagram.** Update the v0.3 node to reflect
   composition-done + integration-in-progress. Keep the diagram style.
7. **Proof track.** Update entry 2 (refactoring invariance): the byte-level
   law tests (alpha-rename, permutation invariance) and the executable
   static-preservation checks now exist; the tractable next proof is the
   universal `staticPreservationStatement`, and lumping 1b remains open.

### 2. Add DECISIONS.md §J14

Append `### J14. Composition integration (2026-07-22)` under section J,
recording the seven frozen decisions from the folder README ("Decisions
frozen for this folder"), in the existing DECISIONS style — decision,
alternatives, reason — one short paragraph each. Copy the load-bearing
phrasing exactly: rule-word-for-Philox/dense-ordinal-for-indexing on CUDA;
local-versus-hardware acceptance split; mixed-arm `compare` rejection; plan
tuples instead of `ir_hash` in plan sweeps; no `--dt` for plans anywhere;
`--all-plan-fixtures` as a sibling flag; structure-widget-only scope.

### 3. What not to do

Do not renumber or rewrite existing DECISIONS sections. Do not touch
DESIGN.md, code, fixtures, or scripts. Do not delete the roadmap's resolved
decision records — supersede with dated notes.

## Allowed files

- `docs/ROADMAP.md`
- `DECISIONS.md` (append §J14 only)
- implementation notes/artifacts created by the managed run

## Non-goals

- Any code, fixture, or script change.
- Re-planning v0.4/v0.5 scope beyond the two dependency sentences above.
- Editing `docs/design/**` (already stamped by the composition track).

## Acceptance criteria

1. `grep -n '2026-07-22' docs/ROADMAP.md` matches the new amendment line;
   the "Where we are" section names the Option D track and the three
   integration gaps.
2. The v0.3 "flat n-box"/"wiring depth" items carry dated superseded notes
   pointing at DECISIONS §J; no resolved-decision text was deleted.
3. The v0.5 section names heterogeneous schedulers (Option D Phase 8) as
   the gating dependency with the coupling-error caveat.
4. `grep -n '^### J14\.' DECISIONS.md` matches; §J14 records all seven
   decisions with their frozen phrasing (spot-check `--all-plan-fixtures`
   and `rule_word` appear).
5. `git diff --stat` shows exactly the two allowed files;
   `./scripts/check.sh` and `git diff --check` pass.
