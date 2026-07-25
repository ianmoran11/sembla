# Demographic slot benchmark

This benchmark is manual benchmark/test tooling. It is not scientific
population generation and does not change the `demographic_slots` model's
interpretation. The synthetic rows exist only to measure the architecture.

## What is measured

`scripts/bench-demographic.sh` builds the release CLI and, for each requested
scale, records:

- deterministic `synth-state` wall time, peak RSS, and artifact bytes;
- a real state-artifact load through a summary-free working model at
  `--ticks 0` (the canonical summaries reject an empty run, while summaries do
  not affect the state schema);
- 24-tick wall time and peak RSS for the full benchmark model, the same model
  without `age_monthly`, and the same model without grouped views;
- state-export wall time, peak RSS, and bytes; and
- ticks/second, ageing cost share `(full - no-ageing) / full`, and grouped
  observation cost share `(full - no-grouped) / full`.

The script uses `/usr/bin/time -l` on Darwin and `/usr/bin/time -v` on Linux.
It records OS, release, architecture, CPU description, RAM, and a supplied
machine class, but never a hostname or workspace path. A negative cost share is
retained: a single noisy local measurement is evidence, not a regression gate.

## Synthetic-state contract

```text
sembla synth-state --model <model-or-plan.json> --slots N --areas K \
  --present-fraction F --streams birth:B,overseas:O,internal:I \
  --seed S --out state.artifact
```

The command is deliberately scoped to the documented demographic column roles:
`area.area_key`; the occupancy, event, sex, age, generation, entry-stream,
entry-age, area-ref, and slot-resource-ref columns on `person_slot`; and the
empty `slot_resource` table. Other shapes are rejected deterministically.
Values use fixed arithmetic coordinate mixing, never OS randomness. The
present count is `floor(slots * present-fraction)`; stream values are integer
weights used to partition the remaining rows, with the final stream receiving
rounding remainder.

Because `sembla.state/v1` enforces declared row counts exactly, the command
writes a canonical legacy companion model to `<out>.model.json`. It rewrites
only `area`, `person_slot`, and `slot_resource` row declarations. A plan input
is validated and its model is emitted as the companion; plan identity is not
silently rewritten. Both outputs refuse overwrite.

The artifact writer validates the caller's columns and streams the canonical
header and little-endian column payloads through a bounded 64 KiB encoding
buffer. It no longer constructs or clones an artifact-sized byte vector.
Small-scale tests and the frozen demographic state fixture verify round-trip
and byte identity.

## Local evidence

The managed-run evidence is in
[`evidence/demographic-bench/local-2026-07-24/`](evidence/demographic-bench/local-2026-07-24/README.md).
It was produced with:

```sh
scripts/bench-demographic.sh \
  --scales 10000,100000,1000000 \
  --seed 9009 \
  --ticks 24 \
  --out docs/evidence/demographic-bench/local-2026-07-24 \
  --machine-class "Apple M2 Pro, 16 GiB, CPU-only moderate-memory local"
```

| Slots | Artifact | Full | No ageing | No grouped | ticks/s | Ageing share | Grouped share | Peak RSS |
|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| 10,000 | 0.46 MiB | 0.51 s | 0.48 s | 0.46 s | 47.059 | 5.9% | 9.8% | 22.9 MiB |
| 100,000 | 4.58 MiB | 5.36 s | 4.87 s | 4.82 s | 4.478 | 9.1% | 10.1% | 129.1 MiB |
| 1,000,000 | 45.78 MiB | 54.68 s | 51.50 s | 56.77 s | 0.439 | **5.8%** | -3.8% | 551.8 MiB |

The measured artifact is approximately 48 bytes/slot for this exact schema.
That is distinct from the use case's earlier 64–80 bytes/slot raw estimate and
from a double-buffered `StateStore` plus runtime overhead. The 1M peak RSS is a
measurement; multiplication to larger scales below is only extrapolation.

### Extended curve — 2026-07-25

[`evidence/demographic-bench/local-2026-07-25/`](evidence/demographic-bench/local-2026-07-25/README.md)
extends the same machine class to 10M:

| Slots | s/tick | × vs 1M | Peak RSS | Artifact B/slot | Ageing share | Grouped share |
|---:|---:|---:|---:|---:|---:|---:|
| 1,000,000 | 2.28 | 1.00 | 0.54 GiB | 48.0 | 5.8% | −3.8% |
| 2,000,000 | 4.52 | 1.99 | 0.72 GiB | 48.0 | 8.1% | 7.9% |
| 5,000,000 | 11.95 | 5.25 | 1.50 GiB | 48.0 | 3.0% | 6.3% |
| 10,000,000 | 24.71 | 10.85 | 1.90 GiB | 48.0 | 11.6% | 12.3% |

Three results change how the numbers below should be read.

**Tick cost is measured linear** over a 10× range (10× slots → 10.85× wall
time), so time extrapolation is now supported rather than assumed: about 67
s/tick at 27M and 124 s/tick at 50M, making a 12-tick annual window roughly 13
and 25 minutes respectively.

**Artifact size is exactly 48 bytes/slot** across the whole range, confirming the
1M figure as a constant.

**The peak-RSS extrapolation below is far too conservative.** A linear
projection from 1M predicts 5.5 GiB at 10M; the measurement is 1.90 GiB, and the
slope decreases with scale. The "roughly 27 GiB at 50M" figure below is
therefore an overestimate — a linear projection from the 10M point gives about
9.7 GiB — but the sublinearity is not characterised, and this 16 GiB machine may
have been reclaiming under pressure at 10M, so the 50M CPU precondition below is
left unchanged until a memory-comfortable host measures it.

## Hardware evidence template — pending

The local 16 GiB machine is not evidence for 10M or 50M. CPU runs require a
machine with at least 32 GiB RAM for 50M, sufficient fast local storage, and no
competing workload. CUDA runs require an H100-class device with enough device
and host memory. Grouped observations are CPU-only under DECISIONS §K6, so CUDA
uses the no-grouped variant and does not report grouped or ageing shares.

**Automated collection (2026-07-25).** All four rows come from one provisioned
Hyperstack GPU VM — it carries both the CUDA device and the RAM the 50M CPU row
needs — via
[`spikes/precision/infra-hyperstack/run-demographic-benchmark.sh`](../spikes/precision/infra-hyperstack/run-demographic-benchmark.sh)
(module README §4b, with operating notes in
[`spikes/precision/infra-hyperstack/RUNBOOK.md`](../spikes/precision/infra-hyperstack/RUNBOOK.md)). After that module's reviewed paid apply, the script waits
for bootstrap, builds `sembla-cli --features cuda`, runs both backends, retrieves
a checksummed evidence directory with GPU/RAM/commit provenance, and then runs
the mandatory destroy itself, failing loudly if any paid resource survives in
state. The manual commands below remain correct for a host provisioned by other
means.

Exact CPU command:

```sh
scripts/bench-demographic.sh \
  --scales 10000000,50000000 \
  --seed 9009 --ticks 24 --backend cpu \
  --out demographic-bench-cpu-hardware \
  --machine-class "REPLACE: >=32 GiB dedicated CPU benchmark host"
```

Exact CUDA command:

```sh
scripts/bench-demographic.sh \
  --scales 10000000,50000000 \
  --seed 9009 --ticks 24 --backend cuda \
  --out demographic-bench-h100 \
  --machine-class "REPLACE: H100-class dedicated CUDA benchmark host"
```

| Hardware run | State | Load | Runtime | ticks/s | Peak RSS | Export | Status |
|---|---:|---:|---:|---:|---:|---:|---|
| 10M CPU full/no-ageing/no-grouped | pending | pending | pending | pending | pending | pending | **pending** |
| 50M CPU full/no-ageing/no-grouped | pending | pending | pending | pending | pending | pending | **pending** |
| 10M CUDA no-grouped | pending | pending | pending | pending | pending | pending | **pending** |
| 50M CUDA no-grouped | pending | pending | pending | pending | pending | pending | **pending** |

No hardware number in this table is inferred or fabricated. The script accepts
`50000000`; generation retains one typed column set and the state writer adds
only bounded encoding storage rather than another artifact-sized copy.

## Measured CUDA/CPU baseline — 2026-07-25

The frozen 10M-slot no-grouped case measured CUDA as **12.3× slower than the CPU
on the same host**:

| Backend | Wall time | s/tick |
|---|---:|---:|
| CUDA (NVIDIA H100 PCIe) | 1h 30m 08s | 225.3 |
| CPU (AMD EPYC 9554, same host) | 7m 19s | 18.3 |

Both arms used the `demographic_slots` no-grouped model for 24 ticks with seed
`9009`, on one Hyperstack host with an NVIDIA H100 PCIe, an AMD EPYC 9554,
177 GiB host RAM, Linux 6.8.0-90-generic, and x86-64. The recorded toolchain is
Rust 1.97.1, CUDA 12.8.93, and NVIDIA driver 570.195.03.

The checked-in CUDA arm records commit `f81fef9d90ad`; the CPU arm records
`8857cb839220`. No `crates/**`, Cargo manifest/lockfile, or fixture files changed
between those commits, and the benchmark-script delta affects only the CUDA
zero-tick export step, not the timed no-grouped run. The 12.3× ratio is retained
as diagnostic baseline evidence, but this evidence pair does not establish an
identical binary or state artifact and does not satisfy §L4's same-commit,
same-artifact, three-replicate gate. The evidence directories are:

- [`hyperstack-cuda-10m-20260725/`](evidence/demographic-bench/hyperstack-cuda-10m-20260725/)
- [`hyperstack-cpu-10m-20260725/`](evidence/demographic-bench/hyperstack-cpu-10m-20260725/)

DECISIONS §L1 records the cause: the generated CUDA simulation kernels are
parallel, but `sembla_validate_claims`, `sembla_validate_transition`,
`sembla_validate_effects`, and `sembla_validate_outputs` each run a per-row
validation loop on one GPU thread, once per claim and per fallible expression
per tick. The emitted source and sustained 100% `utilization.gpu` identify a
resident serial kernel rather than an execution-kernel or transfer stall.

The current CUDA path is not viable for models with contests or `Ref`
dereferences. Existing SIR throughput figures do not generalise to those models:
SIR generates none of the validation loops that dominate this benchmark.

## Interpretation

The three variants intentionally differ by exactly one named concern, guarded
against canonical-model drift by `synth_state.rs`. The cost-share arithmetic is
most useful at 1M, where timer quantization is negligible. It does not isolate
cache, allocator, thermal, or background-process effects. The negative 1M
grouped share demonstrates that a single local run cannot support a grouped
performance claim; it is recorded rather than adjusted.

Linear artifact extrapolation is about 458 MiB at 10M and 2.24 GiB at 50M.
Linear peak-RSS extrapolation from 1M would be roughly 27 GiB at 50M, which is
why the 50M row requires at least 32 GiB and remains pending. These are planning
extrapolations, not measurements.

## Determination

The binding §K2 trigger asks whether one monthly `age_months` write per present
slot is a material cost. The stated threshold is greater than 10% of tick wall
time at 1M slots. On the managed-run machine the measured ageing share is
**5.8%** (`54.68 s` full versus `51.50 s` without `age_monthly`), below that
threshold. A simple scale-linear extrapolation preserves a share rather than
turning 5.8% into a larger percentage; however, cache and memory-bandwidth
behavior at 10M/50M is unknown and the hardware rows remain pending.

**Recommendation:** do not open the deferred `Expr::Tick`/derived-age design
from this local evidence. Keep the explicit monthly ageing write and revisit
only if the pending hardware measurements exceed 10% or show a stated
nonlinear scaling reason. This is an evidence-backed recommendation, not the
decision itself; any decision to open `Expr::Tick` requires a future DECISIONS
§K amendment.

**Amended 2026-07-25 — the trigger condition is now ambiguous, not answered.**
The 10M measurement puts the ageing share at **11.6%**, above the 10% threshold
named above. That does not settle it. The four measured shares are 5.8%, 8.1%,
3.0%, and 11.6%, which is not a trend — it is a single unreplicated difference of
two wall-clock timings at each scale, and the 5M and 10M points cannot both
describe smooth behaviour. The §K2 threshold is also specified *at 1M*, where the
measurement remains 5.8%.

The recommendation is therefore unchanged, with its evidentiary basis restated:
`Expr::Tick` stays closed not because the cost is known small, but because the
cost is not yet known at all. Resolving it needs replicated runs at a fixed
scale with a reported spread, not another single point — the same replication
discipline any threshold decision requires.
