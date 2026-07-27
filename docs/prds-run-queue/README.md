# Run queue

The pending PRDs, gathered from two folders and renamed so one command runs them
all in the right order:

```text
/piprd run docs/prds-run-queue
```

| | binding README | why this order |
|---|---|---|
| `0001-device-observation-ungrouped` | `prds-device-observation` | the mechanism: eligibility predicate, device reductions, skipping the state download |
| `0002-device-observation-grouped` | `prds-device-observation` | the payoff: the driver model's outputs *are* grouped views, and eligibility is all-or-nothing per run |
| `0003-generalise-tiling-constants` | `prds-evaluator-throughput` | insurance, not a gain — deliberately last |

These are the **only** copies. They were moved here rather than duplicated: a
PRD is a specification the reviewer enforces literally, and two copies of one
would eventually disagree, with no way to tell which was authoritative.

## Read this before running

**This folder holds the files but not the rules.** Each PRD begins by naming the
README that binds it — `docs/prds-device-observation/README.md` or
`docs/prds-evaluator-throughput/README.md` — and *that* is the binding context,
not this file. The constraints, the §J14.2 local/hardware split and the
allowed-file lists all live there.

**Status notes and evidence go to the real folders**, as each PRD's allowed-file
list specifies. Nothing should be written back here.

**Move each PRD back to its own folder once it is approved**, so this folder
only ever holds pending work and the folders stay the permanent record. If you
are unsure what is current, trust the work queue in
[`docs/performance-model.md`](../performance-model.md#work-queue).

## What is deliberately not here

- Anything already approved — `prds-evaluator-throughput/0001`–`0007`,
  `prds-cuda-host-path/0001`, and the earlier folders.
- The GPU session that verifies `0001` and `0002` under §J14.2. Both are
  approvable on local criteria alone; their hardware criteria are executed
  later, together, in one Hyperstack trip.
- Work with no PRD yet: device-side reduction of the `wins`/`deferred` arrays
  (22% of CUDA wall, no semantic decision needed), sweep-draw parallelism, and
  the deferred RNG decision.
