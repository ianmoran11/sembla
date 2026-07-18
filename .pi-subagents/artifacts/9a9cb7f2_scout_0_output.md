# Code Context

## Files Retrieved
1. `docs/prds-npe-path/0008-cuda-backend.md` (lines 1-63) - CUDA requirements, fragment, equality and acceptance contract.
2. `crates/sembla-ir/src/model.rs` (lines 10-349) - complete ordered IR schema: model/boxes/tables, expressions, effects, claims, ports/outputs/wires.
3. `crates/sembla-ir/src/validate.rs` (lines 56-119, 429-712) - `ValidatedModel`, stable rule IDs, transition and wire validation.
4. `crates/sembla-runtime/src/eval.rs` (lines 1-14, 648-710, 726-933, 935-1124, 1227-1548) - exact expression, input aggregate, arithmetic, and grouped aggregate semantics.
5. `crates/sembla-runtime/src/executor.rs` (lines 242-391, 633-793, 796-909, 944-1083, 1120-1161) - tick pipeline, candidate order, wire delivery, conflict resolution, effects.
6. `crates/sembla-runtime/src/rng.rs` (lines 1-130) - coordinate packing, Philox, open-uniform conversion and exponential clock.
7. `crates/sembla-runtime/src/state.rs` (lines 11-75, 203-356, 451-527, 717-782) - column layout, double buffering, canonical state hash serialization.
8. `crates/sembla-runtime/src/population.rs` (lines 19-174, 249-302) - deterministic SIR population and initializer helpers.
9. `crates/sembla-runtime/tests/rng.rs` (lines 1-166) - reusable Philox known-answer and distribution vectors.
10. `crates/sembla-runtime/tests/sir.rs` (lines 11-159, 176-273) - SIR loader/simulation helpers, 100k oracle scenario, policy fixture.
11. `crates/sembla-runtime/tests/executor.rs` (lines 50-192, 271-276, 499-503, 650-660) - small model helper, deterministic hashes, tie expectations, rollback.
12. `crates/sembla-runtime/tests/composition.rs` (lines 6-8, 102-104, 211-218) - composed model loading and wire/hash determinism.
13. `crates/sembla-runtime/Cargo.toml` (lines 1-12) and `crates/sembla-runtime/src/lib.rs` (lines 1-11) - current dependency/public module surface.

## Key Code

### IR and supported fragment
- `ValidatedModel` wraps the source `Model` and ordered `ValidatedTransition` metadata (`validate.rs:56-88`). `validate()` assigns `rule_id` by **box declaration order, then transition declaration order**, starting at zero; `u32::MAX-1` and `u32::MAX` are reserved (`validate.rs:91-119`). CUDA codegen should consume this type, never independently renumber rules.
- State types are `Real=f64`, `Int=i64`, `Enum=u16` declaration-order index, and `Ref=u32` row index (`model.rs:122-150`; `state.rs:11-17`). Fixed population is structural: no birth/death operations exist.
- Expression variants (`model.rs:176-259`): literals Real/Int/Bool/Enum, `Param`, `SelfAttr`, Add/Sub/Mul/Div, Eq/Ne, Lt/Le/Gt/Ge, And/Or/Not, `EnumIs`, wired `Input { Aggregate }`, and same-box joined `Agg`. Aggregates are Count or Sum (`model.rs:263-285`).
- Effects are only `Effect::SetAttr` on the transition's current row (`model.rs:154-167`). Destination may be Real/Int/Enum/Ref; a Ref write requires a structurally equal resource claim (`validate.rs:429-543`).
- Claims are Ref-valued and ordered by `RaceTime` or numeric/enum `Key` (`model.rs:313-336`). A candidate with multiple claims fires only if it wins all.
- Wire fragment is `OutputBuilder::PerTable` with scalar Count/Sum fields feeding a schema-identical input. At most one wire targets an input (`model.rs:287-349`; `validate.rs:645-697`). Runtime outputs are one-row tables.
- Views/summaries are observation sinks and are present in current validated IR. PRD 0008 says group-by views beyond execution are non-goals, so GPU backend can plausibly execute state/wires and leave observations to downloaded CPU state, but this boundary must be explicit; “any validated model” otherwise includes views/summaries.

### Exact CPU expression semantics
- Evaluation is against one immutable tick-start `Snapshot`; syntax-tree order is preserved without reassociation (`eval.rs:1-14`, `648-670`). Both operands of And/Or are evaluated (no scalar short-circuit) before elementwise boolean combination (`eval.rs:846-869`).
- Real arithmetic uses ordinary Rust IEEE-754 operations. Division always promotes to Real; mixed numeric operations cast `i64 as f64`; integer Add/Sub/Mul use checked operations and error on overflow (`eval.rs:1227-1278`). Division by zero yields IEEE infinity/NaN. Equality uses normal `==` (therefore NaN unequal and +0 == -0); ordered comparisons use normal `< <= > >=` (`eval.rs:1281-1389`).
- Enum values are declaration-order `u16`; Ref values are target-table row IDs. Top-level Ref uses `eval_typed_ref_column`, preserving target table (`eval.rs:684-710`).
- Aggregate Count is checked `i64`. Aggregate Sum is a **single sequential pass in ascending target row order**, conditionally adding into group slots (`eval.rs:1481-1548` and continuation); real identity is `+0.0`, integer addition is checked. Input aggregates likewise scan input rows ascending (`eval.rs:935-995`). Cache identity is structural, with Real literals compared by `to_bits`, and one cache per snapshot/parameter scope (`eval.rs:261-489`).
- Important mismatch, **high severity**: PRD asks GPU “fixed-shape two-pass trees” with the “same fixed order as CPU,” but current CPU grouped/input/output/view sums are sequential left folds, not two-pass trees. A two-pass CUDA reduction generally cannot be bitwise equal. Either GPU must reproduce the sequential per-group order (possibly sorted segmented scans), or CPU oracle semantics must deliberately change with updated golden hashes/tests.

### Exact tick/order/effect semantics
`execute_tick` (`executor.rs:284-391`) is the oracle pipeline:
1. Snapshot all committed state and current wire inputs.
2. Stage boxes in box declaration order; each box stages transitions in validated/declaration order (`stage_box`, lines 633-793).
3. For each transition evaluate full guard, hazard and claim columns from old snapshot. Iterate rows ascending. Candidate exists iff guard true, `lambda > 0` by `partial_cmp`, and sampled `race_time < dt` by `partial_cmp`. NaN hazard/time therefore does not fire. Entity ID is row converted to `u32`.
4. Draw exactly `exp_f64(seed,tick,rule_id,entity_id,draw_idx=0,lambda)` for eligible positive hazards. Claims share that race time.
5. Resolve claims globally within each box. Claim instances are initially sorted by `(resource table index, resource row, rule_id, entity_id, claim_index)` (`executor.rs:944-970`). For each resource, winner is minimum ordering value: `f64::total_cmp` for race/Real keys, natural Int/Enum order, then lexicographic `(rule_id, entity_id)` (`executor.rs:1036-1083`). Mixed ordering modes/types or mismatched enum domains error. Candidates losing any claim do not fire.
6. Evaluate every effect column from the old snapshot, transition order then effect order, and stage winner-row values. Sort write identities only to detect duplicate `(box,table,attr,row)` writes; duplicates are fatal rather than last-write-wins (`executor.rs:1120-1161`).
7. Clone old state, apply pending writes, build all Moore outputs from prospective new state, then atomically commit and replace inputs. Thus outputs from tick T affect hazards at T+1 (`executor.rs:284-347`, `796-909`). Any error rolls back state/input.
8. Views evaluate after commit and cannot consume RNG or affect execution.

No result-bearing atomic operation can preserve this contract unless it is merely writing a uniquely owned destination. Resource grouping and scatter/effect application need deterministic sorting/segmentation.

### RNG contract
- `draw_u32x4` is Philox4x32-10. Key is `[seed_lo, seed_hi]`; counter is `[tick, rule_id, entity_id, draw_idx]`; constants are in `rng.rs:14-18`, round and key bump in lines 27-65.
- `uniform_f64` takes all lane 0 then high 21 bits of lane 1: `mantissa=(lane0<<21)|(lane1>>11)`, then `(mantissa as f64 + 0.5)*2^-53`; a rounded 1.0 clamps to `f64::from_bits(1.0.to_bits()-1)` (`rng.rs:82-108`).
- `exp_f64`: lambda <= 0 returns +infinity; otherwise `-ln(U)/lambda` (`rng.rs:114-130`).
- **High risk**: CUDA `log()` and host Rust/libm `f64::ln()` are not generally promised bit-identical; compiler contraction/FMA, FTZ, and optimization flags can also alter expression bits. Generated CUDA must disable reassociation/contraction on result paths and establish a bit-identical logarithm strategy or acceptance hashes can fail even when Philox lanes match.

### Hash behavior
`Snapshot::state_hash` (`state.rs:451-527`) is SHA-256 over canonical bytes:
- No input ports: domain `SEMBLA_STATE_V1\0`; with any input ports: `SEMBLA_STATE_V2\0` and input tables appended.
- Counts/lengths are `u64` little-endian. Tables are box-major IR order; columns attribute order; values row order. Names are raw UTF-8 prefixed by byte length.
- Type tags: Real 0, Int 1, Enum 2, Ref 3. Real hashes `to_bits()` little-endian (signed zero and NaN payload preserved); Int i64 LE, Enum u16 LE, Ref u32 LE (`state.rs:717-782`).
- V2 appends inputs in box/port declaration order, including explicit zero-row tick-0 tables. Hash excludes pending writes and includes newly delivered inputs after commit.
- A GPU differential helper should download columns and in-flight inputs into this exact layout and reuse host SHA-256; implementing an independent GPU hash is unnecessary and risky. Final-only default versus per-tick debug controls download cadence, not serialization.

### Concrete implementation map
1. New optional `sembla-cuda` crate: depend on `sembla-ir`; expose constructor taking `&ValidatedModel`, resolved parameters, initial columns, seed. Keep CUDA dependencies behind workspace/member `cuda` features so default workspace resolution does not compile toolkit bindings.
2. Codegen module starts from `ValidatedModel::model()` plus `transitions()`: deterministic symbol mangling/index tables in declaration order; emit typed expression functions matching `eval_expr`; dump exact generated source via documented env var. Golden source should use `examples/sir.json`.
3. Host/device layout module mirrors `ColumnData`/`TableInit`, box-major/table/attribute order, and maintains current/next buffers plus wire input buffers device-resident.
4. Tick kernels mirror `stage_box`: one transition guard+hazard+draw kernel, deterministic candidate compaction, claim evaluation; shared stable sort/segmented argmin; winner-all-claims pass; effect-column evaluation against old buffer; duplicate-write detection; unique scatter to cloned next buffer; output reduction/wire delivery; swap.
5. Oracle adapter/test utility should run CPU `run_tick`, record `StateStore::state_hash()` after every tick, and compare downloaded GPU hashes and fired/deferred vectors. Reuse `SyntheticPopulation::{generate,sir_table_initializers,sir_policy_table_initializers}` (`population.rs:44-114,249-302`).
6. No-device constructor error must be testable without ignored GPU execution and must never instantiate `sembla-runtime` as fallback.

### Test helpers and fixtures
- Philox checked vectors: `tests/rng.rs:8-31` (zero, all-ones, asymmetric packing). Export/share these coordinates rather than duplicating opaque literals in CUDA tests.
- Small deterministic oracle: `tests/executor.rs:50-83` model builder and lines 182-193 50-tick hash comparison. Contention expectations use `exp_f64` and `total_cmp` at lines 271-276 and 499-503.
- SIR: `tests/sir.rs:11-69` loads `examples/sir.json`, resolves beta/gamma, simulates and returns final hash. Existing acceptance-like test is 100k x **100** ticks (`sir.rs:76-106`), while PRD requires 200; add a separate GPU ignored 200-tick per-tick trace.
- Two-box: `tests/sir.rs:176-273` loads `examples/sir_policy.json`, initializes policy state, and freezes one-tick Moore delay. Composition hash parity patterns are in `tests/composition.rs:102-104`.
- Hash golden/mutation coverage is `tests/state.rs:175-244`; use the same host hash method for GPU downloads.
- Non-GPU codegen tests: same model twice/source byte equality; golden SIR generated `.cu`; source-order perturbation tests; local no-device diagnostic. GPU tests `#[ignore]`: Philox vector kernel; SIR 100k/200 ticks; sir_policy; canonical model; same-run-twice.

## Architecture
CPU runtime is a snapshot-isolated columnar interpreter. IR validation freezes types and global rule coordinates. Each tick evaluates all candidate/effect expressions from old state, resolves contests deterministically, commits buffered writes, constructs next-tick wires from prospective state, then observes committed state. CUDA should replace evaluation and deterministic staging/resolution—not reinterpret or reorder the contract. State can stay resident, but equality is anchored to the host canonical hash and exact CPU RNG/math/order.

## Risks / findings
- **High** — Reduction specification conflicts with implementation: sequential CPU folds versus PRD two-pass trees.
- **High** — Host `ln` versus CUDA `log` and FMA/reassociation can prevent bitwise racing-clock/effect equality.
- **High** — “Any validated model” includes nested same-box `Agg`, wired `Input` aggregates, multi-box Moore outputs, Ref effects, multiple claims, views/summaries; implementing only SIR guard/hazard is insufficient.
- **Medium** — Deterministic compaction/sort must preserve rule/entity/claim tie identities; generic unstable GPU sort can change winners.
- **Medium** — GPU state hash must include V2 in-flight input tables, not just model columns.
- **Medium** — Runtime integer overflow and invalid Ref/Enum writes are recoverable errors with atomic rollback; kernels need deterministic error reporting before swap.
- **Medium** — CUDA build-feature wiring is absent today (`sembla-runtime/Cargo.toml` has only IR and sha2); optional dependency/workspace feature behavior must be verified on a toolkit-free machine.
- **Low** — Current SIR test checks final hash at 100 ticks, not required per-tick 200-tick trace.

## Start Here
Open `crates/sembla-runtime/src/executor.rs` at `stage_box` (line 633). It is the executable ordering contract that kernel scheduling must mirror; then implement expression emission from `eval.rs` and freeze serialization against `state.rs::Snapshot::state_hash`.