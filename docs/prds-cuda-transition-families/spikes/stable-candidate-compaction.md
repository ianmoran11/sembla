# Direct spike protocol: stable candidate and claim compaction

This is a **spike protocol, not a PRD**. Do not pass it to `/piprd run`.
`DECISIONS.md` §M1 requires this direct same-result measurement before any
production compaction PRD is drafted.

Run it only when
`docs/evidence/cuda-transition-families/profile-gate.json` verifies and lists
`stable_candidate_compaction` in `authorized_spikes`. Dense-family dispatch does
not need to pass first; capacity and launch work are independent hypotheses.

## Question

Can a small direct arm retain dense logical candidate/claim identities while
storing only required physical work, materially reducing wall time or admitted
VRAM on more than one shape without hurting dense/few-transition controls?

## Minimal direct arm

Build the smallest committed, runnable arm that answers the question. It is not
a production queue or selector.

1. Preserve the logical candidate exactly as
   `candidate_offsets[rule_id] + row` and keep original `rule_id`, `rule_word`,
   row, claim declaration order, conflict keys/times, diagnostics, and effect
   order.
2. Use separately named physical ordinals. A physical ordinal never enters
   Philox, diagnostics, winner comparison, fired/deferred reports, or
   host-visible output.
3. Preserve every canonical guard/hazard/claim/effect validation and error,
   including failures on rows that do not become enabled physical candidates.
4. Build physical work in deterministic member-emission/row/claim order using a
   count, exclusive scan, and stable scatter or an equivalently proved
   algorithm. Arrival-order atomic append is prohibited.
5. Allocate queue/scan/descriptor capacity from checked bounds and record every
   byte. Do not retain an unreported dense arena that makes the memory comparison
   meaningless.
6. Keep every counter/queue lane-local under the tested stream/fused geometry.
7. Retain one dense reference arm in the same binary and keep the direct arm
   hidden/default-off.

A bound violation, overflow, or allocation failure must occur before mutation or
scientific output. Never truncate, wrap, or replay a partially mutated tick.

## Exactness screen

Before timing, compare compact versus dense reference without normalization on:

- the Australian many-transition shape;
- a second structurally distinct sparse/candidate-heavy shape;
- all-enabled/dense and few-transition controls;
- mixed tables/families and zero-row scalar-validation cases; and
- minimum guard/hazard/claim/effect/double-write failures.

Pin physical-to-logical round trips, Philox tuple, minimum logical diagnostic,
claim/winner/effect order, fired/deferred reports, state/hash, grouped sidecars,
and complete scientific bytes. CPU-versus-CUDA uses the parent README's narrow
normalization contract. A deliberate perturbation must fail.

A semantic mismatch is a valid negative result: record it, disable the arm, and
do not time or productionize it.

## Direct timing and capacity screen

Use one locked release binary, adjacent arms, one warm-up, and at least three
timed repetitions. Record whole wall, simulation window, queue occupancy,
scan/scatter costs, launch/API summaries, generated/PTX/NVRTC, VRAM/RSS, queried
free bytes and sample point, estimator versus observed peak, worker/stream mode,
and raw checksums.

A positive result requires exactness plus either:

- at least 1.20x Australian whole-run improvement over dense reference; or
- at least 25% peak/admitted VRAM reduction at an intended explicit worker count
  with no greater than 5% whole-run regression.

It must also show a positive result under the same numerical rule on one second
structurally distinct sparse shape and no greater than 5% regression on dense and
few-transition controls. Do not average shapes or derive a model-name threshold.

Write raw evidence and a machine-recomputed result under
`docs/evidence/cuda-transition-families/stable-compaction-spike/`. The result is
`positive`, `negative`, or `inconclusive`; all are valid. Freeze the exact
structural legality/capacity predicate exercised by a positive arm and append a
measured `DECISIONS.md` note.

Only a positive, exact result authorizes drafting production compaction PRDs.
Whether it is later combined with dense family dispatch depends on the separate
dispatch spike; queried free memory must not silently tune between unmeasured
modes.
