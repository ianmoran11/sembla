# Run queue

**Pending:** `0001-scalar-schema-and-state-domains.md` from the approved Lean IR foundational formalization track.

## Last cleared 2026-08-01 — raw coverage

`0001-raw-ir-and-plan-coverage` was approved and returned as
[`prds-lean-ir-formalization/0002`](../prds-lean-ir-formalization/0002-raw-ir-and-plan-coverage.md)
in implementation commit `837360e`. It established exhaustive coverage for 55
constructors and 163 fields without changing the raw contract. A follow-up
corrected `ClaimOrdering.key` from surface-produced to raw-only accepted, in
line with the existing DSL boundary and PRD 0004.

## Earlier cleared 2026-08-01 — charter and proof policy

`0001-semantic-charter-and-proof-policy` was approved and returned as
[`prds-lean-ir-formalization/0001`](../prds-lean-ir-formalization/0001-semantic-charter-and-proof-policy.md)
in implementation commit `e95570f`. It established the semantic charter,
Mathlib pin, exact module map and automated proof-policy gate.

## Last cleared 2026-07-30

`0001-run-cuda-draws-concurrently` was approved and landed as
[`prds-sweep-throughput/0004`](../prds-sweep-throughput/0004-run-cuda-draws-concurrently.md)
in implementation commit `d862a91`. Local criteria 1–9 passed; hardware
criteria 10–13 remain pending for the later paid GPU session tracked in the
performance-model work queue.

## Last cleared 2026-07-29

`0001-reduce-control-counts-on-device` ran from here and landed as
[`prds-cuda-host-path/0002`](../prds-cuda-host-path/0002-reduce-control-counts-on-device.md).

**It stalled five attempts first, and the PRD was at fault.** Criterion 7 ran
`check-prd-allowlist.py` against `docs/prds-cuda-host-path/0002-*.md` — correct
when written, wrong the moment the PRD was moved here. The glob matched nothing,
the command exited 2, and no in-run action could fix it.

**So: a criterion must never hard-code the PRD's own path.** Files move between
their folder and this queue by design, which makes any such path unsatisfiable
from the other location. Refer to "this PRD at its current path", and run
`python3 scripts/check-prd-allowlist.py <the PRD>` before queueing — it now
reports paths passed to a command that do not resolve, which is exactly this
defect. See `DECISIONS.md` §M5.

This folder exists to gather pending PRDs from several folders under sortable
names, so one command runs them in the right order:

```text
/piprd run docs/prds-run-queue
```

Running it now executes only the approved scalar/schema/state-domain PRD.

## How to use it

Move a pending PRD here, renamed `0001-…`, `0002-…` in the order it should run.
**Move, do not copy** — a PRD is a specification the reviewer enforces literally,
and two copies of one would eventually disagree with no way to tell which was
authoritative.

Each PRD keeps naming the README that binds it, and that stays the binding
context. Status notes and evidence go to the real folders per each PRD's
allowed-file list. When a PRD is approved, move it back.

This directory only orders work that an authoritative track has already
approved. Performance work is authorized and ordered by the work queue in
[`docs/performance/model.md`](../performance/model.md#work-queue); other work
requires explicit approval in its binding track README and must agree with the
canonical [`ROADMAP.md`](../ROADMAP.md).

## Last cleared 2026-07-28

Three PRDs ran from here and all landed:

| | now at |
|---|---|
| device observation, ungrouped | `prds-device-observation/0001` |
| device observation, grouped | `prds-device-observation/0002` |
| generalise the tiling constants | `prds-evaluator-throughput/0008` |

## Historical 2026-07-28 paid-session note

The following was the queue state after the 2026-07-28 clear and is retained as
a workflow record; later performance decisions and evidence supersede its
pending-status claims.

A single Hyperstack session, which three things then waited on:

- `prds-device-observation/0001` and `0002` have **never been compiled** with
  `--features cuda`; their §J14.2 hardware criteria are pending.
- The differential corpus in its **grouped** configuration, which it has never
  covered, because CUDA rejected grouped views until `0002` removed the
  rejection.
- Re-measuring the CUDA phase split, where 71% of wall time is still spent
  moving state to the host — the cost `0001` and `0002` exist to remove.

All three now have automation behind them, added 2026-07-28:
`BENCH_CORPUS=1 BENCH_PROFILE=1 bash run-demographic-benchmark.sh`. Before that
the corpus had no collector invoking it at all, and the profile stage only ever
covered the no-grouped model.

Re-pin `repository_ref` before that session, and read
`spikes/precision/infra-hyperstack/RUNBOOK.md` — including that a mid-run change
of the operator's egress IP silently breaks SSH, which cost most of an evening
on 2026-07-27, and that opening the security-group rule alone does not fix it
because the guest firewall pins the same `/32`.
