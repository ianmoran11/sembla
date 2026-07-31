# PRD track index

Implementation specifications are retained at their established paths because the PRD runner, reviews, evidence, and historical links refer to them. They are records, not maintained user guidance.

The executable pending-work source is [`../prds-run-queue/README.md`](../prds-run-queue/README.md). A track README records its own status and acceptance evidence.

| Track | Purpose |
| --- | --- |
| [`prds/`](README.md) | original V0.1 implementation sequence |
| [`prds-composition/`](../prds-composition/README.md) | canonical composition-source and linker architecture |
| [`prds-composition-integration/`](../prds-composition-integration/README.md) | plan execution, comparison, CUDA, and widgets |
| [`prds-surface-syntax/`](../prds-surface-syntax/README.md) | mathematical Lean surface language |
| [`prds-proof-track/`](../prds-proof-track/README.md) | initial Lean proof work |
| [`prds-npe-path/`](../prds-npe-path/README.md) | neural posterior estimation workflow |
| [`prds-demographic-slots/`](../prds-demographic-slots/README.md) | demographic fixed-slot model, state artifacts, and grouped observations |
| [`prds-precision-spike/`](../prds-precision-spike/README.md) | GPU precision decision evidence |
| [`prds-portable-sampler/`](../prds-portable-sampler/README.md) | portable sampling behavior |
| [`prds-cuda-validation-parallelism/`](../prds-cuda-validation-parallelism/README.md) | validation and claim-resolution CUDA parallelism |
| [`prds-cuda-host-path/`](../prds-cuda-host-path/README.md) | CUDA host-state and control-transfer reductions |
| [`prds-cuda-final-state-readback/`](../prds-cuda-final-state-readback/README.md) | final-state readback and hashing |
| [`prds-device-observation/`](../prds-device-observation/README.md) | device-side observation paths |
| [`prds-evaluator-throughput/`](../prds-evaluator-throughput/README.md) | CPU evaluator parallelism and tiling |
| [`prds-host-evaluator-performance/`](../prds-host-evaluator-performance/README.md) | host evaluator profiling and optimization |
| [`prds-execution-timing/`](../prds-execution-timing/README.md) | phase-level execution timing |
| [`prds-sweep-throughput/`](../prds-sweep-throughput/README.md) | sweep execution and concurrent CUDA draws |
| [`prds-project-hygiene/`](../prds-project-hygiene/README.md) | repository checks, CI, policies, and cleanup |
| [`prds-run-queue/`](../prds-run-queue/README.md) | temporary ordered queue for pending PRDs |

## Record policy

- Move a pending PRD into the run queue only according to its queue contract; do not copy it.
- Return approved PRDs to their binding track.
- Do not rewrite completed PRDs to describe current behavior. Amend the track README or maintained documentation instead.
- Keep `.piprd/` review and implementation state separate from reader documentation.
- Link current user guidance through [`../README.md`](../README.md), not through a PRD.
