# PRD 0001 Review — Attempt 1

## Assessment

**REVISE** — the structural tiling, deterministic partitioning, evidence, and required gates are present, but the tiled parameter path changes an existing observable error contract.

## Blocking issue

### Tiled parameter preparation bypasses the canonical parameter-type check

`crates/sembla-runtime/src/eval.rs:1140-1143` converts whichever `ParamValue` is found in `ParamEnv` directly into a prepared Real or Int node. It does not look up the model declaration and verify that the environment value matches the declared `ParamType`.

The canonical whole-column path does perform this check at `crates/sembla-runtime/src/eval.rs:1341-1358` and returns:

```text
parameter environment value for '<name>' has the wrong type
```

`ParamEnv::defaults` is public and environments are not model-bound (`crates/sembla-runtime/src/eval.rs:229-245`), so a caller can pass an environment from another validated model with the same parameter name but a different type. For an eligible mixed Real expression, the tiled path may promote the wrong Int value and continue, while the column fallback raises the established diagnostic. The result therefore depends on whether the row count/tile size selects tiling, violating the binding error-semantics contract and the goal of identical behaviour for every tile size.

Required revision:

- make prepared `Expr::Param` handling mirror the declaration lookup and `(ParamType, ParamValue)` match in the canonical evaluator; and
- add a regression test that supplies a wrong-typed environment and compares tiled versus fallback behaviour across the worker/tile matrix.

## Acceptance criteria

1. **PASS — partial row tiling:** The permitted partial scope tiles eligible transition expressions and post-commit Count views. The measured 1,024-row constant and conservative L1 arithmetic are recorded in `eval.rs` and `docs/evidence/evaluator-tiled-tick-20260727/`.
2. **PASS — one parallel region and fixed partitioning:** Production has one scoped-thread region. Tile boundaries are fixed before worker assignment and results are restored by task index before transition/tile/row merging.
3. **REVISE — deterministic behaviour:** The required Real-chain, racing-clock, claim-key, complete-report, and aggregate tests cover workers 1/2/4 and tile sizes 257/1,024/4,093. However, the parameter-type discrepancy above is a concrete tiled-versus-fallback behavioural difference not covered by those tests.
4. **PASS — sequential `f64` reductions:** Aggregate expressions are asserted off the prepared path, the canonical ascending-row Real reduction remains sequential, and conflict resolution remains outside the parallel region.
5. **PASS — golden identity:** Evidence records identical CSV, summary, manifest, stdout, and final-state hashes. No tracked example, fixture, frozen-state, golden, or CUDA differential-evidence path changed.
6. **PASS — Rust gates:** `cargo test --locked` and `scripts/check-rust.sh` pass in the current workspace.
7. **PASS — measurement protocol:** Evidence contains five before/default/single-worker runs, fastest uncontended and medians, per-run contention, tile and threshold sweeps, worker count, `available_parallelism()`, and binding-threshold status.
8. **PASS — Markdown links:** `python3 scripts/check-markdown-links.py` passes: 118 local links in 168 tracked Markdown files.

## Scope

After excluding managed `.piprd/**` run artifacts, all implementation changes are within the PRD's allowed files. No dependency, RNG, IR, Lean, CUDA, or CLI implementation change was introduced.
