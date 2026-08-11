# Contract boundary

## Schema of record

`crates/sembla-ir` is the Rust schema and semantic validator for model and executable-plan input. The Lean frontend has independent hand-written encoders held byte-compatible by golden fixtures and parity checks. There is no generated cross-language schema today.

The machine-readable inventory is [`artifact-registry.json`](artifact-registry.json). Every production-source identifier matching the versioned Sembla identifier grammar must name:

- an owner;
- one or more producers;
- one or more consumers;
- its kind; and
- its stability level.

`scripts/check-artifact-registry.py` rejects missing and stale registrations.

## Principal contracts

| Contract | Stable boundary |
| --- | --- |
| Model IR | `Model`: parameters, boxes, tables, transitions, wires and observation sinks |
| Executable plan | `sembla.executable-plan/v1`: canonical flat model, stable identity map and optional linked provenance |
| Validated model/plan | In-memory post-validation capability; deliberately not serialized |
| Run manifest | Seed, resolved theta, model/plan identity, backend identity, flags, hashes and executions |
| State artifact | `sembla.state/v1`: portable committed columnar tables for chained runs |
| Bundle | `sembla.bundle/v1`: source, plan, link report and integrity manifest |
| Estimation pairs | Pairs CSV plus sidecar with `schema_versions.pairs = 1` |
| Observed targets | `sembla.targets/v1`, with fitted/held-out role and data vintage |

## Contract rules

1. Unknown fields are rejected where Rust serde structures use `deny_unknown_fields`.
2. Persisted hashes always travel with an algorithm and domain.
3. Schema versions are per concern rather than one repository-wide integer.
4. Related optional fields are all-present or all-absent; partial identity tuples fail.
5. Canonical ordering and canonical JSON define identity-bearing bytes.
6. Observation contracts are sinks and cannot feed transition semantics.
7. Parameters remain symbolic in the IR and are resolved per run.
8. Backend-specific details do not enter model or plan contracts.

## Compatibility

The current contract is frozen by fixtures but does not yet claim a 1.0 migration policy. Before a stable public guarantee, define supported migrations, historical linker retention and the treatment of legacy unversioned model JSON. Do not introduce shared-schema code generation until actual schema-evolution cost outweighs its build and maintenance burden.
