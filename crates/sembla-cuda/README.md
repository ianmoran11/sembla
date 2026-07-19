# `sembla-cuda`

Production CUDA backend for the validated Sembla v0.1 kernel fragment. It uses
native `f64`, generates one NVRTC kernel per transition, keeps state and wire
inputs device-resident across ticks, and never substitutes a CPU backend.

## Build and runtime contract

Pure code generation is built and tested in the default workspace. CUDA
bindings are opt-in:

```sh
cargo build -p sembla-cuda --features cuda
```

The workspace pins Rust 1.79 because `cudarc` 0.17 uses pointer-alignment APIs
stabilized in that release. `cudarc` dynamically loads the CUDA driver and
NVRTC, so ordinary builds need neither a toolkit nor CUDA headers. At runtime,
construction reports distinct errors for a missing driver, no device, or
missing NVRTC. There is no interpreter or alternate-GPU fallback.

`CudaBackend::new` validates and uploads the initial columnar state once. Calls
to `run` retain current/next state, aggregate, candidate, write-owner, and wire
buffers on the device. `HashMode::FinalOnly` downloads only the final state;
`HashMode::EveryTick` downloads after every tick for differential testing.
Hashes use the runtime's exact V1/V2 canonical byte format, including in-flight
wire inputs.

## Deterministic generation and execution

`generate(&ValidatedModel)` traverses model declarations and validated rule IDs
only in their stable order. It preserves expression-tree operand order and
emits exact `f64` literals from bit patterns. `CudaBackend::new` compiles those
exact bytes with NVRTC (`--fmad=false`, precise division/sqrt, no fast math).
Set `SEMBLA_CUDA_DUMP_DIR` to dump the content-addressed `.cu` source before
compilation.

The CPU oracle's canonical reduction is one ascending-row fold. CUDA preserves
that exact arithmetic order with a deterministic two-stage implementation: one
device worker performs the ordered fold, then a publication kernel copies the
result without another floating-point or checked-integer operation. This
correctness-first shape is used for grouped, input, and wire-output reductions.
Conflict resolution runs one deterministic scan per candidate with
lexicographic `(key, rule_id, entity_id)` ties. Correctness-first, single-thread
device validators reproduce the CPU's recursive column order and surface
precomputed aggregate error facts only at their first syntactic use. Boxes are
scheduled, resolved, and effect-validated in declaration order; all effect
columns are validated before pending writes are checked for duplicates in CPU
`transition -> winner row -> effect` order. Wired outputs are validated in wire
and field declaration order before their parallel publication stage. Checked
binary operands are explicitly sequenced in generated C++14 lambdas, so two
fallible siblings never write the same error flag without sequencing. Effects
are scattered in ascending destination order. No result-bearing path uses
atomics. Prospective aggregates are rebuilt from `next_state` before Moore
outputs, and state is swapped only after every device status check succeeds.

## GPU correctness run

GPU tests are checked in but unconditionally ignored in ordinary CI. Run them
with:

```sh
crates/sembla-cuda/scripts/run-gpu-tests.sh
```

The script records commit, device, driver, toolkit/NVRTC proxy, test output,
and an artifact digest. It is intended to run on a provisioned machine using
[`spikes/precision/infra-hyperstack/README.md`](../../spikes/precision/infra-hyperstack/README.md);
follow that runbook's validation and destruction steps. Correctness results are
valid on any compatible CUDA GPU. Performance claims are valid only on verified
full-rate hardware, following [`spikes/precision/README.md`](../../spikes/precision/README.md).

The CLI differential workflow is documented in
[`docs/cuda-differential-harness.md`](../../docs/cuda-differential-harness.md)
and its clean-checkout evidence runner is
[`scripts/run-differential-corpus.sh`](scripts/run-differential-corpus.sh).
CUDA transitions remain device-executed; the CLI's downloaded committed-state
mirror is used only for read-only view and byte formatting.

Current implementation-time GPU status is recorded in
[`GPU-STATUS.md`](GPU-STATUS.md). An absent GPU is always reported as
**unanswered**, never as a simulated pass.
