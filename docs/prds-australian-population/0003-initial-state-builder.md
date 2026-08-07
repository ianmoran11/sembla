# PRD 0003: The 2010 initial state at three scales

## Context

Read `docs/prds-australian-population/README.md` first. `DECISIONS.md` §N8
(scales), §N10 (Python writes state bytes, Rust validates them) and §K3
(the frozen `sembla.state/v1` format) bind. `docs/guides/state-format.md` is the
normative format spec and is **not** modified by this PRD.

DESIGN §10.5 puts population generation outside the runtime boundary, and
`sembla synth-state` is explicitly labelled non-scientific benchmark tooling. So
the real starting population is built in Python from PRD 0002's extracts and
handed to Sembla as a `sembla.state/v1` artifact. PRD 0004A's standalone
schema export precedes this builder; PRD 0004B's invariants follow the committed
`hundredth` fixture. The runtime already enforces declared `rows :=` counts
exactly for artifact-loaded runs, which makes a mis-sized pool a loud failure
rather than a silent one.

Two things make this harder than "write one row per person". The slot pool must
be sized for fifteen years of entries decided now, and the composition of
*vacant* slots determines the composition of everyone who ever enters — because
activation hazards apply per eligible vacant slot (§K10).

## Goal

`data/abs/build_state.py` emits a deterministic, byte-reproducible
`sembla.state/v1` artifact for 30 June 2010 at `full`, `tenth` and `hundredth`
scale, whose present slots reproduce published ERP by state × age × sex exactly
(up to documented integer rounding at reduced scale), and whose vacant slots are
pre-classified to match ABS entrant composition.

## Specification

### 1. Scaling that sums exactly — `data/abs/scaling.py`

Reducing a cell count by 1:100 must not lose or invent people. A single
largest-remainder (Hare–Niemeyer) pass preserves only the national total and is
therefore insufficient. First apportion the state and age margins to the scaled
national total, then choose floor-to-ceiling cell increments with a deterministic
minimum-cost bipartite flow that satisfies both sets of margins and favours the
largest feasible cell remainders. Ties follow sorted cell-key order.

Record in the module docstring that this constrained apportionment preserves the
national, state and age targets but perturbs individual small cells, and that NT
and ACT single-year cells at `hundredth` scale are frequently 0 or 1 — the known
limitation from §N8, to be surfaced in the build report rather than smoothed.

### 2. Pool sizing — deterministic and recorded

The pool must cover the peak present population plus every entry over
the fifteen run years 2010 to 2024. Compute from PRD 0002's extracts, not
from a guess:

```text
slots = present(2010) + Σ births(2010..2024) + Σ nom_arrivals(2010..2024)
        + headroom
```

Internal moves consume no slots, which is what makes this fit. `headroom` is
10% independently for the birth and overseas streams, rounded up before scale
reduction. Emit the required entries, headroom and total slots per stream as
well as the whole pool in the build report. PRD 0008's saturation diagnostic is
the empirical gate: any zero or near-zero vacancy margin fails calibration and
requires a documented rebuild rather than silently suppressing entries.

### 3. Present slots from ERP

One present slot per scaled person in each `(state, sex, age)` cell of
`erp_state_age_sex.csv` for 2010, with:

- `occupancy := present`, `event := none_`, `prev_area := none_`,
  `generation := 0`;
- `age_months` spread deterministically within the single-year age cell rather
  than all set to `age × 12`, so cohorts do not advance in lockstep across a
  band boundary — use an even spread by within-cell index and record the rule;
- `entry_stream := retired_slot` and `entry_age_months := 0`; retired rows are
  never activation candidates, so an initially present person cannot later
  contaminate the pre-classified birth or overseas entrant mix;
- `area` from the cell's state, `slot_resource` the row's own ordinal.

Row order is canonical: sorted by `(state, sex, age, within-cell index)`. Row
ordinal is the Philox entity coordinate, so this ordering is part of the
reproducibility contract and must be identical across scales' construction
logic.

### 4. Vacant slot pre-classification

Vacant slots are the pool of everyone who will ever be born or arrive, so their
composition is the composition of future entrants:

- `birth_slot`: `entry_age_months := 0`, `sex` assigned to match the ABS sex
  ratio at birth, `area` allocated across states in proportion to each state's
  share of 2010–2024 births.
- `overseas_slot`: counts follow the joint NOM state × sex × published age-band
  arrival distribution. `entry_age_months` is the published band's lower bound
  times 12 (including 65+ → 780); no unsupported within-band ages are
  manufactured.

All allocation is deterministic largest-remainder, never sampled. Birth and
NOM slots are single-use: death and emigration write `retired_slot` before
vacating them. State the consequence in the build report: entrant composition
is fixed at build time, so the calibration in PRD 0008 fits entrant *rates*, not
entrant *mix*.

### 5. Writing `sembla.state/v1` from Python

`data/abs/state_artifact.py` implements the frozen format: `SEMBLA_STATE` magic,
canonical-JSON header, raw little-endian column blobs matching runtime
`ColumnData`. Column order and types are read from the **exported model JSON**,
never hard-coded, so the writer contains no model-specific names beyond the
table names it is asked for. Mirror `synth-state`'s convention of emitting the
paired model JSON alongside the artifact. The canonical feature-bearing export
comes from PRD 0004A. A state companion changes the two table `size_hint` values
and omits only `grouped_views`, because public `sembla validate` has no feature
enable flag; canonical runs use the adjacent executable plan. Tests must prove
that schema, parameters, transitions and scalar views are otherwise identical.

### 6. Cross-language conformance — the acceptance core

Python-written bytes must be exactly what Rust expects. Prove it with existing
tooling rather than a new command:

- `sembla validate` accepts the paired model;
- loading the artifact and immediately exporting it reproduces the input bytes
  exactly — read the CLI first for whether a zero-tick run is supported, and if
  it is not, use the smallest supported run over a model whose transitions
  cannot fire, or compare `sembla state-hash` against a Rust-side reference;
  pick the honest option and record it in the implementation notes;
- `sembla state-hash` of the Python artifact equals the value recorded in the
  build report;
- declared `rows :=` enforcement passes at every scale.

### 7. Build report and fixtures

`data/abs/extracts/initial-state-2010.md`: pool sizing arithmetic and headroom,
per-stream slot counts, exact-match confirmation against published 2010 ERP
cells at `full` scale, rounding error at `tenth` and `hundredth`, the artifact
SHA-256 at each scale, and the small-cell limitation.

Commit the `hundredth` artifact under `fixtures/state/` for downstream tests.
`full` and `tenth` are generated on demand and gitignored — record their hashes
in the report so a rebuild is verifiable.

## Allowed files

- `data/abs/build_state.py`, `data/abs/scaling.py`,
  `data/abs/state_artifact.py`, `data/abs/tests/**` (new)
- `data/abs/extracts/initial-state-2010.md` (new)
- `fixtures/state/**` (new entries only — no existing fixture changes)
- `crates/sembla-cli/tests/**` (conformance test only)
- `scripts/check-abs-data.sh` (extend), `docs/guides/abs-data.md` (extend)
- `.gitignore` (generated-artifact entries only)
- implementation notes/artifacts created by the managed run

## Non-goals

- No change to `sembla.state/v1`, its hash domain, or the Rust loader.
- No new CLI command or flag; `synth-state` is untouched.
- No model, transition or Lean change — PRD 0004 owns the model.
- No sampling: every allocation is deterministic largest-remainder.
- No sub-state geography in the artifact (§N1).

## Acceptance criteria

1. Full check battery plus `scripts/check-abs-data.sh` passes;
   `git diff --check` passes.
2. At `full` scale, present-slot counts match published 30 June 2010 ERP for
   every `(state, sex, age)` cell exactly.
3. At `tenth` and `hundredth`, scaled counts sum exactly to the scaled national
   total and to every state and age margin; residual per-cell rounding error is
   reported.
4. Rebuilding any scale from the committed extracts reproduces the artifact
   byte for byte, and the recorded SHA-256 matches.
5. The conformance check in §6 passes, and the chosen round-trip method is
   recorded with its rationale.
6. Pool sizing is derived from the extracts with the headroom fraction and
   per-stream counts recorded; no magic constant appears in the code without
   the report explaining it.
7. The build report states the fixed-entrant-composition consequence and the
   small-cell limitation at `hundredth` scale.

## Implementation evidence

- `scripts/check-abs-data.sh` rebuilds the committed 1:100 artifact and compares
  its bytes and both hashes on every ABS check.
- Two independent full and 1:10 materialisations reproduced the frozen ordinary
  and domain-separated hashes. Commands, timings, byte sizes and hashes are in
  `docs/evidence/australian-population/initial-state-2010/README.md`.
- The explicit ignored Rust gate decodes both generated artifacts, checks both
  table row counts at each scale, independently computes the Rust state hash,
  and proves that incrementing either paired declaration makes loading fail.
- The existing public `sembla validate` command accepts the actual generated
  validation-safe companion at all three scales. A routine structural test
  proves that restoring grouped views and canonical row counts yields the
  canonical execution model exactly.
- A zero-tick run is unsupported because `final_population` cannot reduce an
  empty run. Conformance therefore uses exact Rust loader values and the frozen
  domain-separated hash rather than mislabelling a changed one-tick state as a
  round trip.
