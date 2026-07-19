# CUDA differential harness

Build the CLI with the native CUDA path and compare one model. The single-model
form accepts the run-time `--dt` and `--params` overrides as well as population,
seed, and ticks:

```sh
cargo run --release -p sembla-cli --features cuda -- diff-backends \
  examples/sir.json --population 100000 --seed 77 --ticks 200 \
  --dt 0.25 --params params.json
```

Corpus mode discovers `examples/*.json`, sorts paths bytewise, and runs every
model with the same numeric population, seed, tick count, and optional `--dt`
override. A shared parameter file is rejected because the examples declare
different parameter schemas. Defaults are population 100, seed 1, and 10 ticks;
explicit evidence runs should record all three:

```sh
cargo run --release -p sembla-cli --features cuda -- diff-backends \
  --all-examples --population 100 --seed 7 --ticks 20
```

For each model the command compares every committed state hash, the exact
results CSV bytes, and the exact summaries CSV bytes. It exits at the first
mismatch and reports the tick and CPU/CUDA hash pair. Successful lines include
informational ticks/second for both backends. The CUDA rate times execution plus
the per-tick downloads and read-only formatting required by this differential
mode; it is not a `FinalOnly` production-throughput claim.

CUDA owns scheduling, conflict resolution, effects, wires, and the evolving
state. The CLI downloads the committed post-tick snapshot only for canonical
hashing and read-only view/result formatting; this observation bridge never
calls the CPU tick executor and is not an execution fallback. CUDA manifests
record the device name and the CUDA Driver API compatibility version returned
by `cuDriverGetVersion` as `gpu_model` and `driver_version`.

Correctness CI may run on any CUDA-capable NVIDIA GPU because native `f64`
produces the selected exact semantics regardless of FP64 throughput.
Performance statements are made only from verified full-rate hardware. This
does not weaken ADR 0001's requirement that production hardware provide
full-rate FP64.

Use `crates/sembla-cuda/scripts/run-differential-corpus.sh` on a clean,
committed remote checkout. On independently verified full-rate hardware, set
`SEMBLA_RUN_FULL_RATE=1`; the runner additionally generates the selected
26,000,000-person / 1,300,000-employer SIR workload shape, executes it for one
tick, and captures its informational rate beside ADR 0001's 1,380.5 ticks/sec
reference. Failure of this optional measurement is recorded but is not a
correctness gate. Provisioning, provenance capture, and teardown
follow `spikes/precision/infra-hyperstack/README.md`. The runner writes a test
log and hardware/driver provenance; copy the verdict and informational
throughput into the dated evidence note before destroying the host.
