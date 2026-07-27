# Device-side observation PRDs

Move observation onto the GPU so the CUDA path stops downloading and rebuilding
the whole state every tick. Run from the Sembla repository with:

```text
/piprd run docs/prds-run-queue
```

`README.md` is ignored by `/piprd run`. Every numbered PRD must read this file
first; the constraints below are binding.

## Why this folder exists

`docs/evidence/demographic-bench/hyperstack-l4-20260727T120050Z/profile/`
measures the CUDA path after `prds-cuda-host-path/0001`, at 5M rows over 2
ticks:

| phase | ms | share |
|---|---:|---:|
| `state_transfer` | 239.6 | 25.6% |
| `state_reconstruct` | 220.7 | 23.6% |
| `readback_control` | 206.3 | 22.0% |
| `report` | 119.9 | 12.8% |
| `observe_views` | 93.5 | 10.0% |
| `other` | 46.9 | 5.0% |
| `kernels` | 9.3 | **1.0%** |

**81% of CUDA wall time exists to serve host-side observation.** The first three
rows — 71% — are the machinery for getting device state onto the host, and they
exist for exactly one reason: `observe_views` runs there. Kernels are 1.0% and
have been roughly 9 ms across every measurement since `DECISIONS.md` §L7.

## What makes this tractable now

§K6 made grouped observations CPU-only in v0.1 and §L5 declined to revisit it
during a performance track. Both were right at the time. But the reason usually
given for keeping reductions on the host — floating-point accumulation order —
**does not apply to this model**:

| view kind | count | reducer | order-independent? |
|---|---:|---|---|
| ungrouped | 18 | `count` | yes — integer |
| ungrouped | 1 | `max` over `int` (`generation`) | yes |
| grouped | 3 | `count` | yes — integer |

Every reduction in `demographic_slots` is integer and associative. For this
model a device-side reduction is **bit-identical by construction**, not by
argument.

## What is genuinely constrained

The evaluator is more general than the model, and a device path must respect
that:

Grouped views reduce by count as well, and their key spaces are small and dense
in practice — `population_cells` is sex(2) × area(4) × ~20 age bands ≈ 160
groups. Enum and `Ref` cardinalities are known from the schema; only banded
`Int` keys are unbounded, and one min/max reduction bounds them exactly.

- **`Sum` over `Real` must stay on the host.** `eval.rs` fixes ascending row
  order as the canonical Level A reduction order. Any tree or per-tile
  reduction changes the sum.
- **`Sum` over `Int` is subtler than it looks.** `checked_add` in a different
  association order can overflow where the sequential pass does not, turning a
  working model into an error. Treat it as ineligible unless a PRD argues
  otherwise explicitly.
- **`Min`/`Max` over `Real`** are order-independent for non-NaN values, but NaN
  handling in `f64::min`/`max` is asymmetric. Argue it or exclude it.
- **Eligibility is a property of the whole run, not of a view.** If any view
  falls back to the host, the state must still be downloaded and the 71% is not
  recovered. The download may only be skipped when *every* view in the model is
  device-eligible.

## Binding constraints

- **Results must not change.** Every golden, every CSV and hash golden, the
  frozen demographic state fixture, the run manifest including
  `final_state_sha256`, and the tracked CUDA differential evidence must be
  **byte-identical**.
- **The differential harness is the safety net and must stay armed.** `DESIGN.md`
  §8 makes the CPU the oracle. A device observation that disagrees shows up as a
  differential mismatch — that is the property being relied on, so no PRD here
  may weaken, skip, or special-case the comparison.
- **Eligibility is decided from the IR**, conservatively, never from the
  benchmark model's shape. Anything unrecognised falls back.
- **No new dependencies.**

## This changes semantics, so it changes DECISIONS

§K6 states "V1 is CPU-only and rejects grouped views deterministically on CUDA",
and §K9 lists CUDA grouped-observation support as a deferred construct. A PRD
here that lands device-side grouped views **must** amend both, in the same
commit, with the eligibility rule stated. Leaving the decision record contradicting
the code is not acceptable.

Ungrouped device observation does not contradict §K6, which is about grouped
views only — but the eligibility rule still belongs in the record.

## Local versus hardware acceptance (inherited from §J14.2)

The CUDA feature cannot be built on the development Mac. Local criteria — the
eligibility predicate, host-path parity tests, goldens unchanged — are required
for approval. Hardware criteria are listed as *pending* and executed in a later
Hyperstack session. Presenting an unbuilt CUDA path as verified is rejected.

## PRDs

Both PRDs currently live in [`docs/prds-run-queue/`](../prds-run-queue/), which
orders the pending work across folders so one `/piprd run` covers it. **This
README still binds them** — they name it in their first line. When they land,
they move back here.

- `0001-device-observation-ungrouped` — evaluate eligible ungrouped views on the
  device and skip the per-tick state download when the whole model qualifies.
- `0002-device-observation-grouped` — the same for grouped views, which is what
  the real workflow actually uses.

**0002 is required, not optional.** The demographic model's calibration and
validation outputs *are* grouped views — `population_cells` (sex × area ×
five-year age band), `deaths_cells`, `vacancy_cells` — and
`docs/demographic-model.md` runs the calibration workflow with
`--enable grouped-observations` throughout. The §L4 benchmark uses the
*no-grouped* configuration only because `main.rs:2660` rejects grouped views on
CUDA.

So 0001 alone delivers nothing for the driver model: eligibility is
all-or-nothing per run, and a model with grouped views downloads the state
regardless. **0001 is the mechanism; 0002 is the payoff.**

## Measurement protocol

The `--timing-json` instrumentation is the instrument. Re-run the
`cuda-l4-20260726` case — 5M rows, 2 ticks — and report the full phase table
against the one above. `state_transfer`, `state_reconstruct` and
`readback_control` are the headline; they should collapse together or not at all.

`BENCH_PROFILE=1 bash run-demographic-benchmark.sh` collects it. Re-pin
`repository_ref` first, and see `spikes/precision/infra-hyperstack/RUNBOOK.md`
for the session hazards — including that a mid-run change of the operator's
egress IP silently breaks SSH.

## PRD 0001 implementation status (local)

Ungrouped device observation now has an all-or-nothing IR gate. The benchmark's
19 scalar views are reported eligible (18 filtered counts and one Int maximum).
Eligible runs return one scalar per view and do not download or reconstruct
state per tick. Host-ineligible and zero-view runs retain the complete download
fallback. `readback_control` is intentionally unchanged: `wins` and `deferred`
still transfer for fired/deferred reporting and remain a separate opportunity.

`HashMode::EveryTick`, used by the differential path, still transfers raw device
state/input bytes through `download_hash`; that transfer is timed separately as
`state_hash`, not hidden in `state_transfer`. Final-only runs download the final
state once after the tick loop when the retained host snapshot is stale.

Per §J14.2, CUDA compilation, CPU/CUDA corpus equality (including the eligible
demographic no-grouped case and an explicit ineligible fallback case), and the
`cuda-l4-20260726` timing rerun remain **hardware-pending**. That rerun must
record the complete before/after phase table, including `kernels`, and preserve
the declaration-ordered eligibility breakdown in its log. No local result is
presented as GPU evidence.
