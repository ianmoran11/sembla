# Run queue

**Pending: one PRD.**

1. [`0001-run-cuda-draws-concurrently`](0001-run-cuda-draws-concurrently.md) —
   promote the measured free-running non-blocking CUDA-stream spike to a
   supported, explicit, bounded, default-off `--draw-workers` interface.
   Local criteria run without a GPU; hardware criteria remain pending for a
   later paid session.

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

Running it now will run only `0001-run-cuda-draws-concurrently`.

## How to use it

Move a pending PRD here, renamed `0001-…`, `0002-…` in the order it should run.
**Move, do not copy** — a PRD is a specification the reviewer enforces literally,
and two copies of one would eventually disagree with no way to tell which was
authoritative.

Each PRD keeps naming the README that binds it, and that stays the binding
context. Status notes and evidence go to the real folders per each PRD's
allowed-file list. When a PRD is approved, move it back.

The authority on what should be queued is the work queue in
[`docs/performance-model.md`](../performance-model.md#work-queue).

## Last cleared 2026-07-28

Three PRDs ran from here and all landed:

| | now at |
|---|---|
| device observation, ungrouped | `prds-device-observation/0001` |
| device observation, grouped | `prds-device-observation/0002` |
| generalise the tiling constants | `prds-evaluator-throughput/0008` |

## What is next is not a PRD

A single Hyperstack session, which three things now wait on:

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
