# External estimation protocol

**Status:** Maintained descriptive architecture. Normative commitments remain in [`DESIGN.md`](../../DESIGN.md) and [`DECISIONS.md`](../../DECISIONS.md), especially §G and §N.

## Purpose

Sembla separates the forward and inverse problems:

```text
forward: parameter values + validated plan + run contract -> simulated observations
inverse: observed targets + forward evaluations          -> estimated parameter values
```

The simulation runtime owns the forward problem. Estimation is an external client that communicates through CLI commands and versioned artifacts. Neither `sembla-ir` nor `sembla-runtime` knows whether parameter values came from a person, a configuration file, an aggregate estimating equation, optimisation, ABC, simulated method of moments, NPE, or a later method.

This document names that boundary as the **external estimation protocol**. It does not define an estimator framework and does not make NPE the abstraction.

## Dependency direction

```text
Lean model/plan
      |
      v
sembla run / sweep <-----------------------+
      |                                    |
      v                                    |
run manifest + observations + state        |
      |                                    |
      v                                    |
external estimator + observed targets      |
      |                                    |
      +---- parameter assignments ---------+
```

The allowed dependency is:

```text
estimation -> CLI and serialized contracts -> simulator
```

The forbidden dependencies are:

```text
runtime/core -> estimation implementation
estimation   -> Rust or Lean implementation internals
estimation   -> CPU or CUDA implementation directly
```

The quarantine is checked by [`calibration/npe/tests/test_quarantine.py`](../../calibration/npe/tests/test_quarantine.py). Production Cargo, Rust, and Lean sources may not name the estimation directories. Python's framework-facing fixture and executable paths are explicitly allowlisted there.

## Contract family

The machine-readable inventory is [`artifact-registry.json`](../architecture/artifact-registry.json). The estimation-facing subset is:

| Concern | Representation | Producer | Consumer | Main invariant |
| --- | --- | --- | --- | --- |
| Parameter declarations | `Model.params` in model/plan JSON | Lean frontend or another IR producer | IR validator, runtime, estimators | Name, type, default and optional prior are model-owned; expressions retain symbolic parameter references |
| One parameter assignment | JSON object `{name: value}` for `--params` | Person or estimator | `sembla run` / `sembla sweep` | Unknown, duplicate, non-finite and type-invalid values are rejected against the validated model |
| Externally selected draws | JSON array of assignments for `--theta-file` | Estimator or experiment designer | `sembla sweep` | Every draw is checked against parameter declarations; pinned `--params` cannot collide with draw assignments |
| Simulated `(theta, x)` pairs | CSV plus `.meta.json`, `schema_versions.pairs = 1` | `sembla sweep --export-pairs` or a declared domain adapter | Estimator | Exact-byte SHA-256, explicit parameter/summary columns, contiguous draw coordinate, recorded noise mode and IR hash |
| Run provenance | `run-manifest.json`, per-concern schema versions | CLI | Verification, scoring, chain and estimation tools | Resolved theta, seed, model/plan identity, backend identity, flags and result/state hashes are recorded |
| Observed targets | `sembla.targets/v1` | `data/abs/targets.py` or another versioned data pipeline | Estimator and scorer | Every target names its model observation, period, source/vintage and fitted or held-out role |
| Target scores | `sembla.target-score/v1` | `data/abs/score.py` | Gates, reports and calibration workflow | Fitted and held-out evidence remain separated; scores bind to target and run artifacts |
| Chained state | `sembla.state/v1` plus chain records | One forward run | A later forward run | Schema and hash bind exact committed tables; no live runtime state crosses the boundary |
| Population input | `SEMBLA_POP` or `sembla.state/v1` | External population pipeline or prior run | CLI/runtime | Population generation remains outside the simulation runtime |

### Parameter transport is intentionally small

`--params` and `--theta-file` are transport shapes, not model definitions. Parameter authority remains in the validated model/plan. An estimator must not maintain an independent unvalidated parameter vocabulary.

The Australian population workflow has generated and derived parameter artifacts under `data/abs/params/`. Their duplication is controlled: [`scripts/check-abs-data.sh`](../../scripts/check-abs-data.sh) requires byte-identical regeneration of derived tables, values and Lean-facing parameter-family data.

### Training pairs

The generic pair exporter uses declared model summaries as `x`. [`calibration/npe/contract.py`](../../calibration/npe/contract.py) rejects:

- an unsupported pairs schema;
- common-random-number (`crn`) noise for NPE training;
- an incorrect exact-byte hash;
- overlapping, duplicated or incorrectly ordered columns;
- non-contiguous draw coordinates; and
- non-numeric or non-finite cells.

A domain workflow may deterministically derive a richer vector from declared observation artifacts, as `data/abs/calibrate.py` does for grouped demographic outputs. That transformation must have its own versioned record and must not reach into runtime memory or backend internals.

### Targets are data

Targets are produced before estimation and carry their fitted/held-out role. An estimation harness must not assemble or relabel the target split after seeing results. `sembla.targets/v1` binds the model observation, period and ABS vintage so a reconstructed fitted cell cannot later be presented as held-out validation.

## Estimator roles

The protocol supports different methods without changing the simulator:

1. **Directly estimable parameters** — fit from occurrence/exposure counts or microdata outside the simulator.
2. **Aggregate-dynamical parameters** — fit with a deterministic or approximate aggregate companion.
3. **Micro-only parameters** — use targeted simulator-based inference through `--theta-file`, `sweep`, observations and pairs.
4. **Coupling/scenario parameters** — estimate from linked evidence or expose explicitly as scenarios.

This classification follows [Demographic spine and modular aggregate-first calibration](demographic-spine-and-modular-calibration.md). That document is a proposal, not an amendment to the normative decision record.

### NPE's current role

[`calibration/npe/`](../../calibration/npe/README.md) is one external consumer of this protocol. Its reference trainer, posterior artifacts and SBC gates are estimator-specific. They are not runtime contracts and do not define a general estimator interface.

The generic simulator-facing pieces—parameter declarations, externally supplied draws, independent-noise sweeps, declared observations, pairs and manifests—also support ABC, simulated method of moments, sequential proposal loops and deterministic aggregate fits. A new method should normally replace or add an external harness, not change the IR or runtime.

`DECISIONS.md` §G5 currently records amortized NPE as the adopted calibration method. Any demotion or replacement of that decision belongs in `DECISIONS.md`; this descriptive protocol does not silently amend it.

## End-to-end obligations

An estimation workflow is conforming when it:

1. identifies the exact model or executable plan and records its semantic hash;
2. identifies the observed target artifact and its fitted/held-out split;
3. supplies theta only through validated assignment contracts;
4. uses independent simulation noise where the estimator requires independent conditional samples;
5. records every forward execution through a run or sweep manifest;
6. records estimator-specific diagnostics outside the runtime;
7. produces parameter values that are revalidated by `sembla run` or `sembla sweep`; and
8. keeps held-out scoring separate from fit reconstruction.

## Deliberate non-contracts

The protocol does **not** standardise:

- an `Estimator` trait or plugin API;
- posterior file formats such as `posterior.pt`;
- optimisation algorithms, loss functions or stopping rules;
- scientific identifiability claims;
- a universal summary vector across models; or
- in-process Python/Rust bindings.

Those concerns are estimator or domain implementation details until multiple implementations demonstrate a smaller stable interface.

## Known gap: estimated parameter provenance

A calibrated parameter file is currently a plain JSON object. The surrounding workflow records provenance, but the parameter file itself does not bind its values to the model hash, targets hash, estimator or diagnostics.

A future `sembla.parameters/v1` envelope is the natural additive contract if this gap becomes operationally costly. It should wrap—not replace—the simple name/value assignment consumed by the CLI. It is **not implemented** and must not be emitted or accepted until adopted through the normal decision and conformance process.
