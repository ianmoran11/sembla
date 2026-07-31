# Evidence index

This directory contains retained benchmark, conformance, and backend-measurement artifacts. Evidence records what happened under a named model, artifact, commit, machine, and command. It does not become current design authority merely by being newer.

Major collections:

- `demographic-bench/` — local and remote demographic benchmark runs.
- `sweep-backend-reuse-20260728/` and the adjacent summary document — sweep backend-reuse evidence.
- `cuda-*` directories — focused CUDA timing, state-transfer, validation, and throughput measurements.
- `npe-path-20260720/` — calibration/NPE workflow evidence.

Interpretation belongs in maintained documents under [`../performance/`](../performance/README.md). Do not edit frozen result files to update conclusions; add a new analysis or measurement that links to the prior evidence.
