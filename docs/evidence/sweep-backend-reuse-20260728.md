# Retained sweep backend implementation and local evidence

Date: 2026-07-28. Baseline source commit:
`c0acc2c03d0676750178686c029ffc0ecdadc0ea`.

## Lifecycle and identity

A sweep now constructs one `SweepBackend` before its sequential draw loop. Draw
zero and every later draw call the same reset API with resolved parameters and
an explicit execution seed. CPU retains one `StateStore`; CUDA retains one
compiled `CudaBackend`, including its context, module, functions, layouts and
device allocations. The previous per-draw `initial_tables.clone()` is gone.
Single-run, compare, replay, differential, and timed-run ownership are unchanged.

The old cross-draw identity comparison is replaced by a stronger invariant: one
retained object services the complete sweep. Its identity is captured once at
construction and written to the existing `backend_identity` manifest field.
No manifest schema changed.

## Complete mutable-surface reset

| Mutable surface | Reset between draws |
|---|---|
| CPU committed `current` state | constructor-validated initial columns overwrite existing vectors in place |
| CPU `next` state and pending-write bit | the same initial values overwrite `next`; pending writes are rejected before mutation and reset leaves `write_prepared=false` |
| CPU delivered inputs | row counts become zero and every typed column vector is cleared in place, retaining capacity |
| CPU parameters and seed | not retained; the current `ParamEnv` and explicit draw seed are passed into a fresh per-draw accumulator/tick loop |
| CPU tick coordinate | every draw loops from tick zero |
| CPU candidates, contests, `wins`, `deferred`, aggregates, diagnostics, observations and double-write scratch | tick-local executor values are recreated; output accumulator and warnings are recreated per draw |
| CUDA active and next state | device-to-device copies from one retained pristine device buffer |
| CUDA host state and unpack scratch | host `StateStore` resets from distinct pristine constructor tables; mutable unpack tables remain scratch |
| CUDA input bytes and counts (`inputs`, `next_inputs`, both count buffers) | zeroed in place |
| CUDA packed parameters | validated/packed before reset mutation, then copied into the existing device slice |
| CUDA seed, `next_tick`, host-current flag | explicit seed assignment, tick zero, host snapshot marked current after reset |
| CUDA diagnostics and validation scratch (`status`) | all 12 words zeroed |
| CUDA conflict state (`wins`, `deferred`) | zeroed |
| CUDA aggregate/candidate scratch (`aggregates`, partials, errors, facts, active, enabled, times, candidate errors, effect active) | zeroed |
| CUDA claim and write-ownership scratch (instance/winner resources, keys, rules, entities and instances; owners and owner values) | zeroed |
| CUDA output scratch (`output_partials`, `output_errors`) | zeroed |
| CUDA observation scratch (values, grouped extrema/minima/cardinalities/histogram, generic enum counts) | zeroed |
| Immutable CUDA surface | validated model, generated source/functions, module/context/stream, layouts/offsets/count metadata, hash mode, observation eligibility and device identity are retained unchanged |

Reset operations use existing slices/vectors. Parameter packing creates only a
small host byte vector; no state column or CUDA device slice is allocated per
draw. CUDA reset operations are ordered on one stream and synchronized before
tick zero.

## Correctness evidence

- Runtime unit tests dirty current/next state and delivered inputs, repeatedly
  reset, and assert values plus pointer/capacity signatures are unchanged.
- A malformed reset has constructor-identical diagnostics and leaves both state
  buffers, inputs and pending-write state unchanged.
- A CLI unit test runs four draws and asserts the backend factory count is one.
- The sweep integration matrix covers a contest model and a grouped-observation
  model under CRN and independent noise. Draw 3 after three preceding draws is
  byte-equal to a fresh `run` using the manifest execution seed; main CSV,
  grouped sidecar and all execution hash fields including final state are
  compared.
- A hardware-gated CUDA test runs prior ticks, resets/reseeds, and compares its
  final state hash with a freshly constructed backend using that seed.
- Existing sweep golden tests remain byte-equal.

## Local CPU measurement

Machine: Apple M2 Pro, macOS. Both arms used the same scaled
`fixtures/demographic/benchmark/demographic_slots.no-grouped.json`, 1,000,000
slots, 24 ticks, 20 draws, independent noise, CPU backend, seed 9009, and no
grouped-observation enable. Model SHA-256:
`601766d8c11443cb05da2500b00bb78fade375b8df2d0323bae35b7d8a17a130`.
State SHA-256:
`896e0062228b74ba24df95e53e28ca368df510f957ed03ef2f49160590a6922b`.

| arm | release binary SHA-256 | measured sweep wall | per draw (wall / 20) | draw 0 including setup | median later draw |
|---|---|---:|---:|---:|---:|
| baseline | `3b7bf73c05a330ef42b570dd24bb92ff072be29245cd4c852b610d1738a3b1ca` | 60.942 s | 3.047 s | 3.654 s | 3.016 s |
| retained backend | `f75f28a1017f93a0e26eb35fde15211d9ca174dc89dbe50ee961fef82d24ee5a` | 59.904 s | 2.995 s | 3.573 s | 2.909 s |

Both arms used the same external observer, polling creation of `draw_N.csv`
every 3 ms, so their draw boundaries cover identical work. The raw records are
retained under
[`sweep-backend-reuse-20260728/`](sweep-backend-reuse-20260728/README.md).
The baseline's separate `/usr/bin/time` run was 61.36 s; table values use the
matched observer runs. On that matched boundary, whole-sweep wall improves by
1.038 s (1.7%) and the later-draw median by 107.175 ms (3.6%). Neither draw
timing is inferred from an average.

The native retained-backend timing is supplementary because it starts after
input initialization and stops each draw before output-file writes. It reports
57.542 s, setup 19.409 ms, draw 0 body 2913.537 ms, and median later draw
2840.534 ms. Its `draw_zero_including_setup_wall_time_ms` is 2932.946 ms, but
these narrower values are not compared with the external baseline.

Reproduction command (substitute the recorded baseline/current binary path):

```bash
/usr/bin/time -p "$BIN" sweep /tmp/sweep-prd-baseline/no-grouped-1m.json \
  --population /tmp/sweep-prd-baseline/initial-1m.state \
  --seed 9009 --draws 20 --ticks 24 --noise independent --backend cpu \
  --timing-json /tmp/sweep-timing.json --out /tmp/sweep-output
```

A separate current full/grouped diagnostic took 133.01 s externally. It is not
presented as the baseline counterpart because its model/features differ.

## Large-state clone/reset isolation and GPU status

A local 10M-slot, 20-draw, zero-tick CPU case isolates initial-state clone and
reset overhead without claiming to be the required 24-tick scientific run. It
used a 480,000,881-byte state with SHA-256
`02934c1f4161ced37395e82dacf64039cdb99f1d12434e83c5a87f0b07c9b57c` and a
temporary no-grouped model with summaries removed because summaries reject an
empty tick series.

| 10M clone/reset isolation | whole wall | draw 0 including setup | median later draw |
|---|---:|---:|---:|
| baseline | 42.707 s | 5.521 s | 1.877 s |
| retained backend | 34.168 s | 5.568 s | 1.421 s |

Raw records are retained under
[`sweep-backend-reuse-20260728/`](sweep-backend-reuse-20260728/README.md).
Both rows use the same external completed-file observer. The 20.0% whole-wall
and 24.3% later-draw-median reductions demonstrate the removed large per-draw
clone separately, but this zero-tick isolation case **is not** substituted for
10M × 24 × 20.
That full local run was not attempted because it would require hundreds of
seconds plus multi-gigabyte working space after the required 1M runs and test
gates. No 24-tick 10M number is fabricated. The reproducible paid-host collector
stage is:

```bash
cd spikes/precision/infra-hyperstack
BENCH_SWEEP=1 \
BENCH_SWEEP_BASELINE_COMMIT=c0acc2c03d0676750178686c029ffc0ecdadc0ea \
  bash run-demographic-benchmark.sh 2>&1 | tee ~/bench-driver.log
```

That opt-in stage builds baseline/current release CUDA binaries, synthesizes
shared 1M and 10M states, runs baseline/current CPU and CUDA 20×24 sweeps,
records whole and per-draw timing for both versions, compares complete CPU/CUDA
output trees, and proves its comparator rejects an isolated grouped-sidecar
perturbation of an otherwise identical copy. GPU build/run/timing and full
10M × 24 × 20 results remain hardware-pending.
