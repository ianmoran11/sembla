# PRD 0006: `(θ, x)` training-pairs export

## Context

`DECISIONS.md` §G5: the posterior workflow lives outside the framework, fed by
a thin, versioned `(θ, x)` export beside the run manifest — the **single**
artifact for which standing-no #5 was narrowly re-opened. `x` is the
declared-summaries construct (PRDs 0002–0003), not a parallel format. The
consumer is the PRD-0007 `sbi` pipeline; the producer is the PRD-0005 sweep.

## Goal

`sembla sweep --export-pairs <path>` emits one training-pairs artifact:
per-draw θ and summary vector, with a canonical metadata sidecar binding it to
the run contract.

## Specification

- `--export-pairs <path>` writes `<path>` as CSV: one row per draw, columns
  `k`, then each parameter (sorted by name), then each declared summary
  (model declaration order). Values formatted with the same deterministic
  float formatting the existing CSVs use.
- Sidecar `<path>.meta.json`, canonical JSON (PRD-0001 conventions):
  `schema_versions: {pairs: 1}`; `ir_hash` + algorithm; model name; master
  seed; `noise_mode`; draw count; `ticks`; `dt`; determinism level;
  `theta_source` tuple; the ordered parameter column names; the ordered
  summary column names; `pairs_sha256` + algorithm over the CSV bytes;
  component versions.
- A model with no declared summaries fails the export with a diagnostic
  telling the modeler to declare summaries (`DESIGN.md` §4.6).
- Exporting under `--noise crn` prints a stderr warning that CRN pairs are
  unsuitable for NPE training (`DECISIONS.md` §G5); the artifact still emits
  (legitimate for diagnostics) and `noise_mode` records the truth. The
  PRD-0007 consumer is where refusal lives.
- Works identically with prior-sampled and `--theta-file` sweeps.

## Non-goals

Posterior import. A second export format. Plots. Run management. Embedding-net
inputs (per-tick view series export is a future PRD if ever needed —
summaries only here).

## Acceptance criteria

1. `cargo test --workspace` green; all prior tests still pass.
2. Determinism: the same export invocation twice ⇒ byte-identical CSV and
   sidecar; `pairs_sha256` in the sidecar matches the CSV bytes (asserted in
   a test that recomputes it).
3. Column contract: parameter columns are sorted by name; summary columns
   match model declaration order; both orders asserted against a fixture
   model with ≥2 params and ≥2 summaries.
4. Values test: for a 3-draw sweep on the fixture model, exported θ equals
   `manifest.csv`'s θ and exported summaries equal the per-draw
   `.summaries.csv` values.
5. No-summaries model ⇒ the specified diagnostic; CRN mode ⇒ the specified
   warning on stderr (both asserted).
6. `docs/examples/sir.md` gains an export section: command, column layout,
   the sidecar's role in the PRD-0007 handoff.
