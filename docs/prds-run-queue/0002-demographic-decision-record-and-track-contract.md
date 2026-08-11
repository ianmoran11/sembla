# PRD 0001: Record the aggregate-first and demographic-spine decisions

## Context

Read `docs/prds-demographic-spine/README.md` first; it is binding. The current
proposal is intentionally non-normative. This PRD makes the foundation track
executable by appending a new decision section before any implementation.

`DECISIONS.md` §N remains the historical authority for the existing Australian
population model and the retained 2026-08-07 calibration run. In particular,
§§N20–N21 explain why raw-coordinate NPE posteriors were rejected and how that
run's point estimates were gated. Those facts are not rewritten. The new
experiment is a subsequent decision based on the measured result.

`DECISIONS.md` §§J9 and J12 also remain binding: one global scheduler domain,
no V1 `Share`/`Identify`, no heterogeneous schedulers, and no silently accepted
deferred constructs. Static person-facet fusion is permitted only because it
constructs one ordinary primitive box before executable IR exists.

## Goal

Append `DECISIONS.md` §O recording the aggregate-first calibration replacement,
the Lean-only person-facet foundation and every retained foreclosure; mark the
design proposal accepted for this foundation scope; and register the track in
maintained planning indexes.

## Specification

### 1. Append `DECISIONS.md` §O

Append after §N:

```markdown
## O. Aggregate-first calibration and demographic spine foundation (accepted 2026-08-11)
```

with subsections O1–O9 in the established decision/alternatives/reason house
style. Preserve these decisions exactly in substance:

1. **O1. Estimate parameters at the cheapest identified level.** Every domain
   partitions parameters into directly estimable, aggregate-dynamical,
   micro-only and coupling/scenario blocks. Published occurrence/exposure or
   stock-flow evidence owns the first two; stochastic simulation is not used to
   relearn them. Rejected: one monolithic NPE over every parameter and target.
2. **O2. The retained NPE run remains immutable evidence.** The 2026-08-07
   artifacts and §N20–N21 remain truthful records of the old declared
   experiment. The aggregate-first path is a new adjacent experiment; no old
   report, parameter or diagnostic is regenerated or relabelled.
3. **O3. Migration age is fitted by pooled profile likelihood.** For every
   positive `peak_months, k` candidate, re-profile all fifteen annual spatial
   coefficients against the 56 O-D cells, then fit one pooled age pair over
   2010–2024 from conditional state/direction/sex/age compositions. Preserve the
   2020 source-total conflict by conditioning within each source group. Report
   clustered/overdispersed uncertainty. Rejected: updating the spatial block
   with weak micro summaries, fitting thirty annual age parameters first, or
   pretending the Poisson inverse Hessian is complete uncertainty.
4. **O4. A deterministic aggregate companion precedes residual simulation.**
   Reproduce the actual monthly race, contest, lifecycle and ageing semantics
   over sufficient aggregate cells. Call a result exact only where closure is
   established; otherwise name and measure the approximation against replicated
   micro-runs. The companion is a quarantined standard-library Python workflow,
   not a new Rust backend or runtime mode.
5. **O5. Calibration selection is blockwise and support preserving.** The
   spatial estimate remains the direct gravity/profile optimum. An identified
   pooled age estimate may replace the age prior centre; otherwise retain that
   centre and report non-identification. Any later residual inference operates
   only on the responsible low-dimensional block in log/logit coordinates. A
   failed residual stage cannot invalidate the spatial fit. Rejected: clipping,
   wholesale raw-coordinate posterior replacement, or tuning a gate after the
   result.
6. **O6. Person facets are statically fused, not shared at runtime.** Lean
   authoring may combine separately declared facets into one table and one
   primitive box, emitting unchanged V1 IR. Generated names are namespaced,
   each mutable attribute has one owner, and the final current `checkModel`
   boundary certifies the result. Facets are absent from serialized plans.
   Rejected: calling the feature `Share`/`Identify`, creating a second model
   checker, or changing CompositionSource/plan/Rust contracts in this track.
7. **O7. Entry reset is atomic and generation-safe; vacant state is inactive.**
   The base demographic model owns occupancy, generation and event choice.
   Every facet field has one literal-or-facet-parameter initializer appended to
   each named base entry transition, and all generated facet transitions/
   observations require `occupancy = present`. Initializers cannot read values
   written by that same entry because effects read tick-start state. Exit leaves
   domain bytes inactive; the next entry overwrites them before
   incremented-generation state becomes visible. A domain event and exit may
   still both be counted in one tau-leap tick; real domain models must report
   timestep sensitivity. Rejected: exit cleanup that can conflict with a
   same-tick domain write, making domain events contest and defer demographic
   exits, independently racing initialization rules, or identity based on row
   ordinal alone.
8. **O8. Fixed geography is unrolled; general semantics remain deferred.** The
   eight-state case may compile to guarded transition and scalar-input families.
   This does not introduce a generic enum-keyed join. Dynamic sparse rows,
   person-granular output projection, cross-row writes, scheduled clocks and
   heterogeneous schedulers retain their §J12/deferred status until a retained
   driver-model failure justifies a separate decision.
9. **O9. The first labour and justice models are synthetic proof models.** They
   test facet ownership, lifecycle, regional feedback and explicit coupling.
   They acquire no real domain data and support no empirical or causal claim.
   Real labour/justice calibration, joint population synthesis and full event
   ontologies require later dedicated decision and PRD tracks.

Each subsection must name rejected alternatives and the reason. Do not edit the
wording or status of §§A–N except for a necessary table-of-contents link if one
exists.

### 2. Promote the design document

Update `docs/design/demographic-spine-and-modular-calibration.md`:

- status becomes `accepted foundation design; implemented by
  docs/prds-demographic-spine/`;
- authority names `DECISIONS.md` §O while retaining §§J/N as predecessor
  authorities; and
- the foundation-only scope explicitly excludes real labour/justice
  calibration.

Do not rewrite the argument, implementation phases or non-claims.

### 3. Register the track

- Add `prds-demographic-spine/` to `docs/prds/TRACKS.md` with a concise purpose.
- Add one link under the roadmap's driver-closure direction. Do not mark any
  implementation capability complete in the current baseline.
- Verify `docs/design/README.md` already links the design document; change it
  only if its status wording must be corrected.

## Allowed files

- `DECISIONS.md`
- `docs/design/demographic-spine-and-modular-calibration.md` (status, authority
  and foundation-scope lines only)
- `docs/design/README.md` (status wording only if needed)
- `docs/prds/TRACKS.md`
- `docs/ROADMAP.md` (one planning link only)
- implementation notes/artifacts created by the managed run

## Non-goals

- No Python, Lean, Rust, model, fixture, parameter or evidence change.
- No editing §§A–N except a generated contents link if present.
- No new serialized schema, version, feature flag or dependency.
- No claim that aggregate parity, age identification or facet fusion already
  passes; later PRDs measure those questions.

## Acceptance criteria

1. Full repository checks, Markdown-link checks, the movable-path allowlist
   review and `git diff --check` pass.
2. `DECISIONS.md` §O contains O1–O9, each with a decision, rejected alternatives
   and reason, and all substance listed in this PRD.
3. §O states that retained NPE evidence is immutable and that the new method is
   a subsequent experiment rather than a rewrite of §N.
4. §O distinguishes static primitive-box fusion from V1 `Share`/`Identify` and
   leaves every named runtime semantic extension deferred.
5. The design status names this track and §O, while its argument and non-claims
   are otherwise unchanged.
6. The track index and roadmap link resolve; the roadmap does not claim the
   foundation is implemented before later PRDs land.
7. No code, artifact, target, parameter or retained evidence byte changes.
