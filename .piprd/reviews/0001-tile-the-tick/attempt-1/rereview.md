# PRD 0001 re-review — Attempt 1

## Assessment

**APPROVED** — no blocking issues remain.

The prior parameter-validation blocker is resolved. `checked_parameter_value` in `crates/sembla-runtime/src/eval.rs:314-332` performs target-model declaration lookup, environment lookup, and exact `ParamType`/`ParamValue` validation. Both prepared and canonical column evaluation call it (`eval.rs:1162` and `eval.rs:1362`). The regression at `crates/sembla-runtime/src/executor.rs:2299-2332` supplies an Int environment to a model whose mixed arithmetic expression declares the parameter Real, then compares the exact fallback diagnostic across workers 1/2/4 and tile sizes 257/1,024/4,093.

## Acceptance criteria

1. **PASS — measured row tiling:** The permitted rigorous partial scope evaluates eligible transition expressions in fixed row tiles and eligible Count views in serial row tiles. The measured 1,024-row production constant, 1,000,000-row threshold, tile sweep, and conservative L1 live-set arithmetic are recorded in source and `docs/evidence/evaluator-tiled-tick-20260727/`.
2. **PASS — one fixed-task region:** Tile boundaries derive from row count and tile size before worker assignment. Worker count changes only task assignment; task outputs are restored by task index and merged in transition/tile/row order. Production contains one scoped-thread region at `executor.rs:786-815`.
3. **PASS — 3×3 bitwise determinism:** Real arithmetic, racing clocks, Real claim keys, and complete tick reports are compared across workers 1/2/4 and tile sizes 257/1,024/4,093. Real values and race/key values use `to_bits`.
4. **PASS — sequential Real reductions:** Aggregate/input and row-fallible expressions are excluded from preparation. The ascending-row Real accumulation remains sequential, its fallback is tested bitwise across the matrix, and conflict sorting/resolution remains after canonical candidate assembly and outside the parallel region.
5. **PASS — byte-identical oracle outputs:** No tracked example, fixture, frozen-state, golden, or CUDA path changed. Revised benchmark runs retain identical primary CSV, summary, manifest, stdout, and `final_state_sha256` hashes. The locked suite covering the tracked oracle artifacts passes.
6. **PASS — Rust gates:** Independent current-workspace runs of `cargo test --locked` and `scripts/check-rust.sh` pass.
7. **PASS — measurement protocol:** Evidence contains the unchanged five-run before corpus and revised five-run default/single-worker runs, fastest uncontended headlines, medians, per-run contention flags, user-time-primary ratios, tile and threshold sweeps, worker count, `available_parallelism()`, L1 arithmetic, and binding-threshold status. The recorded revised binary SHA-256 matches `target/release/sembla`.
8. **PASS — Markdown links:** `python3 scripts/check-markdown-links.py` passes with 118 local links in 168 tracked Markdown files.

## Scope and residual risk

Excluding managed `.piprd/**` artifacts, every modified or untracked path is allowed by the PRD. No dependencies, RNG, IR, Lean, CUDA, or CLI implementation changed; no files are staged. `git diff --check` and evidence JSON validation pass. No residual blocking risk was identified.
