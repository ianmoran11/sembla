# PRD 0006: Grouped observations behind the first feature flag

## Context

Read `docs/prds-demographic-slots/README.md` first; DECISIONS §K6 binds.
This is the largest PRD in the folder and it lands two things at once,
deliberately coupled:

1. **The flag machinery itself** — `grouped-observations` is the first
   default-off feature flag ever landed, so the §5.5 policy (runtime
   option, threaded through validation and execution, recorded in the
   manifest, no inert syntax) gets its reference implementation here.
2. **The construct** — grouped per-tick observation tables
   (`area × sex × age-band` cells for calibration), the DESIGN §4.6
   "until a real model needs more" trigger having fired: the demographic
   model needs ~90,000 cells and hand-authored scalar views do not scale.

Governing invariant, restated because everything hangs on it: observation
is a **sink** (DESIGN §4.6). Grouped views must not change state, draws,
draw coordinates, conflict resolution, or scheduling — two models
differing only in grouped views produce bitwise-identical state hashes,
`results.csv`, and fired traces. Group-by with a commutative-monoid
aggregation is already inside the §4.2 kernel fragment, so the construct
is GPU-shaped in principle; V1 still executes it CPU-only (§K6), with
deterministic rejection on CUDA.

## Goal

The `grouped-observations` flag exists end to end (CLI → validation →
execution → manifest); grouped `count` views elaborate, validate, execute,
and emit deterministic long-format CSV artifacts with hash records; the
sink invariant is mechanically pinned; every schema surface (legacy model
JSON, plan envelopes, composition sources) handles the construct exactly
as frozen in the README.

## Specification

### 1. Flag machinery

- CLI: `--enable <feature>` (repeatable) on `run` and `sweep` (`compare`
  and `diff-backends`: rejected with a not-yet-supported error naming this
  PRD's follow-up — keep their scope frozen). Unknown feature names are
  deterministic errors listing known features.
- Threading: the enabled-feature set is a value passed into validation and
  execution (never a global, never a Cargo feature). Rust validation of a
  model containing `grouped_views` fails unless the set contains
  `grouped-observations`, with an error naming the flag and the construct
  location. Same rule in the Lean elaborator? No — the Lean surface has no
  run-time flag context; the surface **always** elaborates the construct
  fully (meaning is not provisional at authoring time), and enforcement is
  at validation/run time. Record this split in a code comment and the
  implementation notes.
- Manifest: `RunManifest.enabled_features : Vec<String>` — sorted,
  deduplicated, serde skip-if-empty (legacy manifests byte-identical).
  Every run/sweep manifest written under the flag records it.

### 2. IR construct

In `crates/sembla-ir/src/model.rs` and `frontend/Sembla/IR.lean`, add to
`Box` an optional field (serde default + `skip_serializing_if` empty; Lean
default `[]`; legacy JSON bytes unchanged both directions):

```rust
pub struct GroupedViewDecl {
    pub name: String,
    pub table: String,
    pub filter: Option<Box<Expr>>,      // row-local predicate, count-view rules
    pub keys: Vec<GroupKey>,            // 1..=4 keys
}
pub struct GroupKey {
    pub attr: String,
    pub band_width: Option<u64>,        // present iff attr is Int-typed
}
```

Validation (extend the existing validators, deterministic errors): table
exists in the box; every key attr exists; key attr types are enum, Ref, or
Int; `band_width` present iff Int, and ≥ 1; filter follows exactly the
row-local rules of `count` view filters; names unique within the box's
combined view + grouped-view namespace; 1–4 keys. The plan validator's
known-feature set becomes `{"grouped-observations"}`: a plan whose
embedded model has any `grouped_views` must list exactly that feature in
`enabled_features`, and a plan listing it must contain at least one
grouped view (no inert flags in artifacts); everything else still rejects
unknown features. This is the §K6-sanctioned revision — update the
affected composition-track tests minimally and cite §K6 in each edit.
Composition **source** parsing (`frontend/Sembla/Composition/Json.lean`)
rejects primitive bodies containing grouped views with a deterministic
not-yet-supported error (linker support is deferred).

### 3. Surface syntax

```lean
grouped view population_cells :=
  count PersonSlot by sex, area, band age_months 60 where occupancy = present
```

- `by` list: 1–4 keys; each an attribute identifier (enum or Ref typed)
  or `band <ident> <positive-int-literal>` (Int typed). `where` optional,
  same predicate fragment as `count` views. Elaborates through the single
  kernel to `GroupedViewDecl`; exact-IR twin test required.
- Positioned negatives: non-existent attr; Real-typed key; `band` on an
  enum attr; missing band on an Int attr; zero band width; > 4 keys;
  duplicate view name; aggregate in the filter.

### 4. Runtime execution and output

- Per tick, after commit, for each grouped view: evaluate the filter over
  committed state, bucket passing rows by the key tuple (enum → variant
  index, Ref → row index, band → `floor(value / width)`, with negative
  Int handling defined as floor division toward −∞ — pin this in a test
  with a negative value), count per bucket. CPU implementation; a run or
  sweep with grouped views on `--backend cuda` rejects deterministically
  (`grouped observations run on the cpu backend only for now`).
- Output: one file per grouped view, `<out-stem>.grouped.<view>.csv`,
  header `tick,<key1-attr>,…,<keyN-attr>,count`; enum keys rendered as
  variant names, Ref keys as row indices, band keys as band indices;
  **non-empty groups only**, rows sorted by `(tick, key tuple)` with the
  rendered-before-sort ordering pitfall avoided: sort on the underlying
  numeric tuple, not the rendered strings. Deterministic bytes, proven by
  a run-twice test.
- Manifest: `grouped_outputs : Vec<GroupedOutputRecord>` (skip-if-empty),
  each `{view, sha256}` following the existing hash-beside-algorithm
  convention used for other outputs (read how `results_sha256` records are
  shaped and match it; all-present-or-absent per entry).

### 5. The sink-invariant test (load-bearing)

Two models identical except one has grouped views (flag enabled for
both runs): assert bitwise-equal final state hash, output hash,
`results.csv`, and fired columns; assert the grouped CSV exists and its
totals cross-check against scalar views (e.g. the sum of
`population_cells` counts at each tick equals the scalar `population`
view — the internal-consistency check that catches bucketing bugs).
Plus: flag-off rejection names the flag; plan round-trip (direct_stable
export of a grouped-view model carries the feature string; `sembla
validate` accepts it with no flag needed for validation? — decide:
validation of an artifact *describing* the feature needs no runtime flag;
*executing* it does; pin the decision in a test and note it).

### 6. Lean-side export

The canonical exporter and plan exporter emit `grouped_views` (canonical
JSON: the new field participates in plan canonical bytes; absent when
empty). Cross-language: a Lean-exported plan with grouped views passes
Rust validation + canonicality byte-checks (extend the existing walking
test with one new fixture under `fixtures/plans/`).

### 7. Documentation

`docs/composition.md` or the observation docs (find where views/summaries
are documented — extend there): the construct, the flag, the CSV format,
the CPU-only status, and the sink invariant. `DESIGN.md` is **not**
edited (its §4.6 exclusion list said "until a real model needs more" —
the decision record §K6 already carries the resolution).

## Allowed files

- `crates/sembla-ir/src/**`, `crates/sembla-runtime/src/**`,
  `crates/sembla-cli/src/**`, all three crates' tests
- `frontend/Sembla/IR.lean`, `Json.lean` (grouped-view emission),
  `DSL.lean`, `PlanJson.lean`/`PlanExport.lean`,
  `frontend/Sembla/Composition/Json.lean` (rejection only), test modules,
  `frontend/Sembla.lean`
- `frontend/Negative/**`, `frontend/scripts/test-negative.sh` (additions),
  `frontend/scripts/check-parity.sh` (append only, if a fixture export is
  added)
- `fixtures/plans/**` (one new grouped fixture), documentation file(s)
- implementation notes/artifacts created by the managed run

## Non-goals

- `groupSum`/other ops, >4 keys, expression keys, banding on Real, event
  streams, windowing, or external sinks.
- CUDA execution of grouped views (deterministic rejection; follow-up).
- Linker/composition-source support (rejection only).
- Grouped summaries; summaries stay scalar.
- A flag registry, config files, or any flag beyond this one.

## Acceptance criteria

1. Full check battery + negative suite + parity pass; legacy model JSON,
   plan fixtures without grouped views, and legacy manifests are all
   byte-identical.
2. The sink-invariant test passes bitwise, including the
   cross-check of grouped totals against scalar views.
3. Flag semantics: run/sweep without `--enable grouped-observations`
   reject a grouped model naming the flag; enabled runs record it sorted
   in the manifest; unknown features reject listing known ones.
4. Grouped CSV output is deterministic (run-twice bitwise), sparse,
   sorted on underlying values, with the negative-Int band pinned.
5. Plan-level feature gating works both directions (grouped views ⇒
   feature listed; feature listed ⇒ grouped views present), with the
   composition-track test edits each citing §K6.
6. CUDA and composition-source rejections are deterministic and tested;
   the exact-IR surface twin holds; `git diff --check` passes; no new
   dependencies.
