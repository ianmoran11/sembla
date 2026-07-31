# Local demographic benchmark evidence — 2026-07-25

Machine class: Apple M2 Pro (`arm64`), 16 GiB RAM, CPU-only,
moderate-memory local development machine. No hostname or workspace path is
recorded. Same machine class as
[`local-2026-07-24/`](../local-2026-07-24/README.md), so the two sets compose
into one curve.

Command:

```sh
scripts/bench-demographic.sh \
  --scales 2000000,5000000,10000000 \
  --seed 9009 \
  --ticks 24 \
  --out docs/evidence/demographic-bench/local-2026-07-25 \
  --machine-class "Apple M2 Pro, 16 GiB, CPU-only moderate-memory local"
```

- `bench-results.json` is the machine-readable evidence.
- `bench-results.md` is the rendered table.

## Why this run exists

The 2026-07-24 evidence measured one scale worth trusting (1M). Two planning
decisions were resting on a linear extrapolation from that single point: whether
the CUDA path is load-bearing at national scale (~27M slots), and what geography
resolution the demographic model can afford. This run extends the CPU curve to
10M so those decisions rest on a measured shape rather than an assumed one.

## The measured curve

Combining both evidence sets (1M row from 2026-07-24):

| Slots | s/tick | × vs 1M | Peak RSS | Artifact B/slot | Ageing share | Grouped share |
|---:|---:|---:|---:|---:|---:|---:|
| 1,000,000 | 2.28 | 1.00 | 0.54 GiB | 48.0 | 5.8% | −3.8% |
| 2,000,000 | 4.52 | 1.99 | 0.72 GiB | 48.0 | 8.1% | 7.9% |
| 5,000,000 | 11.95 | 5.25 | 1.50 GiB | 48.0 | 3.0% | 6.3% |
| 10,000,000 | 24.71 | 10.85 | 1.90 GiB | 48.0 | 11.6% | 12.3% |

**Tick cost is linear in slots.** Ten times the slots costs 10.85× the wall time
— a mild superlinearity consistent with cache behaviour, not a scaling wall.

**Artifact size is exactly linear** at 48 bytes/slot across a 10× range, which
confirms the 2026-07-24 measurement as a constant rather than a coincidence.

**Peak RSS grows sublinearly and should not be extrapolated confidently.** A
linear projection from 1M predicts 5.5 GiB at 10M; the measurement is 1.90 GiB.
The slope also decreases in a way a memory model would not predict on its own
(186 MiB/M slots between 1M and 2M, 266 between 2M and 5M, 82 between 5M and
10M). This machine has 16 GiB and was not idle, so the 10M figure may reflect OS
reclaim under pressure rather than the true working set. Treat it as an upper
bound on what a memory-comfortable host needs, not as a characterised curve.

## What this does and does not license

Extrapolating tick cost from the 10M point:

| Scale | s/tick | 12-tick annual window | State artifact |
|---:|---:|---:|---:|
| 27,000,000 | 66.7 | 13.3 min | 1.21 GiB |
| 50,000,000 | 123.6 | 24.7 min | 2.24 GiB |

That is a supportable extrapolation for *time*, because time is measured linear
over a 10× range on this machine. It is not a claim about any other machine, and
it says nothing about CUDA, which is unmeasured at every scale.

The consequence for planning is unchanged in direction and sharper in degree: a
single national-scale annual window is minutes of CPU, so a handful of validation
runs is affordable, while a calibration sweep of hundreds of draws is tens of
hours and remains a GPU problem.

## Cost shares are too noisy to decide anything

The ageing share reads 5.8%, 8.1%, 3.0%, 11.6% and the grouped share −3.8%,
7.9%, 6.3%, 12.3% across the four scales. These are single unreplicated runs of
a difference between two wall-clock measurements, so the sequence is dominated
by run-to-run variation — the 5M ageing share (3.0%) and the 10M share (11.6%)
cannot both be describing a smooth trend.

This matters for the DECISIONS §K2 trigger, which asks whether the monthly
`age_months` write costs more than 10% of tick wall time. The 10M measurement is
**11.6%, above that threshold**, and `docs/performance/demographic-benchmark.md`'s standing
recommendation says to revisit if hardware measurements exceed 10%. But one
unreplicated point over a threshold is not evidence that the threshold is
crossed; the honest reading is that the share is unresolved at every scale and
that deciding it requires replicates, not another single run.

No number here is inferred or fabricated. The 10M/50M CUDA rows and the 50M CPU
row remain pending on remote hardware.
