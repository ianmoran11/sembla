# Architectural fitness functions

These checks are intentionally small. They prevent high-cost dependency drift without turning architecture into a separate framework.

| Rule | Enforcement |
| --- | --- |
| Workspace layering is exactly `ir <- runtime <- cuda <- cli` | `scripts/check-rust-architecture.py` over Cargo metadata |
| `sembla-ir` and `sembla-runtime` contain no CUDA implementation vocabulary | `scripts/check-rust-architecture.py` source scan |
| Core libraries do not print directly | `scripts/check-rust-architecture.py`; data/errors return to the CLI |
| Runtime dependencies remain allowlisted and Philox remains local | `scripts/check-rust.sh` using `cargo tree` |
| Production Rust/Cargo cannot depend on estimation implementations | `calibration/npe/tests/test_quarantine.py` |
| Python framework-facing fixture/binary paths are explicit | `calibration/npe/tests/test_quarantine.py` |
| Versioned artifact/protocol identifiers have owners and consumers | `scripts/check-artifact-registry.py` + `artifact-registry.json` |
| Advanced Canvas detail links, non-overlapping cards and style metadata stay valid | `scripts/check-architecture-canvases.py` |
| Lean production/test closures are separate | `sembla-lean/scripts/check-imports.py` + `SemblaTests` Lake target |
| Lean contracts/semantics/composition core obey import direction | `sembla-lean/scripts/check-imports.py` |
| Lean and Rust agree on emitted bytes and selected behavior | `sembla-lean/scripts/check-backend-compat.sh` with an explicit backend checkout |
| Runs and sweeps are byte-deterministic under the claimed contract | `scripts/check-determinism.sh` |
| Lean proof modules stay within the accepted axiom policy | `sembla-lean/scripts/check-proofs.sh` |

## CI placement

Backend `scripts/check.sh` runs documentation checks, package metadata, the
artifact registry, and the Rust contract. Frontend import architecture, proof
hygiene, and cross-repository compatibility run in `sembla-lean`; ordinary
checks in either repository never require an implicit sibling checkout.

GPU differential execution remains a manual hardware evidence workflow because CI has no qualifying GPU runner. Its absence from ordinary CI is explicit rather than a silent skip.

## Adding a rule

Add a fitness function only when all are true:

1. the forbidden dependency would materially increase change scope or invalidate a contract;
2. the rule can be explained in one sentence;
3. the check has a direct failure message and a self-test;
4. legitimate exceptions are few and named; and
5. compliance does not require a new architecture framework.

Do not enforce file-size targets or abstraction counts. Those metrics are easy to game and do not establish information hiding.
