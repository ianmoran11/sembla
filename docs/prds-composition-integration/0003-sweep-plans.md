# PRD 0003: `sweep` over plan envelopes — NPE calibration for composed models

## Context

Read `docs/prds-composition-integration/README.md` first; its constraints
bind — especially decision 4 (plan tuples instead of `ir_hash`).

State of the code this PRD changes:

- `sweep_file_result` (`crates/sembla-cli/src/main.rs:989`) loads via
  `read_validated`, which errors on plan envelopes (`main.rs:451-452`,
  `PLAN_NOT_RUNNABLE`). Everything downstream — prior draws, theta files,
  CRN/independent noise modes, per-draw execution, `--export-pairs`, sweep
  manifests — operates on a `ValidatedModel` and works unchanged once a
  words-carrying model is supplied.
- Plan runs already write `PlanIdentityTuple`/`LinkedSourceTuple` into run
  manifests via `manifest::plan_identity_tuples`
  (`crates/sembla-cli/src/manifest.rs:241-…,348`).
- The seed machinery is already safe for stable identity: sweep replica
  seeds use reserved word `u32::MAX - 1` and prior draws `u32::MAX`
  (`crates/sembla-runtime/src/rng.rs:27`, `prior.rs`), and plan validation
  rejects any transition word in the reserved namespaces — so **no seed or
  namespace change is needed or permitted**.
- Sweep manifests are `RunManifest` with `ManifestKind::Sweep`; θ resolution
  reads `model.params`, which the linker populated from the composition
  source's model-level parameters (DECISIONS §J8), so priors declared there
  sweep exactly like legacy priors.

Why this matters: this PRD is what lets composed models participate in the
v0.4 NPE workflow — prior-predictive `(θ, x)` pairs generated from a linked
plan, conditioned on its declared summaries.

## Goal

`sembla sweep <plan.json> …` works for `direct_stable` and `linked` plans
with every existing sweep feature (draws, theta files, CRN/independent,
pairs export, per-draw manifests, resumable determinism), records the plan
identity tuple in all manifests it writes, and leaves legacy sweeps
byte-identical.

## Specification

### 1. Accept plans in `sweep`

Route sweep input through `parse_input`:

- **Legacy model** → exactly today's path, byte-identical outputs (the
  determinism CI script re-runs a legacy sweep and compares bytes; it must
  keep passing with zero golden changes).
- **Plan envelope** → validate + canonicality check (same helper `run`
  uses), derive the words-carrying model via
  `ValidatedPlan::to_validated_model()`, then continue through the existing
  sweep body unchanged. `--backend cuda` for plan sweeps: allowed iff
  PRD 0002 landed (it did, by run order); it reuses the same backend
  selection the legacy sweep has.

Rejection sites that must keep working: `--export-pairs` without declared
summaries (the existing message, now also exercised by a plan without
summaries); the CRN-pairs warning (`main.rs:999-1002`) fires identically
for plans.

### 2. Manifest identity (README decision 4)

For plan sweeps, in the sweep manifest and every per-draw manifest the sweep
writes:

- set `plan: Some(PlanIdentityTuple)` and `linked_source` per plan origin,
  via the existing `plan_identity_tuples` helper — never hand-build the
  tuples;
- leave `ir_hash` absent (`None`); `model` and `dt` fields keep their
  current population from the derived model;
- all-present-or-all-absent discipline holds: a reader must reject a
  partial tuple (the reader-side check exists from the composition track's
  PRD 0004; add a sweep-manifest test exercising it).

Legacy sweep manifests: byte-identical to before this PRD (serde
`skip_serializing_if` on the tuple fields already guarantees it; prove with
a test comparing a legacy sweep manifest against a pre-change golden — if
no such golden exists, capture one from `git stash`-clean main behavior
first and check it in as a new fixture, noting the provenance).

### 3. Pairs export for composed models

`--export-pairs` must work for a linked plan whose source declared
summaries (the checked-in `two_regions` fixture has `peak_i`; the committed
demo compositions have richer summaries — prefer a fixture from
`fixtures/plans/linked/` so the test is hermetic). The `(θ, x)` CSV format
and `PairsMetadata` sidecar are **unchanged** — `x` columns come from
declared summaries exactly as for legacy models (DESIGN.md §4.6). Add the
plan-sweep pairs file to the golden set: fixed seed, small draw count,
`--noise independent`, byte-compared in a CLI test.

### 4. Tests

All in `crates/sembla-cli/tests/sweep.rs` (extend the existing file,
following its conventions):

1. **Plan sweep determinism.** Sweep `fixtures/plans/linked/two_regions.plan.json`
   twice (`--draws 3`, fixed seed, small ticks, CPU): every output file and
   manifest byte-identical between runs.
2. **Golden plan sweep.** Check in goldens (CSV/summary/manifest with the
   existing environment-field normalization) for one small plan sweep;
   re-run and compare.
3. **Theta-file path.** A plan sweep with `--theta-file` naming the plan's
   θ parameters succeeds; an unknown θ name fails with the existing
   diagnostic.
4. **Noise modes.** CRN vs independent plan sweeps differ in the expected
   way (reuse whatever assertion style the legacy noise-mode test uses).
5. **Manifest tuples.** Plan sweep manifests carry complete plan tuples
   (+ `linked_source` for linked origin, with the digest equal to the
   checked-in source fixture's hash); `direct_stable` sweeps carry no
   `linked_source`; `ir_hash` absent; partial-tuple manifests rejected.
6. **Legacy freeze.** A legacy sweep's manifest and outputs are
   byte-identical to the pre-change golden.
7. **Pairs export.** §3's golden; plus the no-summaries rejection using a
   summary-free plan fixture (e.g. `linked/epidemic_policy.plan.json` if it
   has no summaries — verify, else construct via the existing regeneration
   helper).

### 5. Documentation

Update `USAGE` (`sweep <model-or-plan.json>`) and the sweep section of
`docs/composition.md` with one worked plan-sweep + pairs-export example.
Mention in `calibration/` docs only if they name input constraints
(read before editing; keep the diff minimal).

## Allowed files

- `crates/sembla-cli/src/main.rs`, `crates/sembla-cli/src/manifest.rs`
  (helper reuse only — no new manifest fields), `crates/sembla-cli/tests/**`
- new golden files under the existing CLI test fixture conventions
- `docs/composition.md`, `calibration/**` (docs only, if needed)
- implementation notes/artifacts created by the managed run

## Non-goals

- Named experiment axes / coordinate-derived seeds (v0.4 scope, DESIGN.md
  §5.3 — the index-keyed sweep remains correct for i.i.d. draws).
- New manifest fields, schema strings, or export formats.
- `compare`/`diff-backends` (PRDs 0002/0004); NPE Python-side changes.
- Any seed, namespace, prior, or Philox change.

## Acceptance criteria

1. `./scripts/check.sh` passes; the determinism CI script and every legacy
   sweep golden are byte-unchanged.
2. All seven test groups in §4 pass; plan-sweep determinism is bitwise.
3. Plan sweep manifests: complete `plan` tuple, correct `linked_source`
   presence/absence, `ir_hash` absent — and the linked-source digest
   matches the checked-in source fixture hash.
4. `--export-pairs` produces the checked-in `(θ, x)` golden for the plan
   sweep; format and sidecar schema unchanged.
5. `USAGE` and `docs/composition.md` document plan sweeps.
6. `git diff --check` passes; no new dependencies; no frozen artifact
   changed.
