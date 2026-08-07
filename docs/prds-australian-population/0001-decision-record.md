# PRD 0001: Record the Australian-population decisions

## Context

Read `docs/prds-australian-population/README.md` first; its frozen sections are
the normative spec for this PRD. `DECISIONS.md` §§K (demographic slots) and M
(performance methodology) bind, and §K's deferrals all stand unchanged.

The design was scoped in
[`docs/design/australian-population-model.md`](../design/australian-population-model.md)
against a live reading of the codebase. Three findings drove it and none are
obvious from the model source alone:

- `area` is currently a `Ref` that is **never written** — the landed
  `internal_arrive` vacates a slot and activates a different pre-classified one,
  so no agent moves (§K9).
- The expression language has **no row-literal**: the only Ref-valued expression
  is `SelfAttr`, so a Ref destination can never be *chosen*. Adding one would
  not help, because expressions are deterministic and the randomness that picks
  a destination must come from hazard firing times.
- Consequently `area` must become an **enum**, which permanently forecloses
  `freq (…) over area` population-dependent hazards, because aggregates join
  only on declared Ref keys.

Writing these into `DECISIONS.md` before any code exists is the point of this
PRD. A later reader must not have to re-derive them from `validate.rs` and the
`Expr` enum.

## Goal

`DECISIONS.md` gains a §N recording every decision, its rejected alternatives
and its reason, in the established house style, so that PRDs 0002–0010 have a
single binding authority and the two foreclosures are permanently on record.

## Specification

### 1. Add section N to `DECISIONS.md`

Append after §M:

```markdown
## N. Australian population model (accepted 2026-08-05)
```

with subsections `### N1.`–`### N13.`, each in the house style (decision,
alternatives rejected, reason), sourced from the folder README:

- **N1. Geography is the eight states and territories, with genuine
  individual movement.** `area` is an enum
  `{nsw, vic, qld, sa, wa, tas, nt, act}` written by move transitions, so a
  person persists across a move. The landed aggregate design (vacate a slot,
  activate a different pre-classified one) is *not* used for internal
  migration. Finer geography is rejected on cost, not preference: ordered
  origin→destination pairs grow as A², giving 56 at state level, 1,190 at
  GCCSA, 11,342 at SA4 and 6.1M at SA2, and guards are evaluated per
  transition per row. Sub-state geography is not carried as an attribute at
  all, because a static label under state-level movement would silently drift.

- **N2. Movement is one transition per ordered pair, resolved by racing
  clocks.** Each move declares `contest slot_resource by race_time`, as do
  death and emigration. DESIGN §5.1's argmin over sampled firing times with a
  lexicographic tie-break then yields exactly one event per person per tick,
  and selects the destination with probability proportional to its hazard —
  correct competing-risks multinomial choice obtained from existing
  semantics. A categorical-draw expression is *not* introduced; §K's deferral
  stands. Losers defer and are counted per contested resource, keeping
  saturation a visible diagnostic rather than a silent downward bias on flows.

- **N3. Two foreclosures are accepted, not deferred.** (a) No
  population-dependent hazards keyed on area: `freq (pred) over <ref>` and
  `Agg { on: AggJoin }` join only on declared Ref keys, so with an enum `area`
  no hazard can reference the population of its own state. (b) No
  transcendental functions: the expression language is Add/Sub/Mul/Div only.
  These are recorded as permanent consequences of N1 rather than triggers,
  because reversing either means abandoning individual movement. The
  consequence for science is stated plainly: rates are exogenous, there is no
  crowding or agglomeration feedback, and fertility remains an aggregate
  birth-slot activation, not a rate applied to resident women aged 15–49 —
  §K10's caveat carries over verbatim.

- **N4. `prev_area` is the frozen origin marker.** A nine-variant enum
  (`none_` plus the eight) written on every move and cleared with `event`,
  making origin→destination flows observable as
  `count PersonSlot by prev_area, area where event = interstate_move`.
  Encoding the origin into the `event` enum was rejected: it saves one column
  but overloads `event` and makes every flow view harder to read.

- **N5. Migration is parameterised as gravity times a rational age profile.**
  `hazard(o → d) = interstate_base · push_o · pull_d · ageProfile(age_months)`
  with `ageProfile(a) = 1 / (1 + k · (a − peak) · (a − peak))` and
  `push_nsw ≡ 1` and `pull_nsw ≡ 1` for identifiability: 17 parameters for all
  56 cells. A free O–D matrix (56 per year, or 56 × age band) is rejected as
  uncalibratable. A
  Gompertz or exponential age profile is unavailable under N3(b); the rational
  form is the accepted substitute and its shape limitation is documented, not
  hidden.

- **N6. Fertility and mortality come from ABS and are held fixed during
  inference.** Mortality is five-year age bands by state and sex with hazards
  from annual ABS age-specific death rates; three-year life-table `qx`
  snapshots are validation only. Fertility is per-state birth-slot activation
  with ABS-derived defaults. They are declared as model parameters so they
  remain symbolic in the IR, but are not varied by the sweep. Estimating them
  by simulation-based inference is rejected where direct rates are published.

- **N7. Monthly ticks in chained annual runs, calibrating 2010–2025.** At most
  one event per person per tick follows from N2, so annual ticks would forbid
  a person both moving and dying in a year and would bias flows downward;
  monthly ticks make that bias negligible. Each calendar year is one run with
  its own seed, θ, ticks and manifest, chained by a hashed state artifact —
  §K4's chained-runs reframe, reused unchanged. The window is the fifteen run
  years 2010 to 2024, carrying stocks through 30 June 2025 — the last 30 June
  for which state-level ERP by single year of age and sex is published. The
  30 June 2024 sub-state limit does not bind, because N1 puts sub-state
  geography out of scope. Later years remain projection.

- **N8. Calibrate at 1:100, validate at 1:1.** Per-capita hazards are
  scale-invariant, so a 1:100 pool (~400k slots) supports the thousands of
  simulations NPE needs, and a small number of full-scale runs (~35–40M slots,
  ~5–6.5 GB double-buffered) confirm the result. Small-cell noise in NT and
  ACT at 1:100 is a known limitation and must be reported, not smoothed.

- **N9. The person schema is age, sex and state.** Exactly the joint ERP
  publishes, so every calibration target is a published cell. Household,
  income, labour-force, education, visa and country-of-birth attributes are
  rejected for this folder: they require synthesis or reweighting with
  materially greater uncertainty, which the use-case note scopes as its own
  phase.

- **N10. The ABS pipeline lives in `data/abs/`, quarantined and standard
  library only.** It never imports a Sembla crate, library, model parser or
  runtime API (§G5's quarantine rule, applied to data). Unlike
  `calibration/npe` it takes no third-party dependency at all, deliberately
  avoiding a second hash-locked requirements contract; `urllib`, `csv`,
  `json`, `zipfile` and `hashlib` suffice. Raw downloads are cached and
  SHA-256 pinned, normalised extracts are committed, and re-downloading is an
  explicit opt-in flag so no build has a network side effect. Population
  generation stays outside the runtime boundary, as DESIGN §10.5 requires; the
  Python side *writes* `sembla.state/v1` bytes and Rust validates them.

- **N11. Calibration is a per-year forward walk, and stocks alone are
  insufficient.** Given the state at 30 June *t*, θ_t is fitted against year-*t*
  flows and *t+1* stocks, then the state is exported and the walk advances.
  A joint posterior over the whole 2010–2025 path is rejected as intractable
  at this θ dimension and simulation cost. The consequence is recorded
  honestly: this is a filtering-style procedure, errors compound across years,
  and rolling-origin validation is mandatory rather than optional. Separately,
  annual stocks by state × age × sex identify only the *net* of the four
  components, so published births, deaths, overseas and interstate flow series
  are mandatory targets; calibrating to stocks alone is a failed calibration.

- **N12. Calibration targets are a versioned artifact, `sembla.targets/v1`.**
  Canonical JSON, SHA-256 over exact file bytes, naming each target by model
  observation name, period and source series with its ABS vintage. Targets are
  data, not model, and never enter the IR. Fitted targets and held-out targets
  are distinguished inside the artifact so a validation report cannot
  accidentally quote a fitted cell as evidence.

- **N13. Reporting discipline and the non-claims.** Fit to fitted controls is
  reconstruction, not validation, and must be reported separately from
  held-out evidence. Reports carry signed and absolute cell error, MAE/RMSE
  and maximum error, error by population size, residual detail by state, and
  uncertainty across replicates; correlation alone is never sufficient,
  because large states can carry a high correlation while NT and ACT are poor.
  Published ABS components will not reconcile exactly to ERP — the residual is
  reported, never forced away. A model matching state × age × sex controls is
  not thereby realistic on any omitted relationship, and carries no sub-state
  validity whatsoever.
```

Follow the existing §K/§M formatting exactly: `###` subsection headings, bold
lead sentence, prose body, no bullet-within-bullet nesting beyond one level.

### 2. Update the design document status

In `docs/design/australian-population-model.md`, change the status line from
`scoped, pending PRD authoring` to `accepted; implemented by
docs/prds-australian-population/`, and add a line under the decisions table
pointing at `DECISIONS.md` §N as the authority. Do not restructure or re-argue
the document — it becomes a readable companion to §N, not a competing spec.

### 3. Cross-links

- `docs/prds-australian-population/README.md`: no change (it already points at
  the design doc); verify the link resolves under
  `python3 scripts/check-markdown-links.py`.
- `docs/design/README.md`: add the new design document to its index if that
  file maintains one — read it first and match the existing entry format.

## Allowed files

- `DECISIONS.md`
- `docs/design/australian-population-model.md` (status and authority lines only)
- `docs/design/README.md` (index entry only, if it maintains an index)
- implementation notes/artifacts created by the managed run

## Non-goals

- No code, model, fixture, script or data file — this PRD is a decision record.
- No edits to §§A–M, and no reopening of any §K deferral.
- No new PRD files; 0002–0010 already exist and are not rewritten here.
- No restructuring of the design document's argument or evidence.

## Acceptance criteria

1. Full check battery passes, including
   `python3 scripts/check-markdown-links.py`; `git diff --check` passes.
2. `DECISIONS.md` §N exists with subsections N1–N13, each carrying a decision,
   its rejected alternatives and its reason, in §K/§M house style.
3. N3 records both foreclosures as accepted consequences with named scientific
   costs, explicitly *not* as deferrals with triggers.
4. N11 records both the per-year forward walk and the stocks-alone
   identifiability failure, with rolling-origin validation stated as mandatory.
5. The design document's status line points at this folder and names §N as the
   authority; no other section of it is rewritten.
6. §§A–M are byte-unchanged apart from the appended §N.
