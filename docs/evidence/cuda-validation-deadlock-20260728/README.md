# Parallel validation deadlocks under a multi-thread launch geometry

Evidence for `DECISIONS.md` §L12. Captured 2026-07-28 on an H100 PCIe
(driver 570.195.03), commit `04ada45`, during the session that verified
`prds-device-observation/0001` and `0002`.

## What happened

`crates/sembla-cuda/scripts/run-differential-corpus.sh` reached the negative
diagnostic corpus, passed the first case at launch geometry `1x1`, and then hung
at `1x32`:

```text
running 1 test
diagnostic_case=claim-key-overflow geometry=1x1 status=[10, 2, 0, 0]
test backend::diagnostic_equality_hardware::negative_corpus_matches_cpu_status_under_three_geometries has been running for over 60 seconds
```

`run.log` records nothing after that line. The process was left for **2h31m** at
100% GPU utilisation and 489 MiB before being killed. The same suite completed
in **23.04s** on 2026-07-19 (`spikes/precision/evidence/cuda-differential-corpus-20260719/tests.log`),
before the construct below existed.

## Cause

`sembla_record_validation_failure` in the generated CUDA acquires a spin lock
(`crates/sembla-cuda/src/codegen.rs:2795`):

```cuda
while (atomicCAS(status + 4, 0ULL, 1ULL) != 0ULL) { }
```

`git log -S` attributes it to `8feb168 Implement 0002-parallel-validation-kernels`.

Geometry `1x1` is a single thread, so the lock is never contended and the case
passes. Geometry `1x32` is one full warp, so 32 threads contend and the run
hangs. `GEOMETRIES` is `[(1, 1), (1, 32), (3, 4)]`
(`crates/sembla-cuda/tests/support/diagnostic_cases.rs:6`).

The in-source comment above the function states that the pattern requires
independent thread scheduling, available on sm_70+. An H100 is sm_90, so ITS is
present and the pattern hangs anyway — **the comment states a necessary
condition as if it were sufficient**.

## Reproduction

23 seconds on any CUDA-capable NVIDIA GPU:

```sh
cargo test --locked --release -p sembla-cuda --features cuda --lib \
  negative_corpus_matches_cpu_status_under_three_geometries -- --ignored --nocapture
```

## Fix direction

Select the reported failure with pure atomics and no critical section, as the
surrounding `atomicMin`/`atomicMax` band-extrema reductions already do, rather
than making the lock work. Widening the launch geometry would hide the defect,
and the geometry sweep is the only thing that caught it.

## Files

| file | what it is |
|---|---|
| `run.log` | the corpus run, ending at the hang |
| `diagnostic-corpus.log` | the cargo test output |
| `provenance.txt` | commit, UTC, driver, GPU |
| `commit.txt`, `worktree-status.txt` | the tree the corpus ran against |
| `gpu-provenance.txt` | full `nvidia-smi` detail |

The successful parts of the same session are in
[`demographic-bench/hyperstack-l4-20260728T072119Z/`](../demographic-bench/hyperstack-l4-20260728T072119Z/).
