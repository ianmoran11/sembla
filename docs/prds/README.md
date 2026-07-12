# Sembla v0.1 PRDs

Ordered PRD set for implementing Sembla v0.1, designed to be run by
[pi-piprd](https://github.com/ianmoran11/pi-piprd) (`/piprd run docs/prds`).
This README is excluded from runs.

## Authority

`DESIGN.md` at the repository root is the design authority. Every PRD cites
the sections it implements. Where a PRD and DESIGN.md conflict, flag it in the
implementation notes and follow DESIGN.md.

## Run order

| # | PRD | Layer |
|---|-----|-------|
| 0001 | Rust workspace scaffold | infra |
| 0002 | IR schema, serde types, validator | IR |
| 0003 | Philox RNG by coordinates | runtime |
| 0004 | Columnar state store | runtime |
| 0005 | Kernel expression evaluator | runtime |
| 0006 | Tick executor (racing clocks, conflicts) | runtime |
| 0007 | Composition: boxes, wires, one-tick delay | runtime |
| 0008 | SIR end-to-end at 1M agents | model |
| 0009 | Two-box feedback demo | model |
| 0010 | Lean frontend DSL → IR | frontend |
| 0011 | Lean structure widgets | frontend |
| 0012 | GPU throughput spike (throwaway) | spike |
| 0013 | Prior-predictive sweep runner | model |

## Global conventions (binding on all PRDs)

- **Crates:** `sembla-ir`, `sembla-runtime`, `sembla-cli` in a Cargo
  workspace. CLI binary is `sembla`. Lean code lives in `frontend/`
  (introduced in PRD 0010). The GPU spike lives in `spikes/gpu-throughput/`
  and is never depended on by the workspace.
- **Numerics:** all real-valued attributes and hazard arithmetic are `f64`.
  Entity IDs are `u32` row indices, stable within a run.
- **Determinism (Level A):** same IR + same seed ⇒ byte-identical outputs on
  the same binary/machine. No `HashMap` iteration order may reach any output
  or any random-draw coordinate; use `BTreeMap`/`IndexMap` or sorted vectors
  anywhere order can leak. All randomness flows through the PRD-0003 API —
  `rand::thread_rng` and friends are forbidden in `sembla-runtime`.
- **Identifiers:** `rule_id` = declaration order of transitions within a
  model (u32, model-wide, assigned by the IR validator). Table and box names
  are unique snake_case strings.
- **Parameters:** the run contract is **seed + IR + θ + level**. Parameter
  values are never inlined into the IR (`Expr::Param` only); θ is resolved
  once before tick 0 from declared defaults plus per-run overrides.
  Reserved RNG namespaces: `rule_id = u32::MAX` for prior/parameter draws
  (PRD 0013); synthetic-population generation uses its own documented
  reserved namespace (PRD 0008).
- **State hash:** `sembla-runtime` exposes a canonical SHA-256 over state
  (defined in PRD 0004) used by determinism tests throughout.
- **Testing:** every PRD lands with `cargo test --workspace` green. Tests
  added by earlier PRDs must stay green in later ones.
- **Fixtures:** example IR files live in `examples/*.json` and are the
  compatibility contract; changing one requires updating its golden test.
