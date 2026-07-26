# Host-side profile — 2026-07-26

macOS `sample`, 1 ms interval, 25 s window over `demographic_slots` no-grouped,
1M slots, 24 ticks, **CPU backend**, Apple M2 Pro. Baseline for that run: 49.5 s
wall, 46.8 s user, 2.4 s sys — almost entirely user CPU.

Taken because the CUDA profile showed GPU kernels are 0.09% of wall time
(`cuda-l4-20260726/`), so the remaining cost is host-side. This profiles the CPU
backend, which shares the expression evaluator and state access with the CUDA
host path; the CUDA-specific host path cannot be profiled on Apple Silicon.

## Structure (inclusive counts, 19,098 samples total)

| Branch | Samples | Share |
|---|---:|---:|
| `execute_backend_output_with_features` → `run_tick_with_features` | 17,759 | 93% |
| ├─ `execute_tick` → `observe_views` → `eval_column` | 6,283 | **33%** |
| ├─ `execute_tick` → `eval_column` (transitions) | 4,907 + 3,017 | 42% |
| ├─ `execute_tick` → `eval_typed_ref_column` | 1,094 | 6% |
| `StateStore::state_hash` → `update_state_tables` | 1,339 | 7% |

**Everything routes through `sembla_runtime::eval::eval_expr`**, a recursive
tree-walking interpreter. Its samples cannot be summed across the tree without
double-counting (it recurses), so no single percentage is quoted here — but no
significant branch avoids it.

## Two recurring cost patterns

**1. Column lookup by string comparison, inside the row loop.**
`sembla_runtime::state::find_cell` appears throughout the tree, repeatedly with
`_platform_memcmp` as its child (e.g. 529 → 457, 494 → 417). Columns are being
resolved by name comparison on access rather than by an index resolved once.

**2. A full-length `Vec` allocated per expression node.**
`core::iter::adapters::try_process` → `Vec as SpecFromIter` → `Map as Iterator`
recurs at many nodes (1,837, 1,043, 882, 555 …). Each expression node appears to
materialise a new column-length vector rather than fusing or reusing a buffer.
At 1M rows that is a 1M-element allocation per node per tick.

**3. Observation is a third of runtime.** `observe_views` at 33% is the single
largest identifiable branch, and it runs on the host in both backends. That
alone bounds how far ahead of CPU the CUDA path can get, and is consistent with
the measured 2.56x ceiling.

## Caveats

Single 25 s sample of one configuration on one machine; no replicates. The
percentages are inclusive tree counts, not exclusive self-time — a profiler that
reports self-time (Instruments, `perf`) would sharpen the attribution. Treat this
as direction, not measurement.

## Implication

The next performance work is host-side and free to iterate on locally: resolve
column references once instead of per access, avoid materialising intermediates
per expression node, and consider whether observation can be narrowed or moved.
None of it requires a GPU.
