# CUDA host-state reuse: local implementation evidence

PRD: [`docs/prds-cuda-host-path/0001-reuse-the-host-state-buffer.md`](../../prds-cuda-host-path/0001-reuse-the-host-state-buffer.md)

Managed-run baseline: `4f3fbbd`.

## Implementation notes

`StateStore::refresh_backend_state` runs the same shared initializer validation
as `StateStore::new` before checking the existing store shape or mutating any
column. Once validation and shape checks succeed, each typed committed column is
updated with `copy_from_slice`, retaining its pointer, length, and capacity.
`prepare_next` now copies only typed values into the fixed-shape `next` buffer;
it no longer invokes derived `clone_from`, which replaced the nested enum and
column allocations.

A refresh presented while `write_prepared` is true is rejected with:

```text
cannot refresh backend state: a write buffer is already pending
```

The rejection does not discard staged writes. The regression test commits the
previously staged write after the rejection and observes its value.

`CudaBackend` owns one `host_state` and one same-shaped `host_tables` staging
value for its lifetime. Device bytes are decoded into the staging columns with
`clear` plus `extend`, so the intermediate `TableInit` shape and column
allocations are also retained. The staging value is deliberate: it lets the
shared constructor validation finish before any committed state or backend
input is changed. `refresh_backend_snapshot` validates state and inputs first,
then applies both atomically.

The existing owned `run_tick_observed` API remains compatible for callers that
retain independent tick snapshots. The normal and timed CUDA CLI loops use the
new lending-style path: they observe `CudaBackend::observed_state()` each tick
and move the final store out once, after the loop. This required a minimal
CUDA-only change to `crates/sembla-cli/src/main.rs`; the operator explicitly
approved that exception because returning an owned `StateStore` each tick
necessarily reallocates or clones it and cannot satisfy the PRD's ownership
goal within `backend.rs` alone.

`download_hash`, device transfers, kernels, the CPU tick loop, evaluator logic,
IR, and dependencies are unchanged.

## Local acceptance evidence

The tests cover:

- retained pointers and capacities for every state column across two refreshes;
- retained `next`-buffer pointers and capacities when writes are prepared;
- constructor/refresh diagnostic equality for a column row-count mismatch, an
  out-of-range enum value, and an out-of-bounds reference;
- rejection of a model-valid incoming row-count change relative to the existing
  store;
- rejection of refresh during a prepared write without losing the staged write;
- structural use of one backend-owned store, reusable unpack staging, and the
  lending-style normal and timed CUDA CLI paths.

Local commands run successfully:

```text
cargo test -p sembla-runtime --lib state::resolved_write_tests::backend_refresh_retains_current_and_next_column_allocations -- --exact
cargo test -p sembla-runtime --test state backend_refresh -- --nocapture
cargo test -p sembla-cuda --test host_state_reuse
cargo check -p sembla-cuda --features cuda
cargo check -p sembla-cli --features cuda
cargo test -p sembla-cuda --features cuda
cargo test -p sembla-cli --features cuda
cargo clippy -p sembla-cuda --features cuda --all-targets -- -D warnings
cargo clippy -p sembla-cli --features cuda --all-targets -- -D warnings
scripts/check-rust.sh
```

Final local gates also passed:

```text
cargo test --locked
scripts/check-rust.sh
python3 scripts/check-markdown-links.py
cargo fmt --all -- --check
git diff --check
```

Compared with managed-run baseline `4f3fbbd`, no tracked fixture, golden,
manifest golden, generated CUDA fixture, or pre-existing evidence file changed.
The complete default test suite therefore exercises the existing goldens against
the implementation without moving their expected bytes.

## Hardware criteria pending under §J14.2

No local result is presented as GPU evidence. The later Hyperstack session must
complete all four hardware criteria:

1. build release artifacts with `cargo build --release --features cuda` on the
   GPU host;
2. rerun `cuda-l4-20260726` at 5M rows for 2 ticks with `--timing-json`, reporting
   the full before/after phase table;
3. establish CPU/CUDA differential equality on the complete corpus, including
   the demographic no-grouped model;
4. confirm that `state_transfer` is unchanged, or explain any movement.

The directly comparable baseline is:

| phase | before ms | after ms |
|---|---:|---:|
| `state_reconstruct` | 766.8 | pending hardware |
| `observe_views` | 338.9 | pending hardware |
| `state_transfer` | 232.6 | pending hardware |
| `readback_control` | 131.0 | pending hardware |
| `report` | 123.6 | pending hardware |
| `other` | 72.0 | pending hardware |
| `kernels` | 9.4 | pending hardware |

The headline remains `state_reconstruct`; no §L4 ratio is reported.
