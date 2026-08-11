# Estimation boundary

The maintained contract is described in [External estimation protocol](../design/estimation-protocol.md).

## Ownership

- `data/abs/` owns acquisition, normalisation, aggregate targets, deterministic fits, scoring and Australian population chain orchestration.
- `calibration/npe/` owns the pinned reference NPE implementation, pair validation, posterior artifacts and SBC diagnostics.
- `sembla-cli` owns the simulator-facing commands and manifests.
- `sembla-runtime` owns only the forward simulation; it does not know calibration exists.

## Flow

```text
ABS extracts -> targets/v1 -----------------------+
                                                   |
plan + theta draws -> sweep -> observations/pairs  +-> estimator -> params JSON
                                                        |              |
                                                        + diagnostics  +-> run
                                                                          |
                                                              scores + next state
```

## Method neutrality

The stable simulator-facing interface consists of:

- symbolic parameter declarations and optional priors;
- `--params` for one validated assignment;
- `--theta-file` for externally chosen draws;
- `sweep` with independent or CRN noise as appropriate;
- declared summaries and grouped observation artifacts;
- pairs CSV plus versioned sidecar;
- targets, scores, state artifacts and run manifests.

NPE is one implementation over those contracts. The offline gravity fit already demonstrates a non-NPE estimator feeding the same forward runner. Future ABC, simulated method of moments or sequential proposals should replace/add an external harness rather than modify the runtime.

## Parameter classes

The aggregate-first proposal classifies parameters as directly estimable, aggregate-dynamical, micro-only, or coupling/scenario. Only the micro-only residual block should routinely consume broad simulation budgets. This is a scientific architecture proposal until adopted in `DECISIONS.md`; the canvas marks it as a proposal rather than a settled dependency.

## Known contract gap

Estimated parameters currently return as a plain JSON object. Workflow manifests carry provenance around the file, but the file itself does not bind values to model hash, target hash, estimator or diagnostics. A future versioned parameter envelope is a plausible additive contract, but is not implemented or accepted today.
