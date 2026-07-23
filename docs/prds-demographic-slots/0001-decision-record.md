# PRD 0001: Record the demographic-track decisions

## Context

Read `docs/prds-demographic-slots/README.md` first; its constraints bind.
This PRD is documentation-only. The Australian demographic use case
(external note, read-only) identified a set of candidate framework
extensions; the folder README records which are accepted, which are
reframed, and which are deferred with triggers. Per project rule, those
choices become normative in `DECISIONS.md` before implementation, and the
roadmap must reflect them — several deliberately deviate from the use-case
note, and deviations must be decisions, not drift.

## Goal

`DECISIONS.md` gains section K covering every decision this folder relies
on; `docs/ROADMAP.md` gains the two amendments the track requires; nothing
else changes.

## Specification

### 1. Add section K to `DECISIONS.md`

Append after §J:

```markdown
## K. Demographic slot modeling (accepted 2026-07-23)
```

with subsections `### K1.`–`### K10.`, each in the house style (decision,
alternatives rejected, reason), sourced from the folder README:

- **K1. Fixed-pool slot architecture.** Demographic turnover (births,
  deaths, migration) is modeled inside a fixed table of reusable person
  slots: a row is vacant or present; entries activate vacant rows; exits
  vacate them. Person identity is `(slot ordinal, generation)` — the row
  ordinal is the permanent slot ID and Philox entity coordinate and is
  never a person ID; `generation` increments on activation. Dynamic row
  allocation and the roadmap's stream-compaction birth/death design are
  *not* selected; the latter stays deferred with its original trigger.
  Capacity exhaustion is an explicit, observed failure (saturation
  diagnostics), never silent.
- **K2. Age representation.** `age_months : Int`, advanced by a
  deterministic monthly transition, is the accepted representation now.
  The derived-age design (`Expr::Tick` + `birth_month_index`) is deferred
  behind a measurement trigger: PRD 0009's benchmark must show the
  ageing-write cost is material before `Expr::Tick` (a flagged semantic
  construct touching validation, CPU, CUDA, and reproducibility) is
  designed. Mutable age and an authoritative birth date are never stored
  together unchecked.
- **K3. State artifact format.** `sembla.state/v1` as frozen in the folder
  README: `SEMBLA_STATE` magic, canonical-JSON header, raw little-endian
  column blobs matching runtime `ColumnData`, hash domain
  `sembla.state-artifact/v1` over exact file bytes, no execution metadata
  inside the artifact. For artifact-loaded runs, declared `rows :=` counts
  are enforced exactly, ending their size-hint-only status.
- **K4. Chained annual runs, not checkpoints.** Standing-no #6 (no run
  management/replay subsystem) stands. Annual calibration windows are
  separate runs chained by hashed state artifacts, with
  `initial_state`/`exported_state` manifest tuples. A chained 12+12 run
  pair is explicitly not bitwise-equal to a continuous 24-tick run (tick
  coordinates restart); this is recorded and test-asserted.
- **K5. Rate heterogeneity via loaded columns and per-run θ.** Per-slot
  age/sex/area rate multipliers are `Real` attribute columns supplied in
  the initial state and referenced in hazards; annual rate variation is
  per-run θ across chained runs. A time-indexed rate-table construct is
  rejected for now (no customer under the monthly/annual design).
- **K6. `grouped-observations` is the first §5.5 feature flag.** Runtime
  option, threaded through validation and execution, recorded sorted in
  manifests; models using grouped views without the flag are rejected
  naming it. The plan validator's known-feature set grows from empty to
  `{"grouped-observations"}` — the one sanctioned revision to §J's
  "exactly []" rule; unknown features still reject. Composition sources
  may not carry grouped views yet. Grouped observation is a sink: the §4.6
  invariant extends to it mechanically. V1 is CPU-only, with deterministic
  rejection on CUDA.
- **K7. Contest surface syntax, race_time only.** `contest <ref-attr> by
  race_time` in transition bodies lowers to the existing
  `ResourceClaim`/`ClaimOrdering::RaceTime` IR. This resolves DESIGN.md
  open question §10.1 for the race-time case; keyed orderings (queue
  disciplines) remain v0.5 scope and are not exposed.
- **K8. Surface gaps closed without flags.** Arithmetic `set` effects and
  `Int` parameter declarations expose IR/runtime capability that has
  existed since v0.1 (`Effect::SetAttr` takes a full `Expr`;
  `ParamType.int` exists); per §5.5's rationale, constructs whose meaning
  already exists need no provisional-meaning flag.
- **K9. Deferred constructs and their triggers.** Copy the folder README's
  deferred list verbatim: Expr::Tick (benchmark trigger); categorical
  draws, cross-row writes, mother-linked births, vacant-slot claiming,
  non-exclusive Ref reassignment, household refs (trigger: aggregate model
  demonstrates identity linkage is scientifically required; first artifact
  is a design-options note, not PRDs); paired migration/quotas (trigger:
  reported balance residual unacceptable → synchronized families, Option D
  Phase 6); event-stream sinks; sub-annual rate tables; CUDA grouped
  observations.
- **K10. Aggregate-model interpretation caveats.** The birth-activation
  hazard is a rate per eligible vacant slot, not a fertility hazard, and
  must not be interpreted as one without an explicit scaling derivation.
  The one-tick event-marker lockout (new entrants ineligible for events
  while their marker persists) is a documented, measured model trade-off
  (PRD 0007 counts it), not a framework bug. National internal-migration
  balance holds only in expectation; the residual is reported, never
  silently reconciled.

### 2. Amend `docs/ROADMAP.md`

Two surgical amendments, dated 2026-07-23, in the existing amendment style:

1. **v0.3 deferred-to-demand list:** note that demand arrived in the form
   of the demographic slot track, and that it chose the fixed-slot
   architecture (§K1) *instead of* triggering the flagged stream-compaction
   birth/death design — which therefore remains deferred, trigger
   unchanged. Also note the flag-policy expectation is updated: the first
   landed flag is `grouped-observations` (§K6), not birth/death.
2. **Cross-cutting decision points:** mark DESIGN.md §10.1
   (conflict-scope declaration syntax) resolved for the race-time case by
   §K7, remainder (keyed orderings) still owned by v0.5.

### 3. What not to do

Do not edit DESIGN.md, the use-case note, code, fixtures, or scripts. Do
not renumber existing DECISIONS sections. Copy frozen strings exactly
(`sembla.state/v1`, `SEMBLA_STATE`, `grouped-observations`,
`sembla.state-artifact/v1`).

## Allowed files

- `DECISIONS.md` (append §K only)
- `docs/ROADMAP.md` (the two amendments only)
- implementation notes/artifacts created by the managed run

## Non-goals

- Any code, schema, fixture, or test change.
- Restating the use-case note's science; §K records framework decisions
  only.

## Acceptance criteria

1. `grep -n '^## K\.' DECISIONS.md` and `grep -n '^### K10\.' DECISIONS.md`
   match; K1–K10 exist with the subjects above.
2. Frozen strings appear verbatim in §K: `sembla.state/v1`,
   `SEMBLA_STATE`, `sembla.state-artifact/v1`, `grouped-observations`,
   `race_time`.
3. ROADMAP carries both dated amendments; no resolved-decision text was
   deleted anywhere.
4. `git diff --stat` shows exactly the two allowed files;
   `./scripts/check.sh` and `git diff --check` pass.
