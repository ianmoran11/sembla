# Run queue

**One PRD queued**, 2026-07-28:

- `0001-reduce-control-counts-on-device` — belongs to
  [`docs/prds-cuda-host-path`](../prds-cuda-host-path/README.md), whose README
  binds it. Queued here because that folder's `0001` has already landed, and
  running the folder directly risks the runner re-attempting completed work.

Move it back to `prds-cuda-host-path/0002-…` when it is approved.

This folder exists to gather pending PRDs from several folders under sortable
names, so one command runs them in the right order:

```text
/piprd run docs/prds-run-queue
```

Running it now would do nothing.

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
