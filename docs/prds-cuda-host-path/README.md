# CUDA host-path PRDs

The CUDA backend's cost is almost entirely host-side. This folder addresses the
largest item. Run from the Sembla repository with:

```text
/piprd run docs/prds-cuda-host-path
```

`README.md` is ignored by `/piprd run`. Every numbered PRD must read this file
first; the constraints below are binding.

**Where this sits in the overall order:** see the work queue in
[`docs/performance/model.md`](../performance/model.md#work-queue). Briefly, this
folder's local criteria can be satisfied at any time, but its hardware criteria
should be batched into a single GPU session together with re-measuring the CUDA
phase split.

## Why this folder exists

`DECISIONS.md` §L8 records the §L4 gate passing at 4.207×, and the per-phase
instrumentation built by `prds-execution-timing` shows where the CUDA path's
time actually goes. At 5M rows over 2 ticks
(`docs/evidence/demographic-bench/hyperstack-l4-20260726T140326Z/profile/`):

| phase | ms | share |
|---|---:|---:|
| **`state_reconstruct`** | **766.8** | **45.8%** |
| `observe_views` | 338.9 | 20.2% |
| `state_transfer` | 232.6 | 13.9% |
| `readback_control` | 131.0 | 7.8% |
| `report` | 123.6 | 7.4% |
| `other` | 72.0 | 4.3% |
| `kernels` | 9.4 | 0.56% |

`state_reconstruct` is `CudaBackend::download_state_store` rebuilding the entire
host `StateStore` from scratch, every tick, so host-side `observe_views` has
something to read. Per tick it unpacks the downloaded bytes into a fresh
`Vec<TableInit>`, validates it, allocates every column again in
`StateStore::new`, and then clones the whole thing for the `next` buffer.

**It is host allocation, not PCIe.** Transfer is 232.6 ms against
reconstruction's 766.8 ms. Making the copy faster would buy little; not
rebuilding would buy a great deal.

## The justification changed — read this before scoping

Earlier drafts of this idea were justified as "gets past §L4". **That
justification is gone**: the gate passed at 4.207× on 2026-07-27 with no GPU
change at all. This work now stands on its own merits, which are that it is the
largest measured item in the CUDA path and a local, bit-identical change of the
same shape as the four that landed in `prds-host-evaluator-performance`.

Do not reintroduce gate-clearing as a rationale, and do not treat a §L4
improvement as this folder's success criterion. §L8 also warns against steering
by that ratio at all: further CPU-side work *lowers* it while improving the
product.

## Relationship to device-side observation

`state_reconstruct`, `state_transfer` and host `observe_views` together are ~80%
of CUDA wall time, and all three exist because observation runs on the host.
Moving observation to the device would remove them at a stroke — but that is a
§K6/§L5 semantic decision with its own folder, and this folder must not
pre-empt it.

The sequencing argument is that this fix is cheap, local and reversible, whereas
device-side observation is a design commitment. Doing this first also tells you
how much of the 80% is really about *reconstruction* rather than about
observation, which is exactly the number that decision needs.

## Binding constraints

- **Results must not change.** Every golden, every CSV and hash golden, the
  frozen demographic state fixture, the run manifest including
  `final_state_sha256`, and the tracked CUDA differential evidence must be
  **byte-identical**.
- **Validation must not weaken.** `StateStore::new` runs
  `validate_table_initializers` — row counts, enum ranges, `Ref` bounds. Any
  in-place refresh must perform the *same* checks, producing the same errors
  with the same messages in the same order. A faster path that validates less is
  a failed PRD, and this is the most likely way to fail it.
- **Error behaviour is observable**, including *when* an error is raised.
- **No new dependencies.**

## Local versus hardware acceptance (inherited from §J14.2)

The CUDA feature cannot be built on the development Mac — there is no `nvcc`,
and `scripts/check-rust.sh` does not compile it. Per `DECISIONS.md` §J14.2:

- **Local criteria** must pass in the managed run without a GPU: the
  `StateStore` refresh path implemented and tested directly, the default build
  green, goldens unchanged. A PRD here is approvable on local criteria alone.
- **Hardware criteria** are listed in the implementation notes as *pending* and
  executed in a later Hyperstack session. Presenting an unbuilt CUDA path as
  verified is rejected.

Because the host-side `StateStore` work is testable without a GPU, most of the
correctness argument can and should be made locally.

## PRDs

- `0001-reuse-the-host-state-buffer` — stop rebuilding the host `StateStore`
  every tick. **Landed and hardware-verified** (§L9).
- `0002-reduce-control-counts-on-device` — count fired and deferred candidates
  on the device instead of moving 200 MB per tick to the host to be scanned.
  **Currently queued in [`docs/prds-run-queue`](../prds-run-queue/README.md)** as
  `0001-…`, because running this folder directly would re-attempt the landed
  `0001`. This README still binds it. It moves back here on approval.

### How 0002 was scoped

0001's note above said later PRDs would be re-scoped from the split measured
after it landed. That happened. Of the three candidates it named,
`observe_views` went to **zero** — `prds-device-observation` removed it entirely
— and the other two grew into almost the whole cost:

| phase | after 0001 | after device observation (§L11) |
|---|---:|---:|
| `observe_views` | 20.2% | **0%** |
| `readback_control` | 7.8% | **55.9%** |
| `report` | 7.4% | **32.9%** |

They did not get slower. Everything around them was removed. Together they are
now **88.8%** of CUDA wall time, and they exist to produce at most 13 integers
of diagnostic output.

That is §M1 working as intended: each re-scope was driven by a fresh
measurement, and the ranking inverted twice along the way.

## Measurement protocol

The per-phase `--timing-json` instrumentation from `prds-execution-timing` is
the instrument. Re-run the `cuda-l4-20260726` case — 5M rows, 2 ticks — so the
new split is directly comparable to the table above, and report
`state_reconstruct` before and after as the headline.

`BENCH_PROFILE=1 bash run-demographic-benchmark.sh` collects this; see
`spikes/precision/infra-hyperstack/RUNBOOK.md`. Re-pin `repository_ref` first.

## PRD 0001 implementation status

The local implementation retains one backend-owned `StateStore`, refreshes its
committed columns with shared constructor validation and fixed-shape checks, and
reuses both state-unpack staging and `next` column allocations. Refresh while a
write buffer is prepared is rejected without discarding the staged write. The
normal and timed CUDA CLI loops borrow the refreshed store each tick and move it
out once at run completion; the minimal CUDA-only CLI integration was explicitly
approved because the prior owned-observation contract cannot retain the same
allocation across ticks.

Local tests cover allocation retention, validation/error parity, shape mismatch,
prepared-write behavior, backend structure, and both CLI paths. CUDA-feature
checking, tests, and clippy pass locally without claiming GPU execution. Full
implementation notes and the §J14.2 hardware-pending phase table are in
[`docs/evidence/cuda-host-state-reuse-local-20260727/`](../evidence/cuda-host-state-reuse-local-20260727).
